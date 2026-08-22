use crate::{
    config::Config,
    insights::{Insight, InsightCategory, InsightTone},
    report::{self, Lens, UsageReport},
    steam::SteamResolver,
    storage::{
        AppDayTotals, AppTotals, DayTotals, FocusHeatCell, RawExportRows, Storage,
        TimelineInterval, TitleTotals, WorkspaceTotals,
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
    let report = if options.lens == Lens::Day && options.offset == 0 {
        report::usage_report(
            storage,
            steam,
            config,
            options.lens,
            options.lens.history_days(),
        )?
    } else {
        report::usage_report_for_period(storage, steam, config, options.lens, options.offset)?
    };
    let (start_ts, end_ts) = (report.query_start_ts, report.query_end_ts);
    let mut rollups = storage.focused_rollups_between(start_ts, end_ts, 8, 64)?;
    for row in &mut rollups.daily_apps {
        row.app_class = steam.resolve_class(&row.app_class);
    }
    for interval in &mut rollups.focus_intervals {
        interval.app_class = steam.resolve_class(&interval.app_class);
    }
    let stats = ExportStats::from_data(&report, &rollups.heatmap, &rollups.focus_intervals);
    let titles = storage
        .focused_title_totals_between(start_ts, end_ts, 12)?
        .into_iter()
        .map(|mut row| {
            row.app_class = steam.resolve_class(&row.app_class);
            row
        })
        .collect::<Vec<_>>();
    let lens_cards = lens_cards(storage, steam, config)?;
    let page_title = options
        .title
        .unwrap_or_else(|| format!("Omastat Overview - {}", report.period.label));

    Ok(document(
        &page_title,
        &report,
        &rollups.daily_apps,
        &rollups.heatmap,
        &titles,
        &lens_cards,
        &rollups.workspaces,
        &rollups.focus_intervals,
        &stats,
    ))
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
        timezone: Local::now().offset().to_string(),
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

fn lens_cards(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
) -> Result<Vec<UsageReport>> {
    Lens::ALL
        .into_iter()
        .map(|lens| report::usage_report_for_period(storage, steam, config, lens, 0))
        .collect()
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
        let active_days = report
            .daily
            .iter()
            .filter(|day| day.focused_seconds > 0)
            .count();
        let daily_average_seconds = average(report.total_focused_seconds, total_days);
        let active_day_average_seconds = average(report.total_focused_seconds, active_days);
        let longest_streak_days = longest_focus_streak(&report.daily);

        let mut durations = focus_intervals
            .iter()
            .map(|interval| interval.ended_at.saturating_sub(interval.started_at))
            .filter(|seconds| *seconds > 0)
            .collect::<Vec<_>>();
        let focus_block_count = durations.len();
        let total_block_seconds = durations.iter().sum::<i64>();
        durations.sort_unstable();
        let average_block_seconds = average(total_block_seconds, focus_block_count);
        let median_block_seconds = median(&durations);
        let longest_block_seconds = durations.last().copied().unwrap_or_default();
        let deep_block_count = durations
            .iter()
            .filter(|seconds| **seconds >= 25 * 60)
            .count();
        let deep_block_seconds = durations
            .iter()
            .filter(|seconds| **seconds >= 25 * 60)
            .sum::<i64>();
        let app_switch_count = focus_intervals
            .windows(2)
            .filter(|pair| pair[0].app_class != pair[1].app_class)
            .count();
        let peak_hour = hour_totals(heatmap).into_iter().next();
        let top_app_share = report
            .rows
            .iter()
            .find(|row| row.focused_seconds > 0)
            .map(|row| ratio(row.focused_seconds, report.total_focused_seconds.max(1)))
            .unwrap_or_default();
        let effective_apps = effective_app_count(&report.rows, report.total_focused_seconds);

        Self {
            total_days,
            active_days,
            daily_average_seconds,
            active_day_average_seconds,
            longest_streak_days,
            focus_block_count,
            app_switch_count,
            average_block_seconds,
            median_block_seconds,
            longest_block_seconds,
            deep_block_count,
            deep_block_seconds,
            peak_hour,
            top_app_share,
            effective_apps,
        }
    }
}

fn document(
    page_title: &str,
    report: &UsageReport,
    daily_apps: &[AppDayTotals],
    heatmap: &[FocusHeatCell],
    titles: &[TitleTotals],
    lens_cards: &[UsageReport],
    workspaces: &[WorkspaceTotals],
    focus_intervals: &[TimelineInterval],
    stats: &ExportStats,
) -> String {
    let generated = format_timestamp(report.generated_at);
    let focused = report::format_duration(report.total_focused_seconds);
    let open = report::format_duration(report.total_open_seconds);
    let density = report::percent(ratio(
        report.total_focused_seconds,
        report.total_open_seconds.max(1),
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
    let focus_note = format!("{density} focused while open");
    let insights_html = insight_rows(&report.insights);
    let number_card_rows = vec![
        NumberCard::new("Focused", &focused, &focus_note),
        NumberCard::new("Daily avg", &daily_avg, &daily_note),
        NumberCard::new("Longest session", &longest_session, &session_note),
        NumberCard::new("App mix", &app_mix, &app_note),
        NumberCard::new("Streak", &streak_label, &streak_note),
        NumberCard::new("Peak hour", &peak_hour, "recurring focus window"),
        NumberCard::new("Peak day", &peak_day_label, &peak_day_duration),
        NumberCard::new("Open time", &open, "tracked beside focus"),
    ];
    let number_cards_html = number_cards(&number_card_rows);

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
      <h1>{title}</h1>
      <p class="subhead">{period} · {range} · generated {generated}</p>
    </div>
    <div class="focus-total">
      <small>Focused time</small>
      <strong>{focused}</strong>
      <span>{density} focus density</span>
    </div>
  </header>

  <section class="metric-grid" aria-label="Overview metrics">
    {number_cards}
  </section>

  <section class="grid grid-main">
    <article class="panel panel-wide">
      <div class="panel-heading">
        <div>
          <span class="kicker">Trend</span>
          <h2>Daily pattern</h2>
        </div>
        <p>Bars show focused time; the line shows focus density while apps were open.</p>
      </div>
      {daily_pattern}
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Composition</span>
          <h2>App mix</h2>
        </div>
      </div>
      {app_mix_panel}
    </article>
  </section>

  <section class="grid grid-secondary">
    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Timing</span>
          <h2>Week x hour heatmap</h2>
        </div>
        <p>Sequential color exposes recurring focus windows.</p>
      </div>
      {heatmap_chart}
    </article>

    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Timing</span>
          <h2>Top hours</h2>
        </div>
        <p>Ranked hours make peaks easy to compare.</p>
      </div>
      {top_hours}
    </article>
  </section>

  <section class="grid grid-secondary">
    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Place</span>
          <h2>Workspace focus</h2>
        </div>
        <p>Ranked workspaces show where focused time landed.</p>
      </div>
      {workspace_focus}
    </article>

    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Sessions</span>
          <h2>Focus length distribution</h2>
        </div>
        <p>Histogram bins reveal whether focus came in fragments or blocks.</p>
      </div>
      {session_histogram}
    </article>
  </section>

  <section class="grid grid-main">
    <article class="panel panel-wide">
      <div class="panel-heading">
        <div>
          <span class="kicker">Composition over time</span>
          <h2>App mix by day</h2>
        </div>
        <p>Stacked bars show which apps made up each day's focus.</p>
      </div>
      {stacked_days}
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">System signals</span>
          <h2>Counted vs excluded</h2>
        </div>
      </div>
      {time_breakdown}
    </article>
  </section>

  <section class="grid grid-tertiary">
    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Evaluated facts</span>
          <h2>Period insights</h2>
        </div>
      </div>
      {insights}
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Captured titles</span>
          <h2>Captured moments</h2>
        </div>
      </div>
      {title_rows}
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Lenses</span>
          <h2>Period lenses</h2>
        </div>
      </div>
      {lens_cards}
    </article>
  </section>

  <footer>
    <span>Generated from local Omastat data</span>
    <span>Self-contained HTML/SVG overview</span>
  </footer>
</main>
</body>
</html>
"#,
        title = escape_html(page_title),
        css = stylesheet(),
        period = escape_html(&report.period.label),
        range = escape_html(&period_range_label(report)),
        generated = escape_html(&generated),
        focused = escape_html(&focused),
        density = escape_html(&density),
        number_cards = number_cards_html,
        daily_pattern = daily_pattern_chart(&report.daily),
        app_mix_panel = app_mix_panel(&report.rows, report.total_focused_seconds),
        top_hours = top_hours_chart(heatmap),
        workspace_focus = workspace_focus_chart(workspaces, report.total_focused_seconds),
        session_histogram = session_histogram(focus_intervals, stats),
        stacked_days = stacked_day_chart(&report.daily, daily_apps, &report.rows),
        heatmap_chart = heatmap_chart(heatmap),
        time_breakdown = time_breakdown(report, stats),
        insights = insights_html,
        title_rows = title_rows(titles),
        lens_cards = lens_cards_html(lens_cards),
    )
}

fn stylesheet() -> &'static str {
    r#"
:root {
  color-scheme: dark;
  --bg: #0e1014;
  --bg-grid: rgba(255,255,255,0.035);
  --panel: #171a20;
  --panel-2: #1d2028;
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
  --shadow: 0 18px 50px rgba(0, 0, 0, 0.26);
}
* { box-sizing: border-box; }
body {
  margin: 0;
  color: var(--ink);
  background:
    linear-gradient(180deg, rgba(67,217,232,0.05), transparent 28rem),
    linear-gradient(135deg, #0e1014 0%, #121722 54%, #171219 100%);
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  letter-spacing: 0;
}
body::before {
  content: "";
  position: fixed;
  inset: 0;
  pointer-events: none;
  background-image:
    linear-gradient(var(--bg-grid) 1px, transparent 1px),
    linear-gradient(90deg, var(--bg-grid) 1px, transparent 1px);
  background-size: 40px 40px;
  mask-image: linear-gradient(to bottom, black, transparent 72%);
}
.dashboard {
  position: relative;
  width: min(1420px, calc(100vw - 32px));
  margin: 0 auto;
  padding: 28px 0 38px;
}
.dashboard-header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(280px, 380px);
  gap: 16px;
  align-items: end;
  margin-bottom: 16px;
}
.panel, .number-card, .focus-total {
  border: 1px solid var(--line);
  border-radius: 8px;
  background: linear-gradient(145deg, rgba(29, 32, 40, 0.96), rgba(18, 20, 25, 0.96));
  box-shadow: var(--shadow), inset 0 1px 0 rgba(255,255,255,0.06);
}
.eyebrow, .kicker, .number-card small, .focus-total small, footer, .mini-label {
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
  font-size: clamp(2.6rem, 6vw, 5.6rem);
  line-height: 0.92;
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
.focus-total {
  min-height: 156px;
  padding: 22px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  background:
    linear-gradient(145deg, rgba(246,196,90,0.2), rgba(67,217,232,0.08)),
    linear-gradient(145deg, rgba(32,36,45,0.98), rgba(18,20,25,0.98));
}
.focus-total strong {
  display: block;
  margin: 12px 0 8px;
  font-size: clamp(3rem, 5vw, 4.9rem);
  line-height: 0.88;
}
.focus-total span {
  color: #ffe4a8;
  font-weight: 850;
}
.metric-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
  margin-bottom: 16px;
}
.number-card {
  min-height: 112px;
  padding: 16px;
  overflow: hidden;
}
.number-card strong {
  display: block;
  margin-top: 14px;
  font-size: 1.85rem;
  line-height: 0.95;
  overflow-wrap: anywhere;
}
.number-card span {
  display: block;
  margin-top: 8px;
  color: #dfd1c3;
  font-weight: 750;
}
.grid {
  display: grid;
  gap: 16px;
  margin-bottom: 16px;
}
.grid-main { grid-template-columns: minmax(0, 1.45fr) minmax(360px, 0.75fr); }
.grid-secondary { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
.grid-tertiary { grid-template-columns: repeat(auto-fit, minmax(310px, 1fr)); }
.panel {
  min-width: 0;
  padding: 18px;
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
  border: 1px solid var(--line);
  border-radius: 6px;
  background: rgba(6, 8, 12, 0.34);
  padding: 12px;
}
.chart-frame svg { width: 100%; height: auto; display: block; overflow: visible; }
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
  box-shadow: 0 0 16px currentColor;
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
  border: 1px solid var(--line-strong);
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
  border: 1px solid var(--line);
  background: rgba(255,255,255,0.08);
}
.rank-fill {
  height: 100%;
  box-shadow: 0 0 18px currentColor;
}
.mix-strip, .breakdown-strip {
  display: flex;
  width: 100%;
  height: 18px;
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: rgba(255,255,255,0.08);
  margin-bottom: 16px;
}
.mix-segment, .breakdown-segment {
  min-width: 2px;
  height: 100%;
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
  border: 1px solid var(--line);
  background: rgba(255,255,255,0.055);
}
.hist-fill {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  background: linear-gradient(180deg, var(--cyan), var(--purple));
  box-shadow: 0 0 20px rgba(67,217,232,0.28);
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
.lens-card {
  display: grid;
  grid-template-columns: 76px 1fr;
  gap: 12px;
  align-items: center;
  padding: 12px;
  border-radius: 6px;
  border: 1px solid rgba(255,255,255,0.1);
  background: rgba(255,255,255,0.045);
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
  .dashboard-header, .grid-main, .grid-secondary, .grid-tertiary, .metric-grid {
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
                r#"<article class="number-card"><small>{}</small><strong>{}</strong><span>{}</span></article>"#,
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
    let mut density_points = Vec::new();
    let mut labels = String::new();

    for (index, day) in visible_days.iter().enumerate() {
        let x = left + index as f64 * (bar_w + gap);
        let focus_height = (day.focused_seconds.max(0) as f64 / max_focus as f64) * chart_h;
        let y = top + chart_h - focus_height;
        let density = ratio(day.focused_seconds, day.open_seconds.max(1));
        let density_y = top + chart_h - density * chart_h;
        let cx = x + bar_w / 2.0;
        density_points.push(format!("{cx:.2},{density_y:.2}"));
        let excluded = excluded_seconds(day);
        let opacity = if day.focused_seconds > 0 { 0.95 } else { 0.24 };
        bars.push_str(&format!(
            r##"<rect x="{x:.2}" y="{y:.2}" width="{bar_w:.2}" height="{focus_height:.2}" rx="3" fill="url(#focusBar)" opacity="{opacity:.2}">
  <title>{date}: {focus} focused, {density} focus density, {excluded} excluded</title>
</rect>"##,
            date = escape_html(&day.label),
            focus = escape_html(&report::format_duration(day.focused_seconds)),
            density = escape_html(&report::percent(density)),
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
    let density_line = density_points.join(" ");

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 1080 365" role="img" aria-label="Daily focused time with focus density line">
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
<text x="1012" y="34" font-size="12" font-weight="900" fill="#f6c45a">100%</text>
{bars}
<polyline points="{density_line}" fill="none" stroke="#f6c45a" stroke-width="4" stroke-linecap="round" stroke-linejoin="round" />
{annotation}
{labels}
</svg></div>
<div class="legend-strip">
  <span class="legend-chip"><i class="swatch" style="color:#43d9e8;background:#43d9e8"></i>Focused time</span>
  <span class="legend-chip"><i class="swatch" style="color:#f6c45a;background:#f6c45a"></i>Focus density</span>
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

fn lens_cards_html(reports: &[UsageReport]) -> String {
    let rows = reports
        .iter()
        .map(|report| {
            let top = report
                .rows
                .iter()
                .find(|row| row.focused_seconds > 0)
                .map(|row| report::app_label(&row.app_class))
                .unwrap_or_else(|| "No focus yet".to_string());
            format!(
                r#"<article class="lens-card">
  <div class="lens-label">{}</div>
  <div>
    <div class="lens-total">{}</div>
    <div class="lens-meta">{}</div>
  </div>
</article>"#,
                escape_html(report.lens_label),
                escape_html(&report::format_duration(report.total_focused_seconds)),
                escape_html(&top),
            )
        })
        .collect::<Vec<_>>();
    format!(r#"<div class="lens-list">{}</div>"#, rows.join("\n"))
}

fn longest_focus_streak(days: &[DayTotals]) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for day in days {
        if day.focused_seconds > 0 {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn visible_chart_days(days: &[DayTotals]) -> Vec<&DayTotals> {
    let today = Local::now().format("%Y-%m-%d").to_string();
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

fn average(total: i64, count: usize) -> i64 {
    if count == 0 {
        0
    } else {
        total.max(0) / count as i64
    }
}

fn median(sorted: &[i64]) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2
    } else {
        sorted[mid]
    }
}

fn hour_totals(cells: &[FocusHeatCell]) -> Vec<(u32, i64)> {
    let mut totals = [0_i64; 24];
    for cell in cells {
        if let Some(total) = totals.get_mut(cell.hour as usize) {
            *total += cell.focused_seconds.max(0);
        }
    }
    let mut rows = totals
        .into_iter()
        .enumerate()
        .map(|(hour, focused_seconds)| (hour as u32, focused_seconds))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows
}

fn hour_label(hour: u32) -> String {
    format!("{:02}:00", hour.min(23))
}

fn effective_app_count(rows: &[AppTotals], total_focused_seconds: i64) -> f64 {
    if total_focused_seconds <= 0 {
        return 0.0;
    }

    let entropy = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .map(|row| row.focused_seconds as f64 / total_focused_seconds as f64)
        .filter(|share| *share > 0.0)
        .map(|share| -share * share.ln())
        .sum::<f64>();
    entropy.exp()
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
        config::Config,
        report::Lens,
        steam::SteamResolver,
        storage::{IntervalKind, Storage},
    };
    use chrono::Local;

    #[test]
    fn renders_one_page_export_with_escaped_title_data() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let end = Local::now().timestamp();
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
