use crate::storage::{AppTotals, Storage};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap},
};
use std::{io, time::Duration};

const TABS: [&str; 3] = ["Today", "Week", "All Time"];

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

        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('r') => app = App::load(storage)?,
                    KeyCode::Left | KeyCode::Char('h') => app.previous_tab(),
                    KeyCode::Right | KeyCode::Char('l') => app.next_tab(),
                    KeyCode::Char('1') => app.tab = 0,
                    KeyCode::Char('2') => app.tab = 1,
                    KeyCode::Char('3') => app.tab = 2,
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
    today: Vec<AppTotals>,
    week: Vec<AppTotals>,
    all_time: Vec<AppTotals>,
}

impl App {
    fn load(storage: &Storage) -> Result<Self> {
        Ok(Self {
            tab: 0,
            today: storage.totals_for_today()?,
            week: storage.totals_for_week()?,
            all_time: storage.totals_all_time()?,
        })
    }

    fn rows(&self) -> &[AppTotals] {
        match self.tab {
            0 => &self.today,
            1 => &self.week,
            _ => &self.all_time,
        }
    }

    fn previous_tab(&mut self) {
        self.tab = if self.tab == 0 {
            TABS.len() - 1
        } else {
            self.tab - 1
        };
    }

    fn next_tab(&mut self) {
        self.tab = (self.tab + 1) % TABS.len();
    }
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    render_header(frame, chunks[0], app.rows());
    render_tabs(frame, chunks[1], app.tab);
    render_summary(frame, chunks[2], app.rows());
    render_body(frame, chunks[3], app.rows());
    render_footer(frame, chunks[4]);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, rows: &[AppTotals]) {
    let focused = rows.iter().map(|row| row.focused_seconds).sum::<i64>();
    let open = rows.iter().map(|row| row.open_seconds).sum::<i64>();
    let top = rows
        .first()
        .map(|row| row.app_class.as_str())
        .unwrap_or("No tracked app yet");

    let title = vec![
        Line::from(vec![
            Span::styled(
                "OMASTAT",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("focused app time", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(format_duration(focused), Style::default().fg(Color::Yellow)),
            Span::raw(" focused  /  "),
            Span::styled(format_duration(open), Style::default().fg(Color::Magenta)),
            Span::raw(" open  /  top: "),
            Span::styled(top, Style::default().fg(Color::White)),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(title)
            .block(Block::default().borders(Borders::BOTTOM))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, tab: usize) {
    let titles = TABS
        .iter()
        .map(|title| Line::from(Span::raw(*title)))
        .collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(titles)
            .select(tab)
            .style(Style::default().fg(Color::Gray))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(" / "),
        area,
    );
}

fn render_summary(frame: &mut Frame<'_>, area: Rect, rows: &[AppTotals]) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(area);

    let focused = rows.iter().map(|row| row.focused_seconds).sum::<i64>();
    let open = rows.iter().map(|row| row.open_seconds).sum::<i64>();
    let apps = rows.len();

    stat_card(
        frame,
        chunks[0],
        "Focused",
        &format_duration(focused),
        Color::Yellow,
    );
    stat_card(
        frame,
        chunks[1],
        "Open",
        &format_duration(open),
        Color::Magenta,
    );
    stat_card(frame, chunks[2], "Apps", &apps.to_string(), Color::Cyan);
}

fn stat_card(frame: &mut Frame<'_>, area: Rect, label: &str, value: &str, color: Color) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(label, Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled(
                value,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
        ])
        .block(Block::default().borders(Borders::ALL)),
        area,
    );
}

fn render_body(frame: &mut Frame<'_>, area: Rect, rows: &[AppTotals]) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);
    render_chart(frame, chunks[0], rows);
    render_table(frame, chunks[1], rows);
}

fn render_chart(frame: &mut Frame<'_>, area: Rect, rows: &[AppTotals]) {
    let bars = rows
        .iter()
        .take(8)
        .map(|row| {
            Bar::default()
                .label(Line::from(short_app(&row.app_class, 10)))
                .value(row.focused_seconds.max(0) as u64)
                .text_value(format_duration(row.focused_seconds))
                .style(Style::default().fg(Color::Cyan))
                .value_style(Style::default().fg(Color::Yellow))
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        BarChart::default()
            .block(Block::default().title("Top Focus").borders(Borders::ALL))
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
    let table_rows = rows.iter().take(14).map(|row| {
        Row::new(vec![
            Cell::from(short_app(&row.app_class, 28)),
            Cell::from(format_duration(row.focused_seconds)),
            Cell::from(format_duration(row.open_seconds)),
        ])
    });

    frame.render_widget(
        Table::new(
            table_rows,
            [
                Constraint::Min(14),
                Constraint::Length(12),
                Constraint::Length(12),
            ],
        )
        .header(
            Row::new(["App", "Focused", "Open"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(Block::default().title("Leaderboard").borders(Borders::ALL))
        .column_spacing(2),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new("Left/Right or 1-3 switch views   r refresh   q quit")
            .style(Style::default().fg(Color::DarkGray)),
        area,
    );
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
