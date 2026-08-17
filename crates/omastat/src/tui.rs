use crate::{
    hyprland,
    report::{self, AppBreakdown, Lens, UsageReport},
    steam::SteamResolver,
    storage::{AppTotals, DayTotals, IntervalKind, Storage, StorageStatus, TimelineInterval},
};
use anyhow::Result;
use chrono::{DateTime, Datelike, Local, TimeZone};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use serde_json::Value as JsonValue;
use std::{
    fs, io,
    process::Command,
    time::{Duration, Instant},
};
use toml::Value as TomlValue;

const CLOCK_REFRESH: Duration = Duration::from_secs(1);
const AUTO_REFRESH: Duration = Duration::from_secs(5);

pub fn run(storage: Storage) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _cleanup = TerminalCleanup;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_app(&mut terminal, &storage)
}

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, cursor::Show);
    }
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, storage: &Storage) -> Result<()> {
    let mut app = App::load(storage)?;
    let mut next_clock = Instant::now() + CLOCK_REFRESH;

    loop {
        terminal.draw(|frame| render(frame, &app))?;

        let refresh_deadline = app.last_refresh + AUTO_REFRESH;
        let deadline = refresh_deadline.min(next_clock);

        if event::poll(deadline.saturating_duration_since(Instant::now()))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Char('r') => app.refresh(storage)?,
                    KeyCode::Tab => app.next_view(),
                    KeyCode::BackTab => app.previous_view(),
                    KeyCode::Char('p') => app.show_patterns = !app.show_patterns,
                    KeyCode::Char('[') => app.previous_period(storage)?,
                    KeyCode::Char(']') => app.next_period(storage)?,
                    KeyCode::Left | KeyCode::Char('h') => app.previous_lens(storage)?,
                    KeyCode::Right | KeyCode::Char('l') => app.next_lens(storage)?,
                    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                    KeyCode::PageUp => app.move_selection(-8),
                    KeyCode::PageDown => app.move_selection(8),
                    KeyCode::Home => app.select_first(),
                    KeyCode::End => app.select_last(),
                    KeyCode::Char('1') => app.set_lens(storage, Lens::Day)?,
                    KeyCode::Char('2') => app.set_lens(storage, Lens::Week)?,
                    KeyCode::Char('3') => app.set_lens(storage, Lens::Month)?,
                    KeyCode::Char('4') => app.set_lens(storage, Lens::Year)?,
                    KeyCode::Char('5') => app.set_lens(storage, Lens::Life)?,
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
            continue;
        }

        let now = Instant::now();
        if app.last_refresh.elapsed() >= AUTO_REFRESH {
            app.refresh(storage)?;
        }
        while next_clock <= now {
            next_clock += CLOCK_REFRESH;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Overview,
    Apps,
    Timeline,
    System,
}

impl View {
    const ALL: [Self; 4] = [Self::Overview, Self::Apps, Self::Timeline, Self::System];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Apps => "Apps",
            Self::Timeline => "Timeline",
            Self::System => "System",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Overview => 0,
            Self::Apps => 1,
            Self::Timeline => 2,
            Self::System => 3,
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
struct App {
    view: View,
    lens: Lens,
    period_offset: i32,
    selected: usize,
    show_patterns: bool,
    last_refresh: Instant,
    loaded_at: DateTime<Local>,
    theme: Theme,
    steam: SteamResolver,
    report: UsageReport,
    lens_totals: [Option<(i64, i64)>; 5],
    today_intervals: Vec<TimelineInterval>,
    health: HealthSnapshot,
}

impl App {
    fn load(storage: &Storage) -> Result<Self> {
        let mut steam = SteamResolver::default();
        let lens = Lens::Day;
        let report = report::usage_report_for_period(storage, &mut steam, lens, 0)?;
        let lens_totals = load_lens_totals(storage, &mut steam);
        let today_intervals = resolve_today_intervals(storage, &mut steam)?;

        Ok(Self {
            view: View::Overview,
            lens,
            period_offset: 0,
            selected: 0,
            show_patterns: false,
            last_refresh: Instant::now(),
            loaded_at: Local::now(),
            theme: Theme::load(),
            steam,
            report,
            lens_totals,
            today_intervals,
            health: HealthSnapshot::load(storage),
        })
    }

    fn refresh(&mut self, storage: &Storage) -> Result<()> {
        self.report = report::usage_report_for_period(
            storage,
            &mut self.steam,
            self.lens,
            self.period_offset,
        )?;
        self.lens_totals = load_lens_totals(storage, &mut self.steam);
        self.today_intervals = resolve_today_intervals(storage, &mut self.steam)?;
        self.health = HealthSnapshot::load(storage);
        self.loaded_at = Local::now();
        self.last_refresh = Instant::now();
        self.clamp_selection();
        Ok(())
    }

    fn rows(&self) -> &[AppTotals] {
        &self.report.rows
    }

    fn next_view(&mut self) {
        self.view = self.view.next();
    }

    fn previous_view(&mut self) {
        self.view = self.view.previous();
    }

    fn previous_lens(&mut self, storage: &Storage) -> Result<()> {
        self.set_lens(storage, self.lens.previous())
    }

    fn next_lens(&mut self, storage: &Storage) -> Result<()> {
        self.set_lens(storage, self.lens.next())
    }

    fn previous_period(&mut self, storage: &Storage) -> Result<()> {
        if self.lens == Lens::Life {
            return Ok(());
        }
        self.period_offset -= 1;
        self.report = report::usage_report_for_period(
            storage,
            &mut self.steam,
            self.lens,
            self.period_offset,
        )?;
        self.loaded_at = Local::now();
        self.last_refresh = Instant::now();
        self.clamp_selection();
        Ok(())
    }

    fn next_period(&mut self, storage: &Storage) -> Result<()> {
        if self.lens == Lens::Life || self.period_offset >= 0 {
            return Ok(());
        }
        self.period_offset += 1;
        self.report = report::usage_report_for_period(
            storage,
            &mut self.steam,
            self.lens,
            self.period_offset,
        )?;
        self.loaded_at = Local::now();
        self.last_refresh = Instant::now();
        self.clamp_selection();
        Ok(())
    }

    fn set_lens(&mut self, storage: &Storage, lens: Lens) -> Result<()> {
        self.lens = lens;
        self.period_offset = 0;
        self.report = report::usage_report_for_period(
            storage,
            &mut self.steam,
            self.lens,
            self.period_offset,
        )?;
        self.loaded_at = Local::now();
        self.last_refresh = Instant::now();
        self.clamp_selection();
        Ok(())
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.rows().len();
        if len == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as isize + delta).clamp(0, len as isize - 1) as usize;
    }

    fn select_first(&mut self) {
        self.selected = 0;
    }

    fn select_last(&mut self) {
        self.selected = self.rows().len().saturating_sub(1);
    }

    fn clamp_selection(&mut self) {
        self.selected = self.selected.min(self.rows().len().saturating_sub(1));
    }
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

fn resolve_today_intervals(
    storage: &Storage,
    steam: &mut SteamResolver,
) -> Result<Vec<TimelineInterval>> {
    storage
        .timeline_for_today()?
        .into_iter()
        .map(|mut interval| {
            interval.app_class = steam.resolve_class(&interval.app_class);
            Ok(interval)
        })
        .collect()
}

#[derive(Debug, Clone, Default)]
struct HealthSnapshot {
    storage: StorageStatus,
    service_state: String,
    socket_state: String,
}

impl HealthSnapshot {
    fn load(storage: &Storage) -> Self {
        Self {
            storage: storage.usage_status().unwrap_or_default(),
            service_state: omastatd_state(),
            socket_state: hyprland_socket_state(),
        }
    }

    fn service_color(&self, theme: &Theme) -> Color {
        if self.service_state == "active" {
            theme.success
        } else if self.service_state == "unknown" {
            theme.muted
        } else {
            theme.danger
        }
    }

    fn socket_color(&self, theme: &Theme) -> Color {
        if self.socket_state == "ipc ok" {
            theme.success
        } else {
            theme.warn
        }
    }

    fn last_event_label(&self) -> String {
        match self.storage.last_event_at {
            Some(timestamp) => format!(
                "{} ago",
                compact_duration(Local::now().timestamp() - timestamp)
            ),
            None => "never".to_string(),
        }
    }

    fn live_label(&self) -> String {
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

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let theme = &app.theme;
    fill_area(frame, area, theme.bg);

    if area.width < 52 || area.height < 16 {
        render_tiny(frame, area, app, theme);
        return;
    }

    let [header, body, footer] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(1),
        ])
        .split(area)
    else {
        return;
    };

    render_header(frame, header, app, theme);
    match app.view {
        View::Overview => render_overview(frame, body, app, theme),
        View::Apps => render_apps_view(frame, body, app, theme),
        View::Timeline => render_timeline_view(frame, body, app, theme),
        View::System => render_system_view(frame, body, app, theme),
    }
    render_footer(frame, footer, app, theme);
}

fn render_tiny(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let lines = vec![
        Line::from(vec![
            Span::styled("omastat ", Style::default().fg(theme.primary)),
            Span::styled(app.view.label(), Style::default().fg(theme.text)),
        ]),
        Line::from(Span::styled(
            app.lens.label(),
            Style::default()
                .fg(theme.bg)
                .bg(lens_color(app.lens, theme))
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "terminal too small",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled("q quits", Style::default().fg(theme.dim))),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(panel("OMASTAT", theme, theme.primary)),
        area,
    );
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.bg);
    let clock = if area.width < 86 {
        Local::now().format("%H:%M").to_string()
    } else {
        Local::now().format("%H:%M:%S").to_string()
    };

    let mut first = vec![Span::styled(
        " 󰔟 OMASTAT ",
        Style::default()
            .fg(theme.primary)
            .bg(theme.bg)
            .add_modifier(Modifier::BOLD),
    )];
    first.push(Span::styled(" ", Style::default().bg(theme.bg)));
    for view in View::ALL {
        let selected = view == app.view;
        first.push(Span::styled(
            format!(" {} ", view.label()),
            Style::default()
                .fg(if selected { theme.bg } else { theme.muted })
                .bg(if selected { theme.primary } else { theme.bg })
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
    }

    let mut second = Vec::new();
    second.push(Span::styled(" ", Style::default().bg(theme.bg)));
    for lens in Lens::ALL {
        let selected = lens == app.lens;
        let color = lens_color(lens, theme);
        second.push(Span::styled(
            format!(" {} ", lens.label()),
            Style::default()
                .fg(if selected { theme.bg } else { color })
                .bg(if selected { color } else { theme.bg })
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
    }
    second.extend([
        Span::styled("  period ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(
            fit_text(&app.report.period.label, 22),
            Style::default().fg(theme.text).bg(theme.bg),
        ),
        Span::styled("  focus ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(
            report::format_duration(app.report.total_focused_seconds),
            Style::default().fg(theme.warn).bg(theme.bg),
        ),
        Span::styled("  open ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(
            report::format_duration(app.report.total_open_seconds),
            Style::default().fg(theme.secondary).bg(theme.bg),
        ),
        Span::styled("  density ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(
            report::percent(ratio(
                app.report.total_focused_seconds,
                app.report.total_open_seconds,
            )),
            Style::default().fg(theme.tertiary).bg(theme.bg),
        ),
        Span::styled("  updated ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(
            app.loaded_at.format("%H:%M:%S").to_string(),
            Style::default().fg(theme.muted).bg(theme.bg),
        ),
        Span::styled("  now ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(clock, Style::default().fg(theme.text).bg(theme.bg)),
    ]);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(first),
            Line::from(second),
            Line::from(Span::styled(
                rule(area.width as usize),
                Style::default().fg(theme.border).bg(theme.bg),
            )),
        ])
        .style(Style::default().bg(theme.bg)),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.bg);
    let period_hint = if app.lens == Lens::Life {
        "life".to_string()
    } else if app.period_offset == 0 {
        "current".to_string()
    } else {
        format!("{} back", app.period_offset.abs())
    };
    let left = format!(
        " Tab view  h/l lens  [/] period  1-5 lens  j/k select  p patterns  r refresh  q quit  [{} / {} / {}]",
        app.view.label(),
        app.lens.label(),
        period_hint
    );
    let right = format!("{}s auto", AUTO_REFRESH.as_secs());
    let width = area.width as usize;
    let left_len = left.chars().count();
    let right_len = right.chars().count();
    let mut spans = Vec::new();
    if left_len + right_len < width {
        spans.push(Span::styled(
            left,
            Style::default().fg(theme.muted).bg(theme.bg),
        ));
        spans.push(Span::styled(
            " ".repeat(width - left_len - right_len),
            Style::default().bg(theme.bg),
        ));
        spans.push(Span::styled(
            right,
            Style::default().fg(theme.dim).bg(theme.bg),
        ));
    } else {
        spans.push(Span::styled(
            fit_text(&left, width),
            Style::default().fg(theme.muted).bg(theme.bg),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_overview(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.bg);
    if area.width < 86 {
        let [hero, apps, trend] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Min(8),
                Constraint::Length(7),
            ])
            .split(area)
        else {
            return;
        };
        render_hero(frame, hero, app, theme);
        render_breakdown(frame, apps, app, theme);
        render_trend(frame, trend, app, theme);
        return;
    }

    let [hero, rest] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(10)])
        .split(area)
    else {
        return;
    };
    let [left, right] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(rest)
    else {
        return;
    };
    let [breakdown, trend] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(left)
    else {
        return;
    };
    let [insights, lenses] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(right)
    else {
        return;
    };

    render_hero(frame, hero, app, theme);
    render_breakdown(frame, breakdown, app, theme);
    render_trend(frame, trend, app, theme);
    render_insights(frame, insights, app, theme);
    render_lens_totals(frame, lenses, app, theme);
}

fn render_hero(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel(&app.report.period.label, theme, lens_color(app.lens, theme));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let density = ratio(
        app.report.total_focused_seconds,
        app.report.total_open_seconds,
    );
    let top = app
        .report
        .apps
        .first()
        .map(|app| app.label.clone())
        .unwrap_or_else(|| "no app focus yet".to_string());
    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!(
                    "󰔟 {}",
                    report::format_duration(app.report.total_focused_seconds)
                ),
                Style::default()
                    .fg(theme.warn)
                    .bg(theme.panel)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" focused", Style::default().fg(theme.dim).bg(theme.panel)),
        ]),
        Line::from(vec![
            Span::styled("open ", Style::default().fg(theme.dim).bg(theme.panel)),
            Span::styled(
                report::format_duration(app.report.total_open_seconds),
                Style::default().fg(theme.secondary).bg(theme.panel),
            ),
            Span::styled("  density ", Style::default().fg(theme.dim).bg(theme.panel)),
            Span::styled(
                report::percent(density),
                Style::default().fg(theme.tertiary).bg(theme.panel),
            ),
            Span::styled("  top ", Style::default().fg(theme.dim).bg(theme.panel)),
            Span::styled(top, Style::default().fg(theme.primary).bg(theme.panel)),
        ]),
        Line::from(Span::styled(
            fit_text(
                &format!(
                    "{} apps tracked across {}",
                    app.report.rows.len(),
                    app.report.period.label
                ),
                inner.width as usize,
            ),
            Style::default().fg(theme.muted).bg(theme.panel),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_breakdown(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel("app breakdown", theme, theme.warn);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mut lines = Vec::new();
    if app.report.apps.is_empty() {
        lines.push(Line::from(Span::styled(
            "waiting for focused app time",
            Style::default().fg(theme.muted).bg(theme.panel),
        )));
    } else {
        lines.push(share_bar(
            &app.report.apps,
            inner.width as usize,
            theme.panel,
            theme,
        ));
        lines.push(Line::from(Span::styled(
            rule(inner.width as usize),
            Style::default().fg(theme.dim).bg(theme.panel),
        )));
        for (index, app) in app
            .report
            .apps
            .iter()
            .take(inner.height.saturating_sub(2) as usize)
            .enumerate()
        {
            lines.push(app_breakdown_line(index, app, inner.width as usize, theme));
        }
    }

    pad_lines(
        &mut lines,
        inner.width as usize,
        inner.height as usize,
        theme,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_trend(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel(
        if app.show_patterns {
            "patterns"
        } else {
            "trend"
        },
        theme,
        theme.success,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = daily_bar_lines(&app.report.daily, inner.width as usize, theme);
    if app.show_patterns {
        for row in &app.report.insights {
            lines.push(metric_line(
                &row.label,
                &row.value,
                inner.width as usize,
                theme.primary,
                theme,
            ));
        }
    }
    pad_lines(
        &mut lines,
        inner.width as usize,
        inner.height as usize,
        theme,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_insights(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel("insights", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = if app.report.insights.is_empty() {
        vec![Line::from(Span::styled(
            "No patterns yet for this lens",
            Style::default().fg(theme.muted).bg(theme.panel),
        ))]
    } else {
        app.report
            .insights
            .iter()
            .map(|row| {
                metric_line(
                    &row.label,
                    &row.value,
                    inner.width as usize,
                    theme.text,
                    theme,
                )
            })
            .collect::<Vec<_>>()
    };
    pad_lines(
        &mut lines,
        inner.width as usize,
        inner.height as usize,
        theme,
    );
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.panel)),
        inner,
    );
}

fn render_lens_totals(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel("lenses", theme, theme.secondary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let max = app
        .lens_totals
        .iter()
        .filter_map(|total| total.map(|(focused, _)| focused))
        .max()
        .unwrap_or(1)
        .max(1);
    let width = inner.width as usize;
    let bar_width = width.saturating_sub(20).max(4);
    let mut lines = Vec::new();

    for lens in Lens::ALL.into_iter().take(inner.height as usize) {
        let index = lens.index();
        let selected = lens == app.lens;
        let bg = if selected {
            theme.selection
        } else {
            theme.panel
        };
        let color = lens_color(lens, theme);
        let focused = app.lens_totals[index].map(|total| total.0).unwrap_or(0);
        let mut spans = vec![
            Span::styled(
                if selected { ">" } else { " " },
                Style::default().fg(color).bg(bg),
            ),
            Span::styled(
                format!("{} ", index + 1),
                Style::default().fg(theme.dim).bg(bg),
            ),
            Span::styled(
                fit_text(lens.label(), 5),
                Style::default()
                    .fg(if selected { theme.text } else { color })
                    .bg(bg)
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(" ", Style::default().bg(bg)),
        ];
        spans.extend(bar_spans(ratio(focused, max), bar_width, color, bg, theme));
        spans.push(Span::styled(
            format!(" {}", compact_duration(focused)),
            Style::default().fg(theme.muted).bg(bg),
        ));
        lines.push(Line::from(spans));
    }

    pad_lines(
        &mut lines,
        inner.width as usize,
        inner.height as usize,
        theme,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_apps_view(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    if area.width < 96 {
        render_app_table(frame, area, app, theme);
        return;
    }
    let [table, detail] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area)
    else {
        return;
    };
    render_app_table(frame, table, app, theme);
    render_app_detail(frame, detail, app, theme);
}

fn render_app_table(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel("apps", theme, theme.warn);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = app.rows();
    let width = inner.width as usize;
    let total = app.report.total_focused_seconds.max(1);
    let max_focus = rows
        .iter()
        .map(|row| row.focused_seconds)
        .max()
        .unwrap_or(1)
        .max(1);
    let mut lines = vec![apps_header(width, theme)];

    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "No focused app time for this lens",
            Style::default().fg(theme.muted).bg(theme.panel),
        )));
    } else {
        let visible = inner.height.saturating_sub(1).max(1) as usize;
        let start = if app.selected >= visible {
            app.selected + 1 - visible
        } else {
            0
        };
        for (index, row) in rows.iter().enumerate().skip(start) {
            if lines.len() >= inner.height as usize {
                break;
            }
            lines.push(app_row_line(
                index,
                row,
                index == app.selected,
                max_focus,
                total,
                width,
                theme,
            ));
        }
    }

    pad_lines(&mut lines, width, inner.height as usize, theme);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_app_detail(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel("selected", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let row = app.rows().get(app.selected);
    let mut lines = Vec::new();
    if let Some(row) = row {
        let share = ratio(row.focused_seconds, app.report.total_focused_seconds.max(1));
        let density = ratio(row.focused_seconds, row.open_seconds);
        lines.extend([
            Line::from(Span::styled(
                fit_text(&report::app_label(&row.app_class), inner.width as usize),
                Style::default()
                    .fg(theme.primary)
                    .bg(theme.panel)
                    .add_modifier(Modifier::BOLD),
            )),
            metric_line(
                "Focused",
                &report::format_duration(row.focused_seconds),
                inner.width as usize,
                theme.warn,
                theme,
            ),
            metric_line(
                "Open",
                &report::format_duration(row.open_seconds),
                inner.width as usize,
                theme.secondary,
                theme,
            ),
            metric_line(
                "Share",
                &report::percent(share),
                inner.width as usize,
                theme.tertiary,
                theme,
            ),
            metric_line(
                "Density",
                &report::percent(density),
                inner.width as usize,
                theme.success,
                theme,
            ),
            Line::from(Span::styled(
                rule(inner.width as usize),
                Style::default().fg(theme.dim).bg(theme.panel),
            )),
            timeline_line(
                &app.today_intervals,
                app.rows(),
                Some(row.app_class.as_str()),
                inner.width as usize,
                theme,
            ),
        ]);
    } else {
        lines.push(Line::from(Span::styled(
            "No selected app",
            Style::default().fg(theme.muted).bg(theme.panel),
        )));
    }

    pad_lines(
        &mut lines,
        inner.width as usize,
        inner.height as usize,
        theme,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_timeline_view(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let [map, list] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(8)])
        .split(area)
    else {
        return;
    };
    render_timeline_map(frame, map, app, theme);
    render_interval_list(frame, list, app, theme);
}

fn render_timeline_map(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel("today timeline", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let selected = app
        .rows()
        .get(app.selected)
        .map(|row| row.app_class.as_str());
    let lines = vec![
        timeline_line(
            &app.today_intervals,
            app.rows(),
            selected,
            inner.width as usize,
            theme,
        ),
        Line::from(Span::styled(
            "focused blocks use app color; open-only time is dim",
            Style::default().fg(theme.muted).bg(theme.panel),
        )),
        Line::from(Span::styled(
            fit_text(
                &format!(
                    "{} intervals today, {} selected",
                    app.today_intervals.len(),
                    app.rows()
                        .get(app.selected)
                        .map(|row| report::app_label(&row.app_class))
                        .unwrap_or_else(|| "none".to_string())
                ),
                inner.width as usize,
            ),
            Style::default().fg(theme.dim).bg(theme.panel),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.panel)),
        inner,
    );
}

fn render_interval_list(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel("intervals", theme, theme.secondary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut intervals = app.today_intervals.clone();
    intervals.sort_by_key(|interval| interval.started_at);
    let width = inner.width as usize;
    let mut lines = Vec::new();

    if intervals.is_empty() {
        lines.push(Line::from(Span::styled(
            "No intervals recorded today",
            Style::default().fg(theme.muted).bg(theme.panel),
        )));
    } else {
        for interval in intervals.into_iter().rev().take(inner.height as usize) {
            lines.push(interval_line(&interval, width, app.rows(), theme));
        }
    }

    pad_lines(&mut lines, width, inner.height as usize, theme);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_system_view(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let [left, right] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(area)
    else {
        return;
    };
    render_system_health(frame, left, app, theme);
    render_lens_totals(frame, right, app, theme);
}

fn render_system_health(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = panel("system", theme, theme.success);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;
    let mut lines = vec![
        metric_line(
            "Daemon",
            &app.health.service_state,
            width,
            app.health.service_color(theme),
            theme,
        ),
        metric_line(
            "Hyprland",
            &app.health.socket_state,
            width,
            app.health.socket_color(theme),
            theme,
        ),
        metric_line(
            "Last event",
            &app.health.last_event_label(),
            width,
            theme.muted,
            theme,
        ),
        metric_line(
            "Live",
            &app.health.live_label(),
            width,
            theme.primary,
            theme,
        ),
        metric_line(
            "Intervals",
            &app.health.storage.interval_count.to_string(),
            width,
            theme.text,
            theme,
        ),
        metric_line(
            "Active",
            &format!(
                "{} focused / {} open / {} idle / {} locked",
                app.health.storage.focused_active,
                app.health.storage.open_active,
                app.health.storage.idle_active,
                app.health.storage.locked_active
            ),
            width,
            theme.secondary,
            theme,
        ),
        Line::from(Span::styled(
            rule(width),
            Style::default().fg(theme.dim).bg(theme.panel),
        )),
        metric_line("View", app.view.label(), width, theme.primary, theme),
        metric_line(
            "Lens",
            app.lens.title(),
            width,
            lens_color(app.lens, theme),
            theme,
        ),
        metric_line("Period", &app.report.period.label, width, theme.warn, theme),
        metric_line(
            "Rows",
            &app.report.rows.len().to_string(),
            width,
            theme.text,
            theme,
        ),
        metric_line(
            "Loaded",
            &app.loaded_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            width,
            theme.muted,
            theme,
        ),
    ];
    pad_lines(&mut lines, width, inner.height as usize, theme);
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.panel)),
        inner,
    );
}

fn share_bar(
    apps: &[AppBreakdown],
    width: usize,
    background: Color,
    theme: &Theme,
) -> Line<'static> {
    let width = width.max(1);
    if apps.is_empty() {
        return Line::from(Span::styled(
            " ".repeat(width),
            Style::default().bg(background),
        ));
    }

    let mut remaining = width;
    let mut spans = Vec::new();
    for (index, app) in apps.iter().enumerate() {
        let last = index + 1 == apps.len();
        let len = if last {
            remaining
        } else {
            ((app.share * width as f64).round() as usize)
                .clamp(1, remaining.saturating_sub(apps.len() - index - 1))
        };
        remaining = remaining.saturating_sub(len);
        spans.push(Span::styled(
            "█".repeat(len),
            Style::default().fg(rank_color(index, theme)).bg(background),
        ));
    }
    if remaining > 0 {
        spans.push(Span::styled(
            "░".repeat(remaining),
            Style::default().fg(theme.dim).bg(background),
        ));
    }
    Line::from(spans)
}

fn app_breakdown_line(
    index: usize,
    app: &AppBreakdown,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let color = rank_color(index, theme);
    let name_width = width.saturating_sub(24).max(8);
    Line::from(vec![
        Span::styled("● ", Style::default().fg(color).bg(theme.panel)),
        Span::styled(
            fit_text(&app.label, name_width),
            Style::default().fg(theme.text).bg(theme.panel),
        ),
        Span::styled(
            format!(" {:>7}", report::format_duration(app.focused_seconds)),
            Style::default().fg(theme.warn).bg(theme.panel),
        ),
        Span::styled(
            format!(" {:>4}", report::percent(app.share)),
            Style::default().fg(theme.tertiary).bg(theme.panel),
        ),
    ])
}

fn daily_bar_lines(days: &[DayTotals], width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let max = days
        .iter()
        .map(|day| day.focused_seconds)
        .max()
        .unwrap_or(1)
        .max(1);
    let bar_width = width.saturating_sub(16).max(4);
    days.iter()
        .rev()
        .take(7)
        .rev()
        .map(|day| {
            let mut spans = vec![
                Span::styled(
                    fit_text(&day.label, 3),
                    Style::default().fg(theme.dim).bg(theme.panel),
                ),
                Span::styled(" ", Style::default().bg(theme.panel)),
            ];
            spans.extend(bar_spans(
                ratio(day.focused_seconds, max),
                bar_width,
                theme.success,
                theme.panel,
                theme,
            ));
            spans.push(Span::styled(
                format!(" {}", compact_duration(day.focused_seconds)),
                Style::default().fg(theme.muted).bg(theme.panel),
            ));
            Line::from(spans)
        })
        .collect()
}

fn apps_header(width: usize, theme: &Theme) -> Line<'static> {
    let name_width = width.saturating_sub(46).clamp(12, 34);
    let bar_width = width.saturating_sub(name_width + 34).max(6);
    Line::from(vec![
        Span::styled(" #  ", Style::default().fg(theme.dim).bg(theme.panel_alt)),
        Span::styled(
            fit_text("application", name_width),
            Style::default().fg(theme.muted).bg(theme.panel_alt),
        ),
        Span::styled(" ", Style::default().bg(theme.panel_alt)),
        Span::styled(
            fit_text("focus", bar_width),
            Style::default().fg(theme.muted).bg(theme.panel_alt),
        ),
        Span::styled(
            " focused share dense",
            Style::default().fg(theme.muted).bg(theme.panel_alt),
        ),
    ])
}

fn app_row_line(
    index: usize,
    row: &AppTotals,
    selected: bool,
    max_focus: i64,
    total: i64,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let bg = if selected {
        theme.selection
    } else {
        theme.panel
    };
    let color = rank_color(index, theme);
    let name_width = width.saturating_sub(46).clamp(12, 34);
    let bar_width = width.saturating_sub(name_width + 34).max(6);
    let mut spans = vec![
        Span::styled(
            if selected { ">" } else { " " },
            Style::default().fg(color).bg(bg),
        ),
        Span::styled(
            format!("{:>2} ", index + 1),
            Style::default().fg(theme.dim).bg(bg),
        ),
        Span::styled(
            fit_text(&report::app_label(&row.app_class), name_width),
            Style::default()
                .fg(if selected { theme.text } else { color })
                .bg(bg)
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ),
        Span::styled(" ", Style::default().bg(bg)),
    ];
    spans.extend(bar_spans(
        ratio(row.focused_seconds, max_focus),
        bar_width,
        color,
        bg,
        theme,
    ));
    spans.push(Span::styled(
        format!(" {:>7}", compact_duration(row.focused_seconds)),
        Style::default().fg(theme.warn).bg(bg),
    ));
    spans.push(Span::styled(
        format!(" {:>4}", report::percent(ratio(row.focused_seconds, total))),
        Style::default().fg(theme.tertiary).bg(bg),
    ));
    spans.push(Span::styled(
        format!(
            " {:>4}",
            report::percent(ratio(row.focused_seconds, row.open_seconds))
        ),
        Style::default().fg(theme.success).bg(bg),
    ));
    Line::from(spans)
}

fn interval_line(
    interval: &TimelineInterval,
    width: usize,
    rows: &[AppTotals],
    theme: &Theme,
) -> Line<'static> {
    let rank = rows
        .iter()
        .position(|row| row.app_class == interval.app_class)
        .unwrap_or(usize::MAX);
    let color = if interval.kind == IntervalKind::Focused {
        rank_color(rank, theme)
    } else {
        theme.dim
    };
    let kind = match interval.kind {
        IntervalKind::Focused => "focus",
        IntervalKind::Open => "open",
    };
    let start = format_clock(interval.started_at);
    let end = format_clock(interval.ended_at);
    let duration = compact_duration(interval.ended_at.saturating_sub(interval.started_at));
    let name_width = width.saturating_sub(27).max(8);
    Line::from(vec![
        Span::styled(
            format!("{start}-{end} "),
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
        Span::styled(
            fit_text(kind, 5),
            Style::default().fg(color).bg(theme.panel),
        ),
        Span::styled(" ", Style::default().bg(theme.panel)),
        Span::styled(
            fit_text(&report::app_label(&interval.app_class), name_width),
            Style::default().fg(theme.text).bg(theme.panel),
        ),
        Span::styled(
            format!(" {duration:>5}"),
            Style::default().fg(theme.muted).bg(theme.panel),
        ),
    ])
}

fn timeline_line(
    intervals: &[TimelineInterval],
    rows: &[AppTotals],
    selected_app: Option<&str>,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let Some((start, end)) = today_bounds() else {
        return Line::from(Span::styled(
            "timeline unavailable",
            Style::default().fg(theme.muted).bg(theme.panel),
        ));
    };
    let width = width.max(1);
    if intervals.is_empty() || end <= start {
        return Line::from(Span::styled(
            fit_text("no intervals today", width),
            Style::default().fg(theme.muted).bg(theme.panel),
        ));
    }

    let spans = (0..width)
        .map(|col| {
            let position = (col as f64 + 0.5) / width as f64;
            let timestamp = start + ((end - start) as f64 * position).round() as i64;
            let focused = intervals.iter().find(|interval| {
                interval.kind == IntervalKind::Focused
                    && interval.started_at <= timestamp
                    && interval.ended_at >= timestamp
            });
            let open = intervals.iter().find(|interval| {
                interval.kind == IntervalKind::Open
                    && interval.started_at <= timestamp
                    && interval.ended_at >= timestamp
            });
            let (glyph, color) = if let Some(interval) = focused {
                let selected = selected_app == Some(interval.app_class.as_str());
                let rank = rows
                    .iter()
                    .position(|row| row.app_class == interval.app_class)
                    .unwrap_or(usize::MAX);
                (
                    if selected { "▓" } else { "█" },
                    if selected {
                        theme.text
                    } else {
                        rank_color(rank, theme)
                    },
                )
            } else if open.is_some() {
                ("░", theme.dim)
            } else {
                ("·", theme.dim)
            };
            Span::styled(glyph, Style::default().fg(color).bg(theme.panel))
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

fn today_bounds() -> Option<(i64, i64)> {
    let now = Local::now();
    let start = Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()?
        .timestamp();
    Some((start, now.timestamp()))
}

fn panel(title: &str, theme: &Theme, accent: Color) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent).bg(theme.panel))
        .style(Style::default().bg(theme.panel))
}

fn metric_line(
    label: &str,
    value: &str,
    width: usize,
    color: Color,
    theme: &Theme,
) -> Line<'static> {
    let label_width = width.min(13);
    Line::from(vec![
        Span::styled(
            fit_text(label, label_width),
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
        Span::styled(" ", Style::default().bg(theme.panel)),
        Span::styled(
            fit_text(value, width.saturating_sub(label_width + 1)),
            Style::default().fg(color).bg(theme.panel),
        ),
    ])
}

fn bar_spans(
    value: f64,
    width: usize,
    color: Color,
    background: Color,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let width = width.max(1);
    let filled = ((value.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    (0..width)
        .map(|index| {
            if index < filled {
                Span::styled("█", Style::default().fg(color).bg(background))
            } else {
                Span::styled("░", Style::default().fg(theme.dim).bg(background))
            }
        })
        .collect()
}

fn fill_area(frame: &mut Frame<'_>, area: Rect, color: Color) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line = " ".repeat(area.width as usize);
    let lines = (0..area.height)
        .map(|_| Line::from(Span::styled(line.clone(), Style::default().bg(color))))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(color)),
        area,
    );
}

fn pad_lines(lines: &mut Vec<Line<'static>>, width: usize, height: usize, theme: &Theme) {
    lines.truncate(height);
    while lines.len() < height {
        lines.push(Line::from(Span::styled(
            " ".repeat(width),
            Style::default().bg(theme.panel),
        )));
    }
}

fn rule(width: usize) -> String {
    "─".repeat(width)
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        (numerator.max(0) as f64 / denominator as f64).clamp(0.0, 1.0)
    }
}

fn lens_color(lens: Lens, theme: &Theme) -> Color {
    match lens {
        Lens::Day => theme.primary,
        Lens::Week => theme.success,
        Lens::Month => theme.warn,
        Lens::Year => theme.tertiary,
        Lens::Life => theme.secondary,
    }
}

fn rank_color(index: usize, theme: &Theme) -> Color {
    match index {
        0 => theme.warn,
        1 => theme.primary,
        2 => theme.success,
        3 => theme.tertiary,
        4 => theme.secondary,
        5 => theme.danger,
        _ => theme.muted,
    }
}

fn fit_text(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if value.chars().count() <= width {
        return format!("{value:<width$}");
    }
    let mut out = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    out.push('…');
    out
}

fn compact_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours}h")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}

fn format_clock(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_else(|| "--:--".to_string())
}

#[derive(Debug, Clone)]
struct Theme {
    bg: Color,
    panel: Color,
    panel_alt: Color,
    selection: Color,
    text: Color,
    muted: Color,
    dim: Color,
    border: Color,
    primary: Color,
    secondary: Color,
    tertiary: Color,
    success: Color,
    warn: Color,
    danger: Color,
}

impl Theme {
    fn load() -> Self {
        read_noctalia_theme()
            .or_else(read_omarchy_theme)
            .unwrap_or_else(Self::fallback)
    }

    fn fallback() -> Self {
        Self::from_palette(
            Rgb::new(5, 8, 14),
            Rgb::new(232, 245, 255),
            Rgb::new(34, 211, 238),
            Rgb::new(167, 139, 250),
            Rgb::new(255, 73, 198),
            Rgb::new(255, 83, 112),
            Rgb::new(88, 110, 130),
        )
    }

    fn from_palette(
        bg: Rgb,
        text: Rgb,
        primary: Rgb,
        secondary: Rgb,
        tertiary: Rgb,
        danger: Rgb,
        outline: Rgb,
    ) -> Self {
        let fallback = Self::fallback_accents();
        let primary = if primary.saturation() < 0.08 {
            fallback.0
        } else {
            primary
        };
        let secondary = if secondary.saturation() < 0.08 {
            fallback.1
        } else {
            secondary
        };
        let tertiary = if tertiary.saturation() < 0.08 {
            fallback.2
        } else {
            tertiary
        };

        Self {
            bg: bg.color(),
            panel: bg.mix(text, 0.035).color(),
            panel_alt: bg.mix(primary, 0.14).color(),
            selection: bg.mix(primary, 0.26).color(),
            text: text.color(),
            muted: bg.mix(text, 0.62).color(),
            dim: bg.mix(text, 0.28).color(),
            border: bg.mix(outline, 0.72).color(),
            primary: primary.color(),
            secondary: secondary.color(),
            tertiary: tertiary.color(),
            success: Rgb::new(89, 255, 184).mix(secondary, 0.35).color(),
            warn: Rgb::new(255, 220, 92).mix(tertiary, 0.25).color(),
            danger: danger.color(),
        }
    }

    fn fallback_accents() -> (Rgb, Rgb, Rgb) {
        (
            Rgb::new(34, 211, 238),
            Rgb::new(167, 139, 250),
            Rgb::new(255, 73, 198),
        )
    }
}

#[derive(Clone, Copy)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    fn parse(value: &str) -> Option<Self> {
        let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
        if value.len() != 6 {
            return None;
        }
        Some(Self {
            r: u8::from_str_radix(&value[0..2], 16).ok()?,
            g: u8::from_str_radix(&value[2..4], 16).ok()?,
            b: u8::from_str_radix(&value[4..6], 16).ok()?,
        })
    }

    fn color(self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }

    fn mix(self, other: Self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let inv = 1.0 - amount;
        Self {
            r: (self.r as f64 * inv + other.r as f64 * amount).round() as u8,
            g: (self.g as f64 * inv + other.g as f64 * amount).round() as u8,
            b: (self.b as f64 * inv + other.b as f64 * amount).round() as u8,
        }
    }

    fn saturation(self) -> f64 {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        if max <= f64::EPSILON {
            0.0
        } else {
            (max - min) / max
        }
    }
}

