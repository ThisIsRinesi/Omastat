mod app;
mod data;
mod theme;
mod views;
mod widgets;

use self::app::{App, View};
use crate::{report::Lens, storage::Storage};
use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Frame, Terminal, backend::CrosstermBackend, layout::Rect};
use std::{
    io,
    time::{Duration, Instant},
};

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
        terminal.draw(|frame| render(frame, &mut app))?;

        let refresh_deadline = app.last_refresh() + AUTO_REFRESH;
        let deadline = refresh_deadline.min(next_clock);

        if event::poll(deadline.saturating_duration_since(Instant::now()))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc if !app.help_open() => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Char('?') => app.toggle_help(),
                    KeyCode::Char('q') | KeyCode::Esc => app.close_help(),
                    KeyCode::Char('r') => app.refresh(storage)?,
                    KeyCode::Tab => app.next_view(),
                    KeyCode::BackTab => app.previous_view(),
                    KeyCode::Char('p') => app.toggle_trends(),
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
        if app.last_refresh().elapsed() >= AUTO_REFRESH {
            app.refresh(storage)?;
        }
        while next_clock <= now {
            next_clock += CLOCK_REFRESH;
        }
    }
}

fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let theme = app.theme().clone();
    widgets::fill_area(frame, area, theme.bg);

    if area.width < 52 || area.height < 16 {
        views::render_tiny(frame, area, app, &theme);
        return;
    }

    let [header, body, footer] = *ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Min(12),
        ratatui::layout::Constraint::Length(1),
    ])
    .split(area) else {
        return;
    };

    views::render_header(frame, header, app, &theme);
    match app.view() {
        View::Overview => views::render_overview(frame, body, app, &theme),
        View::Apps => views::render_apps(frame, body, app, &theme),
        View::Timeline => views::render_timeline(frame, body, app, &theme),
        View::System => views::render_system(frame, body, app, &theme),
    }
    views::render_footer(frame, footer, app, &theme);
    if app.help_open() {
        views::render_help(frame, centered(area, 72, 18), app, &theme);
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4)).max(1);
    let height = height.min(area.height.saturating_sub(2)).max(1);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        report::{self, UsageReport},
        steam::SteamResolver,
        storage::{
            AppDayTotals, AppTotals, DayTotals, FocusHeatCell, IntervalKind, StorageStatus,
            TimelineInterval, TitleTotals,
        },
        tui::theme::Theme,
    };
    use chrono::Local;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn overview_renders_rich_dashboard_widgets() {
        let backend = TestBackend::new(128, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Overview);

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        for label in [
            "Overview",
            "Apps",
            "Timeline",
            "System",
            "Focus Flow",
            "Focus Composition",
            "Focus Heat",
            "Peak Hours",
            "Focus Stats",
            "Week of Jan 12",
        ] {
            assert!(rendered.contains(label), "missing {label}");
        }
    }

    #[test]
    fn apps_view_keeps_selected_row_visible() {
        let backend = TestBackend::new(108, 26);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Apps);
        app.replace_rows_for_test(
            (0..30)
                .map(|index| AppTotals {
                    app_class: format!("app-{index:02}"),
                    focused_seconds: 3600 - index as i64,
                    open_seconds: 7200,
                })
                .collect(),
        );
        app.selected = 29;

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(rendered.contains("App 29"), "selected row was not visible");
    }

    #[test]
    fn timeline_view_renders_canvas_and_interval_table() {
        let backend = TestBackend::new(112, 26);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Timeline);

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(rendered.contains("Activity Canvas"));
        assert!(rendered.contains("Intervals"));
        assert!(rendered.contains("Ghostty"));
    }

    #[test]
    fn help_overlay_renders_shortcuts() {
        let backend = TestBackend::new(100, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Overview);
        app.toggle_help();

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(rendered.contains("Help"));
        assert!(rendered.contains("Tab / Shift-Tab"));
    }

    #[test]
    fn narrow_overview_uses_space_aware_widgets() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Overview);

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(rendered.contains("Focus Spark"));
        assert!(rendered.contains("Focus Stats"));
        assert!(!rendered.contains("Need more space"));
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
            query_start_ts: Local::now().timestamp() - 7 * 86400,
            query_end_ts: Local::now().timestamp(),
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
                    value: "Ghostty - 8h".to_string(),
                },
                report::InsightRow {
                    label: "Focus density".to_string(),
                    value: "66%".to_string(),
                },
            ],
        };

        let now = Local::now().timestamp();
        let timeline = vec![
            TimelineInterval {
                kind: IntervalKind::Focused,
                app_class: "com.mitchellh.ghostty".to_string(),
                started_at: now - 3600,
                ended_at: now - 2400,
            },
            TimelineInterval {
                kind: IntervalKind::Open,
                app_class: "discord".to_string(),
                started_at: now - 2000,
                ended_at: now - 1200,
            },
        ];
        let daily_apps = vec![
            AppDayTotals {
                date: "2026-01-13".to_string(),
                label: "Jan 13".to_string(),
                app_class: "com.mitchellh.ghostty".to_string(),
                focused_seconds: 3600,
            },
            AppDayTotals {
                date: "2026-01-14".to_string(),
                label: "Jan 14".to_string(),
                app_class: "com.mitchellh.ghostty".to_string(),
                focused_seconds: 7200,
            },
        ];
        let titles = vec![TitleTotals {
            app_class: "com.mitchellh.ghostty".to_string(),
            title: "Dashboard.rs".to_string(),
            focused_seconds: 3600,
        }];
        let heatmap = (0..7)
            .flat_map(|weekday| {
                (0..24).map(move |hour| FocusHeatCell {
                    weekday,
                    hour,
                    focused_seconds: if weekday == 2 && (9..=12).contains(&hour) {
                        1800
                    } else {
                        0
                    },
                })
            })
            .collect();

        App::from_parts_for_test(app::TestAppParts {
            view,
            report,
            lens_totals: [
                Some((8 * 3600, 12 * 3600)),
                Some((11 * 3600, 21 * 3600)),
                Some((40 * 3600, 80 * 3600)),
                Some((200 * 3600, 420 * 3600)),
                Some((500 * 3600, 900 * 3600)),
            ],
            today_intervals: timeline,
            daily_apps,
            heatmap,
            titles,
            storage: StorageStatus::default(),
            steam: SteamResolver::default(),
            theme: Theme::fallback(),
        })
    }
}
