use crate::{
    steam::SteamResolver,
    storage::{AppTotals, DayTotals, Storage},
};
use anyhow::Result;
use chrono::{DateTime, Local};
use crossterm::{
    event::{self, Event, KeyCode},
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
use std::{
    f64::consts::PI,
    io,
    time::{Duration, Instant},
};

const LENSES: [&str; 5] = ["DAY", "WEEK", "MONTH", "YEAR", "LIFE"];
const FRAME_TIME: Duration = Duration::from_millis(50);
const AUTO_REFRESH: Duration = Duration::from_secs(5);

// Use the terminal palette so matugen/Skwd-wall themes can recolor the TUI.
const BG: Color = Color::Indexed(0);
const PANEL: Color = Color::Indexed(0);
const PANEL_2: Color = Color::Indexed(8);
const SELECTED: Color = Color::Indexed(8);
const TEXT: Color = Color::Indexed(15);
const MUTED: Color = Color::Indexed(7);
const DIM: Color = Color::Indexed(8);
const CYAN: Color = Color::Indexed(14);
const BLUE: Color = Color::Indexed(12);
const GREEN: Color = Color::Indexed(10);
const YELLOW: Color = Color::Indexed(11);
const MAGENTA: Color = Color::Indexed(13);
const ORANGE: Color = Color::Indexed(3);
const RED: Color = Color::Indexed(9);
const SPINNER: [&str; 8] = ["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"];

pub fn run(storage: Storage) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_app(&mut terminal, &storage);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, storage: &Storage) -> Result<()> {
    let mut app = App::load(storage)?;

    loop {
        terminal.draw(|frame| render(frame, &app))?;

        if app.last_refresh.elapsed() >= AUTO_REFRESH {
            app.refresh(storage)?;
        }
        app.tick = app.tick.wrapping_add(1);

        if event::poll(FRAME_TIME)? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('r') => app.refresh(storage)?,
                    KeyCode::Left | KeyCode::Char('h') => app.previous_lens(),
                    KeyCode::Right | KeyCode::Char('l') => app.next_lens(),
                    KeyCode::Up | KeyCode::Char('k') => app.move_selection(-1),
                    KeyCode::Down | KeyCode::Char('j') => app.move_selection(1),
                    KeyCode::PageUp => app.move_selection(-6),
                    KeyCode::PageDown => app.move_selection(6),
                    KeyCode::Home => app.select_first(),
                    KeyCode::End => app.select_last(),
                    KeyCode::Char('1') => app.set_lens(0),
                    KeyCode::Char('2') => app.set_lens(1),
                    KeyCode::Char('3') => app.set_lens(2),
                    KeyCode::Char('4') => app.set_lens(3),
                    KeyCode::Char('5') => app.set_lens(4),
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

#[derive(Debug)]
struct App {
    lens: usize,
    selected: usize,
    tick: u64,
    last_refresh: Instant,
    loaded_at: DateTime<Local>,
    today: Vec<AppTotals>,
    week: Vec<AppTotals>,
    month: Vec<AppTotals>,
    year: Vec<AppTotals>,
    all_time: Vec<AppTotals>,
    days: Vec<DayTotals>,
}

impl App {
    fn load(storage: &Storage) -> Result<Self> {
        let mut steam = SteamResolver::default();
        Ok(Self {
            lens: 0,
            selected: 0,
            tick: 0,
            last_refresh: Instant::now(),
            loaded_at: Local::now(),
            today: steam.resolve_totals(storage.totals_for_today()?),
            week: steam.resolve_totals(storage.totals_for_week()?),
            month: steam.resolve_totals(storage.totals_for_month()?),
            year: steam.resolve_totals(storage.totals_for_year()?),
            all_time: steam.resolve_totals(storage.totals_all_time()?),
            days: storage.daily_totals(14)?,
        })
    }

    fn refresh(&mut self, storage: &Storage) -> Result<()> {
        let lens = self.lens;
        let selected = self.selected;
        let tick = self.tick;
        *self = Self::load(storage)?;
        self.lens = lens;
        self.selected = selected;
        self.tick = tick;
        self.clamp_selection();
        Ok(())
    }

    fn rows(&self) -> &[AppTotals] {
        match self.lens {
            0 => &self.today,
            1 => &self.week,
            2 => &self.month,
            3 => &self.year,
            _ => &self.all_time,
        }
    }

    fn lens_label(&self) -> &'static str {
        LENSES[self.lens]
    }

    fn selected_row(&self) -> Option<&AppTotals> {
        self.rows().get(self.selected)
    }

    fn lens_total(&self, lens: usize) -> i64 {
        focused_total(match lens {
            0 => &self.today,
            1 => &self.week,
            2 => &self.month,
            3 => &self.year,
            _ => &self.all_time,
        })
    }

    fn previous_lens(&mut self) {
        self.set_lens(if self.lens == 0 {
            LENSES.len() - 1
        } else {
            self.lens - 1
        });
    }

    fn next_lens(&mut self) {
        self.set_lens((self.lens + 1) % LENSES.len());
    }

    fn set_lens(&mut self, lens: usize) {
        self.lens = lens.min(LENSES.len() - 1);
        self.tick = 0;
        self.clamp_selection();
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

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    fill_area(frame, area, BG);

    if area.width < 52 || area.height < 16 {
        render_tiny(frame, area, app);
        return;
    }

    let [header, body, footer] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(12),
            Constraint::Length(1),
        ])
        .split(area)
    else {
        return;
    };

    render_header(frame, header, app);
    if area.width < 76 {
        render_narrow_monitor(frame, body, app);
    } else {
        render_monitor(frame, body, app);
    }
    render_footer(frame, footer, app);
}

