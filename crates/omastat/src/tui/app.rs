use super::{
    data::{self, DashboardData, DashboardStats, HealthSnapshot},
    theme::Theme,
};
use crate::{
    config::Config,
    insights::Insight,
    report::{Lens, UsageReport},
    steam::SteamResolver,
    storage::{AppTotals, Storage},
};
use anyhow::Result;
use chrono::{DateTime, Local};
use ratatui::widgets::{ListState, TableState};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum View {
    Overview,
    Insights,
    Apps,
    Timeline,
    System,
}

impl View {
    pub(super) const ALL: [Self; 5] = [
        Self::Overview,
        Self::Insights,
        Self::Apps,
        Self::Timeline,
        Self::System,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Insights => "Insights",
            Self::Apps => "Apps",
            Self::Timeline => "Timeline",
            Self::System => "System",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Insights => 1,
            Self::Apps => 2,
            Self::Timeline => 3,
            Self::System => 4,
        }
    }

    fn from_index(index: usize) -> Self {
        Self::ALL[index.min(Self::ALL.len() - 1)]
    }

    fn next(self) -> Self {
        Self::from_index((self.index() + 1) % Self::ALL.len())
    }

    fn previous(self) -> Self {
        if self.index() == 0 {
            Self::System
        } else {
            Self::from_index(self.index() - 1)
        }
    }
}

#[derive(Debug)]
pub(super) struct App {
    view: View,
    lens: Lens,
    period_offset: i32,
    pub(super) selected: usize,
    insight_selected: usize,
    show_trends: bool,
    help_open: bool,
    last_refresh: Instant,
    loaded_at: DateTime<Local>,
    config: Config,
    theme: Theme,
    steam: SteamResolver,
    data: DashboardData,
    health: HealthSnapshot,
    table_state: TableState,
    insight_state: ListState,
}

impl App {
    pub(super) fn load(storage: &Storage, config: Config) -> Result<Self> {
        let mut steam = SteamResolver::default();
        let lens = Lens::Day;
        let data = data::load_dashboard_data(storage, &mut steam, &config, lens, 0)?;
        let health = HealthSnapshot::load(storage);
        let mut app = Self {
            view: View::Overview,
            lens,
            period_offset: 0,
            selected: 0,
            insight_selected: 0,
            show_trends: true,
            help_open: false,
            last_refresh: Instant::now(),
            loaded_at: Local::now(),
            config,
            theme: Theme::load(),
            steam,
            data,
            health,
            table_state: TableState::new(),
            insight_state: ListState::default(),
        };
        app.sync_selection_state();
        Ok(app)
    }

    pub(super) fn refresh(&mut self, storage: &Storage) -> Result<()> {
        self.reload_data(storage)
    }

    pub(super) fn previous_lens(&mut self, storage: &Storage) -> Result<()> {
        self.set_lens(storage, self.lens.previous())
    }

    pub(super) fn next_lens(&mut self, storage: &Storage) -> Result<()> {
        self.set_lens(storage, self.lens.next())
    }

    pub(super) fn set_lens(&mut self, storage: &Storage, lens: Lens) -> Result<()> {
        self.lens = lens;
        self.period_offset = 0;
        self.reload_data(storage)
    }

    pub(super) fn previous_period(&mut self, storage: &Storage) -> Result<()> {
        if self.lens == Lens::Life {
            return Ok(());
        }
        self.period_offset -= 1;
        self.reload_data(storage)
    }

    pub(super) fn next_period(&mut self, storage: &Storage) -> Result<()> {
        if self.lens == Lens::Life || self.period_offset >= 0 {
            return Ok(());
        }
        self.period_offset += 1;
        self.reload_data(storage)
    }

    pub(super) fn next_view(&mut self, storage: &Storage) {
        self.view = self.view.next();
        self.refresh_health_if_visible(storage);
    }

    pub(super) fn previous_view(&mut self, storage: &Storage) {
        self.view = self.view.previous();
        self.refresh_health_if_visible(storage);
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        if self.view == View::Insights {
            self.move_insight_selection(delta);
        } else {
            self.move_app_selection(delta);
        }
    }

    fn move_app_selection(&mut self, delta: isize) {
        let len = self.rows().len();
        if len == 0 {
            self.selected = 0;
        } else {
            self.selected = (self.selected as isize + delta).clamp(0, len as isize - 1) as usize;
        }
        self.sync_table_state();
    }

    fn move_insight_selection(&mut self, delta: isize) {
        let len = self.insights().len();
        if len == 0 {
            self.insight_selected = 0;
        } else {
            self.insight_selected =
                (self.insight_selected as isize + delta).clamp(0, len as isize - 1) as usize;
        }
        self.sync_insight_state();
    }

    pub(super) fn select_first(&mut self) {
        if self.view == View::Insights {
            self.insight_selected = 0;
            self.sync_insight_state();
        } else {
            self.selected = 0;
            self.sync_table_state();
        }
    }

    pub(super) fn select_last(&mut self) {
        if self.view == View::Insights {
            self.insight_selected = self.insights().len().saturating_sub(1);
            self.sync_insight_state();
        } else {
            self.selected = self.rows().len().saturating_sub(1);
            self.sync_table_state();
        }
    }

    pub(super) fn toggle_trends(&mut self) {
        self.show_trends = !self.show_trends;
    }

    pub(super) fn toggle_help(&mut self) {
        self.help_open = !self.help_open;
    }

    pub(super) fn close_help(&mut self) {
        self.help_open = false;
    }

    pub(super) fn rows(&self) -> &[AppTotals] {
        &self.data.report.rows
    }

