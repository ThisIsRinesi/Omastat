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
    symbols::Marker,
    text::{Line, Span},
    widgets::{
        Axis, Block, BorderType, Borders, Cell, Chart, Dataset, GraphType, Paragraph, Row, Table,
        Wrap,
    },
};
use std::{
    f64::consts::PI,
    io,
    time::{Duration, Instant},
};

const TABS: [&str; 4] = ["Today", "Week", "Year", "All"];
const FRAME_TIME: Duration = Duration::from_millis(66);
const AUTO_REFRESH: Duration = Duration::from_secs(5);
const BG: Color = Color::Rgb(4, 7, 11);
const PANEL: Color = Color::Rgb(8, 13, 20);
const PANEL_ALT: Color = Color::Rgb(11, 17, 25);
const TEXT: Color = Color::Rgb(218, 231, 239);
const MUTED: Color = Color::Rgb(96, 112, 126);
const DIM: Color = Color::Rgb(35, 48, 60);
const CYAN: Color = Color::Rgb(74, 222, 255);
const BLUE: Color = Color::Rgb(85, 145, 255);
const GREEN: Color = Color::Rgb(78, 255, 168);
const YELLOW: Color = Color::Rgb(255, 216, 89);
const MAGENTA: Color = Color::Rgb(255, 91, 210);
const ORANGE: Color = Color::Rgb(255, 151, 82);
const PIE_COLORS: [Color; 6] = [CYAN, MAGENTA, YELLOW, GREEN, BLUE, ORANGE];
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
                    KeyCode::Left | KeyCode::Char('h') => app.previous_tab(),
                    KeyCode::Right | KeyCode::Char('l') => app.next_tab(),
                    KeyCode::Char('1') => app.set_tab(0),
                    KeyCode::Char('2') => app.set_tab(1),
                    KeyCode::Char('3') => app.set_tab(2),
                    KeyCode::Char('4') => app.set_tab(3),
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
    tab: usize,
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
            tab: 0,
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
        let tab = self.tab;
        let tick = self.tick;
        *self = Self::load(storage)?;
        self.tab = tab;
        self.tick = tick;
        Ok(())
    }

    fn rows(&self) -> &[AppTotals] {
        match self.tab {
            0 => &self.today,
            1 => &self.week,
            2 => &self.year,
            _ => &self.all_time,
        }
    }

    fn selected_label(&self) -> &'static str {
        TABS[self.tab]
    }

    fn previous_tab(&mut self) {
        self.set_tab(if self.tab == 0 {
            TABS.len() - 1
        } else {
            self.tab - 1
        });
    }

    fn next_tab(&mut self) {
        self.set_tab((self.tab + 1) % TABS.len());
    }

    fn set_tab(&mut self, tab: usize) {
        self.tab = tab.min(TABS.len() - 1);
        self.tick = 0;
    }
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    render_backdrop(frame, area, app.tick);

    if area.width < 104 || area.height < 26 {
        render_compact(frame, area, app);
        return;
    }

    let header_height = if area.height < 31 { 4 } else { 5 };
    let telemetry_height = if area.height < 31 { 9 } else { 12 };
    let [header, nav, telemetry, main, footer] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Length(3),
            Constraint::Length(telemetry_height),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area)
    else {
        return;
    };

    render_header(frame, header, app);
    render_tabs(frame, nav, app);
    render_telemetry(frame, telemetry, app);
    render_main(frame, main, app);
    render_footer(frame, footer, app);
}

