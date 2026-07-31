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
use serde_json::Value as JsonValue;
use std::{
    f64::consts::PI,
    fs, io,
    time::{Duration, Instant},
};
use toml::Value as TomlValue;

const LENSES: [&str; 5] = ["DAY", "WEEK", "MONTH", "YEAR", "LIFE"];
const CLOCK_REFRESH: Duration = Duration::from_secs(1);
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
    let mut next_clock = Instant::now() + CLOCK_REFRESH;

    loop {
        terminal.draw(|frame| render(frame, &app))?;

        let refresh_deadline = app.last_refresh + AUTO_REFRESH;
        let deadline = if refresh_deadline <= next_clock {
            refresh_deadline
        } else {
            next_clock
        };

        if event::poll(deadline.saturating_duration_since(Instant::now()))? {
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

#[derive(Debug)]
struct App {
    lens: usize,
    selected: usize,
    last_refresh: Instant,
    loaded_at: DateTime<Local>,
    theme: Theme,
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
            last_refresh: Instant::now(),
            loaded_at: Local::now(),
            theme: Theme::load(),
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
        *self = Self::load(storage)?;
        self.lens = lens;
        self.selected = selected;
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
    let theme = &app.theme;
    fill_area(frame, area, theme.bg);

    if area.width < 52 || area.height < 16 {
        render_tiny(frame, area, app, theme);
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

    render_header(frame, header, app, theme);
    if area.width < 82 {
        render_compact(frame, body, app, theme);
    } else {
        render_dashboard(frame, body, app, theme);
    }
    render_footer(frame, footer, app, theme);
}

fn render_tiny(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let lines = vec![
        Line::from(vec![
            Span::styled("omastat ", Style::default().fg(theme.primary)),
            Span::styled(
                app.lens_label(),
                Style::default().fg(lens_color(app.lens, theme)),
            ),
        ]),
        Line::from(Span::styled(
            "terminal too small",
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled("q quits", Style::default().fg(theme.dim))),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(panel("MONITOR", theme, theme.primary))
            .style(Style::default().bg(theme.panel)),
        area,
    );
}

fn render_dashboard(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.bg);

    let top_height = if area.height < 25 {
        8
    } else {
        (area.height / 3).clamp(9, 12)
    };
    let [top, bottom] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(top_height), Constraint::Min(10)])
        .split(area)
    else {
        return;
    };
    let [flow, summary] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(67), Constraint::Percentage(33)])
        .split(top)
    else {
        return;
    };
    let [apps, side] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
        .split(bottom)
    else {
        return;
    };

    render_flow(frame, flow, app, theme);
    render_replay(frame, summary, app, theme);
    render_apps(frame, apps, app, theme);
    render_side(frame, side, app, theme);
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let show_bottom_strip = area.height >= 20;
    let chunks = if show_bottom_strip {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),
                Constraint::Min(8),
                Constraint::Length(6),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(8)])
            .split(area)
    };
    let top = chunks[0];
    let apps = chunks[1];

    let [flow, replay] = *Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(top)
    else {
        return;
    };

    render_flow(frame, flow, app, theme);
    render_replay(frame, replay, app, theme);
    render_apps(frame, apps, app, theme);

    if show_bottom_strip {
        let [mix, lenses] = *Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(chunks[2])
        else {
            return;
        };
        render_mix(frame, mix, app, theme);
        render_lenses(frame, lenses, app, theme);
    }
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.bg);

    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let density = ratio(focused, open);
    let clock = Local::now().format("%H:%M:%S").to_string();
    let mut spans = vec![
        Span::styled(
            " OMASTAT",
            Style::default()
                .fg(theme.primary)
                .bg(theme.bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" :: ", Style::default().fg(theme.dim).bg(theme.bg)),
    ];

    if area.width < 96 {
        spans.extend([
            Span::styled(" lens ", Style::default().fg(theme.dim).bg(theme.bg)),
            Span::styled(
                format!(" {} ", app.lens_label()),
                Style::default()
                    .fg(theme.bg)
                    .bg(lens_color(app.lens, theme))
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
    } else {
        for (index, label) in LENSES.iter().enumerate() {
            let selected = index == app.lens;
            spans.push(Span::styled(
                format!(" {label} "),
                Style::default()
                    .fg(if selected {
                        theme.bg
                    } else {
                        lens_color(index, theme)
                    })
                    .bg(if selected {
                        lens_color(index, theme)
                    } else {
                        theme.bg
                    })
                    .add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ));
        }
    }

    spans.extend([
        Span::styled("  focus ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(
            format_duration(focused),
            Style::default().fg(theme.warn).bg(theme.bg),
        ),
        Span::styled("  open ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(
            format_duration(open),
            Style::default().fg(theme.secondary).bg(theme.bg),
        ),
        Span::styled("  density ", Style::default().fg(theme.dim).bg(theme.bg)),
        Span::styled(
            percent(density),
            Style::default().fg(theme.tertiary).bg(theme.bg),
        ),
    ]);

    if area.width >= 96 {
        spans.extend([
            Span::styled("  updated ", Style::default().fg(theme.dim).bg(theme.bg)),
            Span::styled(
                app.loaded_at.format("%H:%M:%S").to_string(),
                Style::default().fg(theme.muted).bg(theme.bg),
            ),
            Span::styled("  now ", Style::default().fg(theme.dim).bg(theme.bg)),
            Span::styled(clock, Style::default().fg(theme.text).bg(theme.bg)),
        ]);
    } else {
        spans.extend([
            Span::styled("  ", Style::default().bg(theme.bg)),
            Span::styled(clock, Style::default().fg(theme.text).bg(theme.bg)),
        ]);
    }

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(spans),
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

    let mode = format!("{} {}", app.lens + 1, app.lens_label());
    let left = format!(" h/l lens  j/k select  pg jump  r refresh  q quit  [{mode}]");
    let right = format!("{}s refresh", AUTO_REFRESH.as_secs());
    let padding =
        area.width
            .saturating_sub((left.chars().count() + right.chars().count()) as u16) as usize;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, Style::default().fg(theme.muted).bg(theme.bg)),
            Span::styled(" ".repeat(padding), Style::default().bg(theme.bg)),
            Span::styled(right, Style::default().fg(theme.dim).bg(theme.bg)),
        ]))
        .style(Style::default().bg(theme.bg)),
        area,
    );
}

