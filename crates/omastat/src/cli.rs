use crate::config::Config;
use crate::export::DataExportScope;
use crate::hyprland;
use crate::insights::{InsightCategory, InsightConfidence, InsightTone};
use crate::report::{self, AppBreakdown, InsightsReport, Lens, UsageReport, WidgetInsight};
use crate::session;
use crate::steam::SteamResolver;
use crate::storage::{
    AppTotals, PurgeReport, Storage, StorageDiagnostic, StorageQuickCheck, StorageSchemaStatus,
    TitleRepair,
};
use anyhow::Result;
use chrono::{Duration, Local, NaiveDate, TimeZone};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::{Path, PathBuf};

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
    /// Show structured insights for a report lens.
    Insights {
        #[arg(long, value_enum, default_value = "day")]
        lens: LensArg,
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        offset: i32,
    },
    /// Export a one-page visual HTML overview.
    Export {
        #[arg(long, value_enum, default_value = "month")]
        lens: LensArg,
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        offset: i32,
        #[arg(short, long, default_value = "omastat-export.html")]
        output: PathBuf,
        #[arg(long)]
        title: Option<String>,
    },
    /// Export raw and aggregate data as JSON or CSV files.
    ExportData {
        #[arg(long, value_enum, default_value = "month")]
        lens: LensArg,
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        offset: i32,
        #[arg(long, value_enum, default_value = "json")]
        format: DataExportFormatArg,
        #[arg(long, value_enum, default_value = "all")]
        scope: DataExportScopeArg,
        #[arg(short, long, default_value = "omastat-data-export.json")]
        output: PathBuf,
    },
    /// Purge old local telemetry rows.
    Purge {
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        older_than_days: Option<u32>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        vacuum: bool,
        #[arg(long)]
        confirm: bool,
    },
    /// Show configured goals and app/category budget status.
    Goals {
        #[arg(long, value_enum, default_value = "day")]
        lens: LensArg,
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        offset: i32,
    },
    /// Show a compact weekly digest built from the insight engine.
    Digest {
        #[arg(long, value_enum, default_value = "week")]
        lens: LensArg,
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        offset: i32,
    },
    /// Show one high-signal insight suitable for a bar widget.
    WidgetInsight {
        #[arg(long, value_enum, default_value = "day")]
        lens: LensArg,
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        offset: i32,
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

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DataExportFormatArg {
    Json,
    Csv,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DataExportScopeArg {
    All,
    Raw,
    Aggregate,
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

impl From<DataExportScopeArg> for DataExportScope {
    fn from(value: DataExportScopeArg) -> Self {
        match value {
            DataExportScopeArg::All => Self::All,
            DataExportScopeArg::Raw => Self::Raw,
            DataExportScopeArg::Aggregate => Self::Aggregate,
        }
    }
}

pub fn print_report(title: &str, rows: Vec<AppTotals>, config: &Config, json: bool) -> Result<()> {
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
            truncate(
                &config.app_label(&row.app_class, || report::app_label(&row.app_class)),
                32
            ),
            format_duration(row.focused_seconds),
            format_duration(row.open_seconds)
        );
    }

    Ok(())
}

pub fn summary_report(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    days: u32,
) -> Result<UsageReport> {
    report::usage_report(storage, steam, config, Lens::Day, days)
}

pub fn print_summary(report: &UsageReport) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(report)?);
    Ok(())
}