fn render_backdrop(frame: &mut Frame<'_>, area: Rect, tick: u64) {
    let width = area.width as usize;
    let height = area.height as usize;
    let mut lines = Vec::with_capacity(height);

    for y in 0..height {
        let mut row = String::with_capacity(width);
        for x in 0..width {
            let phase = (x * 5 + y * 11 + tick as usize) % 53;
            let ch = match phase {
                0 => '·',
                1 if y % 4 == 0 => '─',
                2 if x % 13 == 0 => '│',
                _ => ' ',
            };
            row.push(ch);
        }
        lines.push(Line::from(Span::styled(
            row,
            Style::default().fg(Color::Rgb(17, 26, 35)).bg(BG),
        )));
    }

    frame.render_widget(Paragraph::new(lines).style(Style::default().bg(BG)), area);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let rows = app.rows();
    let max_rows = area.height.saturating_sub(7) as usize;
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "omastat ",
                Style::default()
                    .fg(pulse_color(app.tick))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                app.selected_label().to_uppercase(),
                Style::default().fg(MUTED),
            ),
        ]),
        Line::from(vec![
            Span::styled("Focus ", Style::default().fg(MUTED)),
            Span::styled(
                format_duration(focused_total(rows)),
                Style::default().fg(YELLOW),
            ),
            Span::raw("  "),
            Span::styled("Open ", Style::default().fg(MUTED)),
            Span::styled(format_duration(open_total(rows)), Style::default().fg(CYAN)),
            Span::raw("  "),
            Span::styled("Apps ", Style::default().fg(MUTED)),
            Span::styled(rows.len().to_string(), Style::default().fg(TEXT)),
        ]),
        Line::from(Span::styled(
            activity_rail(app.tick, area.width.saturating_sub(4) as usize),
            Style::default().fg(DIM),
        )),
    ];

    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "No tracked usage yet.",
            Style::default().fg(MUTED),
        )));
    } else {
        for (index, row) in rows.iter().take(max_rows.max(1)).enumerate() {
            lines.push(Line::from(vec![
                Span::styled(format!("{:>2} ", index + 1), Style::default().fg(DIM)),
                Span::styled(short_app(&row.app_class, 20), Style::default().fg(TEXT)),
                Span::raw(" "),
                Span::styled(
                    duration_fixed(row.focused_seconds),
                    Style::default().fg(YELLOW),
                ),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel("OMASTAT", CYAN))
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let density = ratio(focused, open);
    let top = rows
        .first()
        .map(|row| short_app(&row.app_class, 28))
        .unwrap_or_else(|| "idle".to_string());
    let status = format!("{} LIVE", SPINNER[(app.tick as usize / 2) % SPINNER.len()]);
    let clock = app.loaded_at.format("%H:%M:%S").to_string();
    let accent = pulse_color(app.tick);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                " omastat",
                Style::default()
                    .fg(accent)
                    .bg(BG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" // focus telemetry", Style::default().fg(MUTED).bg(BG)),
            Span::raw("  "),
            Span::styled(status, Style::default().fg(GREEN).bg(BG)),
            Span::styled("  refreshed ", Style::default().fg(DIM).bg(BG)),
            Span::styled(clock, Style::default().fg(TEXT).bg(BG)),
        ]),
        Line::from(vec![
            metric_span("view", app.selected_label(), CYAN),
            metric_span("focused", &format_duration(focused), YELLOW),
            metric_span("open", &format_duration(open), BLUE),
            metric_span("density", &percent(density), MAGENTA),
            metric_span("apps", &rows.len().to_string(), GREEN),
            metric_span("leader", &top, ORANGE),
        ]),
        Line::from(Span::styled(
            activity_rail(app.tick, area.width.saturating_sub(2) as usize),
            Style::default().fg(DIM).bg(BG),
        )),
    ];

    if area.height > 4 {
        lines.push(Line::from(vec![
            Span::styled(" month ", Style::default().fg(DIM).bg(BG)),
            Span::styled(
                format_duration(focused_total(&app.month)),
                Style::default().fg(CYAN).bg(BG),
            ),
            Span::styled("  year ", Style::default().fg(DIM).bg(BG)),
            Span::styled(
                format_duration(focused_total(&app.year)),
                Style::default().fg(YELLOW).bg(BG),
            ),
            Span::styled("  best day ", Style::default().fg(DIM).bg(BG)),
            Span::styled(best_focus_day(&app.days), Style::default().fg(TEXT).bg(BG)),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(BG)).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(accent).bg(BG)),
        ),
        area,
    );
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut tabs = Vec::new();
    tabs.push(Span::raw(" "));
    for (index, title) in TABS.iter().enumerate() {
        let selected = index == app.tab;
        let color = if selected {
            pulse_color(app.tick)
        } else {
            MUTED
        };
        let label = format!(" {}:{} ", index + 1, title.to_uppercase());
        tabs.push(Span::styled(
            label,
            Style::default()
                .fg(color)
                .bg(if selected { PANEL_ALT } else { BG })
                .add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
        ));
        tabs.push(Span::styled(" ", Style::default().fg(DIM).bg(BG)));
    }

    let rail_width = area.width.saturating_sub(2) as usize;
    let lines = vec![
        Line::from(tabs),
        Line::from(Span::styled(
            segmented_rail(app.tick, rail_width),
            Style::default().fg(DIM).bg(BG),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(BG)).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(DIM).bg(BG)),
        ),
        area,
    );
}