fn render_tiny(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lines = vec![
        Line::from(vec![
            Span::styled("omastat ", Style::default().fg(pulse_color(app.tick))),
            Span::styled(app.lens_label(), Style::default().fg(lens_color(app.lens))),
        ]),
        Line::from(Span::styled(
            "terminal too small",
            Style::default().fg(MUTED),
        )),
        Line::from(Span::styled("q quits", Style::default().fg(DIM))),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(panel("MONITOR", CYAN))
            .style(Style::default().bg(PANEL)),
        area,
    );
}

fn render_monitor(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [rail, deck] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(12), Constraint::Min(48)])
        .split(area)
    else {
        return;
    };

    render_mode_rail(frame, rail, app);

    let signal_height = if deck.height < 19 { 7 } else { 8 };
    let [top, bottom] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(signal_height), Constraint::Min(8)])
        .split(deck)
    else {
        return;
    };
    let [flow, core] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(63), Constraint::Percentage(37)])
        .split(top)
    else {
        return;
    };
    let [stack, inspect] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(bottom)
    else {
        return;
    };

    render_timeflow(frame, flow, app);
    render_core(frame, core, app);
    render_app_stack(frame, stack, app);
    render_inspector(frame, inspect, app);
}

fn render_narrow_monitor(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [modes, flow, stack] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(6),
        ])
        .split(area)
    else {
        return;
    };

    render_mode_strip(frame, modes, app);
    render_timeflow(frame, flow, app);
    render_app_stack(frame, stack, app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, BG);

    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let density = ratio(focused, open);
    let accent = pulse_color(app.tick);
    let clock = app.loaded_at.format("%H:%M:%S").to_string();
    let spin = SPINNER[(app.tick as usize / 2) % SPINNER.len()];
    let line = Line::from(vec![
        Span::styled(
            " OMASTAT",
            Style::default()
                .fg(accent)
                .bg(BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("::monitor ", Style::default().fg(MUTED).bg(BG)),
        Span::styled(spin, Style::default().fg(GREEN).bg(BG)),
        Span::styled(" lens ", Style::default().fg(DIM).bg(BG)),
        Span::styled(
            app.lens_label(),
            Style::default()
                .fg(lens_color(app.lens))
                .bg(BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  focus ", Style::default().fg(DIM).bg(BG)),
        Span::styled(format_duration(focused), Style::default().fg(YELLOW).bg(BG)),
        Span::styled("  open ", Style::default().fg(DIM).bg(BG)),
        Span::styled(format_duration(open), Style::default().fg(BLUE).bg(BG)),
        Span::styled("  density ", Style::default().fg(DIM).bg(BG)),
        Span::styled(percent(density), Style::default().fg(MAGENTA).bg(BG)),
        Span::styled("  updated ", Style::default().fg(DIM).bg(BG)),
        Span::styled(clock, Style::default().fg(TEXT).bg(BG)),
    ]);
    let scan = Line::from(Span::styled(
        scan_rail(area.width as usize, app.tick),
        Style::default().fg(DIM).bg(BG),
    ));
    frame.render_widget(
        Paragraph::new(vec![line, scan]).style(Style::default().bg(BG)),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, BG);

    let mode = format!("{} {}", app.lens + 1, app.lens_label());
    let left = format!(" h/l lens  j/k select  pg jump  r refresh  q quit  [{mode}]");
    let right = format!("{}s refresh", AUTO_REFRESH.as_secs());
    let padding =
        area.width
            .saturating_sub((left.chars().count() + right.chars().count()) as u16) as usize;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, Style::default().fg(MUTED).bg(BG)),
            Span::styled(" ".repeat(padding), Style::default().bg(BG)),
            Span::styled(right, Style::default().fg(DIM).bg(BG)),
        ]))
        .style(Style::default().bg(BG)),
        area,
    );
}