fn render_flow(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.panel);

    let block = panel("timeline", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let graph_height = inner.height.saturating_sub(2).max(1) as usize;
    let mut lines = day_graph(&app.days, inner.width as usize, graph_height, theme);
    lines.push(day_footer(&app.days, inner.width as usize, theme));
    lines.push(Line::from(Span::styled(
        rule(inner.width as usize),
        Style::default().fg(theme.dim).bg(theme.panel),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_replay(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.panel);

    let block = panel("replay", theme, theme.secondary);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = app.rows();
    let focused = focused_total(rows);
    let open = open_total(rows);
    let top = rows
        .first()
        .map(|row| short_app(&row.app_class, inner.width.saturating_sub(12) as usize))
        .unwrap_or_else(|| "no signal".to_string());
    let best = best_focus_day(&app.days)
        .map(|day| format!("{} {}", day.label, duration_compact(day.focused_seconds)))
        .unwrap_or_else(|| "no focus".to_string());
    let meter_width = inner.width.saturating_sub(13).max(4) as usize;

    let lines = vec![
        Line::from(Span::styled(
            format_duration(focused),
            Style::default()
                .fg(theme.warn)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        )),
        meter_line(
            "focus",
            ratio(focused, open.max(focused)),
            meter_width,
            theme.warn,
            theme,
        ),
        meter_line(
            "dense",
            ratio(focused, open),
            meter_width,
            theme.tertiary,
            theme,
        ),
        metric_line("open", &format_duration(open), theme.secondary, theme),
        metric_line("top", top.trim(), theme.primary, theme),
        metric_line("best", &best, theme.success, theme),
        metric_line(
            "active",
            &format!("{}/{} days", active_days(&app.days), app.days.len().max(1)),
            theme.tertiary,
            theme,
        ),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_apps(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.panel);

    let block = panel("apps", theme, theme.warn);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = app.rows();
    if rows.is_empty() {
        let mut lines = vec![Line::from(Span::styled(
            "waiting for app intervals",
            Style::default().fg(theme.muted).bg(theme.panel),
        ))];
        lines.extend(texture_lines(
            inner.width as usize,
            inner.height.saturating_sub(1) as usize,
            theme,
        ));
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(theme.panel))
                .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }

    let width = inner.width as usize;
    let total = focused_total(rows).max(1);
    let max_focus = rows
        .first()
        .map(|row| row.focused_seconds.max(1))
        .unwrap_or(1);
    let expanded = inner.height >= 14 && width >= 58;
    let mut lines = vec![apps_header(width, theme)];

    for (index, row) in rows.iter().enumerate() {
        if lines.len() >= inner.height as usize {
            break;
        }

        let selected = index == app.selected;
        lines.push(app_main_line(
            index, row, selected, max_focus, total, width, theme,
        ));
        if expanded && lines.len() < inner.height as usize {
            lines.push(app_detail_line(row, selected, max_focus, width, theme));
        }
    }

    if lines.len() < inner.height as usize {
        lines.extend(texture_lines(
            width,
            inner.height as usize - lines.len(),
            theme,
        ));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_side(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    if area.width < 20 || area.height < 8 {
        return;
    }

    if area.height < 16 {
        let [mix, lenses] = *Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
            .split(area)
        else {
            return;
        };
        render_mix(frame, mix, app, theme);
        render_lenses(frame, lenses, app, theme);
        return;
    }

    let [mix, lenses, days] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(11),
            Constraint::Length(8),
            Constraint::Min(5),
        ])
        .split(area)
    else {
        return;
    };

    render_mix(frame, mix, app, theme);
    render_lenses(frame, lenses, app, theme);
    render_days(frame, days, app, theme);
}

fn render_mix(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.panel);

    let block = panel("mix", theme, theme.tertiary);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = focus_mix_lines(
        app.rows(),
        app.selected,
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

fn render_lenses(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.panel);

    let block = panel("lenses", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max_total = (0..LENSES.len())
        .map(|index| app.lens_total(index))
        .max()
        .unwrap_or(1)
        .max(1);
    let width = inner.width as usize;
    let bar_width = width.saturating_sub(14).max(3);
    let mut lines = Vec::new();

    for (index, label) in LENSES.iter().enumerate().take(inner.height as usize) {
        let selected = index == app.lens;
        let bg = if selected {
            theme.selection
        } else {
            theme.panel
        };
        let color = lens_color(index, theme);
        let mut spans = vec![
            Span::styled(
                if selected { ">" } else { " " },
                Style::default().fg(color).bg(bg).add_modifier(if selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            ),
            Span::styled(
                format!("{} ", index + 1),
                Style::default().fg(theme.dim).bg(bg),
            ),
            Span::styled(
                fit_text(label, 5),
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
            ratio(app.lens_total(index), max_total),
            bar_width,
            color,
            bg,
            theme,
        ));
        spans.push(Span::styled(
            format!(" {}", duration_compact(app.lens_total(index))),
            Style::default().fg(theme.muted).bg(bg),
        ));
        lines.push(Line::from(spans));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_days(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    fill_area(frame, area, theme.panel);

    let block = panel("days", theme, theme.success);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let max = app
        .days
        .iter()
        .map(|day| day.focused_seconds)
        .max()
        .unwrap_or(1)
        .max(1);
    let width = inner.width as usize;
    let bar_width = width.saturating_sub(12).max(3);
    let mut lines = Vec::new();

    for day in app.days.iter().rev().take(inner.height as usize).rev() {
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
            format!(" {}", duration_compact(day.focused_seconds)),
            Style::default().fg(theme.muted).bg(theme.panel),
        ));
        lines.push(Line::from(spans));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn apps_header(width: usize, theme: &Theme) -> Line<'static> {
    let name_width = width.saturating_sub(42).clamp(12, 28);
    let bar_width = width.saturating_sub(name_width + 31).max(8);
    Line::from(vec![
        Span::styled(" #  ", Style::default().fg(theme.dim).bg(theme.panel_alt)),
        Span::styled(
            fit_text("application", name_width),
            Style::default().fg(theme.muted).bg(theme.panel_alt),
        ),
        Span::styled(" ", Style::default().bg(theme.panel_alt)),
        Span::styled(
            fit_text("focus map", bar_width),
            Style::default().fg(theme.muted).bg(theme.panel_alt),
        ),
        Span::styled(
            " focus share dense",
            Style::default().fg(theme.muted).bg(theme.panel_alt),
        ),
    ])
}

fn app_main_line(
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
    let name_width = width.saturating_sub(42).clamp(12, 28);
    let bar_width = width.saturating_sub(name_width + 31).max(8);
    let mut spans = vec![
        Span::styled(
            if selected { ">" } else { " " },
            Style::default().fg(color).bg(bg).add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        ),
        Span::styled(
            format!("{:>2} ", index + 1),
            Style::default().fg(theme.dim).bg(bg),
        ),
        Span::styled(
            fit_text(&short_app(&row.app_class, name_width), name_width),
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
        format!(" {:>6}", duration_compact(row.focused_seconds)),
        Style::default().fg(theme.warn).bg(bg),
    ));
    spans.push(Span::styled(
        format!(" {}", percent(ratio(row.focused_seconds, total))),
        Style::default().fg(theme.tertiary).bg(bg),
    ));
    spans.push(Span::styled(
        format!(" {}", percent(ratio(row.focused_seconds, row.open_seconds))),
        Style::default().fg(theme.success).bg(bg),
    ));
    Line::from(spans)
}

fn app_detail_line(
    row: &AppTotals,
    selected: bool,
    max_focus: i64,
    width: usize,
    theme: &Theme,
) -> Line<'static> {
    let bg = if selected {
        theme.selection
    } else {
        theme.panel
    };
    let prefix = "    ";
    let graph_width = width.saturating_sub(prefix.chars().count() + 23).max(10);
    Line::from(vec![
        Span::styled(prefix, Style::default().bg(bg)),
        Span::styled(
            braille_meter(ratio(row.focused_seconds, max_focus), graph_width),
            Style::default().fg(theme.dim).bg(bg),
        ),
        Span::styled(
            format!(" open {}", duration_compact(row.open_seconds)),
            Style::default().fg(theme.muted).bg(bg),
        ),
    ])
}

fn day_footer(days: &[DayTotals], width: usize, theme: &Theme) -> Line<'static> {
    let latest = days
        .last()
        .map(|day| {
            format!(
                "{}  {} focused / {} open",
                day.label,
                format_duration(day.focused_seconds),
                format_duration(day.open_seconds)
            )
        })
        .unwrap_or_else(|| "no daily samples".to_string());
    Line::from(vec![
        Span::styled(" 14d ", Style::default().fg(theme.bg).bg(theme.primary)),
        Span::styled(
            fit_text(&format!(" {latest}"), width.saturating_sub(5)),
            Style::default().fg(theme.muted).bg(theme.panel),
        ),
    ])
}

fn day_graph(days: &[DayTotals], width: usize, height: usize, theme: &Theme) -> Vec<Line<'static>> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    if days.is_empty() {
        return texture_lines(width, height, theme);
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
    let mut lines = Vec::with_capacity(height);

    for row in 0..height {
        let mut spans = Vec::with_capacity(width);
        for col in 0..width {
            let mut focus_mask = 0u16;
            let mut open_mask = 0u16;
            let mut color = theme.primary;

            for dot_y in 0..4 {
                for dot_x in 0..2 {
                    let virtual_x = (col * 2 + dot_x).min(virtual_width.saturating_sub(1));
                    let position = if virtual_width <= 1 {
                        0.0
                    } else {
                        virtual_x as f64 / (virtual_width - 1) as f64
                    };
                    let focus = interpolated_day_ratio(days, position, focus_max, true);
                    let open = interpolated_day_ratio(days, position, open_max, false);
                    let virtual_y = row * 4 + dot_y;
                    let from_bottom = virtual_height.saturating_sub(virtual_y);
                    let focus_level = (focus * virtual_height as f64).round() as usize;
                    let open_level = (open * virtual_height as f64).round() as usize;
                    let bit = braille_bit(dot_x, dot_y);

                    if focus_level > 0 && from_bottom <= focus_level {
                        focus_mask |= bit;
                        color = heat_color(focus, theme);
                    } else if open_level > 0 && from_bottom <= open_level {
                        open_mask |= bit;
                    }
                }
            }

            let (glyph, fg) = if focus_mask != 0 {
                (braille_char(focus_mask), color)
            } else if open_mask != 0 {
                (braille_char(open_mask), theme.dim)
            } else if row + 1 == height && col % 8 == 0 {
                ("⠄".to_string(), theme.dim)
            } else {
                (" ".to_string(), theme.dim)
            };
            spans.push(Span::styled(glyph, Style::default().fg(fg).bg(theme.panel)));
        }
        lines.push(Line::from(spans));
    }

    lines
}

fn focus_mix_lines(
    rows: &[AppTotals],
    selected: usize,
    width: usize,
    height: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let segments = pie_segments(rows, selected, theme);
    if segments.is_empty() {
        return vec![Line::from(Span::styled(
            "no focus signal",
            Style::default().fg(theme.muted).bg(theme.panel),
        ))];
    }

    let chart_width = width.min(15);
    let chart_height = height.min(7);
    let active = segments
        .iter()
        .position(|segment| segment.selected)
        .unwrap_or(0);
    let mut lines = Vec::new();

    if width >= 44 {
        let legend_width = width.saturating_sub(chart_width + 1);
        for y in 0..chart_height {
            let mut spans = pie_row(&segments, active, chart_width, chart_height, y, theme);
            spans.push(Span::styled(" ", Style::default().bg(theme.panel)));
            if let Some(segment) = segments.get(y) {
                spans.extend(segment_legend(segment, legend_width, theme));
            }
            lines.push(Line::from(spans));
        }
    } else {
        let left_padding = width.saturating_sub(chart_width) / 2;
        let right_padding = width.saturating_sub(chart_width + left_padding);
        for y in 0..chart_height {
            let mut spans = vec![Span::styled(
                " ".repeat(left_padding),
                Style::default().bg(theme.panel),
            )];
            spans.extend(pie_row(
                &segments,
                active,
                chart_width,
                chart_height,
                y,
                theme,
            ));
            spans.push(Span::styled(
                " ".repeat(right_padding),
                Style::default().bg(theme.panel),
            ));
            lines.push(Line::from(spans));
        }
        for segment in segments.iter().take(height.saturating_sub(lines.len())) {
            lines.push(Line::from(segment_legend(segment, width, theme)));
        }
    }

    lines.truncate(height);
    lines
}

#[derive(Clone)]
struct PieSegment {
    label: String,
    share: f64,
    color: Color,
    selected: bool,
}

fn pie_segments(rows: &[AppTotals], selected: usize, theme: &Theme) -> Vec<PieSegment> {
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
            color: rank_color(index, theme),
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
            color: theme.muted,
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
    theme: &Theme,
) -> Vec<Span<'static>> {
    let center_x = (width as f64 - 1.0) / 2.0;
    let center_y = (height as f64 - 1.0) / 2.0;
    let outer_x = (width as f64 / 2.0).max(1.0);
    let outer_y = (height as f64 / 2.0).max(1.0);

    (0..width)
        .map(|col| {
            let dx = (col as f64 - center_x) / outer_x;
            let dy = (row as f64 - center_y) / outer_y;
            let radius = (dx * dx + dy * dy).sqrt();

            if !(0.39..=1.0).contains(&radius) {
                return Span::styled(" ", Style::default().bg(theme.panel));
            }

            let angle = (dy.atan2(dx) + PI * 2.5) % (PI * 2.0);
            let segment_index = pie_segment_index(segments, angle / (PI * 2.0));
            let segment = &segments[segment_index];
            let highlighted = segment.selected || segment_index == active;
            let glyph = if radius > 0.84 { "▓" } else { "█" };
            Span::styled(
                glyph,
                Style::default()
                    .fg(if highlighted {
                        theme.text
                    } else {
                        segment.color
                    })
                    .bg(theme.panel)
                    .add_modifier(if highlighted {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            )
        })
        .collect()
}

fn segment_legend(segment: &PieSegment, width: usize, theme: &Theme) -> Vec<Span<'static>> {
    let label_width = width.saturating_sub(9);
    vec![
        Span::styled(
            if segment.selected { ">" } else { " " },
            Style::default()
                .fg(if segment.selected {
                    theme.text
                } else {
                    segment.color
                })
                .bg(theme.panel),
        ),
        Span::styled("●", Style::default().fg(segment.color).bg(theme.panel)),
        Span::styled(
            format!(" {}", percent(segment.share)),
            Style::default().fg(theme.text).bg(theme.panel),
        ),
        Span::styled(
            format!(" {}", fit_text(&segment.label, label_width)),
            Style::default().fg(theme.muted).bg(theme.panel),
        ),
    ]
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

fn panel(title: &str, theme: &Theme, accent: Color) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent).bg(theme.panel))
        .style(Style::default().bg(theme.panel))
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

fn metric_line(label: &str, value: &str, color: Color, theme: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<7}"),
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
        Span::styled(
            value.to_string(),
            Style::default().fg(color).bg(theme.panel),
        ),
    ])
}