fn read_noctalia_theme() -> Option<Theme> {
    let path = dirs::config_dir()?.join("noctalia/colors.json");
    let contents = fs::read_to_string(path).ok()?;
    let value: JsonValue = serde_json::from_str(&contents).ok()?;
    Some(Theme::from_palette(
        json_color(&value, &["dark", "mSurface"])?,
        json_color(&value, &["dark", "mOnSurface"])?,
        json_color(&value, &["dark", "mPrimary"])?,
        json_color(&value, &["dark", "mSecondary"])?,
        json_color(&value, &["dark", "mTertiary"])?,
        json_color(&value, &["dark", "mError"])?,
        json_color(&value, &["dark", "mOutline"])?,
    ))
}

fn read_omarchy_theme() -> Option<Theme> {
    let path = dirs::state_dir()?.join("omarchy/current/theme/colors.toml");
    let contents = fs::read_to_string(path).ok()?;
    let value: TomlValue = toml::from_str(&contents).ok()?;
    Some(Theme::from_palette(
        toml_color(&value, &["background"])?,
        toml_color(&value, &["foreground"])?,
        toml_color(&value, &["accent"])?,
        toml_color(&value, &["blue"])
            .or_else(|| toml_color(&value, &["cyan"]))
            .unwrap_or_else(|| Rgb::new(167, 139, 250)),
        toml_color(&value, &["magenta"])
            .or_else(|| toml_color(&value, &["yellow"]))
            .unwrap_or_else(|| Rgb::new(255, 73, 198)),
        toml_color(&value, &["red"]).unwrap_or_else(|| Rgb::new(255, 83, 112)),
        toml_color(&value, &["muted"]).unwrap_or_else(|| Rgb::new(88, 110, 130)),
    ))
}

