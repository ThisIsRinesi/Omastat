use crate::{
    identity,
    steam::SteamResolver,
    storage::{AppTotals, DayTotals, Storage},
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
    pub focused_seconds: i64,
    pub open_seconds: i64,
    pub share: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InsightRow {
    pub label: String,
    pub value: String,
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
    pub rows: Vec<AppTotals>,
    pub apps: Vec<AppBreakdown>,
    pub daily: Vec<DayTotals>,
    pub insights: Vec<InsightRow>,
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
    lens: Lens,
    days: u32,
) -> Result<UsageReport> {
    usage_report_for_period_with_days(storage, steam, lens, 0, Some(days.max(1)))
}

pub fn usage_report_for_period(
    storage: &Storage,
    steam: &mut SteamResolver,
    lens: Lens,
    offset: i32,
) -> Result<UsageReport> {
    usage_report_for_period_with_days(storage, steam, lens, offset, None)
}

fn usage_report_for_period_with_days(
    storage: &Storage,
    steam: &mut SteamResolver,
    lens: Lens,
    offset: i32,
    trailing_days_override: Option<u32>,
) -> Result<UsageReport> {
    let period = period_for_lens(lens, offset)?;
    let rows = steam.resolve_totals(rows_for_period(storage, lens, &period)?);
    let daily = daily_for_period(storage, lens, &period, trailing_days_override)?;
    let total_focused_seconds = focused_total(&rows);
    let total_open_seconds = open_total(&rows);
    let session_totals = storage.session_totals_between(period.start_ts, period.query_end_ts)?;
    let today_key = daily
        .last()
        .map(|day| day.date.clone())
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let apps = app_breakdown(&rows, 6);
    let insights = insights(
        &rows,
        &daily,
        &today_key,
        total_focused_seconds,
        total_open_seconds,
    );

    Ok(UsageReport {
        generated_at: chrono::Utc::now().timestamp(),
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
        rows,
        apps,
        daily,
        insights,
    })
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
    let total = focused_total(rows).max(1) as f64;
    let max_items = max_items.max(1);
    let mut apps = Vec::new();

    for row in rows.iter().take(max_items) {
        if row.focused_seconds <= 0 {
            continue;
        }
        apps.push(AppBreakdown {
            app_class: row.app_class.clone(),
            label: app_label(&row.app_class),
            focused_seconds: row.focused_seconds,
            open_seconds: row.open_seconds,
            share: row.focused_seconds as f64 / total,
        });
    }

    let other_focused = rows
        .iter()
        .skip(max_items)
        .map(|row| row.focused_seconds.max(0))
        .sum::<i64>();
    let other_open = rows
        .iter()
        .skip(max_items)
        .map(|row| row.open_seconds.max(0))
        .sum::<i64>();
    if other_focused > 0 {
        apps.push(AppBreakdown {
            app_class: "Other".to_string(),
            label: "Other".to_string(),
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

fn insights(
    rows: &[AppTotals],
    daily: &[DayTotals],
    today_key: &str,
    focused: i64,
    open: i64,
) -> Vec<InsightRow> {
    if focused <= 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    if let Some(top) = rows.iter().find(|row| row.focused_seconds > 0) {
        out.push(InsightRow {
            label: "Top app".to_string(),
            value: format!(
                "{} - {} ({})",
                app_label(&top.app_class),
                format_duration(top.focused_seconds),
                percent(top.focused_seconds as f64 / focused.max(1) as f64)
            ),
        });
    }

    if let Some(yesterday) = yesterday_total(daily, today_key)
        && yesterday > 0
    {
        out.push(InsightRow {
            label: "vs yesterday".to_string(),
            value: signed_duration(focused - yesterday),
        });
    }

    if let Some(best) = daily.iter().max_by_key(|day| day.focused_seconds)
        && best.focused_seconds > 0
    {
        out.push(InsightRow {
            label: "Busiest day".to_string(),
            value: format!(
                "{} - {}",
                relative_day_label(best, today_key),
                format_duration(best.focused_seconds)
            ),
        });
    }

    if open > 0 {
        out.push(InsightRow {
            label: "Focus density".to_string(),
            value: percent(focused as f64 / open.max(1) as f64),
        });
    }

    out
}

struct PeriodBounds {
    meta: Period,
    start_date: Option<NaiveDate>,
    day_count: usize,
    start_ts: i64,
    query_end_ts: i64,
}

fn period_for_lens(lens: Lens, offset: i32) -> Result<PeriodBounds> {
    let now = Local::now();
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
        && period.meta.offset == 0
        && let Some(days) = trailing_days_override
    {
        return storage.daily_totals(days);
    }

    storage.daily_totals_for_local_dates(start_date, period.day_count, period.query_end_ts)
}

fn local_midnight(date: NaiveDate) -> Result<chrono::DateTime<Local>> {
    Local
        .from_local_datetime(&date.and_hms_opt(0, 0, 0).context("invalid date")?)
        .single()
        .context("failed to compute local midnight")
}

fn add_months(date: NaiveDate, months: i32) -> Result<NaiveDate> {
    let month_index = date.year() * 12 + date.month0() as i32 + months;
    let year = month_index.div_euclid(12);
    let month0 = month_index.rem_euclid(12);
    NaiveDate::from_ymd_opt(year, month0 as u32 + 1, 1).context("failed to compute month offset")
}

fn yesterday_total(daily: &[DayTotals], today_key: &str) -> Option<i64> {
    let today = chrono::NaiveDate::parse_from_str(today_key, "%Y-%m-%d").ok()?;
    let yesterday = today.pred_opt()?.format("%Y-%m-%d").to_string();
    daily
        .iter()
        .find(|day| day.date == yesterday)
        .map(|day| day.focused_seconds)
}

fn relative_day_label(day: &DayTotals, today_key: &str) -> String {
    if day.date == today_key {
        return "Today".to_string();
    }

    if let Ok(today) = chrono::NaiveDate::parse_from_str(today_key, "%Y-%m-%d")
        && today
            .pred_opt()
            .is_some_and(|yesterday| day.date == yesterday.format("%Y-%m-%d").to_string())
    {
        return "Yesterday".to_string();
    }

    day.label.clone()
}

fn signed_duration(seconds: i64) -> String {
    let sign = if seconds < 0 { "-" } else { "+" };
    format!("{sign}{}", format_duration(seconds.abs()))
}

pub fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    let rest = minutes % 60;
    if rest == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {rest}m")
    }
}

pub fn percent(value: f64) -> String {
    format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0)
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

        assert_eq!(grouped.len(), 7);
        assert_eq!(grouped.last().unwrap().label, "Other");
        assert_eq!(grouped.last().unwrap().focused_seconds, 20);
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
}
