use crate::{
    analytics, clock,
    config::Config,
    hyprland,
    report::{self, Lens, UsageReport},
    steam::SteamResolver,
    storage::{
        AppDayTotals, AppWorkspaceTotals, FocusHeatCell, Storage, StorageStatus,
        SystemTimelineInterval, TimelineInterval, TitleTotals, WorkspaceTotals,
    },
};
use anyhow::Result;
use std::collections::BTreeMap;
use std::process::Command;

const TITLE_LIMIT_PER_APP: usize = 8;

#[derive(Debug)]
pub(super) struct DashboardData {
    pub(super) report: UsageReport,
    pub(super) lens_totals: [Option<(i64, i64)>; Lens::ALL.len()],
    pub(super) timeline_intervals: Vec<TimelineInterval>,
    pub(super) system_intervals: Vec<SystemTimelineInterval>,
    pub(super) daily_apps: Vec<AppDayTotals>,
    pub(super) heatmap: Vec<FocusHeatCell>,
    pub(super) workspaces: Vec<WorkspaceTotals>,
    pub(super) app_workspaces: Vec<AppWorkspaceTotals>,
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
        let active_days = analytics::active_day_count(&report.daily);
        let daily_average_seconds = analytics::average(report.total_focused_seconds, total_days);
        let active_day_average_seconds =
            analytics::average(report.total_focused_seconds, active_days);
        let best_day = report
            .daily
            .iter()
            .max_by_key(|day| day.focused_seconds)
            .filter(|day| day.focused_seconds > 0);
        let longest_streak_days = analytics::longest_active_streak(&report.daily);

        let block_stats = analytics::focus_block_stats(focus_intervals);
        let app_switch_count = analytics::app_switch_count(focus_intervals);

        let peak_hour = peak_hour(heatmap);
        let peak_weekday = peak_weekday(heatmap);
        let top_app_share = report
            .rows
            .iter()
            .find(|row| row.focused_seconds > 0)
            .map(|row| row.focused_seconds as f64 / report.total_focused_seconds.max(1) as f64)
            .unwrap_or_default()
            .clamp(0.0, 1.0);
        let effective_apps =
            analytics::effective_app_count(&report.rows, report.total_focused_seconds);

        Self {
            total_days,
            active_days,
            daily_average_seconds,
            active_day_average_seconds,
            best_day_label: best_day.map(|day| day.label.clone()),
            best_day_seconds: best_day.map(|day| day.focused_seconds).unwrap_or_default(),
            longest_streak_days,
            focus_block_count: block_stats.count,
            app_switch_count,
            average_block_seconds: block_stats.average_seconds,
            median_block_seconds: block_stats.median_seconds,
            longest_block_seconds: block_stats.longest_seconds,
            deep_block_count: block_stats.deep_count,
            deep_block_seconds: block_stats.deep_seconds,
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
    config: &Config,
    lens: Lens,
    offset: i32,
) -> Result<DashboardData> {
    let report_with_rollups = if lens == Lens::Day && offset == 0 {
        report::usage_report_with_rollups(storage, steam, config, lens, lens.history_days())?
    } else {
        report::usage_report_with_rollups_for_period(storage, steam, config, lens, offset)?
    };
    let report = report_with_rollups.report;
    let lens_totals = load_lens_totals(storage, steam);
    let timeline_intervals = storage
        .timeline_between(report.query_start_ts, report.query_end_ts)?
        .into_iter()
        .map(|mut interval| {
            interval.app_class = steam.resolve_class(&interval.app_class);
            interval
        })
        .collect();
    let system_intervals =
        storage.system_timeline_between(report.query_start_ts, report.query_end_ts)?;
    let rollups = report_with_rollups.rollups;
    let focus_intervals = rollups
        .focus_intervals
        .into_iter()
        .map(|mut interval| {
            interval.app_class = steam.resolve_class(&interval.app_class);
            interval
        })
        .collect::<Vec<_>>();
    let daily_apps = rollups
        .daily_apps
        .into_iter()
        .map(|mut row| {
            row.app_class = steam.resolve_class(&row.app_class);
            row
        })
        .collect();
    let heatmap = rollups.heatmap;
    let workspaces = rollups.workspaces;
    let app_workspaces = resolve_app_workspace_totals(rollups.app_workspaces, steam);
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
        system_intervals,
        daily_apps,
        heatmap,
        workspaces,
        app_workspaces,
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

fn resolve_app_workspace_totals(
    rows: Vec<AppWorkspaceTotals>,
    steam: &mut SteamResolver,
) -> Vec<AppWorkspaceTotals> {
    let mut totals = BTreeMap::<(String, String), i64>::new();
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

#[derive(Debug, Clone, Default)]
pub(super) struct HealthSnapshot {
    pub(super) storage: StorageStatus,
    pub(super) service_state: String,
    pub(super) socket_state: String,
    pub(super) warnings: Vec<String>,
}

impl HealthSnapshot {
    pub(super) fn load(storage: &Storage, config: &Config) -> Self {
        let (storage, mut warnings) = match storage.usage_status() {
            Ok(status) => (status, Vec::new()),
            Err(error) => (
                StorageStatus::default(),
                vec![format!("storage health unavailable: {error:#}")],
            ),
        };
        warnings.extend(
            config
                .warnings()
                .into_iter()
                .map(|warning| format!("config {}: {}", warning.field, warning.message)),
        );
        Self {
            storage,
            service_state: omastatd_state(),
            socket_state: hyprland_socket_state(),
            warnings,
        }
    }

    pub(super) fn last_event_label(&self) -> String {
        match self.storage.last_event_at {
            Some(timestamp) => format!(
                "{} ago",
                super::widgets::compact_duration(clock::local_now().timestamp() - timestamp)
            ),
            None => "never".to_string(),
        }
    }

    pub(super) fn live_label(&self) -> String {
        if let Some(warning) = self.warnings.first() {
            return format!("warning: {warning}");
        }
        if self.storage.interval_count == 0 {
            return "empty db".to_string();
        }
        format!(
            "{} focus / {} open / {} idle / {} locked / {} sleep / {} daemon",
            self.storage.focused_active,
            self.storage.open_active,
            self.storage.idle_active,
            self.storage.locked_active,
            self.storage.sleep_active,
            self.storage.daemon_active
        )
    }

    pub(super) fn last_heartbeat_label(&self) -> String {
        match self.storage.last_heartbeat_at {
            Some(timestamp) => format!(
                "{} ago",
                super::widgets::compact_duration(clock::local_now().timestamp() - timestamp)
            ),
            None => "never".to_string(),
        }
    }

    #[cfg(test)]
    pub(super) fn from_status_for_test(storage: StorageStatus) -> Self {
        Self {
            storage,
            service_state: "active".to_string(),
            socket_state: "ipc ok".to_string(),
            warnings: Vec::new(),
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

fn peak_hour(cells: &[FocusHeatCell]) -> Option<HourTotal> {
    analytics::peak_hour(cells).map(|peak| HourTotal {
        hour: peak.hour,
        focused_seconds: peak.focused_seconds,
    })
}

fn peak_weekday(cells: &[FocusHeatCell]) -> Option<WeekdayTotal> {
    analytics::peak_weekday(cells).map(|peak| WeekdayTotal {
        weekday: peak.weekday,
        focused_seconds: peak.focused_seconds,
    })
}