fn render_mode_rail(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let block = panel("LENS", CYAN);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_total = (0..LENSES.len())
        .map(|index| app.lens_total(index))
        .max()
        .unwrap_or(1)
        .max(1);
    let mut lines = Vec::new();

    for (index, label) in LENSES.iter().enumerate() {
        let selected = index == app.lens;
        let color = lens_color(index);
        let bg = if selected { SELECTED } else { PANEL };
        let marker = if selected { "▶" } else { " " };
        lines.push(Line::from(vec![
            Span::styled(marker, Style::default().fg(color).bg(bg)),
            Span::styled(format!("{} ", index + 1), Style::default().fg(DIM).bg(bg)),
            Span::styled(
                *label,
                Style::default()
                    .fg(if selected { TEXT } else { MUTED })
                    .bg(bg)
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
        ]));
        lines.push(Line::from(Span::styled(
            mini_bar(
                ratio(app.lens_total(index), max_total),
                inner.width as usize,
                color,
            ),
            Style::default().fg(color).bg(bg),
        )));
        lines.push(Line::from(Span::styled(
            format_duration(app.lens_total(index)),
            Style::default().fg(DIM).bg(PANEL),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_mode_strip(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let block = panel("LENS", CYAN);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut spans = Vec::new();
    for (index, label) in LENSES.iter().enumerate() {
        let selected = index == app.lens;
        spans.push(Span::styled(
            format!(" {}:{} ", index + 1, label),
            Style::default()
                .fg(if selected { TEXT } else { MUTED })
                .bg(if selected { SELECTED } else { PANEL })
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(PANEL)),
        inner,
    );
}

fn render_timeflow(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let block = panel("BRAILLE FLOW", GREEN);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let graph_height = inner.height.saturating_sub(1).max(1) as usize;
    let mut lines = day_graph(&app.days, inner.width as usize, graph_height, app.tick);
    let latest = app
        .days
        .last()
        .map(|day| {
            format!(
                " {} {} focused / {} open",
                day.label,
                format_duration(day.focused_seconds),
                format_duration(day.open_seconds)
            )
        })
        .unwrap_or_else(|| " no daily samples".to_string());
    lines.push(Line::from(vec![
        Span::styled("braille 14d", Style::default().fg(DIM).bg(PANEL)),
        Span::styled(latest, Style::default().fg(MUTED).bg(PANEL)),
    ]));

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_core(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let density = ratio(focused, open);
    let top = rows
        .first()
        .map(|row| short_app(&row.app_class, area.width.saturating_sub(8) as usize))
        .unwrap_or_else(|| "no signal".to_string());
    let best = best_focus_day(&app.days)
        .map(|day| format!("{} {}", day.label, duration_compact(day.focused_seconds)))
        .unwrap_or_else(|| "no focus yet".to_string());
    let active = active_days(&app.days);
    let block = panel("REPLAY", pulse_color(app.tick));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let meter_width = inner.width.saturating_sub(12).max(3) as usize;
    let lines = vec![
        Line::from(Span::styled(
            "FOCUS REPLAY",
            Style::default()
                .fg(DIM)
                .bg(PANEL)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format_duration(focused),
            Style::default()
                .fg(YELLOW)
                .bg(PANEL)
                .add_modifier(Modifier::BOLD),
        )),
        meter_line("dense", density, meter_width, MAGENTA, app.tick, PANEL),
        meter_line(
            "apps",
            ratio(rows.len() as i64, 24),
            meter_width,
            CYAN,
            app.tick + 8,
            PANEL,
        ),
        Line::from(vec![
            Span::styled("top ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(top, Style::default().fg(TEXT).bg(PANEL)),
        ]),
        Line::from(vec![
            Span::styled("best ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(best, Style::default().fg(GREEN).bg(PANEL)),
        ]),
        Line::from(vec![
            Span::styled("days ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(
                format!("{active}/{} active", app.days.len().max(1)),
                Style::default().fg(CYAN).bg(PANEL),
            ),
        ]),
        Line::from(Span::styled(
            scan_rail(inner.width as usize, app.tick + 9),
            Style::default().fg(DIM).bg(PANEL),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_app_stack(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let block = panel("APP STACK", BLUE);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = app.rows();
    if rows.is_empty() {
        let mut lines = vec![
            Line::from(Span::styled(
                "waiting for app intervals",
                Style::default().fg(MUTED).bg(PANEL),
            )),
            Line::from(Span::styled(
                scan_rail(inner.width as usize, app.tick),
                Style::default().fg(DIM).bg(PANEL),
            )),
        ];
        lines.extend(empty_matrix(
            inner.width as usize,
            inner.height.saturating_sub(2) as usize,
        ));
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(PANEL))
                .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }

    let max = rows
        .first()
        .map(|row| row.focused_seconds.max(1))
        .unwrap_or(1);
    let focused = focused_total(rows).max(1);
    let visible = inner.height.saturating_sub(1) as usize;
    let width = inner.width as usize;
    let name_width = width.saturating_sub(27).clamp(8, 22);
    let bar_width = width.saturating_sub(name_width + 20).clamp(5, 30);
    let mut lines = vec![Line::from(vec![
        Span::styled(" #  ", Style::default().fg(DIM).bg(PANEL_2)),
        Span::styled(
            fit_text("process", name_width),
            Style::default().fg(MUTED).bg(PANEL_2),
        ),
        Span::styled(" focus map", Style::default().fg(MUTED).bg(PANEL_2)),
    ])];

    for (index, row) in rows.iter().take(visible.max(1)).enumerate() {
        let row_bg = if index == app.selected {
            SELECTED
        } else {
            PANEL
        };
        let color = rank_color(index);
        let marker = if index == app.selected { "▸" } else { " " };
        let mut spans = vec![
            Span::styled(marker, Style::default().fg(color).bg(row_bg)),
            Span::styled(
                format!("{:>2} ", index + 1),
                Style::default().fg(DIM).bg(row_bg),
            ),
            Span::styled(
                fit_text(&short_app(&row.app_class, name_width), name_width),
                Style::default()
                    .fg(if index == app.selected { TEXT } else { color })
                    .bg(row_bg)
                    .add_modifier(if index == app.selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            Span::styled(" ", Style::default().bg(row_bg)),
        ];
        spans.extend(bar_spans(
            ratio(row.focused_seconds, max),
            bar_width,
            color,
            app.tick + index as u64 * 2,
            row_bg,
        ));
        spans.push(Span::styled(
            format!(" {} ", duration_compact(row.focused_seconds)),
            Style::default().fg(YELLOW).bg(row_bg),
        ));
        spans.push(Span::styled(
            percent(ratio(row.focused_seconds, focused)),
            Style::default().fg(MAGENTA).bg(row_bg),
        ));
        lines.push(Line::from(spans));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_inspector(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let block = panel("INSPECT", ORANGE);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = app.rows();
    let total_focus = focused_total(rows).max(1);
    let Some(row) = app.selected_row() else {
        let lines = vec![
            Line::from(Span::styled(
                "no app selected",
                Style::default().fg(MUTED).bg(PANEL),
            )),
            Line::from(Span::styled(
                scan_rail(inner.width as usize, app.tick),
                Style::default().fg(DIM).bg(PANEL),
            )),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(PANEL))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    };

    let width = inner.width as usize;
    let density = ratio(row.focused_seconds, row.open_seconds);
    let share = ratio(row.focused_seconds, total_focus);
    let name = fit_text(&row.app_class, width);
    let meter_width = width.saturating_sub(12).max(3);
    let mut lines = vec![
        Line::from(Span::styled(
            format!("#{:02}", app.selected + 1),
            Style::default().fg(DIM).bg(PANEL),
        )),
        Line::from(Span::styled(
            name,
            Style::default()
                .fg(TEXT)
                .bg(PANEL)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("focus ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(
                format_duration(row.focused_seconds),
                Style::default().fg(YELLOW).bg(PANEL),
            ),
        ]),
        Line::from(vec![
            Span::styled("open  ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(
                format_duration(row.open_seconds),
                Style::default().fg(BLUE).bg(PANEL),
            ),
        ]),
        meter_line("dense", density, meter_width, MAGENTA, app.tick, PANEL),
        meter_line("share", share, meter_width, GREEN, app.tick + 11, PANEL),
    ];
    let remaining = inner.height.saturating_sub(lines.len() as u16) as usize;
    lines.extend(focus_mix_lines(
        rows,
        app.selected,
        width,
        remaining,
        app.tick + 5,
    ));

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn day_graph(days: &[DayTotals], width: usize, height: usize, tick: u64) -> Vec<Line<'static>> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    if days.is_empty() {
        return empty_matrix(width, height);
    }

    let focus_max = days
        .iter()
        .map(|day| day.focused_seconds)
        .max()
        .unwrap_or(0)
        .max(1);
    let open_max = days
        .iter()
        .map(|day| day.open_seconds)
        .max()
        .unwrap_or(0)
        .max(1);
    let virtual_width = width * 2;
    let virtual_height = height * 4;
    let scan_x = (tick as usize / 2) % width;
    let mut lines = Vec::with_capacity(height);

    for row in 0..height {
        let mut spans = Vec::with_capacity(width);
        for col in 0..width {
            let mut focus_mask = 0u16;
            let mut open_mask = 0u16;
            let mut color = DIM;

            for dot_y in 0..4 {
                for dot_x in 0..2 {
                    let virtual_x = (col * 2 + dot_x).min(virtual_width.saturating_sub(1));
                    let index = ((virtual_x * days.len()) / virtual_width).min(days.len() - 1);
                    let day = &days[index];
                    let virtual_y = row * 4 + dot_y;
                    let from_bottom = virtual_height.saturating_sub(virtual_y);
                    let focus_level = (ratio(day.focused_seconds, focus_max)
                        * virtual_height as f64)
                        .ceil() as usize;
                    let open_level =
                        (ratio(day.open_seconds, open_max) * virtual_height as f64).ceil() as usize;
                    let bit = braille_bit(dot_x, dot_y);

                    if focus_level > 0 && from_bottom <= focus_level {
                        focus_mask |= bit;
                        color = heat_color(index, ratio(day.focused_seconds, focus_max));
                    } else if open_level > 0 && from_bottom <= open_level {
                        open_mask |= bit;
                    }
                }
            }

            let sweep = col == scan_x || col.abs_diff(scan_x) <= 1;
            let (glyph, fg) = if focus_mask != 0 {
                (braille_char(focus_mask), if sweep { TEXT } else { color })
            } else if open_mask != 0 {
                (braille_char(open_mask), DIM)
            } else if sweep && (row + col + tick as usize) % 4 == 0 {
                ("⠂".to_string(), DIM)
            } else {
                (" ".to_string(), DIM)
            };
            spans.push(Span::styled(glyph, Style::default().fg(fg).bg(PANEL)));
        }
        lines.push(Line::from(spans));
    }

    lines
}

#[derive(Clone)]
struct PieSegment {
    label: String,
    share: f64,
    color: Color,
    selected: bool,
}

fn focus_mix_lines(
    rows: &[AppTotals],
    selected: usize,
    width: usize,
    height: usize,
    tick: u64,
) -> Vec<Line<'static>> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let segments = pie_segments(rows, selected);
    if segments.is_empty() {
        return vec![Line::from(Span::styled(
            "mix no focus signal",
            Style::default().fg(MUTED).bg(PANEL),
        ))];
    }

    if height < 3 || width < 14 {
        return segments
            .iter()
            .take(height)
            .map(|segment| {
                Line::from(vec![
                    Span::styled("mix ", Style::default().fg(DIM).bg(PANEL)),
                    Span::styled(
                        percent(segment.share),
                        Style::default().fg(segment.color).bg(PANEL),
                    ),
                    Span::styled(
                        format!(" {}", fit_text(&segment.label, width.saturating_sub(9))),
                        Style::default().fg(MUTED).bg(PANEL),
                    ),
                ])
            })
            .collect();
    }

    if width < 30 {
        let chart_width = width.min(13);
        let chart_height = height.min(7);
        let left_padding = width.saturating_sub(chart_width) / 2;
        let right_padding = width.saturating_sub(chart_width + left_padding);
        let active = segments
            .iter()
            .position(|segment| segment.selected)
            .unwrap_or_else(|| ((tick / 18) as usize) % segments.len());

        return (0..chart_height)
            .map(|y| {
                let mut spans = vec![Span::styled(
                    " ".repeat(left_padding),
                    Style::default().bg(PANEL),
                )];
                spans.extend(pie_row(
                    &segments,
                    active,
                    chart_width,
                    chart_height,
                    y,
                    tick,
                ));
                spans.push(Span::styled(
                    " ".repeat(right_padding),
                    Style::default().bg(PANEL),
                ));
                Line::from(spans)
            })
            .collect();
    }

    let chart_width = width.min(15);
    let chart_height = height.min(7);
    let legend_width = width.saturating_sub(chart_width + 1);
    let active = segments
        .iter()
        .position(|segment| segment.selected)
        .unwrap_or_else(|| ((tick / 18) as usize) % segments.len());
    let mut lines = Vec::with_capacity(chart_height);

    for y in 0..chart_height {
        let mut spans = pie_row(&segments, active, chart_width, chart_height, y, tick);
        spans.push(Span::styled(" ", Style::default().bg(PANEL)));

        if legend_width > 0 {
            if let Some(segment) = segments.get(y) {
                let marker = if segment.selected { "▸" } else { " " };
                let label_width = legend_width.saturating_sub(8);
                spans.push(Span::styled(
                    marker,
                    Style::default()
                        .fg(if segment.selected {
                            TEXT
                        } else {
                            segment.color
                        })
                        .bg(PANEL),
                ));
                spans.push(Span::styled(
                    "●",
                    Style::default().fg(segment.color).bg(PANEL),
                ));
                spans.push(Span::styled(
                    format!(" {}", percent(segment.share)),
                    Style::default().fg(TEXT).bg(PANEL),
                ));
                spans.push(Span::styled(
                    format!(" {}", fit_text(&segment.label, label_width)),
                    Style::default().fg(MUTED).bg(PANEL),
                ));
            } else if y + 1 == chart_height {
                spans.push(Span::styled(
                    scan_rail(legend_width, tick + 13),
                    Style::default().fg(DIM).bg(PANEL),
                ));
            }
        }

        lines.push(Line::from(spans));
    }

    lines.truncate(height);
    lines
}

fn empty_matrix(width: usize, height: usize) -> Vec<Line<'static>> {
    (0..height)
        .map(|row| {
            let text = (0..width)
                .map(|col| {
                    if (row * 7 + col * 3) % 31 == 0 {
                        '·'
                    } else {
                        ' '
                    }
                })
                .collect::<String>();
            Line::from(Span::styled(text, Style::default().fg(DIM).bg(PANEL)))
        })
        .collect()
}

fn panel(title: &'static str, accent: Color) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent).bg(PANEL))
        .style(Style::default().bg(PANEL))
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

fn meter_line(
    label: &str,
    value: f64,
    width: usize,
    color: Color,
    tick: u64,
    background: Color,
) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{label:<5} "),
        Style::default().fg(MUTED).bg(background),
    )];
    spans.extend(bar_spans(value, width, color, tick, background));
    spans.push(Span::styled(
        format!(" {}", percent(value)),
        Style::default().fg(TEXT).bg(background),
    ));
    Line::from(spans)
}

fn bar_spans(
    value: f64,
    width: usize,
    color: Color,
    tick: u64,
    background: Color,
) -> Vec<Span<'static>> {
    let width = width.max(1);
    let filled = ((value.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    let sweep = if filled == 0 {
        usize::MAX
    } else {
        (tick as usize / 2) % filled.max(1)
    };
    (0..width)
        .map(|index| {
            if index < filled {
                let style = if index == sweep {
                    Style::default()
                        .fg(TEXT)
                        .bg(background)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(color).bg(background)
                };
                Span::styled(if index == sweep { "▓" } else { "█" }, style)
            } else {
                Span::styled("░", Style::default().fg(DIM).bg(background))
            }
        })
        .collect()
}

fn mini_bar(value: f64, width: usize, _color: Color) -> String {
    let width = width.max(1);
    let filled = ((value.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    format!(
        "{}{}",
        "▀".repeat(filled),
        "˙".repeat(width.saturating_sub(filled))
    )
}

fn scan_rail(width: usize, tick: u64) -> String {
    if width == 0 {
        return String::new();
    }

    let head = (tick as usize / 2) % width;
    (0..width)
        .map(|index| {
            let distance = index.abs_diff(head);
            if distance == 0 {
                '█'
            } else if distance <= 2 {
                '━'
            } else if index % 8 == 0 {
                '╴'
            } else {
                '─'
            }
        })
        .collect()
}

fn pie_segments(rows: &[AppTotals], selected: usize) -> Vec<PieSegment> {
    let total = focused_total(rows).max(1) as f64;
    let top_count = rows.len().min(5);
    let mut segments = Vec::new();

    for (index, row) in rows.iter().take(top_count).enumerate() {
        if row.focused_seconds <= 0 {
            continue;
        }

        segments.push(PieSegment {
            label: short_app(&row.app_class, 18).trim().to_string(),
            share: row.focused_seconds as f64 / total,
            color: rank_color(index),
            selected: index == selected,
        });
    }

    let other_seconds = rows
        .iter()
        .skip(top_count)
        .map(|row| row.focused_seconds.max(0))
        .sum::<i64>();
    if other_seconds > 0 {
        segments.push(PieSegment {
            label: "other".to_string(),
            share: other_seconds as f64 / total,
            color: MUTED,
            selected: selected >= top_count,
        });
    }

    segments
}

fn pie_row(
    segments: &[PieSegment],
    active: usize,
    width: usize,
    height: usize,
    row: usize,
    tick: u64,
) -> Vec<Span<'static>> {
    let center_x = (width as f64 - 1.0) / 2.0;
    let center_y = (height as f64 - 1.0) / 2.0;
    let outer_x = (width as f64 / 2.0).max(1.0);
    let outer_y = (height as f64 / 2.0).max(1.0);
    let rotation = (tick % 240) as f64 / 240.0 * PI * 0.35;

    (0..width)
        .map(|col| {
            let dx = (col as f64 - center_x) / outer_x;
            let dy = (row as f64 - center_y) / outer_y;
            let radius = (dx * dx + dy * dy).sqrt();

            if !(0.38..=1.0).contains(&radius) {
                return Span::styled(" ", Style::default().bg(PANEL));
            }

            let angle = (dy.atan2(dx) + PI * 2.5 + rotation) % (PI * 2.0);
            let segment_index = pie_segment_index(segments, angle / (PI * 2.0));
            let segment = &segments[segment_index];
            let highlighted = segment.selected || segment_index == active;
            let glyph = if highlighted && tick % 20 < 10 {
                "█"
            } else if radius > 0.82 {
                "▓"
            } else {
                "■"
            };
            Span::styled(
                glyph,
                Style::default()
                    .fg(if highlighted { TEXT } else { segment.color })
                    .bg(PANEL)
                    .add_modifier(if highlighted {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )
        })
        .collect()
}

fn pie_segment_index(segments: &[PieSegment], position: f64) -> usize {
    let total = segments.iter().map(|segment| segment.share).sum::<f64>();
    let target = position.clamp(0.0, 1.0) * total.max(f64::EPSILON);
    let mut cumulative = 0.0;

    for (index, segment) in segments.iter().enumerate() {
        cumulative += segment.share;
        if target <= cumulative {
            return index;
        }
    }

    segments.len().saturating_sub(1)
}

fn braille_bit(x: usize, y: usize) -> u16 {
    match (x, y) {
        (0, 0) => 0x01,
        (0, 1) => 0x02,
        (0, 2) => 0x04,
        (0, 3) => 0x40,
        (1, 0) => 0x08,
        (1, 1) => 0x10,
        (1, 2) => 0x20,
        (1, 3) => 0x80,
        _ => 0,
    }
}

fn braille_char(mask: u16) -> String {
    char::from_u32(0x2800 + mask as u32)
        .unwrap_or(' ')
        .to_string()
}

fn focused_total(rows: &[AppTotals]) -> i64 {
    rows.iter().map(|row| row.focused_seconds).sum()
}

fn open_total(rows: &[AppTotals]) -> i64 {
    rows.iter().map(|row| row.open_seconds).sum()
}

fn active_days(days: &[DayTotals]) -> usize {
    days.iter()
        .filter(|day| day.focused_seconds > 0 || day.open_seconds > 0)
        .count()
}

fn best_focus_day(days: &[DayTotals]) -> Option<&DayTotals> {
    days.iter()
        .filter(|day| day.focused_seconds > 0)
        .max_by_key(|day| day.focused_seconds)
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        (numerator.max(0) as f64 / denominator as f64).clamp(0.0, 1.0)
    }
}

fn lens_color(index: usize) -> Color {
    match index {
        0 => CYAN,
        1 => GREEN,
        2 => YELLOW,
        3 => MAGENTA,
        _ => ORANGE,
    }
}

fn pulse_color(tick: u64) -> Color {
    match tick % 40 {
        0..=9 => CYAN,
        10..=19 => GREEN,
        20..=29 => YELLOW,
        _ => MAGENTA,
    }
}

fn rank_color(index: usize) -> Color {
    match index {
        0 => YELLOW,
        1 => CYAN,
        2 => GREEN,
        3 => MAGENTA,
        4 => ORANGE,
        5 => RED,
        _ => MUTED,
    }
}

fn heat_color(index: usize, share: f64) -> Color {
    if share <= 0.0 {
        DIM
    } else {
        match index % 6 {
            0 => CYAN,
            1 => GREEN,
            2 => YELLOW,
            3 => MAGENTA,
            4 => BLUE,
            _ => ORANGE,
        }
    }
}

fn short_app(value: &str, width: usize) -> String {
    let mut value = value
        .trim_start_matches("com.")
        .trim_start_matches("org.")
        .to_string();
    if value.contains('.') {
        value = value.rsplit('.').next().unwrap_or(&value).to_string();
    }
    fit_text(&value, width)
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

fn percent(value: f64) -> String {
    format!("{:>3.0}%", value.clamp(0.0, 1.0) * 100.0)
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

fn duration_compact(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours:>2}h")
    } else if minutes > 0 {
        format!("{minutes:>2}m")
    } else {
        format!("{seconds:>2}s")
    }
}