    pub(super) fn selected_row(&self) -> Option<&AppTotals> {
        self.rows().get(self.selected)
    }

    pub(super) fn insights(&self) -> &[Insight] {
        &self.data.report.insights
    }

    pub(super) fn selected_insight(&self) -> Option<&Insight> {
        self.insights().get(self.insight_selected)
    }

    pub(super) fn selected_insight_index(&self) -> usize {
        self.insight_selected
    }

    pub(super) fn table_state(&mut self) -> &mut TableState {
        &mut self.table_state
    }

    pub(super) fn insight_state(&mut self) -> &mut ListState {
        &mut self.insight_state
    }

    pub(super) fn view(&self) -> View {
        self.view
    }

    pub(super) fn lens(&self) -> Lens {
        self.lens
    }

    pub(super) fn period_offset(&self) -> i32 {
        self.period_offset
    }

    pub(super) fn show_trends(&self) -> bool {
        self.show_trends
    }

    pub(super) fn help_open(&self) -> bool {
        self.help_open
    }

    pub(super) fn last_refresh(&self) -> Instant {
        self.last_refresh
    }

    pub(super) fn loaded_at(&self) -> DateTime<Local> {
        self.loaded_at
    }

    pub(super) fn theme(&self) -> &Theme {
        &self.theme
    }

    pub(super) fn report(&self) -> &UsageReport {
        &self.data.report
    }

    pub(super) fn app_label(&self, app_class: &str) -> String {
        self.config
            .app_label(app_class, || crate::report::app_label(app_class))
    }

    pub(super) fn data(&self) -> &DashboardData {
        &self.data
    }

    pub(super) fn stats(&self) -> &DashboardStats {
        &self.data.stats
    }

    pub(super) fn health(&self) -> &HealthSnapshot {
        &self.health
    }

    fn reload_data(&mut self, storage: &Storage) -> Result<()> {
        self.data = data::load_dashboard_data(
            storage,
            &mut self.steam,
            &self.config,
            self.lens,
            self.period_offset,
        )?;
        self.refresh_health_if_visible(storage);
        self.theme = Theme::load();
        self.loaded_at = Local::now();
        self.last_refresh = Instant::now();
        self.clamp_selection();
        Ok(())
    }

    fn refresh_health_if_visible(&mut self, storage: &Storage) {
        if self.view == View::System {
            self.health = HealthSnapshot::load(storage);
        }
    }

    fn clamp_selection(&mut self) {
        self.selected = self.selected.min(self.rows().len().saturating_sub(1));
        self.insight_selected = self
            .insight_selected
            .min(self.insights().len().saturating_sub(1));
        self.sync_selection_state();
    }

    fn sync_selection_state(&mut self) {
        self.sync_table_state();
        self.sync_insight_state();
    }

    fn sync_table_state(&mut self) {
        self.table_state
            .select((!self.rows().is_empty()).then_some(self.selected));
    }

    fn sync_insight_state(&mut self) {
        let selected = (!self.insights().is_empty()).then_some(self.insight_selected);
        self.insight_state.select(selected);
    }

    #[cfg(test)]
    pub(super) fn from_parts_for_test(parts: TestAppParts) -> Self {
        let focus_intervals = parts
            .timeline_intervals
            .iter()
            .filter(|interval| interval.kind == crate::storage::IntervalKind::Focused)
            .cloned()
            .collect::<Vec<_>>();
        let stats = DashboardStats::from_data(&parts.report, &parts.heatmap, &focus_intervals);
        let mut app = Self {
            view: parts.view,
            lens: parts.report.lens,
            period_offset: parts.report.period.offset,
            selected: 0,
            insight_selected: 0,
            show_trends: true,
            help_open: false,
            last_refresh: Instant::now(),
            loaded_at: Local::now(),
            config: Config::default(),
            theme: parts.theme,
            steam: parts.steam,
            data: DashboardData {
                report: parts.report,
                lens_totals: parts.lens_totals,
                timeline_intervals: parts.timeline_intervals,
                system_intervals: parts.system_intervals,
                daily_apps: parts.daily_apps,
                heatmap: parts.heatmap,
                workspaces: parts.workspaces,
                app_workspaces: parts.app_workspaces,
                titles: parts.titles,
                stats,
            },
            health: HealthSnapshot::from_status_for_test(parts.storage),
            table_state: TableState::new(),
            insight_state: ListState::default(),
        };
        app.sync_selection_state();
        app
    }

    #[cfg(test)]
    pub(super) fn replace_rows_for_test(&mut self, rows: Vec<AppTotals>) {
        self.data.report.rows = rows;
        self.clamp_selection();
    }
}

#[cfg(test)]
pub(super) struct TestAppParts {
    pub(super) view: View,
    pub(super) report: UsageReport,
    pub(super) lens_totals: [Option<(i64, i64)>; 5],
    pub(super) timeline_intervals: Vec<crate::storage::TimelineInterval>,
    pub(super) system_intervals: Vec<crate::storage::SystemTimelineInterval>,
    pub(super) daily_apps: Vec<crate::storage::AppDayTotals>,
    pub(super) heatmap: Vec<crate::storage::FocusHeatCell>,
    pub(super) workspaces: Vec<crate::storage::WorkspaceTotals>,
    pub(super) app_workspaces: Vec<crate::storage::AppWorkspaceTotals>,
    pub(super) titles: Vec<crate::storage::TitleTotals>,
    pub(super) storage: crate::storage::StorageStatus,
    pub(super) steam: SteamResolver,
    pub(super) theme: Theme,
}
