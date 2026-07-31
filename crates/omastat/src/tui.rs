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

const BG: Color = Color::Rgb(2, 5, 8);
const PANEL: Color = Color::Rgb(7, 11, 16);
const PANEL_2: Color = Color::Rgb(10, 17, 24);
const SELECTED: Color = Color::Rgb(22, 33, 45);
const TEXT: Color = Color::Rgb(226, 238, 242);
const MUTED: Color = Color::Rgb(104, 120, 132);
const DIM: Color = Color::Rgb(36, 49, 60);
const CYAN: Color = Color::Rgb(74, 222, 255);
const BLUE: Color = Color::Rgb(92, 144, 255);
const GREEN: Color = Color::Rgb(79, 255, 170);
const YELLOW: Color = Color::Rgb(255, 218, 93);
const MAGENTA: Color = Color::Rgb(255, 93, 211);
const ORANGE: Color = Color::Rgb(255, 150, 80);
const RED: Color = Color::Rgb(255, 92, 112);
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

    let block = panel("TIMEFLOW", GREEN);
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
        Span::styled("14d", Style::default().fg(DIM).bg(PANEL)),
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
    let block = panel("CORE", pulse_color(app.tick));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let meter_width = inner.width.saturating_sub(12).max(3) as usize;
    let lines = vec![
        Line::from(Span::styled(
            "FOCUS CLOCK",
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
        Line::from(orbit_spans(inner.width as usize, app.tick)),
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
                orbit_text(inner.width as usize, app.tick),
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
    let lines = vec![
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
        Line::from(orbit_spans(width, app.tick + 5)),
        Line::from(Span::styled(
            sparkline(&app.days, width, app.tick),
            Style::default().fg(CYAN).bg(PANEL),
        )),
    ];

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

    let days = if days.is_empty() { &[][..] } else { days };
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
    let col_width = if days.is_empty() {
        width
    } else {
        (width / days.len()).max(1)
    };
    let scan_x = (tick as usize / 2) % width.max(1);
    let mut lines = Vec::with_capacity(height);

    for y in (0..height).rev() {
        let mut spans = Vec::with_capacity(width);
        let threshold = (y + 1) as f64 / height as f64;
        let mut x = 0;
        for (index, day) in days.iter().enumerate() {
            let focus = ratio(day.focused_seconds, focus_max);
            let open = ratio(day.open_seconds, open_max);
            let color = heat_color(index, focus);
            for _ in 0..col_width {
                if x >= width {
                    break;
                }
                let glyph = if focus >= threshold {
                    "█"
                } else if open >= threshold {
                    "░"
                } else if x == scan_x || (x + width - scan_x) % width < 2 {
                    "·"
                } else {
                    " "
                };
                let fg = if focus >= threshold {
                    color
                } else if open >= threshold {
                    DIM
                } else {
                    Color::Rgb(18, 27, 34)
                };
                spans.push(Span::styled(glyph, Style::default().fg(fg).bg(PANEL)));
                x += 1;
            }
        }

        while x < width {
            let glyph = if x == scan_x { "·" } else { " " };
            spans.push(Span::styled(
                glyph,
                Style::default().fg(Color::Rgb(18, 27, 34)).bg(PANEL),
            ));
            x += 1;
        }
        lines.push(Line::from(spans));
    }

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
                        .fg(Color::White)
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

fn orbit_spans(width: usize, tick: u64) -> Vec<Span<'static>> {
    let text = orbit_text(width, tick);
    text.chars()
        .enumerate()
        .map(|(index, ch)| {
            let color = match (index + tick as usize) % 17 {
                0..=2 => CYAN,
                3..=4 => GREEN,
                5 => YELLOW,
                _ => DIM,
            };
            Span::styled(ch.to_string(), Style::default().fg(color).bg(PANEL))
        })
        .collect()
}

fn orbit_text(width: usize, tick: u64) -> String {
    if width == 0 {
        return String::new();
    }

    let width = width.min(48);
    let center = (width as f64 - 1.0) / 2.0;
    let phase = (tick % 96) as f64 / 96.0 * PI * 2.0;
    (0..width)
        .map(|index| {
            let x = index as f64 - center;
            let wave = (x / 2.4 + phase).sin().abs();
            if wave > 0.93 {
                '◆'
            } else if wave > 0.78 {
                '◇'
            } else if index % 7 == 0 {
                '·'
            } else {
                ' '
            }
        })
        .collect()
}

fn sparkline(days: &[DayTotals], width: usize, tick: u64) -> String {
    let symbols = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    let max = days
        .iter()
        .map(|day| day.focused_seconds)
        .max()
        .unwrap_or(0)
        .max(1);
    let mut out = String::new();
    for (index, day) in days.iter().enumerate() {
        let value = ratio(day.focused_seconds, max);
        let mut slot = (value * (symbols.len() - 1) as f64).round() as usize;
        if index == days.len().saturating_sub(1) && tick % 20 < 10 && slot < symbols.len() - 1 {
            slot += 1;
        }
        out.push_str(symbols[slot]);
    }
    fit_text(&out, width)
}

fn focused_total(rows: &[AppTotals]) -> i64 {
    rows.iter().map(|row| row.focused_seconds).sum()
}

fn open_total(rows: &[AppTotals]) -> i64 {
    rows.iter().map(|row| row.open_seconds).sum()
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