fn render_telemetry(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [left, middle, right] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
        ])
        .split(area)
    else {
        return;
    };

    render_command_panel(frame, left, app);
    render_focus_mix(frame, middle, app);
    render_rhythm_panel(frame, right, app);
}

fn render_command_panel(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let density = ratio(focused, open);
    let active_days = app
        .days
        .iter()
        .filter(|day| day.focused_seconds > 0)
        .count();
    let top_share = rows
        .first()
        .map(|row| ratio(row.focused_seconds, focused))
        .unwrap_or(0.0);
    let trend = trend_ratio(&app.days);
    let top = rows
        .first()
        .map(|row| short_app(&row.app_class, 22))
        .unwrap_or_else(|| "none".to_string());
    let width = area.width.saturating_sub(6) as usize;
    let meter_width = width.saturating_sub(16).clamp(8, 28);

    let lines = vec![
        Line::from(vec![
            Span::styled("view ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(
                app.selected_label().to_uppercase(),
                Style::default()
                    .fg(CYAN)
                    .bg(PANEL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format_duration(focused),
                Style::default().fg(YELLOW).bg(PANEL),
            ),
            Span::styled(" focused  ", Style::default().fg(MUTED).bg(PANEL)),
            Span::styled(format_duration(open), Style::default().fg(BLUE).bg(PANEL)),
            Span::styled(" open", Style::default().fg(MUTED).bg(PANEL)),
        ]),
        Line::from(vec![
            Span::styled("leader ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(top, Style::default().fg(TEXT).bg(PANEL)),
        ]),
        Line::from(vec![
            Span::styled("active days ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(
                format!("{active_days}/14"),
                Style::default().fg(GREEN).bg(PANEL),
            ),
            Span::styled("  apps ", Style::default().fg(DIM).bg(PANEL)),
            Span::styled(rows.len().to_string(), Style::default().fg(TEXT).bg(PANEL)),
        ]),
        meter_line("density", density, meter_width, MAGENTA, app.tick),
        meter_line("leader", top_share, meter_width, ORANGE, app.tick + 6),
        meter_line("rhythm", trend, meter_width, GREEN, app.tick + 12),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel("COMMAND", CYAN))
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_focus_mix(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let block = panel("FOCUS MIX", MAGENTA);
    let inner = block.inner(area);
    frame.render_widget(block.style(Style::default().bg(PANEL)), area);

    let total = focused_total(app.rows()).max(0);
    if total == 0 {
        let lines = vec![
            Line::from(Span::styled(
                format!(
                    "{} awaiting signal",
                    SPINNER[(app.tick as usize / 2) % SPINNER.len()]
                ),
                Style::default()
                    .fg(MUTED)
                    .bg(PANEL)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                activity_rail(app.tick, inner.width.saturating_sub(2) as usize),
                Style::default().fg(DIM).bg(PANEL),
            )),
            Line::from(Span::styled(
                "No focused intervals in this view.",
                Style::default().fg(MUTED).bg(PANEL),
            )),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(PANEL))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let segments = pie_segments(app.rows(), total);
    let height = inner.height.max(1) as usize;
    let ring_width = if inner.width < 54 {
        inner.width.saturating_sub(20).clamp(11, 19) as usize
    } else {
        23
    };
    let legend_width = inner.width.saturating_sub(ring_width as u16 + 3) as usize;
    let center_x = (ring_width.saturating_sub(1) as f64) / 2.0;
    let center_y = (height.saturating_sub(1) as f64) / 2.0;
    let outer = center_y.min(center_x / 1.9).max(2.2);
    let inner_radius = (outer * 0.45).max(1.0);
    let active = (app.tick / 14) as usize % segments.len().max(1);
    let sweep_angle = ((app.tick % 96) as f64) / 96.0;
    let mut lines = Vec::with_capacity(height);

    for y in 0..height {
        let mut spans = Vec::with_capacity(ring_width + legend_width + 3);
        for x in 0..ring_width {
            let dx = (x as f64 - center_x) / 1.9;
            let dy = y as f64 - center_y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > outer || distance < inner_radius {
                spans.push(Span::styled(" ", Style::default().bg(PANEL)));
                continue;
            }

            let angle = normalized_angle(dy.atan2(dx));
            let segment_index = segment_index(angle, &segments);
            let sweep =
                (angle - sweep_angle).abs() < 0.025 || (angle + 1.0 - sweep_angle).abs() < 0.025;
            let mut style = Style::default().fg(segments[segment_index].color).bg(PANEL);
            if segment_index == active || sweep {
                style = style.add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled(if sweep { "▓" } else { "█" }, style));
        }

        spans.push(Span::styled("  ", Style::default().bg(PANEL)));
        if let Some(segment) = segments.get(y) {
            let marker = if y == active { "◆ " } else { "■ " };
            let label_width = legend_width.saturating_sub(10).clamp(5, 18);
            spans.push(Span::styled(
                marker,
                Style::default()
                    .fg(segment.color)
                    .bg(PANEL)
                    .add_modifier(if y == active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ));
            spans.push(Span::styled(
                format!("{:<label_width$}", short_app(&segment.label, label_width)),
                Style::default().fg(TEXT).bg(PANEL),
            ));
            spans.push(Span::styled(
                format!(" {:>4}", percent(segment.share)),
                Style::default().fg(MUTED).bg(PANEL),
            ));
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(PANEL)),
        inner,
    );
}

fn render_rhythm_panel(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let block = panel("RHYTHM", GREEN);
    let inner = block.inner(area);
    frame.render_widget(block.style(Style::default().bg(PANEL)), area);

    let strip_height = if inner.height > 7 { 2 } else { 1 };
    let [chart, strip, metrics] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(strip_height),
            Constraint::Length(2),
        ])
        .split(inner)
    else {
        return;
    };

    render_focus_trend(frame, chart, &app.days, app.tick);
    render_day_strip(frame, strip, &app.days, app.tick);

    let today = app.days.last().map(|day| day.focused_seconds).unwrap_or(0);
    let average = average_focus(&app.days);
    let lines = vec![Line::from(vec![
        Span::styled("today ", Style::default().fg(DIM).bg(PANEL)),
        Span::styled(
            format_duration(today),
            Style::default().fg(YELLOW).bg(PANEL),
        ),
        Span::styled("  avg ", Style::default().fg(DIM).bg(PANEL)),
        Span::styled(
            format_duration(average),
            Style::default().fg(CYAN).bg(PANEL),
        ),
        Span::styled("  peak ", Style::default().fg(DIM).bg(PANEL)),
        Span::styled(
            best_focus_day(&app.days),
            Style::default().fg(TEXT).bg(PANEL),
        ),
    ])];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: true }),
        metrics,
    );
}

