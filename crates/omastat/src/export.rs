use crate::{
    analytics, clock,
    config::Config,
    insights::{Insight, InsightCategory, InsightTone},
    report::{self, Lens, UsageReport},
    steam::SteamResolver,
    storage::{
        AppDayTotals, AppTotals, DayTotals, FocusHeatCell, IntervalKind, RawExportRows, Storage,
        StorageStatus, SystemIntervalKind, SystemTimelineInterval, TimelineInterval, TitleTotals,
        WorkspaceTotals,
    },
};
use anyhow::Result;
use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::{
    fs,
    path::{Path, PathBuf},
};

const PALETTE: [&str; 10] = [
    "#4de8ff", "#8f7aff", "#46d369", "#ffd166", "#ff667d", "#25c2a0", "#ff9f43", "#c084fc",
    "#7dd3fc", "#f472b6",
];

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub lens: Lens,
    pub offset: i32,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataExportScope {
    All,
    Raw,
    Aggregate,
}

#[derive(Debug, Clone)]
pub struct DataExportOptions {
    pub lens: Lens,
    pub offset: i32,
    pub scope: DataExportScope,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataExport {
    pub schema_version: u32,
    pub generated_at: i64,
    pub timezone: String,
    pub query_start_ts: i64,
    pub query_end_ts: i64,
    pub period: report::Period,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<AggregateExport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawExportRows>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregateExport {
    pub app_totals: Vec<AppTotals>,
    pub app_breakdown: Vec<report::AppBreakdown>,
    pub daily_totals: Vec<DayTotals>,
    pub insights: Vec<Insight>,
}

pub fn render_html(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    options: ExportOptions,
) -> Result<String> {
    let requested_lens = options.lens;
    let requested_offset = if requested_lens == Lens::Life {
        0
    } else {
        options.offset.min(0)
    };
    let initial_key = period_key(requested_lens, requested_offset);
    let (storage_status, mut health_warnings) = match storage.usage_status() {
        Ok(status) => (status, Vec::new()),
        Err(error) => (
            StorageStatus::default(),
            vec![format!("storage health unavailable: {error:#}")],
        ),
    };
    health_warnings.extend(
        config
            .warnings()
            .into_iter()
            .map(|warning| format!("config {}: {}", warning.field, warning.message)),
    );
    let mut periods = Vec::new();
    for lens in Lens::ALL {
        let seed_offset = if lens == requested_lens {
            requested_offset
        } else {
            0
        };
        for offset in offset_window(lens, seed_offset) {
            periods.push(build_dashboard_period(
                storage,
                steam,
                config,
                &storage_status,
                &health_warnings,
                lens,
                offset,
            )?);
        }
    }
    let initial = periods
        .iter()
        .find(|period| period.key == initial_key)
        .or_else(|| {
            periods
                .iter()
                .find(|period| period.key == period_key(requested_lens, 0))
        })
        .or_else(|| periods.first())
        .expect("dashboard should include at least one period");
    let initial_period_label = initial.period_label.clone();
    let initial_key = initial.key.clone();
    let page_title = options
        .title
        .unwrap_or_else(|| format!("Omastat Overview - {}", initial_period_label));
    let dashboard = DashboardPayload {
        initial_key,
        periods,
    };

    Ok(document(&page_title, &dashboard))
}

pub fn build_data_export(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    options: DataExportOptions,
) -> Result<DataExport> {
    let report =
        report::usage_report_for_period(storage, steam, config, options.lens, options.offset)?;
    let raw = matches!(options.scope, DataExportScope::All | DataExportScope::Raw)
        .then(|| storage.raw_export_between(report.query_start_ts, report.query_end_ts))
        .transpose()?;
    let aggregate = matches!(
        options.scope,
        DataExportScope::All | DataExportScope::Aggregate
    )
    .then(|| AggregateExport {
        app_totals: report.rows.clone(),
        app_breakdown: report.apps.clone(),
        daily_totals: report.daily.clone(),
        insights: report.insights.clone(),
    });

    Ok(DataExport {
        schema_version: 1,
        generated_at: report.generated_at,
        timezone: clock::local_now().offset().to_string(),
        query_start_ts: report.query_start_ts,
        query_end_ts: report.query_end_ts,
        period: report.period,
        aggregate,
        raw,
    })
}

pub fn write_data_export_csv(export: &DataExport, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    fs::write(
        output_dir.join("metadata.json"),
        serde_json::to_string_pretty(&ExportMetadata::from(export))?,
    )?;

    if let Some(aggregate) = &export.aggregate {
        write_csv(output_dir.join("app_totals.csv"), &aggregate.app_totals)?;
        write_csv(
            output_dir.join("app_breakdown.csv"),
            &aggregate.app_breakdown,
        )?;
        write_csv(output_dir.join("daily_totals.csv"), &aggregate.daily_totals)?;
        let insight_rows = aggregate
            .insights
            .iter()
            .map(InsightCsvRow::from)
            .collect::<Vec<_>>();
        write_csv(output_dir.join("insights.csv"), &insight_rows)?;
    }

    if let Some(raw) = &export.raw {
        write_csv(output_dir.join("raw_intervals.csv"), &raw.intervals)?;
        write_csv(
            output_dir.join("raw_session_intervals.csv"),
            &raw.session_intervals,
        )?;
        write_csv(
            output_dir.join("raw_system_intervals.csv"),
            &raw.system_intervals,
        )?;
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct ExportMetadata {
    schema_version: u32,
    generated_at: i64,
    timezone: String,
    query_start_ts: i64,
    query_end_ts: i64,
    period_label: String,
    period_start_date: Option<String>,
    period_end_date: Option<String>,
}

impl From<&DataExport> for ExportMetadata {
    fn from(export: &DataExport) -> Self {
        Self {
            schema_version: export.schema_version,
            generated_at: export.generated_at,
            timezone: export.timezone.clone(),
            query_start_ts: export.query_start_ts,
            query_end_ts: export.query_end_ts,
            period_label: export.period.label.clone(),
            period_start_date: export.period.start_date.clone(),
            period_end_date: export.period.end_date.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct InsightCsvRow {
    kind: String,
    category: String,
    tone: String,
    confidence: String,
    title: String,
    value: String,
    explanation: String,
    data_points: usize,
    minimum_data_points: usize,
    observed_focus_seconds: i64,
    observed_open_seconds: i64,
    app_class: Option<String>,
    app_label: Option<String>,
    workspace: Option<String>,
    focused_seconds: Option<i64>,
    open_seconds: Option<i64>,
    excluded_seconds: Option<i64>,
    share: Option<f64>,
}

impl From<&Insight> for InsightCsvRow {
    fn from(insight: &Insight) -> Self {
        Self {
            kind: serde_json::to_value(insight.kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_else(|| format!("{:?}", insight.kind)),
            category: insight_category_label(insight.category).to_string(),
            tone: insight_tone_label(insight.tone).to_string(),
            confidence: format!("{:?}", insight.confidence).to_lowercase(),
            title: insight.title.clone(),
            value: insight.value.clone(),
            explanation: insight.explanation.clone(),
            data_points: insight.evidence.data_points,
            minimum_data_points: insight.evidence.minimum_data_points,
            observed_focus_seconds: insight.evidence.observed_focus_seconds,
            observed_open_seconds: insight.evidence.observed_open_seconds,
            app_class: insight.supporting.app_class.clone(),
            app_label: insight.supporting.app_label.clone(),
            workspace: insight.supporting.workspace.clone(),
            focused_seconds: insight.supporting.focused_seconds,
            open_seconds: insight.supporting.open_seconds,
            excluded_seconds: insight.supporting.excluded_seconds,
            share: insight.supporting.share,
        }
    }
}

fn write_csv<T: Serialize>(path: PathBuf, rows: &[T]) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    for row in rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct ExportStats {
    total_days: usize,
    active_days: usize,
    daily_average_seconds: i64,
    active_day_average_seconds: i64,
    longest_streak_days: usize,
    focus_block_count: usize,
    app_switch_count: usize,
    average_block_seconds: i64,
    median_block_seconds: i64,
    longest_block_seconds: i64,
    deep_block_count: usize,
    deep_block_seconds: i64,
    peak_hour: Option<(u32, i64)>,
    top_app_share: f64,
    effective_apps: f64,
}

impl ExportStats {
    fn from_data(
        report: &UsageReport,
        heatmap: &[FocusHeatCell],
        focus_intervals: &[TimelineInterval],
    ) -> Self {
        let total_days = report.daily.len();
        let active_days = analytics::active_day_count(&report.daily);
        let daily_average_seconds = analytics::average(report.total_focused_seconds, total_days);
        let active_day_average_seconds =
            analytics::average(report.total_focused_seconds, active_days);
        let longest_streak_days = analytics::longest_active_streak(&report.daily);
        let block_stats = analytics::focus_block_stats(focus_intervals);
        let app_switch_count = analytics::app_switch_count(focus_intervals);
        let peak_hour = hour_totals(heatmap).into_iter().next();
        let top_app_share = report
            .rows
            .iter()
            .find(|row| row.focused_seconds > 0)
            .map(|row| ratio(row.focused_seconds, report.total_focused_seconds.max(1)))
            .unwrap_or_default();
        let effective_apps =
            analytics::effective_app_count(&report.rows, report.total_focused_seconds);

        Self {
            total_days,
            active_days,
            daily_average_seconds,
            active_day_average_seconds,
            longest_streak_days,
            focus_block_count: block_stats.count,
            app_switch_count,
            average_block_seconds: block_stats.average_seconds,
            median_block_seconds: block_stats.median_seconds,
            longest_block_seconds: block_stats.longest_seconds,
            deep_block_count: block_stats.deep_count,
            deep_block_seconds: block_stats.deep_seconds,
            peak_hour,
            top_app_share,
            effective_apps,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DashboardPayload {
    initial_key: String,
    periods: Vec<DashboardPeriodPayload>,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardPeriodPayload {
    key: String,
    lens_key: String,
    lens_label: String,
    lens_title: String,
    offset: i32,
    offset_label: String,
    period_label: String,
    range_label: String,
    generated: String,
    focused: String,
    focus_context: String,
    top_app: String,
    number_cards: String,
    daily_pattern: String,
    app_mix_panel: String,
    top_hours: String,
    workspace_focus: String,
    session_histogram: String,
    activity_timeline: String,
    timeline: String,
    interval_table: String,
    gap_summary: String,
    system_health: String,
    app_table: String,
    stacked_days: String,
    heatmap_chart: String,
    time_breakdown: String,
    insights: String,
    title_rows: String,
}

fn build_dashboard_period(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    storage_status: &StorageStatus,
    health_warnings: &[String],
    lens: Lens,
    offset: i32,
) -> Result<DashboardPeriodPayload> {
    let report_with_rollups = if lens == Lens::Day && offset == 0 {
        report::usage_report_with_rollups(storage, steam, config, lens, lens.history_days())?
    } else {
        report::usage_report_with_rollups_for_period(storage, steam, config, lens, offset)?
    };
    let report = report_with_rollups.report;
    let (start_ts, end_ts) = (report.query_start_ts, report.query_end_ts);
    let mut rollups = report_with_rollups.rollups;
    for row in &mut rollups.daily_apps {
        row.app_class = steam.resolve_class(&row.app_class);
    }
    for interval in &mut rollups.focus_intervals {
        interval.app_class = steam.resolve_class(&interval.app_class);
    }
    let mut timeline_intervals = storage.timeline_between(start_ts, end_ts)?;
    for interval in &mut timeline_intervals {
        interval.app_class = steam.resolve_class(&interval.app_class);
    }
    let system_intervals = storage.system_timeline_between(start_ts, end_ts)?;
    let stats = ExportStats::from_data(&report, &rollups.heatmap, &rollups.focus_intervals);
    let titles = storage
        .focused_title_totals_between(start_ts, end_ts, 12)?
        .into_iter()
        .map(|mut row| {
            row.app_class = steam.resolve_class(&row.app_class);
            row
        })
        .collect::<Vec<_>>();

    let focused = report::format_duration(report.total_focused_seconds);
    let focus_share = report::percent(ratio(
        report.total_focused_seconds,
        report.total_elapsed_seconds.max(1),
    ));
    let peak_day = report
        .daily
        .iter()
        .max_by_key(|day| day.focused_seconds)
        .filter(|day| day.focused_seconds > 0);
    let daily_avg = report::format_duration(stats.active_day_average_seconds);
    let daily_note = format!("{} active / {} shown", stats.active_days, stats.total_days);
    let longest_session = report::format_duration(stats.longest_block_seconds);
    let session_note = format!(
        "{} sessions, median {}",
        stats.focus_block_count,
        report::format_duration(stats.median_block_seconds)
    );
    let app_mix = format!("{:.1}", stats.effective_apps);
    let app_note = format!("top app {}", report::percent(stats.top_app_share));
    let streak_label = format!("{}d", stats.longest_streak_days);
    let switch_rate = if report.total_focused_seconds > 0 {
        stats.app_switch_count as f64 / (report.total_focused_seconds as f64 / 3600.0)
    } else {
        0.0
    };
    let streak_note = format!("{:.0} app changes/h", switch_rate);
    let peak_day_label = peak_day
        .map(|day| day.label.clone())
        .unwrap_or_else(|| "none".to_string());
    let peak_day_duration = peak_day
        .map(|day| report::format_duration(day.focused_seconds))
        .unwrap_or_else(|| "no focus".to_string());
    let peak_hour = stats
        .peak_hour
        .map(|(hour, seconds)| {
            format!(
                "{} / {}",
                hour_label(hour),
                report::format_duration(seconds)
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let focus_context = format!("{focus_share} of elapsed");
    let top_app = report
        .rows
        .iter()
        .find(|row| row.focused_seconds > 0)
        .map(|row| report::app_label(&row.app_class))
        .unwrap_or_else(|| "No focus yet".to_string());
    let number_cards_html = {
        let number_card_rows = vec![
            NumberCard::new("Focused", &focused, &focus_context),
            NumberCard::new("Daily avg", &daily_avg, &daily_note),
            NumberCard::new("Longest session", &longest_session, &session_note),
            NumberCard::new("App mix", &app_mix, &app_note),
            NumberCard::new("Streak", &streak_label, &streak_note),
            NumberCard::new("Peak hour", &peak_hour, "recurring focus window"),
            NumberCard::new("Peak day", &peak_day_label, &peak_day_duration),
        ];
        number_cards(&number_card_rows)
    };

    Ok(DashboardPeriodPayload {
        key: period_key(lens, report.period.offset),
        lens_key: lens_key(lens).to_string(),
        lens_label: lens.label().to_string(),
        lens_title: lens.title().to_string(),
        offset: report.period.offset,
        offset_label: offset_label(lens, report.period.offset).to_string(),
        period_label: report.period.label.clone(),
        range_label: period_range_label(&report),
        generated: format_timestamp(report.generated_at),
        focused,
        focus_context,
        top_app,
        number_cards: number_cards_html,
        daily_pattern: daily_pattern_chart(&report.daily),
        app_mix_panel: app_mix_panel(&report.rows, report.total_focused_seconds),
        top_hours: top_hours_chart(&rollups.heatmap),
        workspace_focus: workspace_focus_chart(&rollups.workspaces, report.total_focused_seconds),
        session_histogram: session_histogram(&rollups.focus_intervals, &stats),
        activity_timeline: activity_timeline_chart(
            &timeline_intervals,
            &system_intervals,
            &report.rows,
            start_ts,
            end_ts,
        ),
        timeline: timeline_chart(&rollups.focus_intervals, &report.rows, start_ts, end_ts),
        interval_table: interval_table(
            &timeline_intervals,
            &system_intervals,
            &report.rows,
            start_ts,
            end_ts,
        ),
        gap_summary: gap_summary(&system_intervals, &report),
        system_health: system_health_panel(storage_status, &report, health_warnings),
        app_table: app_table(&report.rows, report.total_focused_seconds),
        stacked_days: stacked_day_chart(&report.daily, &rollups.daily_apps, &report.rows),
        heatmap_chart: heatmap_chart(&rollups.heatmap),
        time_breakdown: time_breakdown(&report, &stats),
        insights: insight_rows(&report.insights),
        title_rows: title_rows(&titles),
    })
}

fn document(page_title: &str, dashboard: &DashboardPayload) -> String {
    let initial = dashboard
        .periods
        .iter()
        .find(|period| period.key == dashboard.initial_key)
        .or_else(|| dashboard.periods.first())
        .expect("dashboard should include at least one period");
    let dashboard_json = dashboard_json(dashboard);
    let lens_controls = lens_controls(&dashboard.periods, &initial.key);
    let lens_cards = lens_cards_html_from_periods(&dashboard.periods);

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
{css}
</style>
</head>
<body>
<main class="dashboard">
  <header class="dashboard-header">
    <div>
      <p class="eyebrow">Omastat overview</p>
      <h1 data-bind="lens_title">{lens_title}</h1>
      <p class="subhead"><span data-bind="period">{period}</span> · <span data-bind="range">{range}</span> · generated <span data-bind="generated">{generated}</span></p>
    </div>
    <div class="focus-summary">
      <small>Focused time</small>
      <strong data-bind="focused">{focused}</strong>
      <span data-bind="focus_context">{focus_context}</span>
    </div>
  </header>

  <section class="control-bar" aria-label="Dashboard controls">
    <div>
      <span class="kicker">Lens</span>
      <div class="segmented" id="lensControls">{lens_controls}</div>
    </div>
    <div>
      <span class="kicker">Period</span>
      <div class="period-nav">
        <button type="button" id="previousPeriod" aria-label="Previous period">&larr;</button>
        <button type="button" id="currentPeriod">Current</button>
        <button type="button" id="nextPeriod" aria-label="Next period">&rarr;</button>
      </div>
      <div class="period-rail" id="periodRail"></div>
    </div>
    <div>
      <span class="kicker">View</span>
      <div class="segmented" id="viewControls">
        <button type="button" class="active" data-view-button="overview">Overview</button>
        <button type="button" data-view-button="apps">Apps</button>
        <button type="button" data-view-button="insights">Insights</button>
        <button type="button" data-view-button="timeline">Timeline</button>
        <button type="button" data-view-button="system">System</button>
      </div>
    </div>
  </section>

  <section class="metric-grid view-block" data-views="overview system" aria-label="Overview metrics">
    <div data-slot="number_cards">{number_cards}</div>
  </section>

  <section class="grid grid-secondary view-block" data-views="system">
    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Status</span>
          <h2>System health</h2>
        </div>
      </div>
      <div data-slot="system_health">{system_health}</div>
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">System signals</span>
          <h2>Counted vs excluded</h2>
        </div>
      </div>
      <div data-slot="time_breakdown">{time_breakdown}</div>
    </article>
  </section>

  <section class="grid grid-main view-block" data-views="overview">
    <article class="panel panel-wide">
      <div class="panel-heading">
        <div>
          <span class="kicker">Trend</span>
          <h2>Daily pattern</h2>
        </div>
        <p>Bars show focused time. The highlighted marker calls out the strongest day.</p>
      </div>
      <div data-slot="daily_pattern">{daily_pattern}</div>
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Composition</span>
          <h2>App mix</h2>
        </div>
      </div>
      <div data-slot="app_mix_panel">{app_mix_panel}</div>
    </article>
  </section>

  <section class="grid grid-main view-block" data-views="apps">
    <article class="panel panel-wide">
      <div class="panel-heading">
        <div>
          <span class="kicker">Applications</span>
          <h2>App table</h2>
        </div>
        <p>Ranked applications by focused time and share of the selected period.</p>
      </div>
      <div data-slot="app_table">{app_table}</div>
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Composition</span>
          <h2>App mix</h2>
        </div>
      </div>
      <div data-slot="app_mix_panel">{app_mix_panel}</div>
    </article>
  </section>

  <section class="grid grid-secondary view-block" data-views="overview">
    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Timing</span>
          <h2>Week x hour heatmap</h2>
        </div>
        <p>Sequential color exposes recurring focus windows.</p>
      </div>
      <div data-slot="heatmap_chart">{heatmap_chart}</div>
    </article>

    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Timing</span>
          <h2>Top hours</h2>
        </div>
        <p>Ranked hours make peaks easy to compare.</p>
      </div>
      <div data-slot="top_hours">{top_hours}</div>
    </article>
  </section>

  <section class="grid grid-secondary view-block" data-views="overview system">
    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Place</span>
          <h2>Workspace focus</h2>
        </div>
        <p>Ranked workspaces show where focused time landed.</p>
      </div>
      <div data-slot="workspace_focus">{workspace_focus}</div>
    </article>

    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Sessions</span>
          <h2>Focus length distribution</h2>
        </div>
        <p>Histogram bins reveal whether focus came in fragments or blocks.</p>
      </div>
      <div data-slot="session_histogram">{session_histogram}</div>
    </article>
  </section>

  <section class="grid grid-main view-block" data-views="timeline">
    <article class="panel panel-wide">
      <div class="panel-heading">
        <div>
          <span class="kicker">Timeline</span>
          <h2>Activity timeline</h2>
        </div>
        <p>Focused, open, sleep, and tracker-off intervals share one time axis.</p>
      </div>
      <div data-slot="activity_timeline">{activity_timeline}</div>
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Not counted</span>
          <h2>Gap summary</h2>
        </div>
      </div>
      <div data-slot="gap_summary">{gap_summary}</div>
    </article>
  </section>

  <section class="grid grid-main view-block" data-views="timeline">
    <article class="panel panel-wide">
      <div class="panel-heading">
        <div>
          <span class="kicker">Timeline</span>
          <h2>Focus intervals</h2>
        </div>
        <p>Horizontal lanes isolate focused blocks by application.</p>
      </div>
      <div data-slot="timeline">{timeline}</div>
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Intervals</span>
          <h2>Recent rows</h2>
        </div>
      </div>
      <div data-slot="interval_table">{interval_table}</div>
    </article>
  </section>

  <section class="grid grid-main view-block" data-views="overview apps system">
    <article class="panel panel-wide">
      <div class="panel-heading">
        <div>
          <span class="kicker">Composition over time</span>
          <h2>App mix by day</h2>
        </div>
        <p>Stacked bars show which apps made up each day's focus.</p>
      </div>
      <div data-slot="stacked_days">{stacked_days}</div>
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">System signals</span>
          <h2>Counted vs excluded</h2>
        </div>
      </div>
      <div data-slot="time_breakdown">{time_breakdown}</div>
    </article>
  </section>

  <section class="grid grid-tertiary view-block" data-views="insights apps system">
    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Evaluated facts</span>
          <h2>Period insights</h2>
        </div>
      </div>
      <div data-slot="insights">{insights}</div>
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Captured titles</span>
          <h2>Captured moments</h2>
        </div>
      </div>
      <div data-slot="title_rows">{title_rows}</div>
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Lenses</span>
          <h2>Period lenses</h2>
        </div>
      </div>
      <div id="lensCards">{lens_cards}</div>
    </article>
  </section>

  <footer>
    <span>Generated from local Omastat data</span>
    <span>Self-contained HTML/SVG overview</span>
  </footer>
</main>
<script type="application/json" id="omastat-data">{dashboard_json}</script>
<script>
{script}
</script>
</body>
</html>
"#,
        title = escape_html(page_title),
        css = stylesheet(),
        lens_title = escape_html(&initial.lens_title),
        period = escape_html(&initial.period_label),
        range = escape_html(&initial.range_label),
        generated = escape_html(&initial.generated),
        focused = escape_html(&initial.focused),
        focus_context = escape_html(&initial.focus_context),
        lens_controls = lens_controls,
        number_cards = initial.number_cards,
        daily_pattern = initial.daily_pattern,
        app_mix_panel = initial.app_mix_panel,
        top_hours = initial.top_hours,
        workspace_focus = initial.workspace_focus,
        session_histogram = initial.session_histogram,
        activity_timeline = initial.activity_timeline,
        timeline = initial.timeline,
        interval_table = initial.interval_table,
        gap_summary = initial.gap_summary,
        system_health = initial.system_health,
        app_table = initial.app_table,
        stacked_days = initial.stacked_days,
        heatmap_chart = initial.heatmap_chart,
        time_breakdown = initial.time_breakdown,
        insights = initial.insights,
        title_rows = initial.title_rows,
        lens_cards = lens_cards,
        dashboard_json = dashboard_json,
        script = dashboard_script(),
    )
}

fn lens_key(lens: Lens) -> &'static str {
    match lens {
        Lens::Day => "day",
        Lens::Week => "week",
        Lens::Month => "month",
        Lens::Year => "year",
        Lens::Life => "life",
    }
}

fn period_key(lens: Lens, offset: i32) -> String {
    format!("{}:{offset}", lens_key(lens))
}

fn offset_window(lens: Lens, requested_offset: i32) -> Vec<i32> {
    if lens == Lens::Life {
        return vec![0];
    }
    let requested_offset = requested_offset.min(0);
    let (default_previous, max_periods) = match lens {
        Lens::Day => (7, 9),
        Lens::Week => (5, 7),
        Lens::Month => (5, 7),
        Lens::Year => (4, 6),
        Lens::Life => unreachable!(),
    };
    let oldest = requested_offset.min(-default_previous);
    let total = oldest.abs() + 1;
    if total <= max_periods {
        return (oldest..=0).collect();
    }
    if requested_offset < -(max_periods - 1) {
        let newest = (requested_offset + max_periods - 2).min(-1);
        let mut offsets = (requested_offset..=newest).collect::<Vec<_>>();
        offsets.push(0);
        offsets
    } else {
        (-(max_periods - 1)..=0).collect()
    }
}

fn offset_label(lens: Lens, offset: i32) -> &'static str {
    match (lens, offset) {
        (Lens::Life, _) => "All time",
        (_, 0) => "Current",
        (Lens::Day, -1) => "Yesterday",
        (Lens::Day, _) => "Previous day",
        (Lens::Week, -1) => "Previous week",
        (Lens::Week, _) => "Past week",
        (Lens::Month, -1) => "Previous month",
        (Lens::Month, _) => "Past month",
        (Lens::Year, -1) => "Previous year",
        (Lens::Year, _) => "Past year",
    }
}

fn dashboard_json(dashboard: &DashboardPayload) -> String {
    serde_json::to_string(dashboard)
        .unwrap_or_else(|_| "{\"initial_key\":\"day:0\",\"periods\":[]}".to_string())
        .replace("</", "<\\/")
}

fn lens_controls(periods: &[DashboardPeriodPayload], active_key: &str) -> String {
    let active_lens_key = periods
        .iter()
        .find(|period| period.key == active_key)
        .map(|period| period.lens_key.as_str())
        .unwrap_or("day");
    Lens::ALL
        .into_iter()
        .filter_map(|lens| {
            let key = lens_key(lens);
            periods
                .iter()
                .find(|period| period.lens_key == key && period.offset == 0)
                .or_else(|| periods.iter().find(|period| period.lens_key == key))
        })
        .map(|period| {
            format!(
                r#"<button type="button" class="{class}" data-lens-button="{key}"><span>{label}</span><small>{focused}</small></button>"#,
                class = if period.lens_key == active_lens_key { "active" } else { "" },
                key = escape_html(&period.lens_key),
                label = escape_html(&period.lens_label),
                focused = escape_html(&period.focused),
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn lens_cards_html_from_periods(periods: &[DashboardPeriodPayload]) -> String {
    let rows = Lens::ALL
        .into_iter()
        .filter_map(|lens| {
            let key = lens_key(lens);
            periods
                .iter()
                .find(|period| period.lens_key == key && period.offset == 0)
                .or_else(|| periods.iter().find(|period| period.lens_key == key))
        })
        .map(|period| {
            format!(
                r#"<article class="lens-row">
  <div class="lens-label">{}</div>
  <div>
    <div class="lens-total">{}</div>
    <div class="lens-meta">{}</div>
  </div>
</article>"#,
                escape_html(&period.lens_label),
                escape_html(&period.focused),
                escape_html(&period.top_app),
            )
        })
        .collect::<Vec<_>>();
    format!(r#"<div class="lens-list">{}</div>"#, rows.join("\n"))
}

fn dashboard_script() -> &'static str {
    r##"
(() => {
  const root = document.querySelector(".dashboard");
  const payloadNode = document.getElementById("omastat-data");
  if (!root || !payloadNode) return;
  const payload = JSON.parse(payloadNode.textContent || "{}");
  const periodList = payload.periods || [];
  const periods = new Map(periodList.map((period) => [period.key, period]));
  const periodsByLens = new Map();
  const lensOrder = [];
  for (const period of periodList) {
    if (!periodsByLens.has(period.lens_key)) {
      periodsByLens.set(period.lens_key, []);
      lensOrder.push(period.lens_key);
    }
    periodsByLens.get(period.lens_key).push(period);
  }
  for (const periodGroup of periodsByLens.values()) {
    periodGroup.sort((left, right) => left.offset - right.offset);
  }
  let activeKey = payload.initial_key || (payload.periods && payload.periods[0] && payload.periods[0].key);
  let activeView = "overview";

  function setText(selector, value) {
    const node = document.querySelector(selector);
    if (node) node.textContent = value || "";
  }

  function setSlot(name, value) {
    document.querySelectorAll(`[data-slot="${name}"]`).forEach((node) => {
      node.innerHTML = value || "";
    });
  }

  function activePeriod() {
    return periods.get(activeKey);
  }

  function periodForLens(lensKey) {
    const periodGroup = periodsByLens.get(lensKey) || [];
    return periodGroup.find((period) => period.offset === 0) || periodGroup[periodGroup.length - 1];
  }

  function neighborPeriod(delta) {
    const period = activePeriod();
    if (!period) return null;
    const periodGroup = periodsByLens.get(period.lens_key) || [];
    const index = periodGroup.findIndex((candidate) => candidate.key === period.key);
    return periodGroup[index + delta] || null;
  }

  function renderPeriodRail(period) {
    const rail = document.getElementById("periodRail");
    if (!rail) return;
    const periodGroup = periodsByLens.get(period.lens_key) || [];
    const fragment = document.createDocumentFragment();
    for (const candidate of periodGroup) {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.periodKey = candidate.key;
      button.className = candidate.key === period.key ? "active" : "";
      const label = document.createElement("span");
      label.textContent = candidate.period_label;
      const meta = document.createElement("small");
      meta.textContent = candidate.offset_label;
      button.append(label, meta);
      fragment.append(button);
    }
    rail.replaceChildren(fragment);
  }

  function setPeriodNavButton(id, period) {
    const button = document.getElementById(id);
    if (!button) return;
    button.disabled = !period;
    button.dataset.periodKey = period ? period.key : "";
  }

  function renderPeriodNavigation(period) {
    setPeriodNavButton("previousPeriod", neighborPeriod(-1));
    setPeriodNavButton("nextPeriod", neighborPeriod(1));
    const current = periodForLens(period.lens_key);
    const currentButton = document.getElementById("currentPeriod");
    if (currentButton) {
      currentButton.textContent = period.offset === 0 ? period.period_label : "Current";
      currentButton.disabled = !current || current.key === period.key;
      currentButton.dataset.periodKey = current ? current.key : "";
    }
    renderPeriodRail(period);
  }

  function renderPeriod(key) {
    const period = periods.get(key);
    if (!period) return;
    activeKey = key;
    root.classList.add("is-swapping");
    window.setTimeout(() => {
      setText('[data-bind="lens_title"]', period.lens_title);
      setText('[data-bind="period"]', period.period_label);
      setText('[data-bind="range"]', period.range_label);
      setText('[data-bind="generated"]', period.generated);
      setText('[data-bind="focused"]', period.focused);
      setText('[data-bind="focus_context"]', period.focus_context);
      for (const name of [
        "number_cards",
        "daily_pattern",
        "app_mix_panel",
        "top_hours",
        "workspace_focus",
        "session_histogram",
        "activity_timeline",
        "timeline",
        "interval_table",
        "gap_summary",
        "system_health",
        "app_table",
        "stacked_days",
        "heatmap_chart",
        "time_breakdown",
        "insights",
        "title_rows",
      ]) {
        setSlot(name, period[name]);
      }
      document.querySelectorAll("[data-lens-button]").forEach((button) => {
        button.classList.toggle("active", button.dataset.lensButton === period.lens_key);
      });
      renderPeriodNavigation(period);
      root.classList.remove("is-swapping");
      root.classList.add("just-swapped");
      window.setTimeout(() => root.classList.remove("just-swapped"), 420);
    }, 130);
  }

  function renderView(view) {
    activeView = view;
    document.querySelectorAll("[data-view-button]").forEach((button) => {
      button.classList.toggle("active", button.dataset.viewButton === view);
    });
    document.querySelectorAll(".view-block").forEach((block) => {
      const views = String(block.dataset.views || "").split(/\s+/);
      block.classList.toggle("view-hidden", !views.includes(view));
    });
  }

  document.querySelectorAll("[data-lens-button]").forEach((button) => {
    button.addEventListener("click", () => {
      const period = periodForLens(button.dataset.lensButton);
      if (period) renderPeriod(period.key);
    });
  });
  document.querySelectorAll("[data-view-button]").forEach((button) => {
    button.addEventListener("click", () => renderView(button.dataset.viewButton));
  });
  document.querySelectorAll("#previousPeriod, #currentPeriod, #nextPeriod").forEach((button) => {
    button.addEventListener("click", () => {
      if (button.dataset.periodKey) renderPeriod(button.dataset.periodKey);
    });
  });
  const periodRail = document.getElementById("periodRail");
  if (periodRail) {
    periodRail.addEventListener("click", (event) => {
      const button = event.target.closest("[data-period-key]");
      if (button) renderPeriod(button.dataset.periodKey);
    });
  }
  document.addEventListener("keydown", (event) => {
    if (event.target && /input|textarea|select/i.test(event.target.tagName)) return;
    const period = activePeriod();
    const lensIndex = period ? lensOrder.indexOf(period.lens_key) : -1;
    if ((event.key === "ArrowLeft" || event.key.toLowerCase() === "h") && lensIndex >= 0) {
      const nextLens = lensOrder[(lensIndex + lensOrder.length - 1) % lensOrder.length];
      const nextPeriod = periodForLens(nextLens);
      if (nextPeriod) renderPeriod(nextPeriod.key);
    }
    if ((event.key === "ArrowRight" || event.key.toLowerCase() === "l") && lensIndex >= 0) {
      const nextLens = lensOrder[(lensIndex + 1) % lensOrder.length];
      const nextPeriod = periodForLens(nextLens);
      if (nextPeriod) renderPeriod(nextPeriod.key);
    }
    if (event.key === "[") {
      const previous = neighborPeriod(-1);
      if (previous) renderPeriod(previous.key);
    }
    if (event.key === "]") {
      const next = neighborPeriod(1);
      if (next) renderPeriod(next.key);
    }
    if (event.key.toLowerCase() === "r") window.location.reload();
    if (event.key >= "1" && event.key <= "5") {
      const lensKey = lensOrder[Number(event.key) - 1];
      const nextPeriod = periodForLens(lensKey);
      if (nextPeriod) renderPeriod(nextPeriod.key);
    }
    const viewByKey = { o: "overview", a: "apps", i: "insights", t: "timeline", s: "system" };
    const view = viewByKey[event.key.toLowerCase()];
    if (view) renderView(view);
  });
  renderView(activeView);
  const initial = activePeriod();
  if (initial) renderPeriodNavigation(initial);
})();
"##
}

fn stylesheet() -> &'static str {
    r#"
:root {
  color-scheme: dark;
  --bg: #0e1014;
  --bg-grid: rgba(255,255,255,0.035);
  --panel: transparent;
  --panel-2: rgba(255,255,255,0.045);
  --ink: #f4f2ec;
  --muted: #a8acb8;
  --soft: #d4d7df;
  --line: rgba(255,255,255,0.11);
  --line-strong: rgba(255,255,255,0.2);
  --cyan: #43d9e8;
  --green: #59d98e;
  --yellow: #f6c45a;
  --pink: #f276b6;
  --purple: #9d83f7;
  --orange: #f59f53;
  --red: #ff6f7f;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  color: var(--ink);
  background: #0e1014;
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  letter-spacing: 0;
}
.dashboard {
  position: relative;
  width: min(1420px, calc(100vw - 32px));
  margin: 0 auto;
  padding: 28px 0 38px;
}
.dashboard-header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(260px, 360px);
  gap: 28px;
  align-items: end;
  margin-bottom: 20px;
  padding-bottom: 20px;
  border-bottom: 1px solid var(--line-strong);
}
.control-bar {
  display: grid;
  grid-template-columns: minmax(320px, 0.95fr) minmax(280px, 0.8fr) minmax(320px, 0.95fr);
  gap: 22px;
  align-items: end;
  margin-bottom: 22px;
  padding-bottom: 18px;
  border-bottom: 1px solid var(--line);
}
.control-bar > div {
  min-width: 0;
}
.segmented {
  display: flex;
  flex-wrap: wrap;
  gap: 0;
  margin-top: 8px;
  min-width: 0;
}
.segmented button {
  appearance: none;
  border: 0;
  border-bottom: 1px solid var(--line);
  border-radius: 0;
  background: transparent;
  color: var(--soft);
  cursor: pointer;
  min-height: 38px;
  padding: 8px 14px;
  font: inherit;
  font-size: 0.82rem;
  font-weight: 900;
  letter-spacing: 0;
  transition: border-color 160ms ease, color 160ms ease, background 160ms ease;
}
.segmented button span,
.segmented button small {
  display: block;
  line-height: 1.05;
}
.segmented button small {
  color: var(--muted);
  font-size: 0.68rem;
  margin-top: 3px;
}
.segmented button:hover {
  border-color: rgba(67,217,232,0.62);
}
.segmented button.active {
  color: var(--ink);
  border-color: var(--cyan);
  background: rgba(67,217,232,0.08);
}
.segmented button.active small {
  color: var(--muted);
}
.period-nav {
  display: grid;
  grid-template-columns: 42px minmax(0, 1fr) 42px;
  gap: 8px;
  margin-top: 8px;
  min-width: 0;
}
.period-nav button,
.period-rail button {
  appearance: none;
  border: 1px solid var(--line);
  border-radius: 4px;
  background: transparent;
  color: var(--soft);
  cursor: pointer;
  min-height: 38px;
  padding: 8px 12px;
  font: inherit;
  font-size: 0.82rem;
  font-weight: 900;
  letter-spacing: 0;
  transition: transform 160ms ease, border-color 160ms ease, background 160ms ease, color 160ms ease, opacity 160ms ease;
}
.segmented button:focus-visible,
.period-nav button:focus-visible,
.period-rail button:focus-visible {
  outline: 2px solid var(--yellow);
  outline-offset: 3px;
  box-shadow: 0 0 0 5px rgba(246,196,90,0.16);
}
.period-nav button:hover:not(:disabled),
.period-rail button:hover {
  border-color: rgba(246,196,90,0.5);
}
.period-nav button:disabled {
  cursor: default;
  opacity: 0.52;
}
#currentPeriod:disabled {
  opacity: 0.78;
}
.period-rail {
  display: flex;
  gap: 7px;
  margin-top: 8px;
  overflow-x: auto;
  padding-bottom: 2px;
  max-width: 100%;
  min-width: 0;
  scrollbar-width: thin;
}
.period-rail button {
  flex: 0 0 auto;
  min-width: 104px;
  text-align: left;
}
.period-rail button span,
.period-rail button small {
  display: block;
  line-height: 1.05;
}
.period-rail button small {
  color: var(--muted);
  font-size: 0.67rem;
  margin-top: 3px;
}
.period-rail button.active {
  color: var(--ink);
  border-color: var(--yellow);
  background: rgba(246,196,90,0.08);
}
.period-rail button.active small {
  color: var(--muted);
}
.panel, .stat-cell, .focus-summary {
  border: 0;
  border-top: 1px solid var(--line);
  border-radius: 0;
  background: transparent;
  animation: reveal-up 300ms cubic-bezier(.2,.8,.2,1) both;
}
.eyebrow, .kicker, .stat-cell small, .focus-summary small, footer, .mini-label {
  color: var(--muted);
  text-transform: uppercase;
  font-size: 0.72rem;
  font-weight: 900;
  letter-spacing: 0;
}
h1, h2, p { margin: 0; }
h1 {
  max-width: 900px;
  margin-top: 8px;
  font-size: clamp(2.4rem, 5vw, 4.9rem);
  line-height: 0.96;
  letter-spacing: 0;
}
h2 {
  margin-top: 4px;
  font-size: 1.5rem;
  line-height: 1;
}
.subhead {
  margin-top: 12px;
  color: var(--soft);
  font-size: 0.98rem;
}
.focus-summary {
  min-height: 132px;
  padding: 18px 0 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
}
.focus-summary strong {
  display: block;
  margin: 12px 0 8px;
  font-size: clamp(3rem, 5vw, 4.9rem);
  line-height: 0.88;
}
.focus-summary span {
  color: #ffe4a8;
  font-weight: 850;
}
.metric-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 0 18px;
  margin-bottom: 22px;
  border-bottom: 1px solid var(--line);
}
.metric-grid > [data-slot] {
  display: contents;
}
.stat-cell {
  min-height: 88px;
  padding: 14px 0 16px;
  overflow: hidden;
}
.stat-cell strong {
  display: block;
  margin-top: 10px;
  font-size: 1.55rem;
  line-height: 0.95;
  overflow-wrap: anywhere;
}
.stat-cell span {
  display: block;
  margin-top: 8px;
  color: var(--soft);
  font-weight: 750;
}
.grid {
  display: grid;
  gap: 18px 26px;
  margin-bottom: 22px;
  transition: opacity 180ms ease, transform 180ms ease;
}
.grid-main { grid-template-columns: minmax(0, 1.45fr) minmax(360px, 0.75fr); }
.grid-secondary { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
.grid-tertiary { grid-template-columns: repeat(auto-fit, minmax(310px, 1fr)); }
.view-hidden {
  display: none;
}
.is-swapping .view-block {
  opacity: 0.35;
  transform: translateY(5px);
}
.just-swapped .panel,
.just-swapped .stat-cell,
.just-swapped .focus-summary {
  animation: panel-rise 380ms ease both;
}
@keyframes panel-rise {
  from { opacity: 0.72; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
@keyframes reveal-up {
  from { opacity: 0.78; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}
@keyframes fill-in {
  from { transform: scaleX(0); }
  to { transform: scaleX(1); }
}
@keyframes chart-pop {
  from { opacity: 0.5; transform: translateY(3px); }
  to { opacity: 1; transform: translateY(0); }
}
.panel {
  min-width: 0;
  padding: 16px 0 0;
}
.panel-heading {
  display: flex;
  justify-content: space-between;
  gap: 18px;
  align-items: flex-start;
  margin-bottom: 18px;
  padding-bottom: 13px;
  border-bottom: 1px solid var(--line);
}
.panel-heading.compact { margin-bottom: 14px; }
.panel-heading p {
  max-width: 360px;
  color: var(--muted);
  font-size: 0.88rem;
  line-height: 1.35;
}
.chart-frame {
  border: 0;
  border-top: 1px solid rgba(255,255,255,0.07);
  border-bottom: 1px solid rgba(255,255,255,0.07);
  border-radius: 0;
  background: transparent;
  padding: 12px 0;
}
.chart-frame svg { width: 100%; height: auto; display: block; overflow: visible; }
.chart-frame rect {
  transition: opacity 180ms ease, filter 180ms ease;
}
.chart-frame rect:hover {
  filter: brightness(1.18);
}
.just-swapped .chart-frame rect {
  animation: chart-pop 360ms ease both;
}
.legend-strip {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
  margin-top: 12px;
}
.summary-note {
  margin-top: 12px;
  color: var(--muted);
  font-size: 0.84rem;
  font-weight: 760;
}
.legend-chip {
  display: inline-grid;
  grid-template-columns: 12px auto;
  align-items: center;
  gap: 7px;
  color: #cfe9ed;
  font-size: 0.8rem;
  font-weight: 800;
}
.swatch {
  width: 12px;
  height: 12px;
  border-radius: 6px;
}
.ranked-list, .title-list, .lens-list, .insight-list, .metric-list {
  display: grid;
  gap: 12px;
}
.rank-row {
  display: grid;
  grid-template-columns: 30px 1fr auto;
  gap: 10px;
  align-items: center;
}
.rank-index {
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  border: 0;
  color: var(--cyan);
  font-weight: 950;
}
.rank-name, .title-name {
  font-weight: 900;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rank-meta, .title-app, .lens-meta {
  color: var(--muted);
  font-size: 0.78rem;
  font-weight: 800;
}
.rank-time, .title-time {
  font-weight: 950;
  font-variant-numeric: tabular-nums;
}
.rank-bar {
  grid-column: 2 / -1;
  height: 10px;
  border: 0;
  background: rgba(255,255,255,0.08);
}
.rank-fill {
  height: 100%;
  transform-origin: left center;
  animation: fill-in 620ms cubic-bezier(.2,.8,.2,1) both;
}
.data-table, .interval-list {
  display: grid;
  gap: 8px;
}
.table-row {
  display: grid;
  grid-template-columns: 42px minmax(0, 1.4fr) 104px 84px;
  gap: 10px;
  align-items: center;
  min-height: 36px;
  padding: 8px 0;
  border-bottom: 1px solid rgba(255,255,255,0.08);
  background: transparent;
}
.table-head {
  min-height: 30px;
  color: var(--muted);
  font-size: 0.72rem;
  font-weight: 950;
  text-transform: uppercase;
  background: transparent;
}
.table-rank {
  color: var(--cyan);
  font-weight: 950;
}
.table-name, .interval-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 900;
}
.table-row span:not(.table-name):not(.table-rank) {
  color: var(--soft);
  font-weight: 850;
  font-variant-numeric: tabular-nums;
}
.interval-row {
  display: grid;
  grid-template-columns: 70px minmax(0, 1fr) 118px 72px;
  gap: 10px;
  align-items: center;
  min-height: 38px;
  padding: 8px 0;
  border-bottom: 1px solid rgba(255,255,255,0.08);
}
.interval-row:last-child {
  border-bottom: 0;
}
.interval-kind {
  font-weight: 950;
}
.interval-time, .interval-duration {
  color: var(--muted);
  font-size: 0.78rem;
  font-weight: 850;
  font-variant-numeric: tabular-nums;
  text-align: right;
}
.mix-strip, .breakdown-strip {
  display: flex;
  width: 100%;
  height: 18px;
  overflow: hidden;
  border: 0;
  border-radius: 0;
  background: rgba(255,255,255,0.08);
  margin-bottom: 16px;
}
.mix-segment, .breakdown-segment {
  min-width: 2px;
  height: 100%;
  transform-origin: left center;
  animation: fill-in 620ms cubic-bezier(.2,.8,.2,1) both;
}
.histogram {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 10px;
  align-items: end;
  min-height: 230px;
}
.hist-bin {
  display: grid;
  grid-template-rows: 1fr auto auto;
  gap: 8px;
  min-width: 0;
}
.hist-track {
  position: relative;
  min-height: 150px;
  border: 0;
  border-bottom: 1px solid var(--line);
  background: rgba(255,255,255,0.055);
}
.hist-fill {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  background: var(--cyan);
  transform-origin: center bottom;
  animation: reveal-bar 650ms cubic-bezier(.2,.8,.2,1) both;
}
@keyframes reveal-bar {
  from { transform: scaleY(0); }
  to { transform: scaleY(1); }
}
.hist-value {
  font-weight: 950;
  text-align: center;
}
.hist-label {
  color: var(--muted);
  font-size: 0.78rem;
  font-weight: 850;
  text-align: center;
}
.breakdown-list {
  display: grid;
  gap: 10px;
}
.metric-list {
  display: grid;
  gap: 10px;
}
.metric-row {
  display: grid;
  grid-template-columns: minmax(0, 0.85fr) minmax(0, 1.15fr);
  gap: 12px;
  align-items: baseline;
  padding-bottom: 10px;
  border-bottom: 1px solid rgba(255,255,255,0.08);
}
.metric-row:last-child {
  border-bottom: 0;
  padding-bottom: 0;
}
.metric-row span {
  color: var(--muted);
  font-size: 0.76rem;
  font-weight: 900;
  text-transform: uppercase;
}
.metric-row strong {
  min-width: 0;
  color: var(--soft);
  font-weight: 950;
  overflow-wrap: anywhere;
}
.breakdown-row {
  display: grid;
  grid-template-columns: 14px minmax(0, 1fr) auto;
  align-items: center;
  gap: 9px;
}
.breakdown-dot {
  width: 14px;
  height: 14px;
  border-radius: 999px;
}
.breakdown-label {
  font-weight: 850;
}
.breakdown-value {
  color: var(--soft);
  font-weight: 900;
  font-variant-numeric: tabular-nums;
}
.title-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(255,255,255,0.08);
}
.title-row:last-child { border-bottom: 0; padding-bottom: 0; }
.insight-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(255,255,255,0.08);
}
.insight-row:last-child { border-bottom: 0; padding-bottom: 0; }
.insight-title {
  margin-top: 2px;
  font-weight: 950;
}
.insight-explanation {
  margin-top: 5px;
  color: var(--soft);
  font-size: 0.82rem;
  line-height: 1.3;
}
.insight-meta {
  color: var(--muted);
  font-size: 0.72rem;
  font-weight: 900;
  text-transform: uppercase;
}
.insight-value {
  color: var(--cyan);
  font-weight: 950;
  font-variant-numeric: tabular-nums;
  text-align: right;
}
.lens-row {
  display: grid;
  grid-template-columns: 76px 1fr;
  gap: 12px;
  align-items: center;
  padding: 10px 0;
  border-radius: 0;
  border: 0;
  border-bottom: 1px solid rgba(255,255,255,0.08);
  background: transparent;
}
.lens-label {
  color: var(--cyan);
  font-weight: 950;
}
.lens-total {
  font-size: 1.24rem;
  font-weight: 950;
}
footer {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  margin-top: 4px;
  padding: 16px 2px 0;
  border-top: 1px solid var(--line);
}
text {
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}
@media (max-width: 1060px) {
  .dashboard-header, .control-bar, .grid-main, .grid-secondary, .grid-tertiary, .metric-grid {
    grid-template-columns: 1fr;
  }
  .panel-heading {
    display: block;
  }
  .panel-heading p {
    max-width: none;
    margin-top: 8px;
  }
  .histogram {
    min-height: 190px;
  }
}
@media (max-width: 720px) {
  .table-row {
    grid-template-columns: 34px minmax(0, 1fr) 72px;
  }
  .hide-narrow {
    display: none;
  }
  .interval-row {
    grid-template-columns: 58px minmax(0, 1fr) 64px;
  }
  .interval-time {
    display: none;
  }
}
@media print {
  body { background: #101114; }
  .dashboard { width: 100%; padding: 0; }
  body::before { display: none; }
}
"#
}

struct NumberCard<'a> {
    label: &'a str,
    value: &'a str,
    note: &'a str,
}

impl<'a> NumberCard<'a> {
    fn new(label: &'a str, value: &'a str, note: &'a str) -> Self {
        Self { label, value, note }
    }
}

fn number_cards(cards: &[NumberCard<'_>]) -> String {
    cards
        .iter()
        .map(|card| {
            format!(
                r#"<article class="stat-cell"><small>{}</small><strong>{}</strong><span>{}</span></article>"#,
                escape_html(card.label),
                escape_html(card.value),
                escape_html(card.note),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn daily_pattern_chart(days: &[DayTotals]) -> String {
    let visible_days = visible_chart_days(days);
    if visible_days.is_empty() {
        return r#"<div class="chart-frame">No daily focus yet.</div>"#.to_string();
    }

    let max_focus = visible_days
        .iter()
        .map(|day| day.focused_seconds.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let width = 1080.0;
    let left = 54.0;
    let top = 30.0;
    let chart_h = 260.0;
    let chart_w = width - left - 30.0;
    let gap = if visible_days.len() > 90 {
        1.0
    } else if visible_days.len() > 45 {
        2.0
    } else {
        5.0
    };
    let bar_w = ((chart_w - gap * (visible_days.len().saturating_sub(1) as f64))
        / visible_days.len() as f64)
        .max(2.0);
    let label_step = (visible_days.len() / 9).max(1);
    let mut bars = String::new();
    let mut labels = String::new();

    for (index, day) in visible_days.iter().enumerate() {
        let x = left + index as f64 * (bar_w + gap);
        let focus_height = (day.focused_seconds.max(0) as f64 / max_focus as f64) * chart_h;
        let y = top + chart_h - focus_height;
        let cx = x + bar_w / 2.0;
        let excluded = excluded_seconds(day);
        let opacity = if day.focused_seconds > 0 { 0.95 } else { 0.24 };
        bars.push_str(&format!(
            r##"<rect x="{x:.2}" y="{y:.2}" width="{bar_w:.2}" height="{focus_height:.2}" rx="3" fill="url(#focusBar)" opacity="{opacity:.2}">
  <title>{date}: {focus} focused, {excluded} not counted</title>
</rect>"##,
            date = escape_html(&day.label),
            focus = escape_html(&report::format_duration(day.focused_seconds)),
            excluded = escape_html(&report::format_duration(excluded)),
        ));

        if index % label_step == 0 || index + 1 == visible_days.len() {
            labels.push_str(&format!(
                r##"<text x="{:.2}" y="336" text-anchor="middle" font-size="12" font-weight="850" fill="#a8acb8">{}</text>"##,
                cx,
                escape_html(&short_date(&day.date)),
            ));
        }
    }

    let best = visible_days
        .iter()
        .enumerate()
        .max_by_key(|(_, day)| day.focused_seconds)
        .filter(|(_, day)| day.focused_seconds > 0);
    let annotation = best
        .map(|(index, day)| {
            let x = left + index as f64 * (bar_w + gap) + bar_w / 2.0;
            let bar_h = (day.focused_seconds as f64 / max_focus as f64) * chart_h;
            let y = top + chart_h - bar_h;
            format!(
                r##"<line x1="{x:.2}" y1="{y:.2}" x2="{x:.2}" y2="18" stroke="#f6c45a" stroke-width="2" />
<rect x="{label_x:.2}" y="2" width="164" height="38" rx="4" fill="#f6c45a" />
<text x="{text_x:.2}" y="18" font-size="11" font-weight="950" fill="#101114">Peak: {date}</text>
<text x="{text_x:.2}" y="32" font-size="11" font-weight="850" fill="#101114">{duration}</text>"##,
                label_x = (x + 8.0).min(width - 174.0),
                text_x = (x + 18.0).min(width - 164.0),
                date = escape_html(&day.label),
                duration = escape_html(&report::format_duration(day.focused_seconds)),
            )
        })
        .unwrap_or_default();
    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 1080 365" role="img" aria-label="Daily focused time">
<defs>
  <linearGradient id="focusBar" x1="0" x2="0" y1="0" y2="1">
    <stop offset="0%" stop-color="#43d9e8" />
    <stop offset="100%" stop-color="#59d98e" />
  </linearGradient>
</defs>
<line x1="54" y1="290" x2="1050" y2="290" stroke="rgba(255,255,255,0.22)" stroke-width="1" />
<line x1="54" y1="160" x2="1050" y2="160" stroke="rgba(255,255,255,0.10)" stroke-width="1" />
<line x1="54" y1="30" x2="1050" y2="30" stroke="rgba(255,255,255,0.08)" stroke-width="1" />
<text x="5" y="34" font-size="12" font-weight="900" fill="#a8acb8">{max_label}</text>
{bars}
{annotation}
{labels}
</svg></div>
<div class="legend-strip">
  <span class="legend-chip"><i class="swatch" style="color:#43d9e8;background:#43d9e8"></i>Focused time</span>
</div>"##,
        max_label = escape_html(&report::format_duration(max_focus)),
    )
}

fn app_mix_panel(rows: &[AppTotals], total: i64) -> String {
    if rows.iter().all(|row| row.focused_seconds <= 0) {
        return "No focused app time in this period.".to_string();
    }
    format!(
        r#"{strip}{ranked}"#,
        strip = composition_strip(rows, total),
        ranked = ranked_apps(rows, total),
    )
}

fn composition_strip(rows: &[AppTotals], total: i64) -> String {
    let total = total.max(1);
    let mut segments = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(8)
        .enumerate()
        .map(|(index, row)| {
            let share = ratio(row.focused_seconds, total);
            let color = PALETTE[index % PALETTE.len()];
            format!(
                r#"<div class="mix-segment" style="width:{:.2}%;background:{color}" title="{} - {}"></div>"#,
                share * 100.0,
                escape_html(&report::app_label(&row.app_class)),
                escape_html(&report::percent(share)),
            )
        })
        .collect::<Vec<_>>();
    let shown = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(8)
        .map(|row| row.focused_seconds)
        .sum::<i64>();
    let other = total.saturating_sub(shown);
    if other > 0 {
        segments.push(format!(
            r#"<div class="mix-segment" style="width:{:.2}%;background:#64748b" title="Other - {}"></div>"#,
            ratio(other, total) * 100.0,
            escape_html(&report::percent(ratio(other, total))),
        ));
    }

    format!(r#"<div class="mix-strip">{}</div>"#, segments.join(""))
}

fn top_hours_chart(cells: &[FocusHeatCell]) -> String {
    let hours = hour_totals(cells);
    if hours.iter().all(|(_, seconds)| *seconds <= 0) {
        return "No hourly focus data for this period.".to_string();
    }
    metric_bars(
        hours
            .into_iter()
            .take(8)
            .map(|(hour, seconds)| (hour_label(hour), seconds))
            .collect(),
        "of the busiest hour",
    )
}

fn workspace_focus_chart(workspaces: &[WorkspaceTotals], total: i64) -> String {
    if workspaces.is_empty() || workspaces.iter().all(|row| row.focused_seconds <= 0) {
        return "No workspace focus data for this period.".to_string();
    }
    metric_bars(
        workspaces
            .iter()
            .take(8)
            .map(|row| (row.workspace.clone(), row.focused_seconds))
            .collect(),
        &format!("of {}", report::format_duration(total)),
    )
}

fn metric_bars(rows: Vec<(String, i64)>, note: &str) -> String {
    let max = rows
        .iter()
        .map(|(_, seconds)| *seconds)
        .max()
        .unwrap_or(1)
        .max(1);
    let rows = rows
        .into_iter()
        .enumerate()
        .map(|(index, (label, seconds))| {
            let width = ratio(seconds, max) * 100.0;
            let color = PALETTE[index % PALETTE.len()];
            format!(
                r#"<div class="rank-row">
  <div class="rank-index">{rank}</div>
  <div>
    <div class="rank-name">{label}</div>
    <div class="rank-meta">{note}</div>
  </div>
  <div class="rank-time">{time}</div>
  <div class="rank-bar"><div class="rank-fill" style="width:{width:.2}%;color:{color};background:{color}"></div></div>
</div>"#,
                rank = index + 1,
                label = escape_html(&label),
                time = escape_html(&report::format_duration(seconds)),
            )
        })
        .collect::<Vec<_>>();
    format!(r#"<div class="ranked-list">{}</div>"#, rows.join("\n"))
}

fn session_histogram(focus_intervals: &[TimelineInterval], stats: &ExportStats) -> String {
    let bins = [
        ("<5m", 0, 5 * 60),
        ("5-15m", 5 * 60, 15 * 60),
        ("15-30m", 15 * 60, 30 * 60),
        ("30-60m", 30 * 60, 60 * 60),
        ("1h+", 60 * 60, i64::MAX),
    ];
    let mut counts = vec![0_usize; bins.len()];
    for seconds in focus_intervals
        .iter()
        .map(|interval| interval.ended_at.saturating_sub(interval.started_at))
        .filter(|seconds| *seconds > 0)
    {
        if let Some((index, _)) = bins
            .iter()
            .enumerate()
            .find(|(_, (_, min, max))| seconds >= *min && seconds < *max)
        {
            counts[index] += 1;
        }
    }

    if counts.iter().all(|count| *count == 0) {
        return "No focus sessions for this period.".to_string();
    }

    let max = counts.iter().copied().max().unwrap_or(1).max(1);
    let bars = bins
        .iter()
        .zip(counts.iter())
        .map(|((label, _, _), count)| {
            let height = (*count as f64 / max as f64 * 100.0).max(4.0);
            format!(
                r#"<div class="hist-bin">
  <div class="hist-track"><div class="hist-fill" style="height:{height:.2}%"></div></div>
  <div class="hist-value">{count}</div>
  <div class="hist-label">{label}</div>
</div>"#,
                label = escape_html(label),
            )
        })
        .collect::<Vec<_>>();
    let note = format!(
        "Average {} · longest {} · {} deep sessions / {}",
        report::format_duration(stats.average_block_seconds),
        report::format_duration(stats.longest_block_seconds),
        stats.deep_block_count,
        report::format_duration(stats.deep_block_seconds),
    );
    format!(
        r#"<div class="histogram">{}</div><p class="summary-note">{}</p>"#,
        bars.join("\n"),
        escape_html(&note),
    )
}

fn app_table(rows: &[AppTotals], total: i64) -> String {
    let rows = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(18)
        .enumerate()
        .map(|(index, row)| {
            let share = ratio(row.focused_seconds, total.max(1));
            format!(
                r#"<div class="table-row">
  <span class="table-rank">{rank}</span>
  <span class="table-name">{name}</span>
  <span>{focused}</span>
  <span class="hide-narrow">{share}</span>
</div>"#,
                rank = index + 1,
                name = escape_html(&report::app_label(&row.app_class)),
                focused = escape_html(&report::format_duration(row.focused_seconds)),
                share = escape_html(&report::percent(share)),
            )
        })
        .collect::<Vec<_>>();

    if rows.is_empty() {
        return "No application usage in this period.".to_string();
    }

    format!(
        r#"<div class="data-table app-data-table">
  <div class="table-row table-head">
    <span>#</span><span>Application</span><span>Focused</span><span class="hide-narrow">Share</span>
  </div>
  {}
</div>"#,
        rows.join("\n")
    )
}

fn activity_timeline_chart(
    intervals: &[TimelineInterval],
    system_intervals: &[SystemTimelineInterval],
    rows: &[AppTotals],
    start_ts: i64,
    end_ts: i64,
) -> String {
    if (intervals.is_empty() && system_intervals.is_empty()) || end_ts <= start_ts {
        return r#"<div class="chart-frame">No intervals recorded for this period.</div>"#
            .to_string();
    }

    let top_classes = rows
        .iter()
        .take(8)
        .map(|row| row.app_class.clone())
        .collect::<Vec<_>>();
    let class_rank = top_classes
        .iter()
        .enumerate()
        .map(|(index, app_class)| (app_class.clone(), index))
        .collect::<HashMap<_, _>>();
    let duration = (end_ts - start_ts).max(1) as f64;
    let left = 118.0;
    let chart_w = 1080.0 - left - 30.0;
    let mut blocks = String::new();

    for interval in intervals.iter().take(1100) {
        let rank = class_rank
            .get(&interval.app_class)
            .copied()
            .unwrap_or(PALETTE.len() - 1);
        let color = PALETTE[rank % PALETTE.len()];
        let (lane_y, height, opacity, kind) = match interval.kind {
            IntervalKind::Focused => (44.0, 15.0, 0.9, "focus"),
            IntervalKind::Open => (79.0, 11.0, 0.38, "open"),
        };
        let x = left + ((interval.started_at.max(start_ts) - start_ts) as f64 / duration) * chart_w;
        let end_x = left + ((interval.ended_at.min(end_ts) - start_ts) as f64 / duration) * chart_w;
        let w = (end_x - x).max(1.2);
        blocks.push_str(&format!(
            r##"<rect x="{x:.2}" y="{lane_y:.2}" width="{w:.2}" height="{height:.2}" rx="4" fill="{color}" opacity="{opacity}">
<title>{kind}: {app} / {duration}</title></rect>"##,
            app = escape_html(&app_name(&interval.app_class)),
            duration = escape_html(&report::format_duration(
                interval.ended_at.saturating_sub(interval.started_at)
            )),
        ));
    }

    for interval in system_intervals.iter().take(400) {
        let (lane_y, color, kind) = match interval.kind {
            SystemIntervalKind::Sleep => (114.0, "#f59f53", "sleep"),
            SystemIntervalKind::Unobserved => (139.0, "#ff6f7f", "tracker off"),
        };
        let x = left + ((interval.started_at.max(start_ts) - start_ts) as f64 / duration) * chart_w;
        let end_x = left + ((interval.ended_at.min(end_ts) - start_ts) as f64 / duration) * chart_w;
        let w = (end_x - x).max(1.2);
        blocks.push_str(&format!(
            r##"<rect x="{x:.2}" y="{lane_y:.2}" width="{w:.2}" height="12" rx="4" fill="{color}" opacity="0.82">
<title>{kind}: {duration}</title></rect>"##,
            duration = escape_html(&report::format_duration(
                interval.ended_at.saturating_sub(interval.started_at)
            )),
        ));
    }

    let labels = [
        ("Focused", 56.0),
        ("Open", 89.0),
        ("Sleep", 124.0),
        ("Tracker off", 149.0),
    ]
    .into_iter()
    .map(|(label, y)| {
        format!(
            r##"<text x="8" y="{y:.2}" font-size="12" font-weight="900" fill="#a8acb8">{}</text>"##,
            escape_html(label)
        )
    })
    .collect::<Vec<_>>()
    .join("");
    let ticks = timeline_ticks(start_ts, end_ts, left, chart_w, 166.0, 188.0);

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 1080 198" role="img" aria-label="Activity timeline">
<rect x="118" y="28" width="{chart_w:.2}" height="134" rx="6" fill="rgba(255,255,255,0.035)" />
{ticks}
{labels}
{blocks}
</svg></div>"##
    )
}

fn interval_table(
    intervals: &[TimelineInterval],
    system_intervals: &[SystemTimelineInterval],
    rows: &[AppTotals],
    start_ts: i64,
    end_ts: i64,
) -> String {
    let class_rank = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (row.app_class.clone(), index))
        .collect::<HashMap<_, _>>();
    let span = end_ts.saturating_sub(start_ts);
    let mut all_rows = intervals
        .iter()
        .map(|interval| {
            let rank = class_rank
                .get(&interval.app_class)
                .copied()
                .unwrap_or(PALETTE.len() - 1);
            let kind = match interval.kind {
                IntervalKind::Focused => "Focus",
                IntervalKind::Open => "Open",
            };
            (
                interval.started_at,
                interval.ended_at,
                kind.to_string(),
                app_name(&interval.app_class),
                PALETTE[rank % PALETTE.len()],
            )
        })
        .chain(system_intervals.iter().map(|interval| {
            let (kind, label, color) = match interval.kind {
                SystemIntervalKind::Sleep => ("Sleep", "System sleep".to_string(), "#f59f53"),
                SystemIntervalKind::Unobserved => (
                    "Gap",
                    interval
                        .source
                        .as_deref()
                        .filter(|source| !source.trim().is_empty())
                        .unwrap_or("Tracker off")
                        .to_string(),
                    "#ff6f7f",
                ),
            };
            (
                interval.started_at,
                interval.ended_at,
                kind.to_string(),
                label,
                color,
            )
        }))
        .collect::<Vec<_>>();
    all_rows.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

    if all_rows.is_empty() {
        return "No intervals recorded for this period.".to_string();
    }

    let rows = all_rows
        .into_iter()
        .take(14)
        .map(|(started_at, ended_at, kind, label, color)| {
            format!(
                r#"<div class="interval-row">
  <span class="interval-kind" style="color:{color}">{kind}</span>
  <span class="interval-name">{name}</span>
  <span class="interval-time">{time}</span>
  <span class="interval-duration">{duration}</span>
</div>"#,
                name = escape_html(&label),
                time = escape_html(&compact_time_range(started_at, ended_at, span)),
                duration = escape_html(&report::format_duration(
                    ended_at.saturating_sub(started_at)
                )),
            )
        })
        .collect::<Vec<_>>();
    format!(r#"<div class="interval-list">{}</div>"#, rows.join("\n"))
}

fn gap_summary(system_intervals: &[SystemTimelineInterval], report: &UsageReport) -> String {
    let sleep_count = system_intervals
        .iter()
        .filter(|interval| interval.kind == SystemIntervalKind::Sleep)
        .count();
    let unobserved_count = system_intervals
        .iter()
        .filter(|interval| interval.kind == SystemIntervalKind::Unobserved)
        .count();
    let excluded = report
        .total_sleep_seconds
        .saturating_add(report.total_unobserved_seconds);
    if excluded <= 0 {
        return "No sleep or tracker-off gaps in this period.".to_string();
    }
    let signal_total = report
        .total_focused_seconds
        .saturating_add(report.total_idle_seconds)
        .saturating_add(report.total_locked_seconds)
        .saturating_add(excluded)
        .max(1);
    metric_rows(&[
        (
            "Sleep",
            format!(
                "{} in {} intervals",
                report::format_duration(report.total_sleep_seconds),
                sleep_count
            ),
        ),
        (
            "Tracker off",
            format!(
                "{} in {} intervals",
                report::format_duration(report.total_unobserved_seconds),
                unobserved_count
            ),
        ),
        (
            "Not counted",
            format!(
                "{} ({})",
                report::format_duration(excluded),
                report::percent(excluded as f64 / signal_total as f64)
            ),
        ),
    ])
}

fn system_health_panel(
    status: &StorageStatus,
    report: &UsageReport,
    warnings: &[String],
) -> String {
    let active_total = status
        .focused_active
        .saturating_add(status.idle_active)
        .saturating_add(status.locked_active)
        .saturating_add(status.sleep_active)
        .saturating_add(status.daemon_active);
    let mut rows = vec![
        ("Rows", compact_count(status.interval_count)),
        ("Active intervals", active_total.to_string()),
        (
            "Live",
            format!(
                "{} focus / {} idle / {} locked / {} sleep / {} daemon",
                status.focused_active,
                status.idle_active,
                status.locked_active,
                status.sleep_active,
                status.daemon_active
            ),
        ),
        ("Last event", ago_label(status.last_event_at)),
        ("Heartbeat", ago_label(status.last_heartbeat_at)),
        (
            "Loaded period",
            report::format_duration(report.total_focused_seconds),
        ),
    ];
    rows.extend(warnings.iter().map(|warning| ("Warning", warning.clone())));
    metric_rows(&rows)
}

fn metric_rows(rows: &[(&str, String)]) -> String {
    let rows = rows
        .iter()
        .map(|(label, value)| {
            format!(
                r#"<div class="metric-row"><span>{}</span><strong>{}</strong></div>"#,
                escape_html(label),
                escape_html(value),
            )
        })
        .collect::<Vec<_>>();
    format!(r#"<div class="metric-list">{}</div>"#, rows.join("\n"))
}

fn timeline_chart(
    focus_intervals: &[TimelineInterval],
    rows: &[AppTotals],
    start_ts: i64,
    end_ts: i64,
) -> String {
    if focus_intervals.is_empty() || end_ts <= start_ts {
        return r#"<div class="chart-frame">No focus intervals for this period.</div>"#.to_string();
    }

    let top_classes = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(8)
        .map(|row| row.app_class.clone())
        .collect::<Vec<_>>();
    let class_rank = top_classes
        .iter()
        .enumerate()
        .map(|(index, app_class)| (app_class.clone(), index))
        .collect::<HashMap<_, _>>();
    let mut lanes = top_classes
        .iter()
        .map(|app_class| (app_class.clone(), report::app_label(app_class)))
        .collect::<Vec<_>>();
    if focus_intervals
        .iter()
        .any(|interval| !class_rank.contains_key(&interval.app_class))
    {
        lanes.push(("Other".to_string(), "Other".to_string()));
    }
    let lane_count = lanes.len().max(1);
    let lane_h = 30.0;
    let top = 34.0;
    let left = 128.0;
    let width = 1080.0;
    let chart_w = width - left - 28.0;
    let height = top + lane_count as f64 * lane_h + 54.0;
    let duration = (end_ts - start_ts).max(1) as f64;
    let mut blocks = String::new();

    for interval in focus_intervals.iter().take(700) {
        let lane_index = class_rank
            .get(&interval.app_class)
            .copied()
            .unwrap_or_else(|| lane_count.saturating_sub(1));
        let x = left + ((interval.started_at.max(start_ts) - start_ts) as f64 / duration) * chart_w;
        let end_x = left + ((interval.ended_at.min(end_ts) - start_ts) as f64 / duration) * chart_w;
        let w = (end_x - x).max(1.5);
        let y = top + lane_index as f64 * lane_h + 5.0;
        let color = PALETTE[lane_index % PALETTE.len()];
        blocks.push_str(&format!(
            r##"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="18" rx="4" fill="{color}" opacity="0.88">
<title>{app}: {duration}</title></rect>"##,
            app = escape_html(&app_name(&interval.app_class)),
            duration = escape_html(&report::format_duration(
                interval.ended_at.saturating_sub(interval.started_at)
            )),
        ));
    }

    let lane_labels = lanes
        .iter()
        .enumerate()
        .map(|(index, (_, label))| {
            format!(
                r##"<text x="8" y="{:.2}" font-size="12" font-weight="900" fill="#a8acb8">{}</text>"##,
                top + index as f64 * lane_h + 19.0,
                escape_html(&widgets_fit(label, 16)),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    let mut ticks = String::new();
    for step in 0..=4 {
        let x = left + chart_w * step as f64 / 4.0;
        let ts = start_ts + ((end_ts - start_ts) as f64 * step as f64 / 4.0).round() as i64;
        ticks.push_str(&format!(
            r##"<line x1="{x:.2}" y1="28" x2="{x:.2}" y2="{axis_y:.2}" stroke="rgba(255,255,255,0.08)" />
<text x="{x:.2}" y="{label_y:.2}" text-anchor="middle" font-size="11" font-weight="850" fill="#a8acb8">{label}</text>"##,
            axis_y = height - 28.0,
            label_y = height - 8.0,
            label = escape_html(&format_time_tick(ts)),
        ));
    }

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 1080 {height:.0}" role="img" aria-label="Focused interval timeline">
<rect x="128" y="28" width="{chart_w:.2}" height="{chart_h:.2}" rx="6" fill="rgba(255,255,255,0.035)" />
{ticks}
{lane_labels}
{blocks}
</svg></div>"##,
        chart_h = height - 58.0,
    )
}

fn time_breakdown(report: &UsageReport, stats: &ExportStats) -> String {
    let rows = [
        ("Focused", report.total_focused_seconds, "#43d9e8"),
        ("Idle", report.total_idle_seconds, "#9d83f7"),
        ("Locked", report.total_locked_seconds, "#f276b6"),
        ("Sleep", report.total_sleep_seconds, "#f59f53"),
        ("Tracker off", report.total_unobserved_seconds, "#ff6f7f"),
    ];
    let total = rows
        .iter()
        .map(|(_, seconds, _)| *seconds)
        .sum::<i64>()
        .max(1);
    let segments = rows
        .iter()
        .filter(|(_, seconds, _)| *seconds > 0)
        .map(|(label, seconds, color)| {
            format!(
                r#"<div class="breakdown-segment" style="width:{:.2}%;background:{color}" title="{} - {}"></div>"#,
                ratio(*seconds, total) * 100.0,
                escape_html(label),
                escape_html(&report::format_duration(*seconds)),
            )
        })
        .collect::<Vec<_>>();
    let list = rows
        .iter()
        .filter(|(_, seconds, _)| *seconds > 0)
        .map(|(label, seconds, color)| {
            format!(
                r#"<div class="breakdown-row">
  <span class="breakdown-dot" style="background:{color}"></span>
  <span class="breakdown-label">{label}</span>
  <span class="breakdown-value">{value}</span>
</div>"#,
                label = escape_html(label),
                value = escape_html(&report::format_duration(*seconds)),
            )
        })
        .collect::<Vec<_>>();
    let note = format!(
        "{} total days · daily average {}",
        stats.total_days,
        report::format_duration(stats.daily_average_seconds)
    );
    format!(
        r#"<div class="breakdown-strip">{}</div><div class="breakdown-list">{}</div><p class="summary-note">{}</p>"#,
        segments.join(""),
        list.join("\n"),
        escape_html(&note),
    )
}

fn stacked_day_chart(
    days: &[DayTotals],
    daily_apps: &[AppDayTotals],
    rows: &[AppTotals],
) -> String {
    let mut visible_days = visible_chart_days(days);
    let app_dates = daily_apps
        .iter()
        .map(|row| row.date.clone())
        .collect::<HashSet<_>>();
    if app_dates.len() == 1 {
        visible_days.retain(|day| app_dates.contains(&day.date));
    }
    if visible_days.is_empty() {
        return r#"<div class="chart-frame">No daily data for this period.</div>"#.to_string();
    }

    let top_classes = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(6)
        .map(|row| row.app_class.clone())
        .collect::<Vec<_>>();
    let top_set = top_classes.iter().cloned().collect::<HashSet<_>>();
    let mut by_day = BTreeMap::<String, BTreeMap<String, i64>>::new();
    for day in &visible_days {
        by_day.entry(day.date.clone()).or_default();
    }
    for row in daily_apps {
        if !by_day.contains_key(&row.date) {
            continue;
        }
        let bucket = if top_set.contains(&row.app_class) {
            row.app_class.clone()
        } else {
            "Other".to_string()
        };
        *by_day
            .entry(row.date.clone())
            .or_default()
            .entry(bucket)
            .or_default() += row.focused_seconds;
    }

    let max_day = visible_days
        .iter()
        .map(|day| day.focused_seconds.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let peak = visible_days
        .iter()
        .enumerate()
        .max_by_key(|(_, day)| day.focused_seconds)
        .map(|(index, day)| (index, *day));
    let width = 1080.0;
    let left = 48.0;
    let top = 34.0;
    let chart_h = 250.0;
    let chart_w = width - left - 30.0;
    let gap = if visible_days.len() > 60 {
        2.0
    } else if visible_days.len() > 32 {
        3.0
    } else {
        5.0
    };
    let bar_w = ((chart_w - gap * (visible_days.len().saturating_sub(1) as f64))
        / visible_days.len() as f64)
        .max(2.0);
    let mut segments = String::new();
    let mut labels = String::new();
    let label_step = (visible_days.len() / 9).max(1);
    let mut series = top_classes.clone();
    series.push("Other".to_string());

    for (index, day) in visible_days.iter().enumerate() {
        let x = left + index as f64 * (bar_w + gap);
        let mut y_cursor = top + chart_h;
        if let Some(buckets) = by_day.get(&day.date) {
            for (series_index, class) in series.iter().enumerate() {
                let seconds = buckets.get(class).copied().unwrap_or(0);
                if seconds <= 0 {
                    continue;
                }
                let h = (seconds as f64 / max_day as f64) * chart_h;
                y_cursor -= h;
                segments.push_str(&format!(
                    r##"<rect x="{x:.2}" y="{y_cursor:.2}" width="{bar_w:.2}" height="{h:.2}" rx="2" fill="{}"><title>{}: {} - {}</title></rect>"##,
                    PALETTE[series_index % PALETTE.len()],
                    escape_html(&day.label),
                    escape_html(&app_name(class)),
                    escape_html(&report::format_duration(seconds)),
                ));
            }
        }

        if index % label_step == 0 || index + 1 == visible_days.len() {
            labels.push_str(&format!(
                r##"<text x="{:.2}" y="326" text-anchor="middle" font-size="12" font-weight="850" fill="#9ab8bd">{}</text>"##,
                x + bar_w / 2.0,
                escape_html(&short_date(&day.date)),
            ));
        }
    }

    let mut annotation = String::new();
    if let Some((index, day)) = peak
        && day.focused_seconds > 0
    {
        let x = left + index as f64 * (bar_w + gap) + bar_w / 2.0;
        let bar_h = (day.focused_seconds as f64 / max_day as f64) * chart_h;
        let y = top + chart_h - bar_h;
        annotation = format!(
            r##"<line x1="{x:.2}" y1="{y:.2}" x2="{x:.2}" y2="18" stroke="#ffd166" stroke-width="2" />
<rect x="{label_x:.2}" y="2" width="164" height="38" rx="4" fill="#ffd166" />
<text x="{text_x:.2}" y="18" font-size="11" font-weight="950" fill="#06181f">Peak: {date}</text>
<text x="{text_x:.2}" y="32" font-size="11" font-weight="850" fill="#06181f">{duration}</text>"##,
            label_x = (x + 8.0).min(width - 174.0),
            text_x = (x + 18.0).min(width - 164.0),
            date = escape_html(&day.label),
            duration = escape_html(&report::format_duration(day.focused_seconds)),
        );
    }

    let legend = series
        .iter()
        .enumerate()
        .filter(|(_, class)| *class == "Other" || top_set.contains(*class))
        .map(|(index, class)| {
            format!(
                r#"<span class="legend-chip"><i class="swatch" style="color:{color};background:{color}"></i>{}</span>"#,
                escape_html(&app_name(class)),
                color = PALETTE[index % PALETTE.len()],
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 1080 360" role="img" aria-label="Stacked daily app focus chart">
<line x1="48" y1="284" x2="1050" y2="284" stroke="rgba(255,255,255,0.22)" stroke-width="1" />
<line x1="48" y1="159" x2="1050" y2="159" stroke="rgba(255,255,255,0.10)" stroke-width="1" />
<text x="6" y="42" font-size="12" font-weight="900" fill="#9ab8bd">{max_label}</text>
{segments}
{annotation}
{labels}
</svg></div><div class="legend-strip">{legend}</div>"##,
        max_label = escape_html(&report::format_duration(max_day)),
    )
}

fn ranked_apps(rows: &[AppTotals], total: i64) -> String {
    let max = rows
        .iter()
        .map(|row| row.focused_seconds.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let ranked = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(8)
        .enumerate()
        .map(|(index, row)| {
            let color = PALETTE[index % PALETTE.len()];
            let width = ratio(row.focused_seconds, max) * 100.0;
            let share = report::percent(ratio(row.focused_seconds, total));
            format!(
                r#"<div class="rank-row">
  <div class="rank-index">{rank}</div>
  <div>
    <div class="rank-name">{name}</div>
    <div class="rank-meta">{share} of focused time</div>
  </div>
  <div class="rank-time">{time}</div>
  <div class="rank-bar"><div class="rank-fill" style="width:{width:.2}%;color:{color};background:{color}"></div></div>
</div>"#,
                rank = index + 1,
                name = escape_html(&report::app_label(&row.app_class)),
                time = escape_html(&report::format_duration(row.focused_seconds)),
            )
        })
        .collect::<Vec<_>>();

    if ranked.is_empty() {
        "No focused app time in this period.".to_string()
    } else {
        format!(r#"<div class="ranked-list">{}</div>"#, ranked.join("\n"))
    }
}

fn heatmap_chart(cells: &[FocusHeatCell]) -> String {
    let max = cells
        .iter()
        .map(|cell| cell.focused_seconds.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let by_key = cells
        .iter()
        .map(|cell| ((cell.weekday, cell.hour), cell.focused_seconds))
        .collect::<HashMap<_, _>>();
    let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut rects = String::new();
    for weekday in 0..7_u32 {
        for hour in 0..24_u32 {
            let value = by_key.get(&(weekday, hour)).copied().unwrap_or(0);
            let x = 58.0 + hour as f64 * 22.0;
            let y = 28.0 + weekday as f64 * 24.0;
            let color = heat_color(ratio(value, max));
            rects.push_str(&format!(
                r##"<rect x="{x:.2}" y="{y:.2}" width="18" height="18" rx="3" fill="{color}">
<title>{day} {hour:02}:00 - {duration}</title></rect>"##,
                day = labels[weekday as usize],
                duration = escape_html(&report::format_duration(value)),
            ));
        }
    }
    let mut hour_labels = String::new();
    for hour in [0, 6, 12, 18, 23] {
        hour_labels.push_str(&format!(
            r##"<text x="{:.2}" y="222" text-anchor="middle" font-size="11" font-weight="850" fill="#9ab8bd">{:02}</text>"##,
            67.0 + hour as f64 * 22.0,
            hour,
        ));
    }
    let day_labels = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            format!(
                r##"<text x="8" y="{:.2}" font-size="12" font-weight="900" fill="#9ab8bd">{}</text>"##,
                42.0 + index as f64 * 24.0,
                label,
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 610 240" role="img" aria-label="Weekday hour heatmap">
{day_labels}
{rects}
{hour_labels}
<text x="540" y="222" font-size="11" font-weight="850" fill="#9ab8bd">hour</text>
</svg></div>"##
    )
}

fn title_rows(titles: &[TitleTotals]) -> String {
    if titles.is_empty() {
        return r#"<div class="title-list">No title data captured for this period.</div>"#
            .to_string();
    }

    let rows = titles
        .iter()
        .take(9)
        .map(|title| {
            format!(
                r#"<div class="title-row">
  <div>
    <div class="title-app">{}</div>
    <div class="title-name">{}</div>
  </div>
  <div class="title-time">{}</div>
</div>"#,
                escape_html(&report::app_label(&title.app_class)),
                escape_html(&title.title),
                escape_html(&report::format_duration(title.focused_seconds))
            )
        })
        .collect::<Vec<_>>();
    format!(r#"<div class="title-list">{}</div>"#, rows.join("\n"))
}

fn insight_rows(insights: &[Insight]) -> String {
    if insights.is_empty() {
        return r#"<div class="insight-list">No evaluated insights for this period yet.</div>"#
            .to_string();
    }

    let rows = insights
        .iter()
        .take(6)
        .map(|insight| {
            format!(
                r#"<div class="insight-row">
  <div>
    <div class="insight-meta">{meta}</div>
    <div class="insight-title">{title}</div>
    <div class="insight-explanation">{explanation}</div>
  </div>
  <div class="insight-value">{value}</div>
</div>"#,
                meta = escape_html(&format!(
                    "{} / {}",
                    insight_category_label(insight.category),
                    insight_tone_label(insight.tone)
                )),
                title = escape_html(&insight.title),
                explanation = escape_html(&insight.explanation),
                value = escape_html(&insight.value),
            )
        })
        .collect::<Vec<_>>();

    format!(r#"<div class="insight-list">{}</div>"#, rows.join("\n"))
}

fn insight_category_label(category: InsightCategory) -> &'static str {
    match category {
        InsightCategory::Patterns => "Patterns",
        InsightCategory::FocusQuality => "Focus quality",
        InsightCategory::Apps => "Apps",
        InsightCategory::SystemSignals => "System signals",
    }
}

fn insight_tone_label(tone: InsightTone) -> &'static str {
    match tone {
        InsightTone::Positive => "Positive",
        InsightTone::Negative => "Negative",
        InsightTone::Neutral => "Neutral",
        InsightTone::Info => "Info",
        InsightTone::Caution => "Caution",
    }
}

fn visible_chart_days(days: &[DayTotals]) -> Vec<&DayTotals> {
    let today = clock::local_now().format("%Y-%m-%d").to_string();
    days.iter()
        .filter(|day| {
            day.date.as_str() <= today.as_str()
                || day.focused_seconds > 0
                || day.open_seconds > 0
                || day.idle_seconds > 0
                || day.locked_seconds > 0
                || day.sleep_seconds > 0
                || day.unobserved_seconds > 0
        })
        .collect()
}

fn heat_color(value: f64) -> String {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.0 {
        return "rgba(255,255,255,0.055)".to_string();
    }
    if value < 0.33 {
        let t = value / 0.33;
        return mix_hex((13, 58, 66), (77, 232, 255), t);
    }
    if value < 0.72 {
        let t = (value - 0.33) / 0.39;
        return mix_hex((77, 232, 255), (70, 211, 105), t);
    }
    let t = (value - 0.72) / 0.28;
    mix_hex((70, 211, 105), (255, 209, 102), t)
}

fn mix_hex(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> String {
    let blend = |left: u8, right: u8| left as f64 + (right as f64 - left as f64) * t;
    format!(
        "#{:02x}{:02x}{:02x}",
        blend(a.0, b.0).round() as u8,
        blend(a.1, b.1).round() as u8,
        blend(a.2, b.2).round() as u8
    )
}

fn app_name(app_class: &str) -> String {
    if app_class == "Other" {
        "Other".to_string()
    } else {
        report::app_label(app_class)
    }
}

fn period_range_label(report: &UsageReport) -> String {
    match (
        report.period.start_date.as_deref(),
        report.period.end_date.as_deref(),
    ) {
        (Some(start), Some(end)) if start == end => start.to_string(),
        (Some(start), Some(end)) => format!("{start} to {end}"),
        _ => "Lifetime".to_string(),
    }
}

fn format_timestamp(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                time.year(),
                time.month(),
                time.day(),
                time.hour(),
                time.minute()
            )
        })
        .unwrap_or_else(|| timestamp.to_string())
}

fn short_date(date: &str) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|date| format!("{}/{}", date.month(), date.day()))
        .unwrap_or_else(|_| date.to_string())
}

fn format_time_tick(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| time.format("%m/%d %H:%M").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn timeline_ticks(
    start_ts: i64,
    end_ts: i64,
    left: f64,
    chart_w: f64,
    axis_y: f64,
    label_y: f64,
) -> String {
    let mut ticks = String::new();
    for step in 0..=4 {
        let x = left + chart_w * step as f64 / 4.0;
        let ts = start_ts + ((end_ts - start_ts) as f64 * step as f64 / 4.0).round() as i64;
        ticks.push_str(&format!(
            r##"<line x1="{x:.2}" y1="28" x2="{x:.2}" y2="{axis_y:.2}" stroke="rgba(255,255,255,0.08)" />
<text x="{x:.2}" y="{label_y:.2}" text-anchor="middle" font-size="11" font-weight="850" fill="#a8acb8">{label}</text>"##,
            label = escape_html(&format_time_tick(ts)),
        ));
    }
    ticks
}

fn compact_time_range(started_at: i64, ended_at: i64, span_seconds: i64) -> String {
    format!(
        "{}-{}",
        compact_time_endpoint(started_at, span_seconds),
        compact_time_endpoint(ended_at, span_seconds)
    )
}

fn compact_time_endpoint(timestamp: i64, span_seconds: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| {
            if span_seconds <= 36 * 3600 {
                time.format("%H:%M").to_string()
            } else {
                time.format("%m/%d %H:%M").to_string()
            }
        })
        .unwrap_or_else(|| timestamp.to_string())
}

fn compact_count(value: i64) -> String {
    if value.abs() >= 1_000_000 {
        format!("{:.1}m", value as f64 / 1_000_000.0)
    } else if value.abs() >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn ago_label(timestamp: Option<i64>) -> String {
    let Some(timestamp) = timestamp else {
        return "never".to_string();
    };
    let seconds = clock::local_now()
        .timestamp()
        .saturating_sub(timestamp)
        .max(0);
    format!("{} ago", report::format_duration(seconds))
}

fn widgets_fit(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    value
        .chars()
        .take(width.saturating_sub(3))
        .collect::<String>()
        + "..."
}

fn hour_totals(cells: &[FocusHeatCell]) -> Vec<(u32, i64)> {
    analytics::hour_totals(cells)
}

fn hour_label(hour: u32) -> String {
    format!("{:02}:00", hour.min(23))
}

fn excluded_seconds(day: &DayTotals) -> i64 {
    day.idle_seconds
        .saturating_add(day.locked_seconds)
        .saturating_add(day.sleep_seconds)
        .saturating_add(day.unobserved_seconds)
}

fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        (value.max(0) as f64 / total as f64).clamp(0.0, 1.0)
    }
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{
        DataExportOptions, DataExportScope, ExportOptions, build_data_export, render_html,
        write_data_export_csv,
    };
    use crate::{
        clock,
        config::Config,
        report::Lens,
        steam::SteamResolver,
        storage::{IntervalKind, Storage},
    };

    #[test]
    fn renders_one_page_export_with_escaped_title_data() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let end = clock::local_now().timestamp();
        let start = end - 1800;
        let focused = storage
            .start_interval(
                IntervalKind::Focused,
                "firefox",
                None,
                Some("Docs <Dashboard>"),
                start,
            )
            .unwrap();
        storage.close_interval(focused, end).unwrap();

        let mut steam = SteamResolver::default();
        let html = render_html(
            &storage,
            &mut steam,
            &config,
            ExportOptions {
                lens: Lens::Day,
                offset: 0,
                title: Some("Usage Export".to_string()),
            },
        )
        .unwrap();

        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("Daily pattern"));
        assert!(html.contains("Period insights"));
        assert!(html.contains("Week x hour heatmap"));
        assert!(html.contains("Focus length distribution"));
        assert!(html.contains("Workspace focus"));
        assert!(html.contains("App table"));
        assert!(html.contains("Activity timeline"));
        assert!(html.contains("System health"));
        assert!(html.contains("omastat-data"));
        assert!(html.contains("Docs &lt;Dashboard&gt;"));
    }

    #[test]
    fn builds_json_data_export_with_raw_and_aggregate_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let focused = storage
            .start_interval(IntervalKind::Focused, "firefox", None, Some("Docs"), 100)
            .unwrap();
        storage.close_interval(focused, 220).unwrap();

        let mut steam = SteamResolver::default();
        let export = build_data_export(
            &storage,
            &mut steam,
            &config,
            DataExportOptions {
                lens: Lens::Life,
                offset: 0,
                scope: DataExportScope::All,
            },
        )
        .unwrap();

        assert!(export.aggregate.as_ref().unwrap().app_totals.len() == 1);
        assert!(export.raw.as_ref().unwrap().intervals.len() == 1);
        let json = serde_json::to_value(&export).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert!(json["raw"]["intervals"][0]["local_start"].is_string());
    }

    #[test]
    fn writes_csv_data_export_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let focused = storage
            .start_interval(IntervalKind::Focused, "firefox", None, None, 100)
            .unwrap();
        storage.close_interval(focused, 200).unwrap();

        let mut steam = SteamResolver::default();
        let export = build_data_export(
            &storage,
            &mut steam,
            &config,
            DataExportOptions {
                lens: Lens::Life,
                offset: 0,
                scope: DataExportScope::All,
            },
        )
        .unwrap();
        let out = dir.path().join("csv");
        write_data_export_csv(&export, &out).unwrap();

        assert!(out.join("metadata.json").exists());
        assert!(out.join("app_totals.csv").exists());
        assert!(out.join("raw_intervals.csv").exists());
    }
}
