use crate::{
    hyprland,
    report::{self, Lens, UsageReport},
    steam::SteamResolver,
    storage::{
        AppDayTotals, FocusHeatCell, Storage, StorageStatus, TimelineInterval, TitleTotals,
        WorkspaceTotals,
    },
};
use anyhow::Result;
use chrono::Local;
use std::collections::BTreeMap;
use std::process::Command;

const TITLE_LIMIT_PER_APP: usize = 8;

#[derive(Debug)]
pub(super) struct DashboardData {
    pub(super) report: UsageReport,
    pub(super) lens_totals: [Option<(i64, i64)>; Lens::ALL.len()],
    pub(super) timeline_intervals: Vec<TimelineInterval>,
    pub(super) daily_apps: Vec<AppDayTotals>,
    pub(super) heatmap: Vec<FocusHeatCell>,
    pub(super) workspaces: Vec<WorkspaceTotals>,
    pub(super) titles: Vec<TitleTotals>,
    pub(super) stats: DashboardStats,
}

#[derive(Debug, Clone, Default)]
pub(super) struct DashboardStats {
    pub(super) total_days: usize,
    pub(super) active_days: usize,
    pub(super) daily_average_seconds: i64,
    pub(super) active_day_average_seconds: i64,
    pub(super) best_day_label: Option<String>,
    pub(super) best_day_seconds: i64,
    pub(super) longest_streak_days: usize,
    pub(super) focus_block_count: usize,
    pub(super) app_switch_count: usize,
    pub(super) average_block_seconds: i64,
    pub(super) median_block_seconds: i64,
    pub(super) longest_block_seconds: i64,
    pub(super) deep_block_count: usize,
    pub(super) deep_block_seconds: i64,
    pub(super) peak_hour: Option<HourTotal>,
    pub(super) peak_weekday: Option<WeekdayTotal>,
    pub(super) top_app_share: f64,
    pub(super) effective_apps: f64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HourTotal {
    pub(super) hour: u32,
    pub(super) focused_seconds: i64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WeekdayTotal {
    pub(super) weekday: u32,
    pub(super) focused_seconds: i64,
}

impl DashboardStats {
    pub(super) fn from_data(
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
        let best_day = report
            .daily
            .iter()
            .max_by_key(|day| day.focused_seconds)
            .filter(|day| day.focused_seconds > 0);
        let longest_streak_days = longest_active_streak(&report.daily);

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

        let peak_hour = peak_hour(heatmap);
        let peak_weekday = peak_weekday(heatmap);
        let top_app_share = report
            .rows
            .iter()
            .find(|row| row.focused_seconds > 0)
            .map(|row| row.focused_seconds as f64 / report.total_focused_seconds.max(1) as f64)
            .unwrap_or_default()
            .clamp(0.0, 1.0);
        let effective_apps = effective_app_count(&report.rows, report.total_focused_seconds);

        Self {
            total_days,
            active_days,
            daily_average_seconds,
            active_day_average_seconds,
            best_day_label: best_day.map(|day| day.label.clone()),
            best_day_seconds: best_day.map(|day| day.focused_seconds).unwrap_or_default(),
            longest_streak_days,
            focus_block_count,
            app_switch_count,
            average_block_seconds,
            median_block_seconds,
            longest_block_seconds,
            deep_block_count,
            deep_block_seconds,
            peak_hour,
            peak_weekday,
            top_app_share,
            effective_apps,
        }
    }
}

pub(super) fn load_dashboard_data(
    storage: &Storage,
    steam: &mut SteamResolver,
    lens: Lens,
    offset: i32,
) -> Result<DashboardData> {
    let report = if lens == Lens::Day && offset == 0 {
        report::usage_report(storage, steam, lens, lens.history_days())?
    } else {
        report::usage_report_for_period(storage, steam, lens, offset)?
    };
    let lens_totals = load_lens_totals(storage, steam);
    let timeline_intervals = storage
        .timeline_between(report.query_start_ts, report.query_end_ts)?
        .into_iter()
        .map(|mut interval| {
            interval.app_class = steam.resolve_class(&interval.app_class);
            interval
        })
        .collect();
    let focus_intervals = storage
        .focused_timeline_between(report.query_start_ts, report.query_end_ts)?
        .into_iter()
        .map(|mut interval| {
            interval.app_class = steam.resolve_class(&interval.app_class);
            interval
        })
        .collect::<Vec<_>>();
    let daily_apps = storage
        .focused_app_daily_totals_between(report.query_start_ts, report.query_end_ts)?
        .into_iter()
        .map(|mut row| {
            row.app_class = steam.resolve_class(&row.app_class);
            row
        })
        .collect();
    let heatmap = storage.focus_heatmap_between(report.query_start_ts, report.query_end_ts)?;
    let workspaces =
        storage.focused_workspace_totals_between(report.query_start_ts, report.query_end_ts, 8)?;
    let titles = storage
        .focused_title_totals_by_app_between(
            report.query_start_ts,
            report.query_end_ts,
            TITLE_LIMIT_PER_APP,
        )
        .map(|rows| resolve_title_totals(rows, steam))?;
    let stats = DashboardStats::from_data(&report, &heatmap, &focus_intervals);

    Ok(DashboardData {
        report,
        lens_totals,
        timeline_intervals,
        daily_apps,
        heatmap,
        workspaces,
        titles,
        stats,
    })
}

fn load_lens_totals(
    storage: &Storage,
    steam: &mut SteamResolver,
) -> [Option<(i64, i64)>; Lens::ALL.len()] {
    std::array::from_fn(|index| {
        let rows = report::rows_for_lens(storage, Lens::from_index(index)).ok()?;
        let rows = steam.resolve_totals(rows);
        Some((report::focused_total(&rows), report::open_total(&rows)))
    })
}

fn resolve_title_totals(rows: Vec<TitleTotals>, steam: &mut SteamResolver) -> Vec<TitleTotals> {
    let mut totals = BTreeMap::<(String, String), i64>::new();
    for row in rows {
        let app_class = steam.resolve_class(&row.app_class);
        *totals.entry((app_class, row.title)).or_default() += row.focused_seconds.max(0);
    }

    let mut rows = totals
        .into_iter()
        .map(|((app_class, title), focused_seconds)| TitleTotals {
            app_class,
            title,
            focused_seconds,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.app_class
            .cmp(&right.app_class)
            .then_with(|| right.focused_seconds.cmp(&left.focused_seconds))
            .then_with(|| left.title.cmp(&right.title))
    });
    rows
}

#[derive(Debug, Clone, Default)]
pub(super) struct HealthSnapshot {
    pub(super) storage: StorageStatus,
    pub(super) service_state: String,
    pub(super) socket_state: String,
}

impl HealthSnapshot {
    pub(super) fn load(storage: &Storage) -> Self {
        Self {
            storage: storage.usage_status().unwrap_or_default(),
            service_state: omastatd_state(),
            socket_state: hyprland_socket_state(),
        }
    }

    pub(super) fn last_event_label(&self) -> String {
        match self.storage.last_event_at {
            Some(timestamp) => format!(
                "{} ago",
                super::widgets::compact_duration(Local::now().timestamp() - timestamp)
            ),
            None => "never".to_string(),
        }
    }

    pub(super) fn live_label(&self) -> String {
        if self.storage.interval_count == 0 {
            return "empty db".to_string();
        }
        format!(
            "{} focus / {} open / {} idle / {} locked",
            self.storage.focused_active,
            self.storage.open_active,
            self.storage.idle_active,
            self.storage.locked_active
        )
    }

    #[cfg(test)]
    pub(super) fn from_status_for_test(storage: StorageStatus) -> Self {
        Self {
            storage,
            service_state: "active".to_string(),
            socket_state: "ipc ok".to_string(),
        }
    }
}

fn omastatd_state() -> String {
    let Ok(output) = Command::new("systemctl")
        .args(["--user", "is-active", "omastat.service"])
        .output()
    else {
        return "unknown".to_string();
    };

    let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if state.is_empty() {
        "unknown".to_string()
    } else {
        state
    }
}

fn hyprland_socket_state() -> String {
    match hyprland::socket_paths() {
        Ok(paths) if paths.request.exists() && paths.event.exists() => "ipc ok".to_string(),
        Ok(_) => "ipc missing".to_string(),
        Err(_) => "env missing".to_string(),
    }
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

fn longest_active_streak(days: &[crate::storage::DayTotals]) -> usize {
    let mut current = 0;
    let mut best = 0;
    for day in days {
        if day.focused_seconds > 0 {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best
}

fn peak_hour(cells: &[FocusHeatCell]) -> Option<HourTotal> {
    let mut hours = [0_i64; 24];
    for cell in cells {
        if let Some(total) = hours.get_mut(cell.hour as usize) {
            *total += cell.focused_seconds.max(0);
        }
    }
    hours
        .into_iter()
        .enumerate()
        .max_by_key(|(_, seconds)| *seconds)
        .filter(|(_, seconds)| *seconds > 0)
        .map(|(hour, focused_seconds)| HourTotal {
            hour: hour as u32,
            focused_seconds,
        })
}

fn peak_weekday(cells: &[FocusHeatCell]) -> Option<WeekdayTotal> {
    let mut weekdays = [0_i64; 7];
    for cell in cells {
        if let Some(total) = weekdays.get_mut(cell.weekday as usize) {
            *total += cell.focused_seconds.max(0);
        }
    }
    weekdays
        .into_iter()
        .enumerate()
        .max_by_key(|(_, seconds)| *seconds)
        .filter(|(_, seconds)| *seconds > 0)
        .map(|(weekday, focused_seconds)| WeekdayTotal {
            weekday: weekday as u32,
            focused_seconds,
        })
}

fn effective_app_count(rows: &[crate::storage::AppTotals], total_focused_seconds: i64) -> f64 {
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