fn render_focus_trend(frame: &mut Frame<'_>, area: Rect, days: &[DayTotals], tick: u64) {
    let mut focus_points = days
        .iter()
        .enumerate()
        .map(|(index, day)| (index as f64, day.focused_seconds.max(0) as f64 / 3600.0))
        .collect::<Vec<_>>();

    if let Some(last) = focus_points.last_mut() {
        let shimmer = ((tick % 32) as f64 / 32.0 * PI).sin().max(0.0) * 0.05;
        last.1 += shimmer;
    }

    let max_hours = focus_points
        .iter()
        .map(|(_, hours)| *hours)
        .fold(0.0, f64::max)
        .max(1.0);
    let last_index = days.len().saturating_sub(1).max(1) as f64;
    let first_label = days.first().map(|day| day.label.as_str()).unwrap_or("");
    let last_label = days.last().map(|day| day.label.as_str()).unwrap_or("");
    let mid_label = format!("{:.1}h", max_hours / 2.0);
    let top_label = format!("{max_hours:.1}h");
    let datasets = vec![
        Dataset::default()
            .name("fill")
            .marker(Marker::Braille)
            .graph_type(GraphType::Area)
            .fill_to_y(0.0)
            .style(Style::default().fg(Color::Rgb(22, 44, 55)).bg(PANEL))
            .data(&focus_points),
        Dataset::default()
            .name("focus")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(
                Style::default()
                    .fg(CYAN)
                    .bg(PANEL)
                    .add_modifier(Modifier::BOLD),
            )
            .data(&focus_points),
    ];
    let chart = Chart::new(datasets)
        .style(Style::default().bg(PANEL))
        .x_axis(
            Axis::default()
                .bounds([0.0, last_index])
                .labels([first_label, last_label])
                .style(Style::default().fg(MUTED).bg(PANEL)),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, max_hours])
                .labels(["0h", mid_label.as_str(), top_label.as_str()])
                .style(Style::default().fg(MUTED).bg(PANEL)),
        );
    frame.render_widget(chart, area);
}

