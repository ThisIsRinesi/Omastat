mod app;
mod data;
mod theme;
mod views;
mod widgets;

use self::app::{App, View};
use crate::{config::Config, report::Lens, storage::Storage};
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
const AUTO_REFRESH: Duration = Duration::from_secs(30);

pub fn run(storage: Storage, config: Config) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _cleanup = TerminalCleanup;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_app(&mut terminal, &storage, config)
}

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, cursor::Show);
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    storage: &Storage,
    config: Config,
) -> Result<()> {
    let mut app = App::load(storage, config)?;
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
                    KeyCode::Tab => app.next_view(storage),
                    KeyCode::BackTab => app.previous_view(storage),
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
        View::Insights => views::render_insights(frame, body, app, &theme),
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
        insights::{
            Insight, InsightCategory, InsightConfidence, InsightEvidence, InsightKind,
            InsightSupport, InsightTone,
        },
        report::{self, UsageReport},
        steam::SteamResolver,
        storage::{
            AppDayTotals, AppTotals, AppWorkspaceTotals, DayTotals, FocusHeatCell, IntervalKind,
            StorageStatus, SystemIntervalKind, SystemTimelineInterval, TimelineInterval,
            TitleTotals, WorkspaceTotals,
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
            "Insights",
            "Apps",
            "Timeline",
            "System",
            "Daily Pattern",
            "Focus %",
            "App Mix",
            "When Focus Happens",
            "Less focus",
            "More focus",
            "Busiest:",
            "Workspace Focus",
            "Top Hours",
            "Focus Sessions",
            "Week of Jan 12",
        ] {
            assert!(rendered.contains(label), "missing {label}");
        }
        for old_label in [
            "Focus Flow",
            "Focus Composition",
            "Focus Heat",
            "focus area",
            "cell max",
            "darkest =",
        ] {
            assert!(
                !rendered.contains(old_label),
                "old label still rendered: {old_label}"
            );
        }
    }

    #[test]
    fn overview_selection_inspects_daily_pattern() {
        let backend = TestBackend::new(128, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Overview);

        assert_eq!(
            app.selected_day().map(|day| day.label.as_str()),
            Some("D13")
        );
        app.move_selection(-2);
        assert_eq!(
            app.selected_day().map(|day| day.label.as_str()),
            Some("D11")
        );

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(rendered.contains("D11:"), "selected day detail missing");
        assert!(rendered.contains("focused while open"));
    }

    #[test]
    fn insights_view_renders_grouped_list_and_selected_detail() {
        let backend = TestBackend::new(128, 34);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Insights);

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        for label in [
            "Insights",
            "Insight Details",
            "Patterns",
            "Focus",
            "Apps",
            "System",
            "Top app",
            "Ghostty",
            "Observed",
        ] {
            assert!(rendered.contains(label), "missing {label}");
        }
    }

    #[test]
    fn insights_view_empty_state_explains_insufficient_data() {
        let backend = TestBackend::new(96, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app_with_insights(View::Insights, Vec::new());

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(rendered.contains("No evaluated insights for this period yet"));
        assert!(rendered.contains("broader lens"));
    }

    #[test]
    fn insights_selection_does_not_move_selected_app() {
        let mut app = sample_app(View::Insights);
        app.selected = 2;

        app.move_selection(1);

        assert_eq!(app.selected, 2);
        assert_eq!(app.selected_insight_index(), 1);
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
    fn apps_view_renders_selected_app_facts() {
        let backend = TestBackend::new(128, 34);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Apps);

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        for label in [
            "Typical hour",
            "Best session",
            "Interruptions",
            "Workspace",
            "code",
        ] {
            assert!(rendered.contains(label), "missing {label}");
        }
    }

    #[test]
    fn timeline_view_renders_canvas_and_interval_table() {
        let backend = TestBackend::new(112, 34);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = sample_app(View::Timeline);

        terminal.draw(|frame| render(frame, &mut app)).unwrap();

        let rendered = rendered_text(&terminal);
        assert!(rendered.contains("Activity Canvas"));
        assert!(rendered.contains("Not Counted"));
        assert!(rendered.contains("Intervals"));
        assert!(rendered.contains("Ghostty"));
        assert!(rendered.contains("System sleep"));
        assert!(rendered.contains("Tracker off gap"));
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
        assert!(rendered.contains("Daily Pattern"));
        assert!(rendered.contains("Focus Sessions"));
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
        sample_app_with_insights(view, sample_insights())
    }

    fn sample_app_with_insights(view: View, insights: Vec<Insight>) -> App {
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
                sleep_seconds: if index == 4 { 1800 } else { 0 },
                unobserved_seconds: if index == 2 { 900 } else { 0 },
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
            total_sleep_seconds: daily.iter().map(|day| day.sleep_seconds).sum(),
            total_unobserved_seconds: daily.iter().map(|day| day.unobserved_seconds).sum(),
            rows,
            apps,
            daily,
            heatmap: Vec::new(),
            insights,
            widget_insight: None,
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
        let system_intervals = vec![
            SystemTimelineInterval {
                kind: SystemIntervalKind::Sleep,
                source: Some("logind".to_string()),
                started_at: now - 2300,
                ended_at: now - 2100,
            },
            SystemTimelineInterval {
                kind: SystemIntervalKind::Unobserved,
                source: Some("daemon-recovery".to_string()),
                started_at: now - 1800,
                ended_at: now - 1600,
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
        let workspaces = vec![
            WorkspaceTotals {
                workspace: "code".to_string(),
                focused_seconds: 7 * 3600,
            },
            WorkspaceTotals {
                workspace: "chat".to_string(),
                focused_seconds: 3600,
            },
        ];
        let app_workspaces = vec![
            AppWorkspaceTotals {
                workspace: "code".to_string(),
                app_class: "com.mitchellh.ghostty".to_string(),
                focused_seconds: 7 * 3600,
            },
            AppWorkspaceTotals {
                workspace: "chat".to_string(),
                app_class: "discord".to_string(),
                focused_seconds: 3600,
            },
        ];

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
            timeline_intervals: timeline,
            system_intervals,
            daily_apps,
            heatmap,
            workspaces,
            app_workspaces,
            titles,
            storage: StorageStatus::default(),
            steam: SteamResolver::default(),
            theme: Theme::fallback(),
        })
    }

    fn sample_insights() -> Vec<Insight> {
        vec![
            sample_insight(
                InsightKind::TopApp,
                InsightCategory::Apps,
                InsightTone::Neutral,
                "Top app share",
                "Ghostty - 8h (73%)",
                "The app with the largest share of focused time in this period.",
                InsightSupport {
                    period_label: Some("Week of Jan 12, 2026".to_string()),
                    app_class: Some("com.mitchellh.ghostty".to_string()),
                    app_label: Some("Ghostty".to_string()),
                    focused_seconds: Some(8 * 3600),
                    open_seconds: Some(12 * 3600),
                    share: Some(0.73),
                    ..InsightSupport::default()
                },
            ),
            sample_insight(
                InsightKind::PeriodComparison,
                InsightCategory::Patterns,
                InsightTone::Positive,
                "vs previous week",
                "+2h",
                "Compares focused time for this week with the previous week.",
                InsightSupport {
                    period_label: Some("Week of Jan 12, 2026".to_string()),
                    comparison_label: Some("Previous week".to_string()),
                    focused_seconds: Some(11 * 3600),
                    comparison_seconds: Some(9 * 3600),
                    delta_seconds: Some(2 * 3600),
                    ..InsightSupport::default()
                },
            ),
            sample_insight(
                InsightKind::DeepWorkBlocks,
                InsightCategory::FocusQuality,
                InsightTone::Positive,
                "Deep work blocks",
                "4 blocks / 3h 20m",
                "Counts focused blocks at or above the deep-work threshold.",
                InsightSupport {
                    block_count: Some(4),
                    total_seconds: Some(12_000),
                    longest_seconds: Some(3_600),
                    median_seconds: Some(2_400),
                    threshold_seconds: Some(25 * 60),
                    ..InsightSupport::default()
                },
            ),
            sample_insight(
                InsightKind::UnobservedExcluded,
                InsightCategory::SystemSignals,
                InsightTone::Caution,
                "Unobserved time excluded",
                "15m excluded",
                "Daemon downtime was excluded from focused time.",
                InsightSupport {
                    unobserved_seconds: Some(900),
                    excluded_seconds: Some(900),
                    share: Some(0.03),
                    ..InsightSupport::default()
                },
            ),
        ]
    }

    fn sample_insight(
        kind: InsightKind,
        category: InsightCategory,
        tone: InsightTone,
        title: &str,
        value: &str,
        explanation: &str,
        supporting: InsightSupport,
    ) -> Insight {
        Insight {
            kind,
            category,
            tone,
            title: title.to_string(),
            value: value.to_string(),
            explanation: explanation.to_string(),
            confidence: InsightConfidence::High,
            evidence: InsightEvidence {
                data_points: 7,
                minimum_data_points: 2,
                observed_focus_seconds: 11 * 3600,
                observed_open_seconds: 21 * 3600,
            },
            supporting,
        }
    }
}
