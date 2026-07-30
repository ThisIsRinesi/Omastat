use crate::config::Config;
use crate::hyprland;
use crate::session;
use crate::storage::{AppTotals, Storage};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "hours-played")]
#[command(about = "Track focused and open application time on Hyprland/Omarchy")]
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
    /// Run the foreground tracker daemon.
    Daemon,
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
    /// Open the interactive terminal dashboard.
    Tui,
    /// Check environment, IPC, config, and storage paths.
    Doctor,
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