fn render_day_strip(frame: &mut Frame<'_>, area: Rect, days: &[DayTotals], tick: u64) {
    let max = days
        .iter()
        .map(|day| day.focused_seconds)
        .max()
        .unwrap_or(0)
        .max(1);
    let mut spans = vec![Span::styled("14d ", Style::default().fg(DIM).bg(PANEL))];

    for (index, day) in days.iter().enumerate() {
        let share = ratio(day.focused_seconds, max);
        let symbol = heat_symbol(share);
        let color = if day.focused_seconds <= 0 {
            DIM
        } else if index == days.len().saturating_sub(1) && tick % 24 < 12 {
            YELLOW
        } else if share > 0.75 {
            GREEN
        } else if share > 0.4 {
            CYAN
        } else {
            BLUE
        };
        spans.push(Span::styled(symbol, Style::default().fg(color).bg(PANEL)));
    }

    let latest = days
        .last()
        .map(|day| format!("  {} {}", day.label, format_duration(day.focused_seconds)))
        .unwrap_or_default();
    spans.push(Span::styled(latest, Style::default().fg(MUTED).bg(PANEL)));
    frame.render_widget(
        Paragraph::new(vec![Line::from(spans)]).style(Style::default().bg(PANEL)),
        area,
    );
}

fn render_main(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [bars, table] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area)
    else {
        return;
    };
    render_app_load(frame, bars, app);
    render_table(frame, table, app);
}