fn meter_line(label: &str, value: f64, width: usize, color: Color, theme: &Theme) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!("{label:<7}"),
        Style::default().fg(theme.dim).bg(theme.panel),
    )];
    spans.extend(bar_spans(value, width, color, theme.panel, theme));
    spans.push(Span::styled(
        format!(" {}", percent(value)),
        Style::default().fg(theme.text).bg(theme.panel),
    ));
    Line::from(spans)
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

fn braille_meter(value: f64, width: usize) -> String {
    let width = width.max(1);
    let filled = ((value.clamp(0.0, 1.0) * width as f64).round() as usize).min(width);
    format!(
        "{}{}",
        "⣿".repeat(filled),
        "⠄".repeat(width.saturating_sub(filled))
    )
}

fn texture_lines(width: usize, height: usize, theme: &Theme) -> Vec<Line<'static>> {
    (0..height)
        .map(|row| {
            let text = (0..width)
                .map(|col| {
                    if (row * 11 + col * 5) % 47 == 0 {
                        '·'
                    } else {
                        ' '
                    }
                })
                .collect::<String>();
            Line::from(Span::styled(
                text,
                Style::default().fg(theme.dim).bg(theme.panel),
            ))
        })
        .collect()
}

fn rule(width: usize) -> String {
    "─".repeat(width)
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

fn interpolated_day_ratio(days: &[DayTotals], position: f64, max: i64, focused: bool) -> f64 {
    if days.is_empty() {
        return 0.0;
    }

    let scaled = position.clamp(0.0, 1.0) * days.len().saturating_sub(1) as f64;
    let left = scaled.floor() as usize;
    let right = scaled.ceil() as usize;
    let t = scaled - left as f64;
    let left_value = if focused {
        days[left].focused_seconds
    } else {
        days[left].open_seconds
    };
    let right_value = if focused {
        days[right].focused_seconds
    } else {
        days[right].open_seconds
    };
    let value = left_value as f64 + (right_value - left_value) as f64 * t;
    (value.max(0.0) / max.max(1) as f64).clamp(0.0, 1.0)
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        (numerator.max(0) as f64 / denominator as f64).clamp(0.0, 1.0)
    }
}

