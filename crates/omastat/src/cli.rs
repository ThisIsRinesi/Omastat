use crate::config::Config;
use crate::hyprland;
use crate::report::{self, Lens, UsageReport};
use crate::session;
use crate::steam::SteamResolver;
use crate::storage::{AppTotals, Storage, TitleRepair};
use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "omastat")]
#[command(about = "Measure focused and open application time on Hyprland/Omarchy")]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[arg(long, global = true)]
    pub database: Option<PathBuf>,

    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Show today's app totals.
    Today,
    /// Show current week app totals.
    Week,
    /// Show all-time app totals.
    Apps,
    /// Show totals for an inclusive local date range.
    Range {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
    },
    /// Show panel-ready JSON with today's apps and recent daily totals.
    Summary {
        #[arg(long, default_value_t = 7)]
        days: u32,
    },
    /// Export a one-page visual HTML replay.
    Export {
        #[arg(long, value_enum, default_value = "month")]
        lens: LensArg,
        #[arg(long, default_value_t = 0)]
        offset: i32,
        #[arg(short, long, default_value = "omastat-export.html")]
        output: PathBuf,
        #[arg(long)]
        title: Option<String>,
    },
    /// Open the interactive terminal dashboard.
    Tui,
    /// Normalize app names and fill missing focused titles in existing data.
    RepairTitles {
        #[arg(long)]
        dry_run: bool,
    },
    /// Check environment, IPC, config, and storage paths.
    Doctor,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LensArg {
    Day,
    Week,
    Month,
    Year,
    Life,
}

impl From<LensArg> for Lens {
    fn from(value: LensArg) -> Self {
        match value {
            LensArg::Day => Self::Day,
            LensArg::Week => Self::Week,
            LensArg::Month => Self::Month,
            LensArg::Year => Self::Year,
            LensArg::Life => Self::Life,
        }
    }
}

pub fn print_report(title: &str, rows: Vec<AppTotals>, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    println!("{title}");
    println!("{:<32} {:>12} {:>12}", "App", "Focused", "Open");
    println!("{:-<58}", "");

    if rows.is_empty() {
        println!("No tracked usage yet.");
        return Ok(());
    }

    for row in rows {
        println!(
            "{:<32} {:>12} {:>12}",
            truncate(&row.app_class, 32),
            format_duration(row.focused_seconds),
            format_duration(row.open_seconds)
        );
    }

    Ok(())
}

pub fn summary_report(
    storage: &Storage,
    steam: &mut SteamResolver,
    days: u32,
) -> Result<UsageReport> {
    report::usage_report(storage, steam, Lens::Day, days)
}

pub fn print_summary(report: &UsageReport) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(report)?);
    Ok(())
}

pub fn print_title_repair(repair: &TitleRepair, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(repair)?);
        return Ok(());
    }

    if repair.dry_run {
        println!("Title repair dry run");
    } else {
        println!("Title repair applied");
    }
    println!("Rewritten class rows: {}", repair.rewritten_rows);
    println!("Filled focused titles: {}", repair.filled_titles);
    println!("Normalized focused titles: {}", repair.normalized_titles);

    if !repair.class_updates.is_empty() {
        println!();
        println!("Class rewrites:");
        for update in repair.class_updates.iter().take(20) {
            println!("  {} -> {} ({})", update.from, update.to, update.rows);
        }
        if repair.class_updates.len() > 20 {
            println!("  ... {} more", repair.class_updates.len() - 20);
        }
    }

    if !repair.title_updates.is_empty() {
        println!();
        println!("Title fills:");
        for update in repair.title_updates.iter().take(20) {
            println!(
                "  {} = {:?} ({})",
                update.app_class, update.title, update.rows
            );
        }
        if repair.title_updates.len() > 20 {
            println!("  ... {} more", repair.title_updates.len() - 20);
        }
    }

    if !repair.title_normalizations.is_empty() {
        println!();
        println!("Title normalizations:");
        for update in repair.title_normalizations.iter().take(20) {
            println!(
                "  {}: {:?} -> {:?} ({})",
                update.app_class, update.from, update.to, update.rows
            );
        }
        if repair.title_normalizations.len() > 20 {
            println!("  ... {} more", repair.title_normalizations.len() - 20);
        }
    }

    Ok(())
}

pub async fn doctor(config: &Config, storage: &Storage) -> Result<()> {
    println!("Config: {}", config.path.display());
    println!("Database: {}", storage.path().display());
    println!("Title capture: {:?}", config.privacy.title_capture);

    match hyprland::socket_paths() {
        Ok(paths) => {
            println!("Hyprland request socket: {}", paths.request.display());
            println!("Hyprland event socket: {}", paths.event.display());
            println!("Request socket exists: {}", paths.request.exists());
            println!("Event socket exists: {}", paths.event.exists());
        }
        Err(error) => {
            println!("Hyprland socket discovery failed: {error}");
        }
    }

    match hyprland::snapshot().await {
        Ok(snapshot) => {
            println!("Open windows: {}", snapshot.windows.len());
            println!(
                "Active window: {}",
                snapshot
                    .active_address
                    .as_deref()
                    .unwrap_or("none or unavailable")
            );
        }
        Err(error) => {
            println!("Hyprland snapshot failed: {error}");
        }
    }

    match session::status().await {
        Ok(status) => {
            println!("Session idle: {}", status.idle);
            println!("Session locked: {}", status.locked);
            println!("Session stay awake: {}", status.stay_awake);
            println!("Session audio playing: {}", status.audio_playing);
            println!("Session source: {}", status.source);
        }
        Err(error) => {
            println!("Session status unavailable: {error}");
        }
    }

    storage.healthcheck()?;
    println!("SQLite healthcheck: ok");
    Ok(())
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }

    let mut out = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}

fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds < 60 {
        return format!("{seconds}s");
    }

    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;

    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}