fn render_app_load(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let rows = app.rows();
    let max = rows
        .first()
        .map(|row| row.focused_seconds.max(1))
        .unwrap_or(1);
    let visible = area.height.saturating_sub(3) as usize;
    let width = area.width.saturating_sub(8) as usize;
    let bar_width = width.saturating_sub(20).clamp(8, 46);
    let reveal = ((app.tick.min(18) + 1) as f64 / 19.0).clamp(0.0, 1.0);
    let mut lines = Vec::new();

    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "No app usage recorded for this view.",
            Style::default().fg(MUTED).bg(PANEL),
        )));
    } else {
        for (index, row) in rows.iter().take(visible.max(1)).enumerate() {
            let share = ratio(row.focused_seconds, max);
            let color = rank_color(index);
            let name_width = width.saturating_sub(bar_width + 12).clamp(8, 18);
            let mut spans = vec![
                Span::styled(
                    format!("{:>2} ", index + 1),
                    Style::default().fg(DIM).bg(PANEL),
                ),
                Span::styled(
                    format!("{:<name_width$}", short_app(&row.app_class, name_width)),
                    Style::default().fg(TEXT).bg(PANEL),
                ),
                Span::styled(" ", Style::default().bg(PANEL)),
            ];
            spans.extend(bar_spans(
                share * reveal,
                bar_width,
                color,
                app.tick + index as u64 * 3,
                PANEL,
            ));
            spans.push(Span::styled(
                duration_fixed(row.focused_seconds),
                Style::default().fg(YELLOW).bg(PANEL),
            ));
            lines.push(Line::from(spans));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(panel("PROCESS LOAD", BLUE))
            .style(Style::default().bg(PANEL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_table(frame: &mut Frame<'_>, area: Rect, app: &App) {
    fill_area(frame, area, PANEL);

    let rows = app.rows();
    let focused = focused_total(rows).max(1);
    let visible = area.height.saturating_sub(4) as usize;
    let table_rows = rows
        .iter()
        .take(visible.max(1))
        .enumerate()
        .map(|(index, row)| {
            let color = rank_color(index);
            let density = ratio(row.focused_seconds, row.open_seconds);
            let share = ratio(row.focused_seconds, focused);
            Row::new(vec![
                Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(DIM).bg(PANEL)),
                Cell::from(short_app(&row.app_class, 30))
                    .style(Style::default().fg(color).bg(PANEL)),
                Cell::from(duration_fixed(row.focused_seconds))
                    .style(Style::default().fg(YELLOW).bg(PANEL)),
                Cell::from(duration_fixed(row.open_seconds))
                    .style(Style::default().fg(BLUE).bg(PANEL)),
                Cell::from(percent(density)).style(Style::default().fg(MAGENTA).bg(PANEL)),
                Cell::from(micro_bar(share, 8)).style(Style::default().fg(color).bg(PANEL)),
            ])
        });

    frame.render_widget(
        Table::new(
            table_rows,
            [
                Constraint::Length(3),
                Constraint::Min(14),
                Constraint::Length(9),
                Constraint::Length(9),
                Constraint::Length(6),
                Constraint::Length(8),
            ],
        )
        .header(
            Row::new(["#", "App", "Focus", "Open", "Dense", "Share"]).style(
                Style::default()
                    .fg(TEXT)
                    .bg(PANEL_ALT)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(panel("LEADERBOARD", YELLOW))
        .style(Style::default().bg(PANEL))
        .column_spacing(1),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let left = format!(
        "  ←/h  →/l   1-4 views   r refresh   q quit   {}",
        SPINNER[(app.tick as usize / 3) % SPINNER.len()]
    );
    let right = format!("auto-refresh {}s", AUTO_REFRESH.as_secs());
    let padding =
        area.width
            .saturating_sub((left.chars().count() + right.chars().count()) as u16) as usize;
    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(MUTED).bg(BG)),
        Span::styled(" ".repeat(padding), Style::default().bg(BG)),
        Span::styled(right, Style::default().fg(DIM).bg(BG)),
    ]);
    frame.render_widget(
        Paragraph::new(vec![line])
            .style(Style::default().bg(BG))
            .alignment(Alignment::Left),
        area,
    );
}

fn panel(title: &'static str, accent: Color) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent).bg(PANEL))
        .style(Style::default().bg(PANEL))
}

fn fill_area(frame: &mut Frame<'_>, area: Rect, color: Color) {
    let line = " ".repeat(area.width as usize);
    let lines = (0..area.height)
        .map(|_| Line::from(Span::styled(line.clone(), Style::default().bg(color))))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(color)),
        area,
    );
}

fn metric_span(label: &str, value: &str, color: Color) -> Span<'static> {
    Span::styled(
        format!(" {label} {value} "),
        Style::default()
            .fg(color)
            .bg(PANEL_ALT)
            .add_modifier(Modifier::BOLD),
    )
}