fn lens_color(index: usize, theme: &Theme) -> Color {
    match index {
        0 => theme.primary,
        1 => theme.success,
        2 => theme.warn,
        3 => theme.tertiary,
        _ => theme.secondary,
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

fn heat_color(share: f64, theme: &Theme) -> Color {
    if share > 0.72 {
        theme.warn
    } else if share > 0.45 {
        theme.primary
    } else if share > 0.18 {
        theme.success
    } else {
        theme.dim
    }
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
    fn compact_80_column_layout_keeps_core_panels() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = sample_app();

        terminal.draw(|frame| render(frame, &app)).unwrap();

        let rendered = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        for label in ["timeline", "replay", "apps", "mix", "lenses"] {
            assert!(rendered.contains(label), "missing compact panel {label}");
        }
    }

    fn sample_app() -> App {
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
        let days = (0..14)
            .map(|index| DayTotals {
                label: format!("D{index}"),
                focused_seconds: (index as i64 + 1) * 300,
                open_seconds: (index as i64 + 2) * 500,
            })
            .collect();

        App {
            lens: 1,
            selected: 0,
            last_refresh: Instant::now(),
            loaded_at: Local::now(),
            theme: Theme::fallback(),
            today: rows.clone(),
            week: rows.clone(),
            month: rows.clone(),
            year: rows.clone(),
            all_time: rows,
            days,
        }
    }
}
