use crate::{
    steam::SteamResolver,
    storage::{AppTotals, DayTotals, Storage},
};
use anyhow::Result;
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
        Axis, Block, BorderType, Borders, Cell, Chart, Dataset, Gauge, GraphType, Paragraph, Row,
        Table, Tabs, Wrap,
    },
};
use std::{
    f64::consts::PI,
    io,
    time::{Duration, Instant},
};

const TABS: [&str; 4] = ["Today", "Week", "Replay", "All Time"];
const FRAME_TIME: Duration = Duration::from_millis(83);
const AUTO_REFRESH: Duration = Duration::from_secs(5);
const PIE_COLORS: [Color; 6] = [
    Color::Yellow,
    Color::Cyan,
    Color::Magenta,
    Color::LightGreen,
    Color::LightBlue,
    Color::Gray,
];

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
    if area.width < 72 || area.height < 24 {
        render_compact(frame, area);
        return;
    }

    let [header, tabs, hero, main, footer] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Length(11),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(area)
    else {
        return;
    };

    render_header(frame, header, app);
    render_tabs(frame, tabs, app.tab);
    render_hero(frame, hero, app);
    render_main(frame, main, app);
    render_footer(frame, footer);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new("Omastat needs at least 72x24 for the dashboard.")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Omastat")),
        area,
    );
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let pulse = pulse_color(app.tick);
    let rows = app.rows();
    let focused = focused_total(rows);
    let top = rows
        .first()
        .map(|row| short_app(&row.app_class, 32))
        .unwrap_or_else(|| "No app usage yet".to_string());
    let sweep = activity_rail(app.tick, area.width.saturating_sub(2) as usize);

    let lines = vec![
        Line::from(vec![
            Span::styled(
                "OMASTAT",
                Style::default().fg(pulse).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                app.selected_label().to_uppercase(),
                Style::default().fg(Color::Gray),
            ),
            Span::raw("  "),
            Span::styled(format_duration(focused), Style::default().fg(Color::Yellow)),
            Span::raw(" focused  "),
            Span::styled(top, Style::default().fg(Color::White)),
        ]),
        Line::from(Span::styled(sweep, Style::default().fg(Color::DarkGray))),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::BOTTOM))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, tab: usize) {
    let titles = TABS
        .iter()
        .enumerate()
        .map(|(index, title)| {
            Line::from(Span::styled(
                format!(" {} {} ", index + 1, title),
                Style::default().add_modifier(Modifier::BOLD),
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(titles)
            .select(tab)
            .style(Style::default().fg(Color::DarkGray))
            .highlight_style(Style::default().fg(Color::Cyan))
            .divider(""),
        area,
    );
}

fn render_hero(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [left, middle, right] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(31),
            Constraint::Percentage(34),
            Constraint::Percentage(35),
        ])
        .split(area)
    else {
        return;
    };
    render_replay_card(frame, left, app);
    render_focus_pie(frame, middle, app.rows(), app.tick);
    render_trend_card(frame, right, app);
}

fn render_replay_card(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let ratio = ratio(focused, open);
    let month_focused = focused_total(&app.month);
    let year_focused = focused_total(&app.year);
    let best_day = best_focus_day(&app.days);
    let active_days = app
        .days
        .iter()
        .filter(|day| day.focused_seconds > 0)
        .count();
    let top = rows
        .first()
        .map(|row| short_app(&row.app_class, 24))
        .unwrap_or_else(|| "No app yet".to_string());

    let lines = vec![
        Line::from(Span::styled(
            match app.tab {
                2 => "Year in progress",
                _ => "Session recap",
            },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(format_duration(focused), Style::default().fg(Color::Yellow)),
            Span::raw(" focused across "),
            Span::styled(rows.len().to_string(), Style::default().fg(Color::White)),
            Span::raw(" apps"),
        ]),
        Line::from(vec![
            Span::raw("Top app "),
            Span::styled(top, Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("Focus density "),
            Span::styled(
                format!("{:.0}%", ratio * 100.0),
                Style::default().fg(Color::Magenta),
            ),
        ]),
        Line::from(vec![
            Span::raw("Month "),
            Span::styled(
                format_duration(month_focused),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  Year "),
            Span::styled(
                format_duration(year_focused),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("Best "),
            Span::styled(best_day, Style::default().fg(Color::White)),
            Span::raw("  Days "),
            Span::styled(
                format!("{active_days}/14"),
                Style::default().fg(Color::LightGreen),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(card("Replay"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_focus_pie(frame: &mut Frame<'_>, area: Rect, rows: &[AppTotals], tick: u64) {
    let total = focused_total(rows).max(0);
    if total == 0 {
        frame.render_widget(
            Paragraph::new("No focused time recorded for this view.")
                .alignment(Alignment::Center)
                .block(card("Focus mix"))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let segments = pie_segments(rows, total);
    let height = area.height.saturating_sub(2).min(9) as usize;
    let inner_width = area.width.saturating_sub(2) as usize;
    let width = if inner_width < 42 {
        inner_width.saturating_sub(19).clamp(11, 21)
    } else {
        21
    };
    let legend_width = inner_width.saturating_sub(width + 2);
    let center_x = (width.saturating_sub(1) as f64) / 2.0;
    let center_y = (height.saturating_sub(1) as f64) / 2.0;
    let outer = center_y.min(center_x / 1.85).max(2.5);
    let inner = (outer * 0.46).max(1.2);
    let active = (tick / 18) as usize % segments.len().max(1);

    let mut lines = Vec::with_capacity(height);
    for y in 0..height {
        let mut spans = Vec::with_capacity(width + 3);
        for x in 0..width {
            let dx = (x as f64 - center_x) / 1.85;
            let dy = y as f64 - center_y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > outer || distance < inner {
                spans.push(Span::raw(" "));
                continue;
            }

            let angle = normalized_angle(dy.atan2(dx));
            let segment_index = segment_index(angle, &segments);
            let mut style = Style::default().fg(segments[segment_index].color);
            if segment_index == active {
                style = style.add_modifier(Modifier::BOLD);
            }
            spans.push(Span::styled("█", style));
        }

        if let Some(segment) = segments.get(y) {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                "■ ",
                Style::default()
                    .fg(segment.color)
                    .add_modifier(if y == active {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ));
            let label_width = legend_width.saturating_sub(7).clamp(6, 12);
            spans.push(Span::styled(
                format!(
                    "{:<label_width$} {:>3.0}%",
                    short_app(&segment.label, label_width),
                    segment.share * 100.0
                ),
                Style::default().fg(Color::Gray),
            ));
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines).block(card("Focus mix")), area);
}

fn render_trend_card(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = card("Rhythm");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [chart, strip, gauges] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(3),
        ])
        .split(inner)
    else {
        return;
    };

    render_focus_trend(frame, chart, &app.days);
    render_day_strip(frame, strip, &app.days);

    let [focus_area, open_area] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(gauges)
    else {
        return;
    };
    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let density = ratio(focused, open);
    let top_share = rows
        .first()
        .map(|row| ratio(row.focused_seconds, focused))
        .unwrap_or(0.0);

    render_gauge(frame, focus_area, "focus density", density, Color::Yellow);
    render_gauge(frame, open_area, "top app share", top_share, Color::Magenta);
}

fn render_gauge(frame: &mut Frame<'_>, area: Rect, label: &'static str, value: f64, color: Color) {
    let [label_area, gauge_area] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area)
    else {
        return;
    };
    frame.render_widget(
        Paragraph::new(label).style(Style::default().fg(Color::DarkGray)),
        label_area,
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .ratio(value.clamp(0.0, 1.0)),
        gauge_area,
    );
}

fn render_focus_trend(frame: &mut Frame<'_>, area: Rect, days: &[DayTotals]) {
    let focus_points = days
        .iter()
        .enumerate()
        .map(|(index, day)| (index as f64, day.focused_seconds.max(0) as f64 / 3600.0))
        .collect::<Vec<_>>();
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
            .name("focus area")
            .marker(Marker::Braille)
            .graph_type(GraphType::Area)
            .fill_to_y(0.0)
            .style(Style::default().fg(Color::DarkGray))
            .data(&focus_points),
        Dataset::default()
            .name("focus")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .data(&focus_points),
    ];
    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .bounds([0.0, last_index])
                .labels([first_label, last_label]),
        )
        .y_axis(Axis::default().bounds([0.0, max_hours]).labels([
            "0h",
            mid_label.as_str(),
            top_label.as_str(),
        ]));
    frame.render_widget(chart, area);
}

fn render_day_strip(frame: &mut Frame<'_>, area: Rect, days: &[DayTotals]) {
    let max = days
        .iter()
        .map(|day| day.focused_seconds)
        .max()
        .unwrap_or(0)
        .max(1);
    let blocks = days
        .iter()
        .map(|day| {
            let share = ratio(day.focused_seconds, max);
            let symbol = if share <= 0.0 {
                "░"
            } else if share < 0.25 {
                "▂"
            } else if share < 0.5 {
                "▄"
            } else if share < 0.75 {
                "▆"
            } else {
                "█"
            };
            Span::styled(
                symbol,
                Style::default().fg(if day.focused_seconds > 0 {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            )
        })
        .collect::<Vec<_>>();
    let latest = days
        .last()
        .map(|day| format!("  {} {}", day.label, format_duration(day.focused_seconds)))
        .unwrap_or_default();

    let mut spans = vec![Span::styled("14d ", Style::default().fg(Color::DarkGray))];
    spans.extend(blocks);
    spans.push(Span::styled(latest, Style::default().fg(Color::Gray)));

    frame.render_widget(Paragraph::new(vec![Line::from(spans)]), area);
}

fn render_main(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [bars, table] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area)
    else {
        return;
    };
    render_chart(frame, bars, app.rows(), app.tick);
    render_table(frame, table, app.rows());
}

fn render_chart(frame: &mut Frame<'_>, area: Rect, rows: &[AppTotals], tick: u64) {
    let reveal = ((tick.min(12) + 1) as f64 / 13.0).clamp(0.0, 1.0);
    let max = rows
        .first()
        .map(|row| row.focused_seconds.max(1))
        .unwrap_or(1);
    let inner_width = area.width.saturating_sub(4) as usize;
    let bar_width = inner_width.saturating_sub(24).clamp(10, 42);
    let lines = rows
        .iter()
        .take(10)
        .map(|row| {
            let share = ratio(row.focused_seconds, max);
            let filled = ((share * bar_width as f64) * reveal).round() as usize;
            let empty = bar_width.saturating_sub(filled);
            Line::from(vec![
                Span::styled(
                    format!("{:<12}", short_app(&row.app_class, 12)),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled("█".repeat(filled), Style::default().fg(Color::Cyan)),
                Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(
                    duration_fixed(row.focused_seconds),
                    Style::default().fg(Color::Yellow),
                ),
            ])
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        Paragraph::new(lines)
            .block(card("Top focus"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_table(frame: &mut Frame<'_>, area: Rect, rows: &[AppTotals]) {
    let table_rows = rows.iter().take(14).enumerate().map(|(index, row)| {
        let color = match index {
            0 => Color::Yellow,
            1 | 2 => Color::Cyan,
            _ => Color::Gray,
        };
        Row::new(vec![
            Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(Color::DarkGray)),
            Cell::from(short_app(&row.app_class, 28)).style(Style::default().fg(color)),
            Cell::from(duration_fixed(row.focused_seconds)),
            Cell::from(duration_fixed(row.open_seconds)),
            Cell::from(format!(
                "{:>6.0}%",
                ratio(row.focused_seconds, row.open_seconds) * 100.0
            )),
        ])
    });

    frame.render_widget(
        Table::new(
            table_rows,
            [
                Constraint::Length(3),
                Constraint::Min(14),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(8),
            ],
        )
        .header(
            Row::new(["#", "App", "Focus", "Open", "Density"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(card("Leaderboard"))
        .column_spacing(1),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new("h/l or Left/Right switch views   1-4 jump   r refresh   q quit")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        area,
    );
}

fn card(title: &'static str) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
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

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        (numerator.max(0) as f64 / denominator as f64).clamp(0.0, 1.0)
    }
}

fn pulse_color(tick: u64) -> Color {
    match tick % 24 {
        0..=5 => Color::Cyan,
        6..=11 => Color::LightCyan,
        12..=17 => Color::Yellow,
        _ => Color::LightCyan,
    }
}

fn activity_rail(tick: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let slots = (width / 4).max(1);
    let position = (tick as usize / 2) % slots;
    (0..width)
        .map(|index| {
            let slot = index / 4;
            if slot == position && index % 4 != 3 {
                '━'
            } else if index % 4 == 1 {
                '·'
            } else {
                ' '
            }
        })
        .collect()
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
