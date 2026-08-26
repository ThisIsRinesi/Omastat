use crate::{
    clock,
    config::Config,
    identity,
    insights::{
        self, AnalysisComparisonPeriod, AnalysisInput, AnalysisLens, AnalysisPeriod, Insight,
    },
    steam::SteamResolver,
    storage::{AppTotals, AppWorkspaceTotals, DayTotals, FocusHeatCell, FocusedRollups, Storage},
};
use anyhow::{Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lens {
    Day,
    Week,
    Month,
    Year,
    Life,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppBreakdown {
    pub app_class: String,
    pub label: String,
    pub category: String,
    pub focused_seconds: i64,
    pub open_seconds: i64,
    pub share: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Period {
    pub label: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub offset: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageReport {
    pub generated_at: i64,
    pub query_start_ts: i64,
    pub query_end_ts: i64,
    pub today_key: String,
    pub lens: Lens,
    pub lens_label: &'static str,
    pub period: Period,
    pub total_focused_seconds: i64,
    pub total_open_seconds: i64,
    pub total_idle_seconds: i64,
    pub total_locked_seconds: i64,
    pub total_sleep_seconds: i64,
    pub total_unobserved_seconds: i64,
    pub rows: Vec<AppTotals>,
    pub apps: Vec<AppBreakdown>,
    pub daily: Vec<DayTotals>,
    pub heatmap: Vec<FocusHeatCell>,
    pub insights: Vec<Insight>,
    pub widget_insight: Option<WidgetInsight>,
}

#[derive(Debug, Clone)]
pub struct UsageReportWithRollups {
    pub report: UsageReport,
    pub rollups: FocusedRollups,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsightsReport {
    pub schema_version: u32,
    pub generated_at: i64,
    pub query_start_ts: i64,
    pub query_end_ts: i64,
    pub lens: Lens,
    pub lens_label: &'static str,
    pub period: Period,
    pub totals: InsightsTotals,
    pub insights: Vec<Insight>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsightsTotals {
    pub focused_seconds: i64,
    pub open_seconds: i64,
    pub idle_seconds: i64,
    pub locked_seconds: i64,
    pub sleep_seconds: i64,
    pub unobserved_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WidgetInsight {
    pub title: String,
    pub value: String,
    pub tone: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WidgetSummaryReport {
    pub schema_version: u32,
    pub generated_at: i64,
    pub query_start_ts: i64,
    pub query_end_ts: i64,
    pub today_key: String,
    pub lens: Lens,
    pub lens_label: &'static str,
    pub period: Period,
    pub total_focused_seconds: i64,
    pub total_open_seconds: i64,
    pub total_idle_seconds: i64,
    pub total_locked_seconds: i64,
    pub total_sleep_seconds: i64,
    pub total_unobserved_seconds: i64,
    pub top_app: Option<AppBreakdown>,
    pub display_value: String,
    pub tooltip: String,
    pub status_text: String,
}

impl Lens {
    pub const ALL: [Self; 5] = [Self::Day, Self::Week, Self::Month, Self::Year, Self::Life];

    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "DAY",
            Self::Week => "WEEK",
            Self::Month => "MONTH",
            Self::Year => "YEAR",
            Self::Life => "LIFE",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Day => "Today",
            Self::Week => "This Week",
            Self::Month => "This Month",
            Self::Year => "This Year",
            Self::Life => "Lifetime",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Day => 0,
            Self::Week => 1,
            Self::Month => 2,
            Self::Year => 3,
            Self::Life => 4,
        }
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % Self::ALL.len())
    }

    pub fn previous(self) -> Self {
        if self.index() == 0 {
            Self::Life
        } else {
            Self::from_index(self.index() - 1)
        }
    }

    pub fn history_days(self) -> u32 {
        match self {
            Self::Day => 7,
            Self::Week => 14,
            Self::Month => 31,
            Self::Year | Self::Life => 90,
        }
    }
}

pub fn usage_report(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    days: u32,
) -> Result<UsageReport> {
    Ok(usage_report_with_rollups_for_period_with_days(
        storage,
        steam,
        config,
        lens,
        0,
        Some(days.max(1)),
    )?
    .report)
}

pub fn usage_report_for_period(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<UsageReport> {
    Ok(
        usage_report_with_rollups_for_period_with_days(storage, steam, config, lens, offset, None)?
            .report,
    )
}

pub fn usage_report_for_period_with_days(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
    trailing_days_override: Option<u32>,
) -> Result<UsageReport> {
    Ok(usage_report_with_rollups_for_period_with_days(
        storage,
        steam,
        config,
        lens,
        offset,
        trailing_days_override,
    )?
    .report)
}

pub fn usage_report_with_rollups(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    days: u32,
) -> Result<UsageReportWithRollups> {
    usage_report_with_rollups_for_period_with_days(
        storage,
        steam,
        config,
        lens,
        0,
        Some(days.max(1)),
    )
}

pub fn usage_report_with_rollups_for_period(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<UsageReportWithRollups> {
    usage_report_with_rollups_for_period_with_days(storage, steam, config, lens, offset, None)
}

pub fn insights_report_for_period(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<InsightsReport> {
    usage_report_for_period(storage, steam, config, lens, offset).map(InsightsReport::from)
}

pub fn widget_summary_for_period(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<WidgetSummaryReport> {
    let period = period_for_lens(lens, offset)?;
    let rows = steam.resolve_totals(rows_for_period(storage, lens, &period)?);
    let total_focused_seconds = focused_total(&rows);
    let total_open_seconds = open_total(&rows);
    let session_totals = storage.session_totals_between(period.start_ts, period.query_end_ts)?;
    let top_app = rows.iter().find(|row| row.focused_seconds > 0).map(|row| {
        let label = config.app_label(&row.app_class, || app_label(&row.app_class));
        let category = config.app_category(&row.app_class);
        AppBreakdown {
            app_class: row.app_class.clone(),
            label,
            category,
            focused_seconds: row.focused_seconds,
            open_seconds: row.open_seconds,
            share: crate::analytics::ratio(row.focused_seconds, total_focused_seconds.max(1)),
        }
    });
    let display_value = format_duration(total_focused_seconds);
    let period_label = period.meta.label.clone();
    let tooltip = match top_app.as_ref() {
        Some(app) if total_focused_seconds > 0 => format!(
            "{period_label}: {} focused\nOpen: {}\nTop: {} ({})",
            format_duration(total_focused_seconds),
            format_duration(total_open_seconds),
            app.label,
            format_duration(app.focused_seconds)
        ),
        _ => format!("{period_label}: no focused time"),
    };
    let status_text = match top_app.as_ref() {
        Some(app) if total_focused_seconds > 0 => format!(
            "{} focused, top {}",
            format_duration(total_focused_seconds),
            app.label
        ),
        _ => "No focused time".to_string(),
    };

    Ok(WidgetSummaryReport {
        schema_version: 1,
        generated_at: clock::unix_now(),
        query_start_ts: period.start_ts,
        query_end_ts: period.query_end_ts,
        today_key: clock::local_now().format("%Y-%m-%d").to_string(),
        lens,
        lens_label: lens.label(),
        period: period.meta,
        total_focused_seconds,
        total_open_seconds,
        total_idle_seconds: session_totals.idle_seconds,
        total_locked_seconds: session_totals.locked_seconds,
        total_sleep_seconds: session_totals.sleep_seconds,
        total_unobserved_seconds: session_totals.unobserved_seconds,
        top_app,
        display_value,
        tooltip,
        status_text,
    })
}

fn usage_report_with_rollups_for_period_with_days(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    lens: Lens,
    offset: i32,
    trailing_days_override: Option<u32>,
) -> Result<UsageReportWithRollups> {
    let period = period_for_lens(lens, offset)?;
    let rows = steam.resolve_totals(rows_for_period(storage, lens, &period)?);
    let daily = daily_for_period(storage, lens, &period, trailing_days_override)?;
    let total_focused_seconds = focused_total(&rows);
    let total_open_seconds = open_total(&rows);
    let session_totals = storage.session_totals_between(period.start_ts, period.query_end_ts)?;
    let today_key = clock::local_now().format("%Y-%m-%d").to_string();
    let selected_day_key = selected_day_key(lens, &period, &daily, &today_key);
    let apps = app_breakdown_with_config(&rows, 6, config);
    let rollups = storage.focused_rollups_between(period.start_ts, period.query_end_ts, 8, 64)?;
    let heatmap = rollups.heatmap.clone();
    let focus_intervals = rollups
        .focus_intervals
        .iter()
        .cloned()
        .map(|mut interval| {
            interval.app_class = steam.resolve_class(&interval.app_class);
            interval
        })
        .collect::<Vec<_>>();
    let workspaces = rollups.workspaces.clone();
    let app_workspaces = resolve_app_workspace_totals(rollups.app_workspaces.clone(), steam);
    let previous_period = previous_period_comparison(storage, steam, lens, &period)?;
    let mut insights = insights::analyze(AnalysisInput {
        rows: &rows,
        daily: &daily,
        heatmap: &heatmap,
        focus_intervals: &focus_intervals,
        workspaces: &workspaces,
        app_workspaces: &app_workspaces,
        today_key: &today_key,
        selected_day_key: &selected_day_key,
        period: AnalysisPeriod {
            lens: lens.into(),
            label: &period.meta.label,
            start_date: period.meta.start_date.as_deref(),
            end_date: period.meta.end_date.as_deref(),
        },
        previous_period,
        total_focused_seconds,
        total_open_seconds,
        total_idle_seconds: session_totals.idle_seconds,
        total_locked_seconds: session_totals.locked_seconds,
        total_sleep_seconds: session_totals.sleep_seconds,
        total_unobserved_seconds: session_totals.unobserved_seconds,
    });
    apply_configured_insight_labels(&mut insights, config);

    let generated_at = clock::unix_now();
    let widget_insight = widget_insight_for(&insights, generated_at);

    let report = UsageReport {
        generated_at,
        query_start_ts: period.start_ts,
        query_end_ts: period.query_end_ts,
        today_key,
        lens,
        lens_label: lens.label(),
        period: period.meta,
        total_focused_seconds,
        total_open_seconds,
        total_idle_seconds: session_totals.idle_seconds,
        total_locked_seconds: session_totals.locked_seconds,
        total_sleep_seconds: session_totals.sleep_seconds,
        total_unobserved_seconds: session_totals.unobserved_seconds,
        rows,
        apps,
        daily,
        heatmap,
        insights,
        widget_insight,
    };
    Ok(UsageReportWithRollups { report, rollups })
}

impl From<UsageReport> for InsightsReport {
    fn from(report: UsageReport) -> Self {
        Self {
            schema_version: 1,
            generated_at: report.generated_at,
            query_start_ts: report.query_start_ts,
            query_end_ts: report.query_end_ts,
            lens: report.lens,
            lens_label: report.lens_label,
            period: report.period,
            totals: InsightsTotals {
                focused_seconds: report.total_focused_seconds,
                open_seconds: report.total_open_seconds,
                idle_seconds: report.total_idle_seconds,
                locked_seconds: report.total_locked_seconds,
                sleep_seconds: report.total_sleep_seconds,
                unobserved_seconds: report.total_unobserved_seconds,
            },
            insights: report.insights,
        }
    }
}

pub fn rows_for_lens(storage: &Storage, lens: Lens) -> Result<Vec<AppTotals>> {
    match lens {
        Lens::Day => storage.totals_for_today(),
        Lens::Week => storage.totals_for_week(),
        Lens::Month => storage.totals_for_month(),
        Lens::Year => storage.totals_for_year(),
        Lens::Life => storage.totals_all_time(),
    }
}

fn rows_for_period(storage: &Storage, lens: Lens, period: &PeriodBounds) -> Result<Vec<AppTotals>> {
    if lens == Lens::Life {
        return storage.totals_all_time();
    }
    storage.totals_between(period.start_ts, period.query_end_ts)
}

pub fn focused_total(rows: &[AppTotals]) -> i64 {
    rows.iter().map(|row| row.focused_seconds).sum()
}

pub fn open_total(rows: &[AppTotals]) -> i64 {
    rows.iter().map(|row| row.open_seconds).sum()
}

pub fn app_breakdown(rows: &[AppTotals], max_items: usize) -> Vec<AppBreakdown> {
    app_breakdown_with_labeler(rows, max_items, |app_class| {
        (app_label(app_class), "neutral".to_string())
    })
}

pub fn app_breakdown_with_config(
    rows: &[AppTotals],
    max_items: usize,
    config: &Config,
) -> Vec<AppBreakdown> {
    app_breakdown_with_labeler(rows, max_items, |app_class| {
        (
            config.app_label(app_class, || app_label(app_class)),
            config.app_category(app_class),
        )
    })
}

fn app_breakdown_with_labeler<F>(
    rows: &[AppTotals],
    max_items: usize,
    mut labeler: F,
) -> Vec<AppBreakdown>
where
    F: FnMut(&str) -> (String, String),
{
    let total = focused_total(rows).max(1) as f64;
    let max_items = max_items.max(1);
    let mut apps = Vec::new();

    let head_count = if rows.len() > max_items {
        max_items.saturating_sub(1)
    } else {
        max_items
    };

    for row in rows.iter().take(head_count) {
        if row.focused_seconds <= 0 {
            continue;
        }
        let (label, category) = labeler(&row.app_class);
        apps.push(AppBreakdown {
            app_class: row.app_class.clone(),
            label,
            category,
            focused_seconds: row.focused_seconds,
            open_seconds: row.open_seconds,
            share: row.focused_seconds as f64 / total,
        });
    }

    let other_focused = rows
        .iter()
        .skip(head_count)
        .map(|row| row.focused_seconds.max(0))
        .sum::<i64>();
    let other_open = rows
        .iter()
        .skip(head_count)
        .map(|row| row.open_seconds.max(0))
        .sum::<i64>();
    if other_focused > 0 {
        apps.push(AppBreakdown {
            app_class: "Other".to_string(),
            label: "Other".to_string(),
            category: "mixed".to_string(),
            focused_seconds: other_focused,
            open_seconds: other_open,
            share: other_focused as f64 / total,
        });
    }

    apps
}

pub fn app_label(app_class: &str) -> String {
    identity::display_name(app_class)
}

pub fn widget_insight_for(insights: &[Insight], generated_at: i64) -> Option<WidgetInsight> {
    let candidates = insights
        .iter()
        .filter(|insight| {
            !matches!(insight.tone, insights::InsightTone::Neutral)
                || matches!(
                    insight.category,
                    insights::InsightCategory::Patterns
                        | insights::InsightCategory::FocusQuality
                        | insights::InsightCategory::SystemSignals
                )
        })
        .collect::<Vec<_>>();
    let candidates = if candidates.is_empty() {
        insights.iter().collect::<Vec<_>>()
    } else {
        candidates
    };
    if candidates.is_empty() {
        return None;
    }

    let index = (generated_at.div_euclid(300) as usize) % candidates.len();
    let insight = candidates[index];
    let text = format!("{}: {}", insight.title, insight.value);
    Some(WidgetInsight {
        title: insight.title.clone(),
        value: insight.value.clone(),
        tone: format!("{:?}", insight.tone).to_lowercase(),
        text,
    })
}

impl From<Lens> for AnalysisLens {
    fn from(value: Lens) -> Self {
        match value {
            Lens::Day => Self::Day,
            Lens::Week => Self::Week,
            Lens::Month => Self::Month,
            Lens::Year => Self::Year,
            Lens::Life => Self::Life,
        }
    }
}

struct PeriodBounds {
    meta: Period,
    start_date: Option<NaiveDate>,
    day_count: usize,
    start_ts: i64,
    query_end_ts: i64,
}

fn period_for_lens(lens: Lens, offset: i32) -> Result<PeriodBounds> {
    let now = clock::local_now();
    let today = now.date_naive();
    let offset = offset.min(0);

    if lens == Lens::Life {
        return Ok(PeriodBounds {
            meta: Period {
                label: "Lifetime".to_string(),
                start_date: None,
                end_date: None,
                offset: 0,
            },
            start_date: None,
            day_count: 90,
            start_ts: 0,
            query_end_ts: now.timestamp(),
        });
    }

    let (start_date, end_date, label) = match lens {
        Lens::Day => {
            let start = today + Duration::days(offset as i64);
            let label = if offset == 0 {
                "Today".to_string()
            } else if offset == -1 {
                "Yesterday".to_string()
            } else {
                start.format("%b %-d, %Y").to_string()
            };
            (start, start + Duration::days(1), label)
        }
        Lens::Week => {
            let days_from_monday = today.weekday().num_days_from_monday() as i64;
            let current_start = today - Duration::days(days_from_monday);
            let start = current_start + Duration::weeks(offset as i64);
            (
                start,
                start + Duration::days(7),
                format!("Week of {}", start.format("%b %-d, %Y")),
            )
        }
        Lens::Month => {
            let current_start = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                .context("failed to compute month start")?;
            let start = add_months(current_start, offset)?;
            (
                start,
                add_months(start, 1)?,
                start.format("%B %Y").to_string(),
            )
        }
        Lens::Year => {
            let year = today.year() + offset;
            let start = NaiveDate::from_ymd_opt(year, 1, 1).context("invalid year start")?;
            let end = NaiveDate::from_ymd_opt(year + 1, 1, 1).context("invalid year end")?;
            (start, end, year.to_string())
        }
        Lens::Life => unreachable!("handled above"),
    };

    let period_start = local_midnight(start_date)?;
    let period_end = local_midnight(end_date)?;
    let query_end_ts = if offset == 0 {
        now.timestamp().min(period_end.timestamp())
    } else {
        period_end.timestamp()
    };
    let day_count = (end_date - start_date).num_days().max(1) as usize;

    Ok(PeriodBounds {
        meta: Period {
            label,
            start_date: Some(start_date.format("%Y-%m-%d").to_string()),
            end_date: Some(
                (end_date - Duration::days(1))
                    .format("%Y-%m-%d")
                    .to_string(),
            ),
            offset,
        },
        start_date: Some(start_date),
        day_count,
        start_ts: period_start.timestamp(),
        query_end_ts,
    })
}

fn daily_for_period(
    storage: &Storage,
    lens: Lens,
    period: &PeriodBounds,
    trailing_days_override: Option<u32>,
) -> Result<Vec<DayTotals>> {
    if lens == Lens::Life {
        return storage.daily_totals(trailing_days_override.unwrap_or_else(|| lens.history_days()));
    }

    let Some(start_date) = period.start_date else {
        return storage.daily_totals(trailing_days_override.unwrap_or_else(|| lens.history_days()));
    };

    if lens == Lens::Day
        && let Some(days) = trailing_days_override
    {
        if period.meta.offset == 0 {
            return storage.daily_totals(days);
        }

        let days = days.max(1) as usize;
        let start_date = start_date - Duration::days(days.saturating_sub(1) as i64);
        return storage.daily_totals_for_local_dates(start_date, days, period.query_end_ts);
    }

    let day_count = if period.meta.offset == 0 {
        elapsed_day_count(
            start_date,
            period.start_ts,
            period.query_end_ts,
            period.day_count,
        )?
    } else {
        period.day_count
    };
    storage.daily_totals_for_local_dates(start_date, day_count, period.query_end_ts)
}

fn selected_day_key(
    lens: Lens,
    period: &PeriodBounds,
    daily: &[DayTotals],
    today_key: &str,
) -> String {
    if lens != Lens::Day {
        return today_key.to_string();
    }

    period
        .meta
        .start_date
        .clone()
        .or_else(|| daily.last().map(|day| day.date.clone()))
        .unwrap_or_else(|| today_key.to_string())
}

fn previous_period_comparison(
    storage: &Storage,
    steam: &mut SteamResolver,
    lens: Lens,
    period: &PeriodBounds,
) -> Result<Option<AnalysisComparisonPeriod>> {
    if !matches!(lens, Lens::Week | Lens::Month | Lens::Year) {
        return Ok(None);
    }

    let previous = period_for_lens(lens, period.meta.offset.saturating_sub(1))?;
    let matched_elapsed = period.meta.offset == 0;
    let previous_query_end_ts = if matched_elapsed {
        let elapsed = period.query_end_ts.saturating_sub(period.start_ts);
        previous
            .query_end_ts
            .min(previous.start_ts.saturating_add(elapsed))
    } else {
        previous.query_end_ts
    };
    let rows =
        steam.resolve_totals(storage.totals_between(previous.start_ts, previous_query_end_ts)?);

    Ok(Some(AnalysisComparisonPeriod {
        label: previous.meta.label,
        start_date: previous.meta.start_date,
        end_date: previous.meta.end_date,
        focused_seconds: focused_total(&rows),
        matched_elapsed,
    }))
}

fn apply_configured_insight_labels(insights: &mut [Insight], config: &Config) {
    for insight in insights {
        let Some(app_class) = insight.supporting.app_class.clone() else {
            continue;
        };
        let fallback = insight
            .supporting
            .app_label
            .clone()
            .unwrap_or_else(|| app_label(&app_class));
        let configured = config.app_label(&app_class, || fallback.clone());
        if configured == fallback {
            continue;
        }
        if !fallback.is_empty() {
            insight.value = insight.value.replacen(&fallback, &configured, 1);
        }
        insight.supporting.app_label = Some(configured);
    }
}

fn resolve_app_workspace_totals(
    rows: Vec<AppWorkspaceTotals>,
    steam: &mut SteamResolver,
) -> Vec<AppWorkspaceTotals> {
    let mut totals = std::collections::BTreeMap::<(String, String), i64>::new();
    for row in rows {
        let app_class = steam.resolve_class(&row.app_class);
        *totals.entry((row.workspace, app_class)).or_default() += row.focused_seconds.max(0);
    }

    let mut rows = totals
        .into_iter()
        .map(
            |((workspace, app_class), focused_seconds)| AppWorkspaceTotals {
                workspace,
                app_class,
                focused_seconds,
            },
        )
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .focused_seconds
            .cmp(&left.focused_seconds)
            .then_with(|| left.workspace.cmp(&right.workspace))
            .then_with(|| left.app_class.cmp(&right.app_class))
    });
    rows
}

fn local_midnight(date: NaiveDate) -> Result<chrono::DateTime<Local>> {
    let naive = date.and_hms_opt(0, 0, 0).context("invalid date")?;
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(time) => Ok(time),
        chrono::LocalResult::Ambiguous(earliest, _) => Ok(earliest),
        chrono::LocalResult::None => {
            let mut candidate = naive;
            for _ in 0..180 {
                candidate += Duration::minutes(1);
                match Local.from_local_datetime(&candidate) {
                    chrono::LocalResult::Single(time) => return Ok(time),
                    chrono::LocalResult::Ambiguous(earliest, _) => return Ok(earliest),
                    chrono::LocalResult::None => {}
                }
            }
            anyhow::bail!("failed to compute local midnight")
        }
    }
}

fn add_months(date: NaiveDate, months: i32) -> Result<NaiveDate> {
    let month_index = date.year() * 12 + date.month0() as i32 + months;
    let year = month_index.div_euclid(12);
    let month0 = month_index.rem_euclid(12);
    NaiveDate::from_ymd_opt(year, month0 as u32 + 1, 1).context("failed to compute month offset")
}

fn elapsed_day_count(
    start_date: NaiveDate,
    start_ts: i64,
    query_end_ts: i64,
    max_days: usize,
) -> Result<usize> {
    if query_end_ts <= start_ts {
        return Ok(1);
    }
    let end_ts = query_end_ts.saturating_sub(1).max(start_ts);
    let end_date = Local
        .timestamp_opt(end_ts, 0)
        .single()
        .context("failed to compute elapsed report day")?
        .date_naive();
    let days = (end_date - start_date).num_days().max(0) as usize + 1;
    Ok(days.clamp(1, max_days.max(1)))
}

pub fn format_duration(seconds: i64) -> String {
    crate::analytics::format_duration(seconds)
}

pub fn percent(value: f64) -> String {
    crate::analytics::percent(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_apps_with_other_bucket() {
        let rows = (0..8)
            .map(|index| AppTotals {
                app_class: format!("app-{index}"),
                focused_seconds: 10,
                open_seconds: 20,
            })
            .collect::<Vec<_>>();

        let grouped = app_breakdown(&rows, 6);

        assert_eq!(grouped.len(), 6);
        assert_eq!(grouped.last().unwrap().label, "Other");
        assert_eq!(grouped.last().unwrap().focused_seconds, 30);
    }

    #[test]
    fn app_breakdown_applies_config_alias_and_category() {
        let mut config = Config::default();
        config.apps.insert(
            "code".to_string(),
            crate::config::AppConfig {
                alias: Some("Editor".to_string()),
                category: Some("productive".to_string()),
            },
        );
        let rows = vec![AppTotals {
            app_class: "code".to_string(),
            focused_seconds: 60,
            open_seconds: 120,
        }];

        let grouped = app_breakdown_with_config(&rows, 6, &config);

        assert_eq!(grouped[0].label, "Editor");
        assert_eq!(grouped[0].category, "productive");
    }

    #[test]
    fn labels_package_names_compactly() {
        assert_eq!(app_label("com.mitchellh.ghostty"), "Ghostty");
        assert_eq!(app_label("org.omarchy.terminal"), "Terminal");
        assert_eq!(app_label("zen"), "Zen Browser");
    }

    #[test]
    fn previous_month_period_has_replay_label() {
        let period = period_for_lens(Lens::Month, -1).unwrap();

        assert_eq!(period.meta.offset, -1);
        assert!(period.meta.start_date.is_some());
        assert!(period.meta.end_date.is_some());
        assert_ne!(period.meta.label, "This Month");
    }

    #[test]
    fn life_period_ignores_offsets() {
        let period = period_for_lens(Lens::Life, -12).unwrap();

        assert_eq!(period.meta.label, "Lifetime");
        assert_eq!(period.meta.offset, 0);
        assert!(period.meta.start_date.is_none());
    }

    #[test]
    fn period_bounds_expose_query_range_for_reports() {
        let period = period_for_lens(Lens::Week, -1).unwrap();

        assert!(period.start_ts < period.query_end_ts);
    }

    #[test]
    fn current_month_daily_stops_at_today_instead_of_future_dates() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let db = dir.path().join("omastat.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config)?;
        let mut steam = SteamResolver::default();

        let report = usage_report_for_period(&storage, &mut steam, &config, Lens::Month, 0)?;

        assert_eq!(
            report.daily.last().map(|day| day.date.as_str()),
            Some(report.today_key.as_str())
        );
        assert!(
            report
                .daily
                .iter()
                .all(|day| day.date.as_str() <= report.today_key.as_str())
        );

        Ok(())
    }

    #[test]
    fn historical_day_summary_includes_trailing_days_for_comparison() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let db = dir.path().join("omastat.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config)?;
        let selected_date = Local::now().date_naive() - Duration::days(1);
        let previous_date = selected_date - Duration::days(1);
        let previous_start = local_midnight(previous_date)?.timestamp() + 60 * 60;
        let selected_start = local_midnight(selected_date)?.timestamp() + 60 * 60;

        let previous = storage.start_interval(
            crate::storage::IntervalKind::Focused,
            "code",
            None,
            None,
            previous_start,
        )?;
        storage.close_interval(previous, previous_start + 30 * 60)?;

        let selected = storage.start_interval(
            crate::storage::IntervalKind::Focused,
            "code",
            None,
            None,
            selected_start,
        )?;
        storage.close_interval(selected, selected_start + 40 * 60)?;

        let mut steam = SteamResolver::default();
        let report = usage_report_for_period_with_days(
            &storage,
            &mut steam,
            &config,
            Lens::Day,
            -1,
            Some(2),
        )?;

        let selected_key = selected_date.format("%Y-%m-%d").to_string();
        let previous_key = previous_date.format("%Y-%m-%d").to_string();

        assert_eq!(
            report.period.start_date.as_deref(),
            Some(selected_key.as_str())
        );
        assert_eq!(report.daily.len(), 2);
        assert_eq!(report.daily[0].date, previous_key);
        assert_eq!(report.daily[0].focused_seconds, 30 * 60);
        assert_eq!(report.daily[1].date, selected_key);
        assert_eq!(report.daily[1].focused_seconds, 40 * 60);

        let comparison = report
            .insights
            .iter()
            .find(|insight| insight.kind == crate::insights::InsightKind::DayComparison)
            .expect("historical report should compare with the previous day");

        assert_eq!(comparison.title, "vs previous day");
        assert_eq!(comparison.value, "+10m");

        Ok(())
    }

    #[test]
    fn insights_report_json_keeps_structured_payload_compact() {
        let usage = UsageReport {
            generated_at: 1234,
            query_start_ts: 1000,
            query_end_ts: 2000,
            today_key: "2026-08-22".to_string(),
            lens: Lens::Week,
            lens_label: Lens::Week.label(),
            period: Period {
                label: "Week of Aug 17, 2026".to_string(),
                start_date: Some("2026-08-17".to_string()),
                end_date: Some("2026-08-23".to_string()),
                offset: 0,
            },
            total_focused_seconds: 3600,
            total_open_seconds: 5400,
            total_idle_seconds: 300,
            total_locked_seconds: 0,
            total_sleep_seconds: 0,
            total_unobserved_seconds: 0,
            rows: vec![AppTotals {
                app_class: "code".to_string(),
                focused_seconds: 3600,
                open_seconds: 5400,
            }],
            apps: Vec::new(),
            daily: Vec::new(),
            heatmap: vec![FocusHeatCell {
                weekday: 1,
                hour: 9,
                focused_seconds: 1800,
            }],
            insights: vec![Insight {
                kind: crate::insights::InsightKind::TopApp,
                category: crate::insights::InsightCategory::Apps,
                tone: crate::insights::InsightTone::Neutral,
                title: "Top app share".to_string(),
                value: "Code - 1h (100%)".to_string(),
                explanation: "The app with the largest share of focused time.".to_string(),
                confidence: crate::insights::InsightConfidence::High,
                evidence: crate::insights::InsightEvidence {
                    data_points: 7,
                    minimum_data_points: 1,
                    observed_focus_seconds: 3600,
                    observed_open_seconds: 5400,
                },
                supporting: crate::insights::InsightSupport::default(),
            }],
            widget_insight: None,
        };

        let summary_json = serde_json::to_value(&usage).unwrap();

        assert_eq!(summary_json["heatmap"][0]["weekday"], 1);
        assert_eq!(summary_json["heatmap"][0]["hour"], 9);
        assert_eq!(summary_json["heatmap"][0]["focused_seconds"], 1800);

        let json = serde_json::to_value(InsightsReport::from(usage)).unwrap();

        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["lens"], "week");
        assert_eq!(json["period"]["label"], "Week of Aug 17, 2026");
        assert_eq!(json["totals"]["focused_seconds"], 3600);
        assert_eq!(json["insights"][0]["kind"], "top-app");
        assert!(json.get("rows").is_none());
        assert!(json.get("daily").is_none());
        assert!(json.get("apps").is_none());
        assert!(json.get("heatmap").is_none());
    }
}
