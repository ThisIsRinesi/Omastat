use crate::storage::{AppTotals, DayTotals, Storage};
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
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, BorderType, Borders, Cell, Gauge, Paragraph, Row,
        Sparkline, Table, Tabs, Wrap,
    },
};
use std::{
    io,
    time::{Duration, Instant},
};

const TABS: [&str; 3] = ["Today", "Week", "All Time"];
const FRAME_TIME: Duration = Duration::from_millis(83);
const AUTO_REFRESH: Duration = Duration::from_secs(5);

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
    all_time: Vec<AppTotals>,
    days: Vec<DayTotals>,
    total_observed_seconds: i64,
}

impl App {
    fn load(storage: &Storage) -> Result<Self> {
        Ok(Self {
            tab: 0,
            tick: 0,
            last_refresh: Instant::now(),
            today: storage.totals_for_today()?,
            week: storage.totals_for_week()?,
            all_time: storage.totals_all_time()?,
            days: storage.daily_totals(14)?,
            total_observed_seconds: storage.total_duration()?,
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
            Constraint::Length(8),
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
        .map(|row| row.app_class.as_str())
        .unwrap_or("No app usage yet");
    let sweep = scanline(app.tick, area.width.saturating_sub(2) as usize);

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
    let [left, right] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(46), Constraint::Percentage(54)])
        .split(area)
    else {
        return;
    };
    render_replay_card(frame, left, app);
    render_trend_card(frame, right, app);
}

fn render_replay_card(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let ratio = ratio(focused, open);
    let top = rows
        .first()
        .map(|row| short_app(&row.app_class, 24))
        .unwrap_or_else(|| "No app yet".to_string());

    let lines = vec![
        Line::from(Span::styled(
            "Session recap",
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
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(card("Replay"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_trend_card(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let [spark, gauges] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(3)])
        .split(area)
    else {
        return;
    };

    let data = app
        .days
        .iter()
        .map(|day| day.focused_seconds.max(0) as u64)
        .collect::<Vec<_>>();
    let max = data.iter().copied().max().unwrap_or(1).max(1);
    frame.render_widget(
        Sparkline::default()
            .block(card("14 day focus"))
            .data(&data)
            .max(max)
            .style(Style::default().fg(Color::Cyan)),
        spark,
    );

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
    let total_observed = app.total_observed_seconds.max(1);

    render_gauge(
        frame,
        focus_area,
        "focused share",
        ratio(focused, total_observed),
        Color::Yellow,
    );
    render_gauge(
        frame,
        open_area,
        "open share",
        ratio(open, total_observed),
        Color::Magenta,
    );
}

fn render_gauge(frame: &mut Frame<'_>, area: Rect, label: &'static str, value: f64, color: Color) {
    frame.render_widget(
        Gauge::default()
            .block(card(label))
            .gauge_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .ratio(value.clamp(0.0, 1.0)),
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
    render_chart(frame, bars, app.rows(), app.tick);
    render_table(frame, table, app.rows());
}

fn render_chart(frame: &mut Frame<'_>, area: Rect, rows: &[AppTotals], tick: u64) {
    let reveal = ((tick.min(12) + 1) as f64 / 13.0).clamp(0.0, 1.0);
    let bars = rows
        .iter()
        .take(8)
        .map(|row| {
            let value = ((row.focused_seconds.max(0) as f64) * reveal).round() as u64;
            Bar::default()
                .label(Line::from(short_app(&row.app_class, 10)))
                .value(value)
                .text_value(format_duration(row.focused_seconds))
                .style(Style::default().fg(Color::Cyan))
                .value_style(Style::default().fg(Color::Yellow))
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        BarChart::default()
            .block(card("Top focus"))
            .bar_width(8)
            .bar_gap(2)
            .group_gap(1)
            .data(BarGroup::default().bars(&bars))
            .max(
                rows.first()
                    .map(|row| row.focused_seconds.max(1) as u64)
                    .unwrap_or(1),
            ),
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
            Cell::from(format_duration(row.focused_seconds)),
            Cell::from(format_duration(row.open_seconds)),
            Cell::from(format!(
                "{:.0}%",
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
                Constraint::Length(7),
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
        Paragraph::new("h/l or Left/Right switch views   1-3 jump   r refresh   q quit")
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

fn scanline(tick: u64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let position = (tick as usize) % width;
    (0..width)
        .map(|index| {
            if index == position {
                '>'
            } else if index + 1 == position || index == position + 1 {
                '-'
            } else {
                '.'
            }
        })
        .collect()
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