fn json_color(value: &JsonValue, path: &[&str]) -> Option<Rgb> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().and_then(Rgb::parse)
}

fn toml_color(value: &TomlValue, path: &[&str]) -> Option<Rgb> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().and_then(Rgb::parse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn overview_keeps_all_lenses_visible() {
        let backend = TestBackend::new(120, 32);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = sample_app(View::Overview);

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered = rendered_text(&terminal);
        for label in [
            "Overview",
            "Apps",
            "Timeline",
            "System",
            "MONTH",
            "YEAR",
            "LIFE",
            "Week of Jan 12",
        ] {
            assert!(rendered.contains(label), "missing {label}");
        }
        assert!(rendered.contains("app breakdown"));
        assert!(rendered.contains("insights"));
    }

    #[test]
    fn selected_app_stays_visible_when_list_is_longer_than_viewport() {
        let backend = TestBackend::new(90, 22);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Apps);
        app.report.rows = (0..30)
            .map(|index| AppTotals {
                app_class: format!("app-{index:02}"),
                focused_seconds: 3600 - index as i64,
                open_seconds: 7200,
            })
            .collect();
        app.selected = 29;

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(
            rendered.contains(">30 App 29"),
            "selected row was not visible"
        );
    }

    #[test]
    fn timeline_view_renders_interval_panel() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = sample_app(View::Timeline);

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(rendered.contains("today timeline"));
        assert!(rendered.contains("intervals"));
    }

    fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    fn sample_app(view: View) -> App {
        let rows = vec![
            AppTotals {
                app_class: "com.mitchellh.ghostty".to_string(),
                focused_seconds: 8 * 3600,
                open_seconds: 12 * 3600,
            },
            AppTotals {
                app_class: "steam_app_1675200".to_string(),
                focused_seconds: 2 * 3600,
                open_seconds: 5 * 3600,
            },
            AppTotals {
                app_class: "discord".to_string(),
                focused_seconds: 900,
                open_seconds: 4 * 3600,
            },
        ];
        let daily = (0..14)
            .map(|index| DayTotals {
                date: format!("2026-01-{:02}", index + 1),
                label: format!("D{index}"),
                focused_seconds: (index as i64 + 1) * 300,
                open_seconds: (index as i64 + 2) * 500,
                idle_seconds: index as i64 * 120,
                locked_seconds: 0,
            })
            .collect::<Vec<_>>();
        let apps = report::app_breakdown(&rows, 6);
        let report = UsageReport {
            generated_at: 0,
            today_key: "2026-01-14".to_string(),
            lens: Lens::Week,
            lens_label: Lens::Week.label(),
            period: report::Period {
                label: "Week of Jan 12, 2026".to_string(),
                start_date: Some("2026-01-12".to_string()),
                end_date: Some("2026-01-18".to_string()),
                offset: 0,
            },
            total_focused_seconds: report::focused_total(&rows),
            total_open_seconds: report::open_total(&rows),
            total_idle_seconds: daily.iter().map(|day| day.idle_seconds).sum(),
            total_locked_seconds: 0,
            rows,
            apps,
            daily,
            insights: vec![
                report::InsightRow {
                    label: "Top app".to_string(),
                    value: "ghostty - 8h".to_string(),
                },
                report::InsightRow {
                    label: "Focus density".to_string(),
                    value: "66%".to_string(),
                },
            ],
        };

        App {
            view,
            lens: Lens::Week,
            period_offset: 0,
            selected: 0,
            show_patterns: false,
            last_refresh: Instant::now(),
            loaded_at: Local::now(),
            theme: Theme::fallback(),
            steam: SteamResolver::default(),
            report,
            lens_totals: [
                Some((8 * 3600, 12 * 3600)),
                Some((11 * 3600, 21 * 3600)),
                Some((40 * 3600, 80 * 3600)),
                Some((200 * 3600, 420 * 3600)),
                Some((500 * 3600, 900 * 3600)),
            ],
            today_intervals: vec![TimelineInterval {
                kind: IntervalKind::Focused,
                app_class: "com.mitchellh.ghostty".to_string(),
                started_at: Local::now().timestamp() - 1200,
                ended_at: Local::now().timestamp() - 600,
            }],
            health: HealthSnapshot::default(),
        }
    }
}