fn meter_line(label: &str, value: f64, width: usize, color: Color, tick: u64) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{label:<8} "),
        Style::default().fg(MUTED).bg(PANEL),
    )];
    spans.extend(bar_spans(value, width, color, tick, PANEL));
    spans.push(Span::styled(
        format!(" {:>4}", percent(value)),
        Style::default().fg(TEXT).bg(PANEL),
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

fn micro_bar(value: f64, width: usize) -> String {
    let filled = ((value.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    format!(
        "{}{}",
        "▰".repeat(filled),
        "▱".repeat(width.saturating_sub(filled))
    )
}

#[derive(Debug)]
struct PieSegment {
    label: String,
    share: f64,
    cumulative_share: f64,
    color: Color,
}

fn pie_segments(rows: &[AppTotals], total: i64) -> Vec<PieSegment> {
    let mut segments = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(5)
        .enumerate()
        .scan(0.0, |cumulative, (index, row)| {
            let share = ratio(row.focused_seconds, total);
            *cumulative += share;
            Some(PieSegment {
                label: row.app_class.clone(),
                share,
                cumulative_share: *cumulative,
                color: PIE_COLORS[index],
            })
        })
        .collect::<Vec<_>>();

    let represented = segments
        .last()
        .map(|segment| segment.cumulative_share)
        .unwrap_or(0.0);
    if represented < 0.995 {
        segments.push(PieSegment {
            label: "Other".to_string(),
            share: 1.0 - represented,
            cumulative_share: 1.0,
            color: PIE_COLORS[5],
        });
    } else if let Some(last) = segments.last_mut() {
        last.cumulative_share = 1.0;
    }

    segments
}

fn normalized_angle(angle: f64) -> f64 {
    ((angle + PI * 2.5) % (PI * 2.0)) / (PI * 2.0)
}

fn segment_index(angle_share: f64, segments: &[PieSegment]) -> usize {
    segments
        .iter()
        .position(|segment| angle_share <= segment.cumulative_share)
        .unwrap_or_else(|| segments.len().saturating_sub(1))
}

fn focused_total(rows: &[AppTotals]) -> i64 {
    rows.iter().map(|row| row.focused_seconds).sum()
}

fn open_total(rows: &[AppTotals]) -> i64 {
    rows.iter().map(|row| row.open_seconds).sum()
}

fn average_focus(days: &[DayTotals]) -> i64 {
    if days.is_empty() {
        return 0;
    }
    days.iter().map(|day| day.focused_seconds).sum::<i64>() / days.len() as i64
}

fn trend_ratio(days: &[DayTotals]) -> f64 {
    let today = days.last().map(|day| day.focused_seconds).unwrap_or(0);
    let peak = days
        .iter()
        .map(|day| day.focused_seconds)
        .max()
        .unwrap_or(0)
        .max(1);
    ratio(today, peak)
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        (numerator.max(0) as f64 / denominator as f64).clamp(0.0, 1.0)
    }
}

fn pulse_color(tick: u64) -> Color {
    match tick % 36 {
        0..=8 => CYAN,
        9..=17 => GREEN,
        18..=26 => YELLOW,
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
        _ => MUTED,
    }
}

fn activity_rail(tick: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let slots = (width / 5).max(1);
    let position = (tick as usize / 2) % slots;
    (0..width)
        .map(|index| {
            let slot = index / 5;
            if slot == position && index % 5 < 3 {
                '━'
            } else if index % 8 == 3 {
                '╴'
            } else if index % 5 == 1 {
                '·'
            } else {
                ' '
            }
        })
        .collect()
}

fn segmented_rail(tick: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    (0..width)
        .map(|index| {
            let phase = (index + tick as usize) % 29;
            match phase {
                0..=2 => '━',
                8 | 16 | 24 => '╸',
                _ => '─',
            }
        })
        .collect()
}

fn heat_symbol(share: f64) -> &'static str {
    if share <= 0.0 {
        "░"
    } else if share < 0.2 {
        "▁"
    } else if share < 0.4 {
        "▃"
    } else if share < 0.6 {
        "▅"
    } else if share < 0.8 {
        "▆"
    } else {
        "█"
    }
}

fn best_focus_day(days: &[DayTotals]) -> String {
    days.iter()
        .max_by_key(|day| day.focused_seconds)
        .filter(|day| day.focused_seconds > 0)
        .map(|day| format!("{} {}", day.label, format_duration(day.focused_seconds)))
        .unwrap_or_else(|| "none".to_string())
}

fn short_app(value: &str, width: usize) -> String {
    let mut value = value
        .trim_start_matches("com.")
        .trim_start_matches("org.")
        .to_string();
    if value.contains('.') {
        value = value.rsplit('.').next().unwrap_or(&value).to_string();
    }
    if value.chars().count() <= width {
        return value;
    }

    let mut out = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    out.push('.');
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

fn duration_fixed(seconds: i64) -> String {
    format!("{:>9}", format_duration(seconds))
}