pub fn insights_report(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<InsightsReport> {
    if lens == Lens::Day && offset == 0 {
        return Ok(report::usage_report(storage, steam, config, lens, lens.history_days())?.into());
    }

    report::insights_report_for_period(storage, steam, config, lens, offset)
}

pub fn print_insights(report: &InsightsReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!("Insights - {}", report.period.label);
    println!(
        "Focused {} | Open {}",
        format_duration(report.totals.focused_seconds),
        format_duration(report.totals.open_seconds)
    );

    if report.insights.is_empty() {
        println!("No insights for this period yet. Track focused time or choose a broader lens.");
        return Ok(());
    }

    for category in [
        InsightCategory::Patterns,
        InsightCategory::FocusQuality,
        InsightCategory::Apps,
        InsightCategory::SystemSignals,
    ] {
        let insights = report
            .insights
            .iter()
            .filter(|insight| insight.category == category)
            .collect::<Vec<_>>();
        if insights.is_empty() {
            continue;
        }

        println!();
        println!("{}", insight_category_label(category));
        for insight in insights {
            println!(
                "  [{}] {}: {}",
                insight_tone_label(insight.tone),
                insight.title,
                insight.value
            );
            println!(
                "      {} confidence - {}",
                insight_confidence_label(insight.confidence),
                insight.explanation
            );
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct GoalReport {
    pub schema_version: u32,
    pub period_label: String,
    pub lens: Lens,
    pub focused_seconds: i64,
    pub target_seconds: Option<i64>,
    pub remaining_seconds: Option<i64>,
    pub percent: Option<f64>,
    pub status: String,
    pub budgets: Vec<BudgetStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetStatus {
    pub name: String,
    pub kind: String,
    pub limit_seconds: i64,
    pub used_seconds: i64,
    pub remaining_seconds: i64,
    pub percent: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DigestReport {
    pub schema_version: u32,
    pub period_label: String,
    pub lens: Lens,
    pub focused_seconds: i64,
    pub open_seconds: i64,
    pub excluded_seconds: i64,
    pub top_apps: Vec<AppBreakdown>,
    pub widget_insight: Option<WidgetInsight>,
    pub insights: Vec<crate::insights::Insight>,
    pub goals: GoalReport,
}

pub fn goal_report(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<GoalReport> {
    let report = report::usage_report_for_period(storage, steam, config, lens, offset)?;
    Ok(build_goal_report(&report, config))
}

pub fn print_goal_report(report: &GoalReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!("Goals - {}", report.period_label);
    match report.target_seconds {
        Some(target) => println!(
            "Focus target: {} / {} ({})",
            format_duration(report.focused_seconds),
            format_duration(target),
            report
                .percent
                .map(report::percent)
                .unwrap_or_else(|| "--".to_string())
        ),
        None => println!("Focus target: not configured"),
    }
    if report.budgets.is_empty() {
        println!("Budgets: none configured");
    } else {
        println!("Budgets:");
        for budget in &report.budgets {
            println!(
                "  [{}] {} {} / {}",
                budget.status,
                budget.name,
                format_duration(budget.used_seconds),
                format_duration(budget.limit_seconds)
            );
        }
    }
    Ok(())
}

pub fn digest_report(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<DigestReport> {
    let report = report::usage_report_for_period(storage, steam, config, lens, offset)?;
    let goals = build_goal_report(&report, config);
    Ok(DigestReport {
        schema_version: 1,
        period_label: report.period.label.clone(),
        lens: report.lens,
        focused_seconds: report.total_focused_seconds,
        open_seconds: report.total_open_seconds,
        excluded_seconds: report
            .total_idle_seconds
            .saturating_add(report.total_locked_seconds)
            .saturating_add(report.total_sleep_seconds)
            .saturating_add(report.total_unobserved_seconds),
        top_apps: report.apps.iter().take(5).cloned().collect(),
        widget_insight: report.widget_insight.clone(),
        insights: report.insights.iter().take(8).cloned().collect(),
        goals,
    })
}

pub fn print_digest(report: &DigestReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!("Digest - {}", report.period_label);
    println!(
        "Focused {} | Open {} | Excluded {}",
        format_duration(report.focused_seconds),
        format_duration(report.open_seconds),
        format_duration(report.excluded_seconds)
    );
    if let Some(insight) = &report.widget_insight {
        println!("Insight: {}", insight.text);
    }
    if !report.top_apps.is_empty() {
        println!("Top apps:");
        for app in &report.top_apps {
            println!("  {} - {}", app.label, format_duration(app.focused_seconds));
        }
    }
    Ok(())
}

pub fn widget_insight_report(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<Option<WidgetInsight>> {
    let report = report::usage_report_for_period(storage, steam, config, lens, offset)?;
    Ok(report.widget_insight)
}

pub fn print_widget_insight(insight: Option<WidgetInsight>, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&insight)?);
        return Ok(());
    }
    match insight {
        Some(insight) => println!("{}", insight.text),
        None => println!("No insight for this period yet."),
    }
    Ok(())
}

pub fn purge_cutoff(
    before: Option<&str>,
    older_than_days: Option<u32>,
    all: bool,
) -> Result<Option<i64>> {
    let selected = before.is_some() as u8 + older_than_days.is_some() as u8 + all as u8;
    if selected != 1 {
        anyhow::bail!("choose exactly one of --before, --older-than-days, or --all");
    }
    if all {
        return Ok(None);
    }
    if let Some(days) = older_than_days {
        return Ok(Some(
            (Local::now() - Duration::days(days as i64)).timestamp(),
        ));
    }
    let date = NaiveDate::parse_from_str(before.unwrap_or_default(), "%Y-%m-%d")?;
    let cutoff = Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .ok_or_else(|| anyhow::anyhow!("failed to resolve local purge cutoff"))?;
    Ok(Some(cutoff.timestamp()))
}

pub fn print_purge_report(report: &PurgeReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!(
        "{} purge",
        if report.dry_run {
            "Planned"
        } else {
            "Completed"
        }
    );
    if let Some(cutoff) = &report.cutoff_local {
        println!("Cutoff: {cutoff}");
    } else {
        println!("Cutoff: all local telemetry rows");
    }
    println!(
        "Deleted: {} app, {} session, {} system, {} daemon events, {} daemon runs",
        report.intervals_deleted,
        report.session_intervals_deleted,
        report.system_intervals_deleted,
        report.daemon_events_deleted,
        report.daemon_runs_deleted
    );
    println!(
        "Trimmed: {} app, {} session, {} system",
        report.intervals_trimmed, report.session_intervals_trimmed, report.system_intervals_trimmed
    );
    if report.vacuumed {
        println!("Vacuum: completed");
    }
    Ok(())
}

fn build_goal_report(report: &UsageReport, config: &Config) -> GoalReport {
    let period_days = report.daily.len().max(1) as i64;
    let target_seconds = config
        .daily_focus_target_seconds()
        .map(|target| match report.lens {
            Lens::Day => target,
            Lens::Week | Lens::Month | Lens::Year => target.saturating_mul(period_days),
            Lens::Life => target,
        });
    let percent = target_seconds
        .map(|target| (report.total_focused_seconds.max(0) as f64 / target.max(1) as f64).max(0.0));
    let remaining_seconds =
        target_seconds.map(|target| target.saturating_sub(report.total_focused_seconds));
    let status = match percent {
        Some(value) if value >= 1.0 => "met",
        Some(value) if value >= 0.8 => "close",
        Some(_) => "open",
        None => "unconfigured",
    }
    .to_string();

    let budgets = config
        .goals
        .app_budgets
        .iter()
        .filter_map(|budget| {
            let limit = match report.lens {
                Lens::Day => budget.daily_limit_seconds(),
                Lens::Week => budget
                    .weekly_limit_seconds()
                    .or_else(|| budget.daily_limit_seconds().map(|seconds| seconds * 7)),
                Lens::Month | Lens::Year | Lens::Life => budget.weekly_limit_seconds(),
            }?;
            let (kind, name, used_seconds) = if let Some(app_match) = budget.app.as_deref() {
                let used = report
                    .rows
                    .iter()
                    .filter(|row| {
                        row.app_class == app_match
                            || config
                                .app_label(&row.app_class, || report::app_label(&row.app_class))
                                == app_match
                    })
                    .map(|row| row.focused_seconds.max(0))
                    .sum::<i64>();
                ("app".to_string(), app_match.to_string(), used)
            } else {
                let category = budget.category.as_deref()?;
                let category = category.trim().to_lowercase().replace([' ', '_'], "-");
                let used = report
                    .rows
                    .iter()
                    .filter(|row| config.app_category(&row.app_class) == category)
                    .map(|row| row.focused_seconds.max(0))
                    .sum::<i64>();
                ("category".to_string(), category, used)
            };
            let percent = used_seconds.max(0) as f64 / limit.max(1) as f64;
            let status = if used_seconds > limit {
                "over"
            } else if percent >= 0.8 {
                "near"
            } else {
                "ok"
            }
            .to_string();
            Some(BudgetStatus {
                name,
                kind,
                limit_seconds: limit,
                used_seconds,
                remaining_seconds: limit.saturating_sub(used_seconds),
                percent,
                status,
            })
        })
        .collect();

    GoalReport {
        schema_version: 1,
        period_label: report.period.label.clone(),
        lens: report.lens,
        focused_seconds: report.total_focused_seconds,
        target_seconds,
        remaining_seconds,
        percent,
        status,
        budgets,
    }
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

pub async fn doctor(config: &Config, database: Option<&Path>) -> Result<()> {
    println!("Config");
    println!("  Path: {}", config.path.display());
    println!("  Title capture: {:?}", config.privacy.title_capture);

    print_storage_diagnostic(&Storage::diagnose(database));

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

    Ok(())
}

fn print_storage_diagnostic(diagnostic: &StorageDiagnostic) {
    println!("Storage");
    println!("  Database path: {}", diagnostic.path.display());
    println!("  Exists: {}", yes_no(diagnostic.exists));

    match &diagnostic.schema_status {
        StorageSchemaStatus::Missing => {
            println!("  Schema status: missing");
            println!("  Migration need: initialize database");
        }
        StorageSchemaStatus::NotInitialized { reason } => {
            println!("  Schema status: not initialized ({reason})");
            println!("  Migration need: initialize database");
        }
        StorageSchemaStatus::Current { applied_migrations } => {
            println!(
                "  Schema status: current (migrations {})",
                format_migrations(applied_migrations)
            );
            println!("  Migration need: none");
        }
        StorageSchemaStatus::NeedsMigration {
            version,
            description,
            applied_migrations,
        } => {
            println!(
                "  Schema status: needs migration {version} ({description}); applied {}",
                format_migrations(applied_migrations)
            );
            println!("  Migration need: apply migration {version} ({description})");
        }
        StorageSchemaStatus::UnknownMigration {
            version,
            applied_migrations,
        } => {
            println!(
                "  Schema status: newer than this binary (unknown migration {version}); applied {}",
                format_migrations(applied_migrations)
            );
            println!("  Migration need: unknown until Omastat is updated");
        }
        StorageSchemaStatus::Invalid { error } => {
            println!("  Schema status: invalid ({error})");
            println!("  Migration need: unknown until schema is readable");
        }
        StorageSchemaStatus::Unreadable { error } => {
            println!("  Schema status: unreadable ({error})");
            println!("  Migration need: unknown until database can be opened");
        }
    }

    match &diagnostic.quick_check {
        StorageQuickCheck::Ok => println!("  SQLite quick check: ok"),
        StorageQuickCheck::Problem(problem) => {
            println!("  SQLite quick check: problem ({problem})")
        }
        StorageQuickCheck::Skipped(reason) => println!("  SQLite quick check: skipped ({reason})"),
        StorageQuickCheck::Error(error) => println!("  SQLite quick check: failed ({error})"),
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn format_migrations(migrations: &[i64]) -> String {
    if migrations.is_empty() {
        return "none".to_string();
    }
    migrations
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn insight_category_label(category: InsightCategory) -> &'static str {
    match category {
        InsightCategory::Patterns => "Patterns",
        InsightCategory::FocusQuality => "Focus Quality",
        InsightCategory::Apps => "Apps",
        InsightCategory::SystemSignals => "System Signals",
    }
}

fn insight_tone_label(tone: InsightTone) -> &'static str {
    match tone {
        InsightTone::Positive => "positive",
        InsightTone::Negative => "negative",
        InsightTone::Neutral => "neutral",
        InsightTone::Info => "info",
        InsightTone::Caution => "caution",
    }
}

fn insight_confidence_label(confidence: InsightConfidence) -> &'static str {
    match confidence {
        InsightConfidence::Low => "low",
        InsightConfidence::Medium => "medium",
        InsightConfidence::High => "high",
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_insights_json_after_subcommand() {
        let cli = Cli::try_parse_from([
            "omastat", "insights", "--json", "--lens", "week", "--offset", "-1",
        ])
        .unwrap();

        assert!(cli.json);
        match cli.command {
            Commands::Insights { lens, offset } => {
                assert!(matches!(lens, LensArg::Week));
                assert_eq!(offset, -1);
            }
            other => panic!("expected insights command, got {other:?}"),
        }
    }

    #[test]
    fn parses_negative_export_offset_as_period_value() {
        let cli = Cli::try_parse_from(["omastat", "export", "--lens", "month", "--offset", "-2"])
            .unwrap();

        match cli.command {
            Commands::Export { lens, offset, .. } => {
                assert!(matches!(lens, LensArg::Month));
                assert_eq!(offset, -2);
            }
            other => panic!("expected export command, got {other:?}"),
        }
    }

    #[test]
    fn parses_export_data_csv_scope() {
        let cli = Cli::try_parse_from([
            "omastat",
            "export-data",
            "--format",
            "csv",
            "--scope",
            "raw",
            "--output",
            "out-dir",
        ])
        .unwrap();

        match cli.command {
            Commands::ExportData {
                format,
                scope,
                output,
                ..
            } => {
                assert!(matches!(format, DataExportFormatArg::Csv));
                assert!(matches!(scope, DataExportScopeArg::Raw));
                assert_eq!(output, PathBuf::from("out-dir"));
            }
            other => panic!("expected export-data command, got {other:?}"),
        }
    }

    #[test]
    fn purge_cutoff_requires_one_selector() {
        assert!(purge_cutoff(None, None, false).is_err());
        assert!(purge_cutoff(Some("2026-01-01"), Some(7), false).is_err());
        assert_eq!(purge_cutoff(None, None, true).unwrap(), None);
        assert!(purge_cutoff(Some("2026-01-01"), None, false).is_ok());
    }
}
