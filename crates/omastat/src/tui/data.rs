use crate::{
    hyprland,
    report::{self, Lens, UsageReport},
    steam::SteamResolver,
    storage::{AppDayTotals, FocusHeatCell, Storage, StorageStatus, TimelineInterval, TitleTotals},
};
use anyhow::Result;
use chrono::Local;
use std::process::Command;

#[derive(Debug)]
pub(super) struct DashboardData {
    pub(super) report: UsageReport,
    pub(super) lens_totals: [Option<(i64, i64)>; Lens::ALL.len()],
    pub(super) today_intervals: Vec<TimelineInterval>,
    pub(super) daily_apps: Vec<AppDayTotals>,
    pub(super) heatmap: Vec<FocusHeatCell>,
    pub(super) titles: Vec<TitleTotals>,
    pub(super) health: HealthSnapshot,
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
    let today_intervals = storage
        .timeline_for_today()?
        .into_iter()
        .map(|mut interval| {
            interval.app_class = steam.resolve_class(&interval.app_class);
            interval
        })
        .collect();
    let daily_apps =
        storage.focused_app_daily_totals_between(report.query_start_ts, report.query_end_ts)?;
    let heatmap = storage.focus_heatmap_between(report.query_start_ts, report.query_end_ts)?;
    let titles =
        storage.focused_title_totals_between(report.query_start_ts, report.query_end_ts, 24)?;
    let health = HealthSnapshot::load(storage);

    Ok(DashboardData {
        report,
        lens_totals,
        today_intervals,
        daily_apps,
        heatmap,
        titles,
        health,
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

#[derive(Debug, Clone, Default)]
pub(super) struct HealthSnapshot {
    pub(super) storage: StorageStatus,
    pub(super) service_state: String,
    pub(super) socket_state: String,
}

impl HealthSnapshot {
    fn load(storage: &Storage) -> Self {
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
