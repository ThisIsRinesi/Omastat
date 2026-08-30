use super::{
    app::{App, View},
    theme::Theme,
    widgets,
};
use crate::{
    clock,
    insights::{
        Insight, InsightCategory, InsightConfidence, InsightKind, InsightSupport, InsightTone,
    },
    report::{self, Lens},
    storage::{
        AppDayTotals, AppTotals, DayTotals, FocusHeatCell, IntervalKind, SystemIntervalKind,
        SystemTimelineInterval, TimelineInterval, TitleTotals, WorkspaceTotals,
    },
};
use chrono::{Datelike, Local, TimeZone, Timelike};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{
        Bar, BarChart, Block, Cell, Clear, HighlightSpacing, LineGauge, List, ListItem, Paragraph,
        Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Sparkline, Table, Wrap, canvas,
    },
};
use tui_piechart::{LegendLayout, LegendPosition, PieChart, PieSlice, Resolution};

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const DAILY_LANE_CHARS: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '█'];
const INSIGHT_CATEGORIES: [InsightCategory; 4] = [
    InsightCategory::Patterns,
    InsightCategory::FocusQuality,
    InsightCategory::Apps,
    InsightCategory::SystemSignals,
];

pub(super) fn render_tiny(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let lines = vec![
        Line::from(vec![
            Span::styled("omastat ", Style::default().fg(theme.primary)),
            Span::styled(app.view().label(), Style::default().fg(theme.text)),
        ]),
        Line::from(Span::styled(
            app.lens().label(),
            Style::default()
                .fg(theme.bg)
                .bg(widgets::lens_color(app.lens(), theme))
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
            .block(widgets::panel("OMASTAT", theme, theme.primary)),
        area,
    );
}

pub(super) fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    let clock = if area.width < 86 {
        clock::local_now().format("%H:%M").to_string()
    } else {
        clock::local_now().format("%H:%M:%S").to_string()
    };

    let mut first = vec![
        Span::styled(
            " 󰔟 OMASTAT ",
            Style::default()
                .fg(theme.primary)
                .bg(theme.bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ", Style::default().bg(theme.bg)),
    ];
    for view in View::ALL {
        first.push(widgets::pill(
            view.label(),
            view == app.view(),
            theme.primary,
            theme,
        ));
    }

    let mut second = Vec::new();
    second.push(Span::styled(" ", Style::default().bg(theme.bg)));
    for lens in Lens::ALL {
        second.push(widgets::pill(
            lens.label(),
            lens == app.lens(),
            widgets::lens_color(lens, theme),
            theme,
        ));
    }
    if area.width < 96 {
        let used = second
            .iter()
            .map(|span| span.content.chars().count())
            .sum::<usize>()
            + 2;
        second.extend([
            Span::styled("  ", Style::default().bg(theme.bg)),
            Span::styled(
                widgets::fit_text(
                    &format!("{}  {clock}", app.report().period.label),
                    (area.width as usize).saturating_sub(used),
                ),
                Style::default().fg(theme.text).bg(theme.bg),
            ),
        ]);
    } else {
        second.extend([
            Span::styled("  period ", Style::default().fg(theme.dim).bg(theme.bg)),
            Span::styled(
                widgets::fit_text(&app.report().period.label, 28),
                Style::default().fg(theme.text).bg(theme.bg),
            ),
            Span::styled("  updated ", Style::default().fg(theme.dim).bg(theme.bg)),
            Span::styled(
                app.loaded_at().format("%H:%M:%S").to_string(),
                Style::default().fg(theme.muted).bg(theme.bg),
            ),
            Span::styled("  now ", Style::default().fg(theme.dim).bg(theme.bg)),
            Span::styled(clock, Style::default().fg(theme.text).bg(theme.bg)),
        ]);
    }

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(first),
            Line::from(second),
            Line::from(Span::styled(
                "─".repeat(area.width as usize),
                Style::default().fg(theme.border).bg(theme.bg),
            )),
        ])
        .style(Style::default().bg(theme.bg)),
        area,
    );
}

pub(super) fn render_footer(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    let period_hint = if app.lens() == Lens::Life {
        "life".to_string()
    } else if app.period_offset() == 0 {
        "current".to_string()
    } else {
        format!("{} back", app.period_offset().abs())
    };
    let left = match app.view() {
        View::Overview => "j/k inspect day  h/l lens  [/] period  p switch detail  ? help  q quit",
        View::Insights => "j/k select  PgUp/PgDn jump  h/l lens  [/] period  ? help  q quit",
        View::Apps => "j/k select  PgUp/PgDn jump  h/l lens  [/] period  ? help  q quit",
        View::Timeline => "j/k highlight app  h/l lens  [/] period  ? help  q quit",
        View::System => "Tab view  h/l lens  r refresh  ? help  q quit",
    };
    let status = format!(
        "[{} / {} / {period_hint}]",
        app.view().label(),
        app.lens().label()
    );
    let right = format!("30s auto {status}");
    let width = area.width as usize;
    let left_len = left.chars().count();
    let right_len = right.chars().count();
    let mut spans = Vec::new();
    if left_len + right_len < width {
        spans.push(Span::styled(
            left.to_string(),
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
            widgets::fit_text(left, width),
            Style::default().fg(theme.muted).bg(theme.bg),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub(super) fn render_overview(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    if area.width < 92 {
        let [kpis, flow, apps, hours, stats] = *Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Min(2),
        ])
        .split(area) else {
            return;
        };
        render_kpis(frame, kpis, app, theme);
        render_focus_sparkline(frame, flow, app, theme);
        render_focus_composition(frame, apps, app, theme);
        render_peak_hours(frame, hours, app, theme);
        if app.show_trends() {
            render_focus_stats(frame, stats, app, theme);
        } else {
            render_period_signals(frame, stats, app, theme);
        }
        return;
    }

    let [kpis, body] = *Layout::vertical([Constraint::Length(4), Constraint::Min(10)]).split(area)
    else {
        return;
    };
    render_kpis(frame, kpis, app, theme);
    if body.height < 20 {
        let [flow, lower] =
            *Layout::vertical([Constraint::Percentage(58), Constraint::Percentage(42)]).split(body)
        else {
            return;
        };
        let [composition, stats] =
            *Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(lower)
        else {
            return;
        };
        render_focus_chart(frame, flow, app, theme);
        render_focus_composition(frame, composition, app, theme);
        if app.show_trends() {
            render_focus_stats(frame, stats, app, theme);
        } else {
            render_period_signals(frame, stats, app, theme);
        }
        return;
    }

    let [primary, context] =
        *Layout::horizontal([Constraint::Percentage(68), Constraint::Percentage(32)]).split(body)
    else {
        return;
    };
    let [flow, heat] =
        *Layout::vertical([Constraint::Percentage(43), Constraint::Percentage(57)]).split(primary)
    else {
        return;
    };
    let [composition, hours, workspaces, stats] = *Layout::vertical([
        Constraint::Percentage(36),
        Constraint::Percentage(20),
        Constraint::Percentage(18),
        Constraint::Percentage(26),
    ])
    .split(context) else {
        return;
    };

    render_focus_chart(frame, flow, app, theme);
    render_heatmap(frame, heat, app, theme);
    render_focus_composition(frame, composition, app, theme);
    render_peak_hours(frame, hours, app, theme);
    render_workspace_focus(frame, workspaces, app, theme);
    if app.show_trends() {
        render_focus_stats(frame, stats, app, theme);
    } else {
        render_period_signals(frame, stats, app, theme);
    }
}

pub(super) fn render_insights(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    if app.insights().is_empty() {
        let block = widgets::panel("Insights", theme, theme.primary);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        widgets::render_empty(
            frame,
            inner,
            "No evaluated insights for this period yet. Track focused time or choose a broader lens.",
            theme,
        );
        return;
    }

    if area.width < 98 {
        let [list, detail] =
            *Layout::vertical([Constraint::Percentage(58), Constraint::Percentage(42)]).split(area)
        else {
            return;
        };
        render_insight_list(frame, list, app, theme);
        render_insight_detail(frame, detail, app, theme);
        return;
    }

    let [list, detail] =
        *Layout::horizontal([Constraint::Percentage(46), Constraint::Percentage(54)]).split(area)
    else {
        return;
    };
    render_insight_list(frame, list, app, theme);
    render_insight_detail(frame, detail, app, theme);
}

fn render_insight_list(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    let insights = app.insights().to_vec();
    let selected_index = app.selected_insight_index();
    let entries = insight_entries(&insights);
    let selected_entry = entries
        .iter()
        .position(|entry| entry.insight_index() == Some(selected_index));
    let items = entries
        .iter()
        .map(|entry| match *entry {
            InsightEntry::Header { category, count } => ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", insight_category_label(category)),
                    Style::default()
                        .fg(theme.primary)
                        .bg(theme.panel)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("({count})"),
                    Style::default().fg(theme.dim).bg(theme.panel),
                ),
            ]))
            .style(Style::default().bg(theme.panel_alt)),
            InsightEntry::Insight { index } => {
                let insight = &insights[index];
                let selected = index == selected_index;
                let color = insight_tone_color(insight.tone, theme);
                let title = insight_display_title(insight);
                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        insight_tone_marker(insight.tone),
                        Style::default().fg(color).bg(theme.panel),
                    ),
                    Span::styled(" ", Style::default().bg(theme.panel)),
                    Span::styled(
                        widgets::fit_text(&title, 22),
                        Style::default()
                            .fg(theme.text)
                            .bg(theme.panel)
                            .add_modifier(if selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            }),
                    ),
                    Span::styled(" ", Style::default().bg(theme.panel)),
                    Span::styled(
                        widgets::fit_text(&insight.value, 28),
                        Style::default().fg(color).bg(theme.panel),
                    ),
                ])];
                if selected {
                    let explanation = insight_display_explanation(insight);
                    lines.push(Line::from(Span::styled(
                        widgets::fit_text(&explanation, area.width.saturating_sub(6) as usize),
                        Style::default().fg(theme.muted).bg(theme.panel),
                    )));
                }
                ListItem::new(Text::from(lines)).style(Style::default().bg(theme.panel))
            }
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(widgets::panel("Insights", theme, theme.primary))
        .style(Style::default().fg(theme.text).bg(theme.panel))
        .highlight_style(Style::default().fg(theme.text).bg(theme.selection))
        .highlight_symbol("▌");

    let state = app.insight_state();
    state.select(selected_entry);
    frame.render_stateful_widget(list, area, state);

    let visible_rows = area.height.saturating_sub(3) as usize;
    if entries.len() > visible_rows {
        let mut scrollbar_state =
            ScrollbarState::new(entries.len()).position(selected_entry.unwrap_or_default());
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(theme.primary))
            .track_style(Style::default().fg(theme.dim));
        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn render_insight_detail(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Insight Details", theme, theme.secondary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(insight) = app.selected_insight() else {
        widgets::render_empty(
            frame,
            inner,
            "Select an insight to see supporting values",
            theme,
        );
        return;
    };
    let title = insight_display_title(insight);
    let explanation = insight_display_explanation(insight);

    if inner.height < 8 {
        let lines = vec![
            Line::from(Span::styled(
                widgets::fit_text(&title, inner.width as usize),
                Style::default()
                    .fg(theme.text)
                    .bg(theme.panel)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                widgets::fit_text(&insight.value, inner.width as usize),
                Style::default()
                    .fg(insight_tone_color(insight.tone, theme))
                    .bg(theme.panel),
            )),
            Line::from(Span::styled(
                widgets::fit_text(&explanation, inner.width as usize),
                Style::default().fg(theme.muted).bg(theme.panel),
            )),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(theme.panel))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let [summary, support] =
        *Layout::vertical([Constraint::Length(6), Constraint::Min(4)]).split(inner)
    else {
        return;
    };
    let color = insight_tone_color(insight.tone, theme);
    let summary_lines = vec![
        Line::from(vec![
            Span::styled(
                insight_category_label(insight.category),
                Style::default().fg(theme.primary).bg(theme.panel),
            ),
            Span::styled("  ", Style::default().bg(theme.panel)),
            Span::styled(
                insight_tone_label(insight.tone),
                Style::default().fg(color).bg(theme.panel),
            ),
            Span::styled("  ", Style::default().bg(theme.panel)),
            Span::styled(
                insight_confidence_label(insight.confidence),
                Style::default().fg(theme.dim).bg(theme.panel),
            ),
        ]),
        Line::from(Span::styled(
            widgets::fit_text(&title, summary.width as usize),
            Style::default()
                .fg(theme.text)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            widgets::fit_text(&insight.value, summary.width as usize),
            Style::default().fg(color).bg(theme.panel),
        )),
        Line::from(Span::styled(
            widgets::fit_text(&explanation, summary.width as usize),
            Style::default().fg(theme.muted).bg(theme.panel),
        )),
    ];
    frame.render_widget(
        Paragraph::new(summary_lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: true }),
        summary,
    );

    let mut lines = insight_support_lines(&insight.supporting, support.width as usize, theme);
    lines.push(widgets::metric_line(
        "Samples",
        &format!(
            "{} / {} needed",
            insight.evidence.data_points, insight.evidence.minimum_data_points
        ),
        support.width as usize,
        theme.muted,
        theme,
    ));
    lines.push(widgets::metric_line(
        "Observed",
        &report::format_duration(insight.evidence.observed_focus_seconds),
        support.width as usize,
        theme.muted,
        theme,
    ));

    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        support,
    );
}

fn render_kpis(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let report = app.report();
    let stats = app.stats();
    let chunks = Layout::horizontal([
        Constraint::Percentage(26),
        Constraint::Percentage(24),
        Constraint::Percentage(22),
        Constraint::Percentage(28),
    ])
    .spacing(1)
    .split(area);
    render_kpi(
        frame,
        chunks[0],
        "Focused",
        &report::format_duration(report.total_focused_seconds),
        &format!("{} active days", stats.active_days),
        theme.warn,
        theme,
    );
    render_kpi(
        frame,
        chunks[1],
        "Daily Avg",
        &report::format_duration(stats.active_day_average_seconds),
        &format!("{} tracked days", stats.total_days),
        theme.success,
        theme,
    );
    render_kpi(
        frame,
        chunks[2],
        "Longest",
        &report::format_duration(stats.longest_block_seconds),
        &format!(
            "{} focus sessions",
            widgets::compact_count(stats.focus_block_count as i64)
        ),
        theme.secondary,
        theme,
    );
    render_kpi(
        frame,
        chunks[3],
        "App Mix",
        &format!("{:.1} apps", stats.effective_apps),
        &format!("top {}", report::percent(stats.top_app_share)),
        theme.primary,
        theme,
    );
}

fn render_kpi(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    value: &str,
    note: &str,
    color: ratatui::style::Color,
    theme: &Theme,
) {
    let block = widgets::panel(label, theme, color);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = vec![
        Line::from(Span::styled(
            value.to_string(),
            Style::default()
                .fg(color)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            widgets::fit_text(note, inner.width as usize),
            Style::default().fg(theme.dim).bg(theme.panel),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_focus_chart(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Daily Pattern", theme, theme.success);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.report().daily.is_empty() {
        widgets::render_empty(frame, inner, "No daily focus yet", theme);
        return;
    }

    let summary_height = if inner.height >= 10 { 3 } else { 2 }.min(inner.height);
    let [summary, lanes] =
        *Layout::vertical([Constraint::Length(summary_height), Constraint::Min(3)]).split(inner)
    else {
        return;
    };

    frame.render_widget(
        Paragraph::new(daily_pattern_summary_lines(
            app,
            summary.width as usize,
            theme,
        ))
        .style(Style::default().bg(theme.panel))
        .wrap(Wrap { trim: false }),
        summary,
    );

    let mut rows = daily_pattern_lane_rows(app, lanes.width as usize, theme);
    rows.truncate(lanes.height as usize);
    frame.render_widget(
        Paragraph::new(rows)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        lanes,
    );
}

fn daily_pattern_summary_lines(app: &App, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let selected = app
        .selected_day()
        .map(|day| {
            let not_counted = day_not_counted_seconds(day);
            format!(
                "{}: {} focus, {} not counted",
                day.label,
                report::format_duration(day.focused_seconds),
                report::format_duration(not_counted)
            )
        })
        .unwrap_or_else(|| "No day selected".to_string());
    let best = app
        .stats()
        .best_day_label
        .as_ref()
        .map(|label| {
            format!(
                "best {label} {}",
                report::format_duration(app.stats().best_day_seconds)
            )
        })
        .unwrap_or_else(|| "best none".to_string());
    let mut lines = vec![
        Line::from(Span::styled(
            widgets::fit_text(&selected, width),
            Style::default().fg(theme.text).bg(theme.panel),
        )),
        Line::from(Span::styled(
            widgets::fit_text(
                &format!(
                    "{} days with focus | daily average {} | {best}",
                    app.stats().active_days,
                    report::format_duration(app.stats().active_day_average_seconds)
                ),
                width,
            ),
            Style::default().fg(theme.dim).bg(theme.panel),
        )),
    ];

    lines.push(Line::from(vec![
        Span::styled("Focus", Style::default().fg(theme.warn).bg(theme.panel)),
        Span::styled(
            " Not counted",
            Style::default().fg(theme.tertiary).bg(theme.panel),
        ),
    ]));
    lines
}

fn daily_pattern_lane_rows(app: &App, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let days = &app.report().daily;
    let label_width = if width < 96 { 11 } else { 13 };
    let value_width = if width < 96 { 7 } else { 10 };
    let available = width.saturating_sub(label_width + value_width + 2);
    let cell_width = (available / days.len().max(1)).clamp(1, 5);

    let focus_values = days
        .iter()
        .map(|day| day.focused_seconds.max(0) as f64)
        .collect::<Vec<_>>();
    let not_counted_values = days
        .iter()
        .map(|day| day_not_counted_seconds(day).max(0) as f64)
        .collect::<Vec<_>>();

    vec![
        daily_lane_axis(days, label_width, value_width, cell_width, theme),
        daily_lane_line(DailyLane {
            label: "Focus",
            values: &focus_values,
            max_label: lane_duration_label(&focus_values),
            selected_index: app.selected_day_index(),
            label_width,
            value_width,
            cell_width,
            color: theme.warn,
            theme,
        }),
        daily_lane_line(DailyLane {
            label: "Not counted",
            values: &not_counted_values,
            max_label: lane_duration_label(&not_counted_values),
            selected_index: app.selected_day_index(),
            label_width,
            value_width,
            cell_width,
            color: theme.tertiary,
            theme,
        }),
    ]
}

struct DailyLane<'a> {
    label: &'static str,
    values: &'a [f64],
    max_label: String,
    selected_index: usize,
    label_width: usize,
    value_width: usize,
    cell_width: usize,
    color: Color,
    theme: &'a Theme,
}

fn daily_lane_line(lane: DailyLane<'_>) -> Line<'static> {
    let max = lane.values.iter().copied().fold(0.0_f64, f64::max).max(1.0);
    let mut spans = vec![
        Span::styled(
            widgets::fit_text(lane.label, lane.label_width),
            Style::default().fg(lane.color).bg(lane.theme.panel),
        ),
        Span::styled(" ", Style::default().bg(lane.theme.panel)),
    ];
    for (index, value) in lane.values.iter().enumerate() {
        let intensity = (*value / max).clamp(0.0, 1.0);
        let glyph = daily_lane_glyph(intensity);
        let selected = index == lane.selected_index;
        let style = Style::default()
            .fg(if selected {
                lane.theme.text
            } else {
                lane.color
            })
            .bg(if selected {
                lane.theme.selection
            } else {
                lane.theme.panel
            })
            .add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        spans.push(Span::styled(
            glyph.to_string().repeat(lane.cell_width),
            style,
        ));
    }
    spans.push(Span::styled(" ", Style::default().bg(lane.theme.panel)));
    spans.push(Span::styled(
        widgets::fit_text(&lane.max_label, lane.value_width),
        Style::default().fg(lane.theme.dim).bg(lane.theme.panel),
    ));
    Line::from(spans)
}

fn daily_lane_axis(
    days: &[DayTotals],
    label_width: usize,
    value_width: usize,
    cell_width: usize,
    theme: &Theme,
) -> Line<'static> {
    let mut axis = vec![' '; days.len().saturating_mul(cell_width)];
    if let Some(first) = days.first() {
        place_text(&mut axis, 0, &first.label);
    }
    if days.len() > 2 {
        let mid = days.len() / 2;
        place_text(&mut axis, mid.saturating_mul(cell_width), &days[mid].label);
    }
    if days.len() > 1
        && let Some(last) = days.last()
    {
        let start = days
            .len()
            .saturating_mul(cell_width)
            .saturating_sub(last.label.chars().count());
        place_text(&mut axis, start, &last.label);
    }
    Line::from(vec![
        Span::styled(
            " ".repeat(label_width + 1),
            Style::default().bg(theme.panel),
        ),
        Span::styled(
            axis.into_iter().collect::<String>(),
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
        Span::styled(
            " ".repeat(value_width + 1),
            Style::default().bg(theme.panel),
        ),
    ])
}

fn place_text(target: &mut [char], start: usize, text: &str) {
    for (offset, ch) in text.chars().enumerate() {
        if let Some(slot) = target.get_mut(start + offset) {
            *slot = ch;
        }
    }
}

fn daily_lane_glyph(intensity: f64) -> char {
    if intensity <= 0.0 {
        ' '
    } else {
        let bucket = (intensity * (DAILY_LANE_CHARS.len() as f64 - 1.0))
            .ceil()
            .clamp(1.0, DAILY_LANE_CHARS.len() as f64 - 1.0) as usize;
        DAILY_LANE_CHARS[bucket]
    }
}

fn lane_duration_label(values: &[f64]) -> String {
    let max_seconds = values.iter().copied().fold(0.0_f64, f64::max).round() as i64;
    format!("top {}", widgets::compact_duration(max_seconds))
}

fn day_not_counted_seconds(day: &DayTotals) -> i64 {
    day.idle_seconds
        .saturating_add(day.locked_seconds)
        .saturating_add(day.sleep_seconds)
        .saturating_add(day.unobserved_seconds)
}

fn render_focus_sparkline(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Daily Pattern", theme, theme.success);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let values = app
        .report()
        .daily
        .iter()
        .map(|day| day.focused_seconds.max(0) as u64)
        .collect::<Vec<_>>();
    if values.is_empty() {
        widgets::render_empty(frame, inner, "No daily focus yet", theme);
        return;
    }

    let [spark_area, caption] =
        *Layout::vertical([Constraint::Min(2), Constraint::Length(1)]).split(inner)
    else {
        return;
    };
    let max = values.iter().copied().max().unwrap_or(1).max(1);
    frame.render_widget(
        Sparkline::default()
            .data(values)
            .max(max)
            .style(Style::default().fg(theme.success).bg(theme.panel)),
        spark_area,
    );

    let first = app
        .report()
        .daily
        .first()
        .map(|day| day.label.as_str())
        .unwrap_or("");
    let last = app
        .report()
        .daily
        .last()
        .map(|day| day.label.as_str())
        .unwrap_or("");
    let selected = app
        .selected_day()
        .map(|day| format!("selected {}", day.label))
        .unwrap_or_else(|| "no day selected".to_string());
    frame.render_widget(
        Paragraph::new(widgets::fit_text(
            &format!("{first} -> {last} | {selected}"),
            caption.width as usize,
        ))
        .style(Style::default().fg(theme.dim).bg(theme.panel))
        .alignment(Alignment::Center),
        caption,
    );
}

fn render_focus_composition(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("App Mix", theme, theme.warn);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.report().apps.is_empty() {
        widgets::render_empty(frame, inner, "No focused app time for this lens", theme);
        return;
    }

    render_composition_list(frame, inner, app, theme);
}

fn render_composition_list(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let header_height = if area.height >= 7 { 2 } else { 1 };
    let [summary, list] =
        *Layout::vertical([Constraint::Length(header_height), Constraint::Min(1)]).split(area)
    else {
        return;
    };
    let mut summary_lines = vec![widgets::app_share_line(
        &app.report().apps,
        summary.width as usize,
        theme,
    )];
    if summary.height > 1 {
        let top = app.report().apps.first();
        let text = top
            .map(|row| {
                format!(
                    "Top: {} {} ({})",
                    row.label,
                    widgets::compact_duration(row.focused_seconds),
                    report::percent(row.share)
                )
            })
            .unwrap_or_else(|| "no focused app time".to_string());
        summary_lines.push(Line::from(Span::styled(
            widgets::fit_text(&text, summary.width as usize),
            Style::default().fg(theme.dim).bg(theme.panel),
        )));
    }
    frame.render_widget(
        Paragraph::new(summary_lines).style(Style::default().bg(theme.panel)),
        summary,
    );
    let label_width = if list.width < 40 { 12 } else { 16 };
    let bars = app
        .report()
        .apps
        .iter()
        .enumerate()
        .take(list.height as usize)
        .map(|(index, row)| {
            Bar::with_label(
                widgets::fit_text(&row.label, label_width),
                row.focused_seconds.max(0) as u64,
            )
            .text_value(format!(
                "{} {}",
                widgets::compact_duration(row.focused_seconds),
                report::percent(row.share)
            ))
            .style(Style::default().fg(widgets::rank_color(index, theme)))
            .value_style(Style::default().fg(theme.text))
        })
        .collect::<Vec<_>>();
    let max = app
        .report()
        .apps
        .iter()
        .map(|row| row.focused_seconds)
        .max()
        .unwrap_or(1)
        .max(1) as u64;
    let chart = BarChart::horizontal(bars)
        .max(max)
        .bar_width(1)
        .bar_gap(0)
        .bar_style(Style::default().fg(theme.warn).bg(theme.panel))
        .value_style(Style::default().fg(theme.text).bg(theme.panel))
        .label_style(Style::default().fg(theme.muted).bg(theme.panel))
        .style(Style::default().bg(theme.panel));
    frame.render_widget(chart, list);
}

fn render_peak_hours(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Top Hours", theme, theme.secondary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let hours = hour_totals(&app.data().heatmap);
    if hours.iter().all(|(_, seconds)| *seconds <= 0) {
        widgets::render_empty(frame, inner, "No hourly focus data for this period", theme);
        return;
    }

    let limit = (inner.height as usize).saturating_sub(1).clamp(1, 8);
    let max = hours
        .iter()
        .map(|(_, seconds)| *seconds)
        .max()
        .unwrap_or(1)
        .max(1) as u64;
    let bars = hours
        .iter()
        .take(limit)
        .enumerate()
        .map(|(index, (hour, seconds))| {
            Bar::with_label(hour_label(*hour), (*seconds).max(0) as u64)
                .text_value(widgets::compact_duration(*seconds))
                .style(Style::default().fg(widgets::rank_color(index, theme)))
                .value_style(Style::default().fg(theme.text))
        })
        .collect::<Vec<_>>();
    let chart = BarChart::horizontal(bars)
        .max(max)
        .bar_width(1)
        .bar_gap(0)
        .bar_style(Style::default().fg(theme.secondary).bg(theme.panel))
        .value_style(Style::default().fg(theme.text).bg(theme.panel))
        .label_style(Style::default().fg(theme.muted).bg(theme.panel))
        .style(Style::default().bg(theme.panel));
    frame.render_widget(chart, inner);
}

fn render_workspace_focus(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Workspace Focus", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.data().workspaces.is_empty() {
        widgets::render_empty(frame, inner, "No workspace focus data yet", theme);
        return;
    }

    let limit = (inner.height as usize).saturating_sub(1).clamp(1, 6);
    let bars = workspace_bars(&app.data().workspaces, limit, theme);
    let max = app
        .data()
        .workspaces
        .iter()
        .map(|row| row.focused_seconds)
        .max()
        .unwrap_or(1)
        .max(1) as u64;
    let chart = BarChart::horizontal(bars)
        .max(max)
        .bar_width(1)
        .bar_gap(0)
        .bar_style(Style::default().fg(theme.primary).bg(theme.panel))
        .value_style(Style::default().fg(theme.text).bg(theme.panel))
        .label_style(Style::default().fg(theme.muted).bg(theme.panel))
        .style(Style::default().bg(theme.panel));
    frame.render_widget(chart, inner);
}

fn render_focus_stats(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Focus Sessions", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let stats = app.stats();
    let switch_rate = if app.report().total_focused_seconds > 0 {
        stats.app_switch_count as f64 / (app.report().total_focused_seconds as f64 / 3600.0)
    } else {
        0.0
    };
    let best_day = stats
        .best_day_label
        .as_ref()
        .map(|label| {
            format!(
                "{label} - {}",
                report::format_duration(stats.best_day_seconds)
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let peak_hour = stats
        .peak_hour
        .map(|peak| {
            format!(
                "{} - {}",
                hour_label(peak.hour),
                report::format_duration(peak.focused_seconds)
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let peak_day = stats
        .peak_weekday
        .map(|peak| {
            format!(
                "{} - {}",
                weekday_label(peak.weekday),
                report::format_duration(peak.focused_seconds)
            )
        })
        .unwrap_or_else(|| "none".to_string());
    let lines = vec![
        widgets::metric_line(
            "Best day",
            &best_day,
            inner.width as usize,
            theme.warn,
            theme,
        ),
        widgets::metric_line(
            "Top hour",
            &peak_hour,
            inner.width as usize,
            theme.secondary,
            theme,
        ),
        widgets::metric_line(
            "Top weekday",
            &peak_day,
            inner.width as usize,
            theme.tertiary,
            theme,
        ),
        widgets::metric_line(
            "Avg session",
            &report::format_duration(stats.average_block_seconds),
            inner.width as usize,
            theme.success,
            theme,
        ),
        widgets::metric_line(
            "Typical",
            &report::format_duration(stats.median_block_seconds),
            inner.width as usize,
            theme.success,
            theme,
        ),
        widgets::metric_line(
            "Long sessions",
            &format!(
                "{} / {}",
                stats.deep_block_count,
                report::format_duration(stats.deep_block_seconds)
            ),
            inner.width as usize,
            theme.primary,
            theme,
        ),
        widgets::metric_line(
            "App changes",
            &format!(
                "{} ({switch_rate:.0}/h)",
                widgets::compact_count(stats.app_switch_count as i64)
            ),
            inner.width as usize,
            theme.muted,
            theme,
        ),
        widgets::metric_line(
            "Streak",
            &format!("{} days", stats.longest_streak_days),
            inner.width as usize,
            theme.text,
            theme,
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_period_signals(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Time Breakdown", theme, theme.success);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 6 {
        let lines = vec![
            widgets::metric_line(
                "Focused",
                &report::format_duration(app.report().total_focused_seconds),
                inner.width as usize,
                theme.warn,
                theme,
            ),
            widgets::metric_line(
                "Away",
                &report::format_duration(app.report().total_idle_seconds),
                inner.width as usize,
                theme.tertiary,
                theme,
            ),
            widgets::metric_line(
                "Sleep",
                &report::format_duration(app.report().total_sleep_seconds),
                inner.width as usize,
                theme.tertiary,
                theme,
            ),
            widgets::metric_line(
                "Tracker off",
                &report::format_duration(app.report().total_unobserved_seconds),
                inner.width as usize,
                theme.warn,
                theme,
            ),
        ];
        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(theme.panel)),
            inner,
        );
        return;
    }

    let [gauges, details] =
        *Layout::vertical([Constraint::Length(6), Constraint::Min(2)]).split(inner)
    else {
        return;
    };
    let signal_total = app
        .report()
        .total_focused_seconds
        .saturating_add(app.report().total_idle_seconds)
        .saturating_add(app.report().total_locked_seconds)
        .saturating_add(app.report().total_sleep_seconds)
        .saturating_add(app.report().total_unobserved_seconds)
        .max(1);
    let gauge_lines = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .split(gauges);
    frame.render_widget(
        LineGauge::default()
            .label(format!(
                "focus time {}",
                report::percent(app.report().total_focused_seconds as f64 / signal_total as f64)
            ))
            .ratio(app.report().total_focused_seconds as f64 / signal_total as f64)
            .filled_style(Style::default().fg(theme.warn))
            .unfilled_style(Style::default().fg(theme.dim))
            .style(Style::default().bg(theme.panel)),
        gauge_lines[0],
    );
    frame.render_widget(
        LineGauge::default()
            .label(format!(
                "focus time {}",
                report::percent(app.report().total_focused_seconds as f64 / signal_total as f64)
            ))
            .ratio(app.report().total_focused_seconds as f64 / signal_total as f64)
            .filled_style(Style::default().fg(theme.warn))
            .unfilled_style(Style::default().fg(theme.dim))
            .style(Style::default().bg(theme.panel)),
        gauge_lines[1],
    );
    frame.render_widget(
        LineGauge::default()
            .label(format!(
                "not counted {}",
                report::percent(
                    (app.report().total_idle_seconds
                        + app.report().total_locked_seconds
                        + app.report().total_sleep_seconds
                        + app.report().total_unobserved_seconds) as f64
                        / signal_total as f64
                )
            ))
            .ratio(
                (app.report().total_idle_seconds
                    + app.report().total_locked_seconds
                    + app.report().total_sleep_seconds
                    + app.report().total_unobserved_seconds) as f64
                    / signal_total as f64,
            )
            .filled_style(Style::default().fg(theme.tertiary))
            .unfilled_style(Style::default().fg(theme.dim))
            .style(Style::default().bg(theme.panel)),
        gauge_lines[2],
    );
    let lines = vec![
        widgets::metric_line(
            "Away total",
            &report::format_duration(app.report().total_idle_seconds),
            details.width as usize,
            theme.tertiary,
            theme,
        ),
        widgets::metric_line(
            "Locked",
            &report::format_duration(app.report().total_locked_seconds),
            details.width as usize,
            theme.danger,
            theme,
        ),
        widgets::metric_line(
            "Sleep",
            &report::format_duration(app.report().total_sleep_seconds),
            details.width as usize,
            theme.tertiary,
            theme,
        ),
        widgets::metric_line(
            "Tracker off",
            &report::format_duration(app.report().total_unobserved_seconds),
            details.width as usize,
            theme.warn,
            theme,
        ),
        widgets::metric_line(
            "Daily average",
            &report::format_duration(app.stats().daily_average_seconds),
            details.width as usize,
            theme.success,
            theme,
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        details,
    );
}

fn render_heatmap(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("When Focus Happens", theme, theme.tertiary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 32 || inner.height < 8 {
        widgets::render_empty(frame, inner, "Need more space for hourly view", theme);
        return;
    }
    let max = app
        .data()
        .heatmap
        .iter()
        .map(|cell| cell.focused_seconds)
        .max()
        .unwrap_or(0);
    if max <= 0 {
        widgets::render_empty(frame, inner, "No hourly focus data for this period", theme);
        return;
    }
    let label_width = 4_usize;
    let available = (inner.width as usize).saturating_sub(label_width).max(24);
    let cell_width = (available / 24).clamp(1, 6);
    let heat_width = cell_width * 24;
    let left_pad = available.saturating_sub(heat_width) / 2;
    let layout = HeatmapLayout {
        max,
        label_width,
        left_pad,
        cell_width,
    };
    let leading_width = layout.label_width + layout.left_pad;
    let summary_height = usize::from(inner.height >= 10);
    let row_height = 1;

    let mut rows = Vec::new();
    rows.push(heatmap_axis_line(leading_width, cell_width, theme));
    for weekday in 0..7 {
        for repeat in 0..row_height {
            rows.push(heatmap_weekday_line(
                &app.data().heatmap,
                weekday,
                layout,
                repeat == 0,
                theme,
            ));
        }
    }
    if summary_height > 0 {
        rows.push(heatmap_summary_line(&app.data().heatmap, max, theme));
    }
    frame.render_widget(
        Paragraph::new(rows)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn heatmap_axis_line(leading_width: usize, cell_width: usize, theme: &Theme) -> Line<'static> {
    let mut axis = vec![' '; cell_width * 24];
    for hour in (0..24).step_by(3) {
        let start = hour * cell_width;
        let label = format!("{hour:02}");
        for (offset, ch) in label.chars().enumerate() {
            if let Some(slot) = axis.get_mut(start + offset) {
                *slot = ch;
            }
        }
    }
    Line::from(vec![
        Span::styled(" ".repeat(leading_width), Style::default().bg(theme.panel)),
        Span::styled(
            axis.into_iter().collect::<String>(),
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
    ])
}

#[derive(Clone, Copy)]
struct HeatmapLayout {
    max: i64,
    label_width: usize,
    left_pad: usize,
    cell_width: usize,
}

fn heatmap_weekday_line(
    cells: &[FocusHeatCell],
    weekday: u32,
    layout: HeatmapLayout,
    show_label: bool,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            if show_label {
                format!("{:<3} ", WEEKDAYS[weekday as usize])
            } else {
                " ".repeat(layout.label_width)
            },
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
        Span::styled(
            " ".repeat(layout.left_pad),
            Style::default().bg(theme.panel),
        ),
    ];
    for hour in 0..24 {
        let seconds = heat_value(cells, weekday, hour);
        let intensity = seconds as f64 / layout.max as f64;
        let glyph = if seconds > 0 { '█' } else { ' ' };
        spans.push(Span::styled(
            glyph.to_string().repeat(layout.cell_width),
            Style::default()
                .fg(if seconds > 0 {
                    widgets::density_color(intensity, theme)
                } else {
                    theme.dim
                })
                .bg(theme.panel),
        ));
    }
    Line::from(spans)
}

fn heatmap_summary_line(cells: &[FocusHeatCell], max: i64, theme: &Theme) -> Line<'static> {
    let peak = cells
        .iter()
        .max_by_key(|cell| cell.focused_seconds)
        .filter(|cell| cell.focused_seconds > 0);
    let peak_label = peak
        .map(|cell| {
            format!(
                "Busiest: {} {} {}",
                weekday_label(cell.weekday),
                hour_label(cell.hour),
                report::format_duration(cell.focused_seconds)
            )
        })
        .unwrap_or_else(|| "Busiest: none".to_string());
    let max_label = format!("One-hour top {}", report::format_duration(max));
    Line::from(vec![
        Span::styled(
            "Less focus ",
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
        Span::styled("█", Style::default().fg(theme.secondary).bg(theme.panel)),
        Span::styled("█", Style::default().fg(theme.success).bg(theme.panel)),
        Span::styled("█", Style::default().fg(theme.warn).bg(theme.panel)),
        Span::styled(
            " More focus  ",
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
        Span::styled(peak_label, Style::default().fg(theme.text).bg(theme.panel)),
        Span::styled("  ", Style::default().bg(theme.panel)),
        Span::styled(max_label, Style::default().fg(theme.dim).bg(theme.panel)),
    ])
}

fn render_lens_totals(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Lens Comparison", theme, theme.secondary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let max = app
        .data()
        .lens_totals
        .iter()
        .filter_map(|row| row.map(|(focused, _)| focused))
        .max()
        .unwrap_or(1)
        .max(1) as u64;
    let bars = Lens::ALL
        .into_iter()
        .map(|lens| {
            let focused = app.data().lens_totals[lens.index()]
                .map(|row| row.0)
                .unwrap_or(0);
            Bar::with_label(lens.label(), focused.max(0) as u64)
                .text_value(widgets::compact_duration(focused))
                .style(Style::default().fg(widgets::lens_color(lens, theme)))
                .value_style(Style::default().fg(theme.text))
        })
        .collect::<Vec<_>>();
    let chart = BarChart::horizontal(bars)
        .max(max)
        .bar_style(Style::default().fg(theme.secondary).bg(theme.panel))
        .value_style(Style::default().fg(theme.text).bg(theme.panel))
        .label_style(Style::default().fg(theme.muted).bg(theme.panel))
        .style(Style::default().bg(theme.panel));
    frame.render_widget(chart, inner);
}

pub(super) fn render_apps(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    if area.width < 100 {
        render_app_table(frame, area, app, theme);
        return;
    }
    let [table, detail] =
        *Layout::horizontal([Constraint::Percentage(66), Constraint::Percentage(34)]).split(area)
    else {
        return;
    };
    render_app_table(frame, table, app, theme);
    render_app_detail(frame, detail, app, theme);
}

fn render_app_table(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    let block = widgets::panel("Apps", theme, theme.warn);
    let rows_data = app.rows().to_vec();
    let row_count = rows_data.len();
    let selected = app.selected;
    let total = app.report().total_focused_seconds.max(1);
    let rows = rows_data
        .iter()
        .enumerate()
        .map(|(index, row)| {
            Row::new(vec![
                Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(theme.dim)),
                Cell::from(app.app_label(&row.app_class)),
                Cell::from(report::format_duration(row.focused_seconds))
                    .style(Style::default().fg(theme.warn)),
                Cell::from(report::percent(widgets::ratio(row.focused_seconds, total)))
                    .style(Style::default().fg(theme.tertiary)),
            ])
            .style(Style::default().fg(theme.text).bg(theme.panel))
        })
        .collect::<Vec<_>>();
    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Min(18),
            Constraint::Length(9),
            Constraint::Length(7),
        ],
    )
    .header(
        Row::new(["#", "Application", "Focused", "Share"])
            .style(Style::default().fg(theme.muted).bg(theme.panel_alt))
            .bottom_margin(1),
    )
    .block(block)
    .column_spacing(1)
    .row_highlight_style(Style::default().fg(theme.text).bg(theme.selection))
    .highlight_symbol("▌")
    .highlight_spacing(HighlightSpacing::Always)
    .style(Style::default().bg(theme.panel));

    frame.render_stateful_widget(table, area, app.table_state());
    let visible_rows = area.height.saturating_sub(3) as usize;
    if row_count > visible_rows {
        let mut scrollbar_state = ScrollbarState::new(row_count).position(selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_style(Style::default().fg(theme.primary))
            .track_style(Style::default().fg(theme.dim));
        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn render_app_detail(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Selected App", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(row) = app.selected_row() else {
        widgets::render_empty(frame, inner, "No selected app", theme);
        return;
    };
    let share = widgets::ratio(
        row.focused_seconds,
        app.report().total_focused_seconds.max(1),
    );
    let facts = app_detail_facts(app, row);
    let [summary, spark, titles] = *Layout::vertical([
        Constraint::Length(if inner.height < 20 { 9 } else { 11 }),
        Constraint::Length(5),
        Constraint::Min(5),
    ])
    .split(inner) else {
        return;
    };
    let lines = vec![
        Line::from(Span::styled(
            widgets::fit_text(&app.app_label(&row.app_class), summary.width as usize),
            Style::default()
                .fg(theme.primary)
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        )),
        widgets::metric_line(
            "Focused",
            &report::format_duration(row.focused_seconds),
            summary.width as usize,
            theme.warn,
            theme,
        ),
        widgets::metric_line(
            "Share",
            &report::percent(share),
            summary.width as usize,
            theme.tertiary,
            theme,
        ),
        widgets::metric_line(
            "Typical hour",
            &facts.typical_hour_label,
            summary.width as usize,
            theme.tertiary,
            theme,
        ),
        widgets::metric_line(
            "Best session",
            &facts.longest_block_label,
            summary.width as usize,
            theme.success,
            theme,
        ),
        widgets::metric_line(
            "Interruptions",
            &facts.fragmentation_label,
            summary.width as usize,
            theme.warn,
            theme,
        ),
        widgets::metric_line(
            "Workspace",
            &facts.workspace_label,
            summary.width as usize,
            theme.primary,
            theme,
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.panel)),
        summary,
    );

    let spark_values =
        app_daily_values(&app.data().daily_apps, &app.report().daily, &row.app_class);
    let sparkline = Sparkline::default()
        .block(widgets::panel("Daily Sparkline", theme, theme.success))
        .data(spark_values)
        .style(Style::default().fg(theme.success).bg(theme.panel))
        .max(
            app.report()
                .daily
                .iter()
                .map(|day| day.focused_seconds.max(0) as u64)
                .max()
                .unwrap_or(1)
                .max(1),
        );
    frame.render_widget(sparkline, spark);
    render_title_list(frame, titles, app, row, theme);
}

fn render_title_list(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    selected: &AppTotals,
    theme: &Theme,
) {
    let block = widgets::panel("Title Mix", theme, theme.tertiary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let titles = app
        .data()
        .titles
        .iter()
        .filter(|title| title.app_class == selected.app_class)
        .collect::<Vec<_>>();

    if titles.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No focused titles for this app",
                Style::default().fg(theme.muted).bg(theme.panel),
            )))
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    if inner.width < 34 || inner.height < 8 {
        render_title_rows(frame, inner, &titles, theme);
        return;
    }

    let labels = titles
        .iter()
        .take(6)
        .map(|title| widgets::fit_text(&title.title, 14).trim_end().to_string())
        .collect::<Vec<_>>();
    let slices = titles
        .iter()
        .take(6)
        .enumerate()
        .zip(labels.iter())
        .map(|((index, title), label)| {
            PieSlice::new(
                label.as_str(),
                title.focused_seconds.max(0) as f64,
                widgets::rank_color(index, theme),
            )
        })
        .collect::<Vec<_>>();
    let pie = PieChart::new(slices)
        .style(Style::default().bg(theme.panel).fg(theme.text))
        .resolution(Resolution::Braille)
        .show_legend(true)
        .show_percentages(true)
        .legend_position(if inner.width < 42 {
            LegendPosition::Bottom
        } else {
            LegendPosition::Right
        })
        .legend_layout(LegendLayout::Vertical)
        .legend_marker("■")
        .pie_char('█');
    frame.render_widget(pie, inner);
}

fn render_title_rows(frame: &mut Frame<'_>, area: Rect, titles: &[&TitleTotals], theme: &Theme) {
    let lines = titles
        .iter()
        .take(area.height as usize)
        .map(|title| {
            widgets::metric_line(
                &widgets::fit_text(&title.title, 18),
                &report::format_duration(title.focused_seconds),
                area.width as usize,
                theme.text,
                theme,
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub(super) fn render_timeline(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    if area.height < 22 {
        let [canvas_area, table_area] =
            *Layout::vertical([Constraint::Length(9), Constraint::Min(6)]).split(area)
        else {
            return;
        };
        render_activity_canvas(frame, canvas_area, app, theme);
        render_interval_table(frame, table_area, app, theme);
        return;
    }

    let [canvas_area, gaps_area, table_area] = *Layout::vertical([
        Constraint::Length(10),
        Constraint::Length(5),
        Constraint::Min(7),
    ])
    .split(area) else {
        return;
    };
    render_activity_canvas(frame, canvas_area, app, theme);
    render_gap_summary(frame, gaps_area, app, theme);
    render_interval_table(frame, table_area, app, theme);
}

fn render_activity_canvas(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Activity Canvas", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let start = app.report().query_start_ts;
    let end = app.report().query_end_ts;
    if (app.data().timeline_intervals.is_empty() && app.data().system_intervals.is_empty())
        || end <= start
    {
        widgets::render_empty(frame, inner, "No intervals recorded for this period", theme);
        return;
    }
    let selected = app.selected_row().map(|row| row.app_class.as_str());
    let rows = app.rows().to_vec();
    let intervals = app.data().timeline_intervals.clone();
    let system_intervals = app.data().system_intervals.clone();
    let canvas = canvas::Canvas::default()
        .x_bounds([start as f64, end as f64])
        .y_bounds([0.0, 3.0])
        .marker(symbols::Marker::Braille)
        .background_color(theme.panel)
        .paint(move |ctx| {
            for interval in &intervals {
                let y = if interval.kind == IntervalKind::Focused {
                    2.1
                } else {
                    0.9
                };
                let rank = rows
                    .iter()
                    .position(|row| row.app_class == interval.app_class)
                    .unwrap_or(usize::MAX);
                let mut color = if interval.kind == IntervalKind::Focused {
                    widgets::rank_color(rank, theme)
                } else {
                    theme.dim
                };
                if selected == Some(interval.app_class.as_str()) {
                    color = theme.text;
                }
                ctx.draw(&canvas::Line {
                    x1: interval.started_at as f64,
                    y1: y,
                    x2: interval.ended_at as f64,
                    y2: y,
                    color,
                });
            }
            for interval in &system_intervals {
                let (y, color) = match interval.kind {
                    SystemIntervalKind::Sleep => (0.35, theme.tertiary),
                    SystemIntervalKind::Unobserved => (0.2, theme.warn),
                };
                ctx.draw(&canvas::Line {
                    x1: interval.started_at as f64,
                    y1: y,
                    x2: interval.ended_at as f64,
                    y2: y,
                    color,
                });
            }
        });
    frame.render_widget(canvas, inner);

    let span = end.saturating_sub(start);
    let labels = vec![
        Line::from(Span::styled(
            format!("{} start", timeline_label(start, span)),
            Style::default().fg(theme.dim).bg(theme.panel),
        )),
        Line::from(Span::styled(
            format!("{} end", timeline_label(end, span)),
            Style::default().fg(theme.dim).bg(theme.panel),
        )),
    ];
    let label_area = inner.inner(Margin {
        vertical: 0,
        horizontal: 1,
    });
    frame.render_widget(
        Paragraph::new(labels)
            .alignment(Alignment::Right)
            .style(Style::default().bg(theme.panel)),
        label_area,
    );
}

fn render_gap_summary(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Not Counted", theme, theme.warn);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let sleep_count = app
        .data()
        .system_intervals
        .iter()
        .filter(|interval| interval.kind == SystemIntervalKind::Sleep)
        .count();
    let unobserved_count = app
        .data()
        .system_intervals
        .iter()
        .filter(|interval| interval.kind == SystemIntervalKind::Unobserved)
        .count();
    let excluded = app
        .report()
        .total_sleep_seconds
        .saturating_add(app.report().total_unobserved_seconds);
    if excluded <= 0 {
        widgets::render_empty(
            frame,
            inner,
            "No sleep or tracker-off gaps in this period",
            theme,
        );
        return;
    }

    let signal_total = app
        .report()
        .total_focused_seconds
        .saturating_add(app.report().total_idle_seconds)
        .saturating_add(app.report().total_locked_seconds)
        .saturating_add(excluded)
        .max(1);
    let lines = vec![
        widgets::metric_line(
            "Sleep",
            &format!(
                "{} in {} intervals",
                report::format_duration(app.report().total_sleep_seconds),
                sleep_count
            ),
            inner.width as usize,
            theme.tertiary,
            theme,
        ),
        widgets::metric_line(
            "Tracker off",
            &format!(
                "{} in {} intervals",
                report::format_duration(app.report().total_unobserved_seconds),
                unobserved_count
            ),
            inner.width as usize,
            theme.warn,
            theme,
        ),
        widgets::metric_line(
            "Not counted",
            &format!(
                "{} ({})",
                report::format_duration(excluded),
                report::percent(excluded as f64 / signal_total as f64)
            ),
            inner.width as usize,
            theme.warn,
            theme,
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_interval_table(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Intervals", theme, theme.secondary);
    let mut intervals = app
        .data()
        .timeline_intervals
        .iter()
        .cloned()
        .map(TimelineRow::App)
        .chain(
            app.data()
                .system_intervals
                .iter()
                .cloned()
                .map(TimelineRow::System),
        )
        .collect::<Vec<_>>();
    intervals.sort_by_key(|interval| (interval.started_at(), interval.ended_at()));
    let span = app
        .report()
        .query_end_ts
        .saturating_sub(app.report().query_start_ts);
    let rows = intervals
        .into_iter()
        .rev()
        .map(|interval| match interval {
            TimelineRow::App(interval) => interval_row(interval, span, app, theme),
            TimelineRow::System(interval) => system_interval_row(interval, span, theme),
        })
        .collect::<Vec<_>>();
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Min(18),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(["Time", "Kind", "Application", "Duration"])
            .style(Style::default().fg(theme.muted).bg(theme.panel_alt))
            .bottom_margin(1),
    )
    .block(block)
    .column_spacing(1)
    .style(Style::default().bg(theme.panel));
    frame.render_widget(table, area);
}

#[derive(Debug, Clone)]
enum TimelineRow {
    App(TimelineInterval),
    System(SystemTimelineInterval),
}

impl TimelineRow {
    fn started_at(&self) -> i64 {
        match self {
            Self::App(interval) => interval.started_at,
            Self::System(interval) => interval.started_at,
        }
    }

    fn ended_at(&self) -> i64 {
        match self {
            Self::App(interval) => interval.ended_at,
            Self::System(interval) => interval.ended_at,
        }
    }
}

fn interval_row(
    interval: TimelineInterval,
    span_seconds: i64,
    app: &App,
    theme: &Theme,
) -> Row<'static> {
    let rank = app
        .rows()
        .iter()
        .position(|row| row.app_class == interval.app_class)
        .unwrap_or(usize::MAX);
    let color = if interval.kind == IntervalKind::Focused {
        widgets::rank_color(rank, theme)
    } else {
        theme.dim
    };
    let kind = match interval.kind {
        IntervalKind::Focused => "focus",
        IntervalKind::Open => "open",
    };
    Row::new(vec![
        Cell::from(format!(
            "{}-{}",
            interval_time_label(interval.started_at, span_seconds),
            interval_time_label(interval.ended_at, span_seconds)
        ))
        .style(Style::default().fg(theme.dim)),
        Cell::from(kind).style(Style::default().fg(color)),
        Cell::from(app.app_label(&interval.app_class)),
        Cell::from(widgets::compact_duration(
            interval.ended_at.saturating_sub(interval.started_at),
        ))
        .style(Style::default().fg(theme.muted)),
    ])
    .style(Style::default().fg(theme.text).bg(theme.panel))
}

fn system_interval_row(
    interval: SystemTimelineInterval,
    span_seconds: i64,
    theme: &Theme,
) -> Row<'static> {
    let (kind, source, color) = match interval.kind {
        SystemIntervalKind::Sleep => (
            "sleep",
            system_interval_source("System sleep", interval.source.as_deref()),
            theme.tertiary,
        ),
        SystemIntervalKind::Unobserved => (
            "tracker",
            system_interval_source("Tracker off gap", interval.source.as_deref()),
            theme.warn,
        ),
    };
    Row::new(vec![
        Cell::from(format!(
            "{}-{}",
            interval_time_label(interval.started_at, span_seconds),
            interval_time_label(interval.ended_at, span_seconds)
        ))
        .style(Style::default().fg(theme.dim)),
        Cell::from(kind).style(Style::default().fg(color)),
        Cell::from(source).style(Style::default().fg(color)),
        Cell::from(widgets::compact_duration(
            interval.ended_at.saturating_sub(interval.started_at),
        ))
        .style(Style::default().fg(theme.muted)),
    ])
    .style(Style::default().fg(theme.text).bg(theme.panel))
}

fn system_interval_source(label: &str, source: Option<&str>) -> String {
    source
        .filter(|source| !source.trim().is_empty())
        .map(|source| {
            format!(
                "{label} ({})",
                widgets::fit_text(source.trim(), 18).trim_end()
            )
        })
        .unwrap_or_else(|| label.to_string())
}

pub(super) fn render_system(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    let [health, right] =
        *Layout::horizontal([Constraint::Percentage(54), Constraint::Percentage(46)]).split(area)
    else {
        return;
    };
    let [lenses, signals] =
        *Layout::vertical([Constraint::Percentage(48), Constraint::Percentage(52)]).split(right)
    else {
        return;
    };
    render_system_health(frame, health, app, theme);
    render_lens_totals(frame, lenses, app, theme);
    render_period_signals(frame, signals, app, theme);
}

fn render_system_health(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("System Health", theme, theme.success);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [status, gauges, details] = *Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Min(5),
    ])
    .split(inner) else {
        return;
    };
    let service_color = if app.health().service_state == "active" {
        theme.success
    } else if app.health().service_state == "unknown" {
        theme.muted
    } else {
        theme.danger
    };
    let socket_color = if app.health().socket_state == "ipc ok" {
        theme.success
    } else {
        theme.warn
    };
    let lines = vec![
        widgets::metric_line(
            "Daemon",
            &app.health().service_state,
            status.width as usize,
            service_color,
            theme,
        ),
        widgets::metric_line(
            "Hyprland",
            &app.health().socket_state,
            status.width as usize,
            socket_color,
            theme,
        ),
        widgets::metric_line(
            "Last event",
            &app.health().last_event_label(),
            status.width as usize,
            theme.muted,
            theme,
        ),
        widgets::metric_line(
            "Heartbeat",
            &app.health().last_heartbeat_label(),
            status.width as usize,
            theme.muted,
            theme,
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.panel)),
        status,
    );

    let active_total = app.health().storage.focused_active
        + app.health().storage.idle_active
        + app.health().storage.locked_active
        + app.health().storage.sleep_active
        + app.health().storage.daemon_active;
    let focus_ratio = widgets::ratio(
        app.report().total_focused_seconds,
        app.report().total_elapsed_seconds,
    );
    let gauge_lines = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .split(gauges);
    frame.render_widget(
        LineGauge::default()
            .label(format!("focused elapsed {}", report::percent(focus_ratio)))
            .ratio(focus_ratio)
            .filled_style(Style::default().fg(theme.warn))
            .unfilled_style(Style::default().fg(theme.dim))
            .style(Style::default().bg(theme.panel)),
        gauge_lines[0],
    );
    frame.render_widget(
        LineGauge::default()
            .label(format!("active intervals {active_total}"))
            .ratio(widgets::ratio(active_total, 6))
            .filled_style(Style::default().fg(theme.primary))
            .unfilled_style(Style::default().fg(theme.dim))
            .style(Style::default().bg(theme.panel)),
        gauge_lines[1],
    );
    frame.render_widget(
        LineGauge::default()
            .label(format!("rows {}", app.health().storage.interval_count))
            .ratio(widgets::ratio(app.health().storage.interval_count, 10_000))
            .filled_style(Style::default().fg(theme.secondary))
            .unfilled_style(Style::default().fg(theme.dim))
            .style(Style::default().bg(theme.panel)),
        gauge_lines[2],
    );

    let lines = vec![
        widgets::metric_line(
            "Live",
            &app.health().live_label(),
            details.width as usize,
            theme.primary,
            theme,
        ),
        widgets::metric_line(
            "Away",
            &report::format_duration(app.report().total_idle_seconds),
            details.width as usize,
            theme.secondary,
            theme,
        ),
        widgets::metric_line(
            "Locked",
            &report::format_duration(app.report().total_locked_seconds),
            details.width as usize,
            theme.tertiary,
            theme,
        ),
        widgets::metric_line(
            "Sleep",
            &report::format_duration(app.report().total_sleep_seconds),
            details.width as usize,
            theme.tertiary,
            theme,
        ),
        widgets::metric_line(
            "Tracker off",
            &report::format_duration(app.report().total_unobserved_seconds),
            details.width as usize,
            theme.warn,
            theme,
        ),
        widgets::metric_line(
            "Loaded",
            &app.loaded_at().format("%Y-%m-%d %H:%M:%S").to_string(),
            details.width as usize,
            theme.muted,
            theme,
        ),
    ];
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.panel)),
        details,
    );
}

pub(super) fn render_help(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(" Help ")
        .border_style(Style::default().fg(theme.primary).bg(theme.panel))
        .style(Style::default().bg(theme.panel));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = vec![
        Line::from(vec![
            Span::styled("Tab / Shift-Tab", Style::default().fg(theme.primary)),
            Span::styled("  switch dashboard view", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("h / l or ← / →", Style::default().fg(theme.primary)),
            Span::styled("  change lens", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("[ / ]", Style::default().fg(theme.primary)),
            Span::styled("  previous / next period", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("j / k or ↑ / ↓", Style::default().fg(theme.primary)),
            Span::styled(
                "  inspect day or move selection",
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("1-5", Style::default().fg(theme.primary)),
            Span::styled(
                "  day week month year life",
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("p", Style::default().fg(theme.primary)),
            Span::styled(
                "  switch overview detail panel",
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("r", Style::default().fg(theme.primary)),
            Span::styled("  refresh data", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("q / Esc", Style::default().fg(theme.primary)),
            Span::styled("  close help or quit", Style::default().fg(theme.text)),
        ]),
        Line::from(Span::styled(
            format!("Current: {} / {}", app.view().label(), app.lens().label()),
            Style::default().fg(theme.dim),
        )),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(Style::default().fg(theme.text).bg(theme.panel))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true }),
        inner,
    );
}

#[derive(Debug, Clone, Copy)]
enum InsightEntry {
    Header {
        category: InsightCategory,
        count: usize,
    },
    Insight {
        index: usize,
    },
}

impl InsightEntry {
    fn insight_index(self) -> Option<usize> {
        match self {
            Self::Header { .. } => None,
            Self::Insight { index } => Some(index),
        }
    }
}

fn insight_entries(insights: &[Insight]) -> Vec<InsightEntry> {
    let mut entries = Vec::new();
    for category in INSIGHT_CATEGORIES {
        let count = insights
            .iter()
            .filter(|insight| insight.category == category)
            .count();
        if count == 0 {
            continue;
        }
        entries.push(InsightEntry::Header { category, count });
        entries.extend(
            insights
                .iter()
                .enumerate()
                .filter(|(_, insight)| insight.category == category)
                .map(|(index, _)| InsightEntry::Insight { index }),
        );
    }
    entries
}

fn insight_category_label(category: InsightCategory) -> &'static str {
    match category {
        InsightCategory::Patterns => "Patterns",
        InsightCategory::FocusQuality => "Focus",
        InsightCategory::Apps => "Apps",
        InsightCategory::SystemSignals => "System",
    }
}

fn insight_display_title(insight: &Insight) -> String {
    match insight.kind {
        InsightKind::TopApp => "Top app".to_string(),
        InsightKind::DayComparison => "Compared with yesterday".to_string(),
        InsightKind::PeriodComparison => "Compared with last period".to_string(),
        InsightKind::BestDay => "Best day".to_string(),
        InsightKind::WorstActiveDay => "Lightest day".to_string(),
        InsightKind::CurrentStreak => "Current streak".to_string(),
        InsightKind::LongestStreak => "Best streak".to_string(),
        InsightKind::PeakFocusHour => "Top hour".to_string(),
        InsightKind::PeakFocusWeekday => "Top weekday".to_string(),
        InsightKind::DeepWorkBlocks => "Long sessions".to_string(),
        InsightKind::AppSwitchRate => "App changes".to_string(),
        InsightKind::FragmentedApp => "Most interrupted app".to_string(),
        InsightKind::FocusDensity => "Focus share".to_string(),
        InsightKind::AppFocusDensity if insight.title.to_lowercase().contains("lowest") => {
            "Lowest focus share app".to_string()
        }
        InsightKind::AppFocusDensity => "Highest focus share app".to_string(),
        InsightKind::EffectiveApps => "Focus spread".to_string(),
        InsightKind::StrongestWorkspace => "Top workspace".to_string(),
        InsightKind::WorkspaceAppAffinity => "Workspace pairing".to_string(),
        InsightKind::IdleExcluded => "Away time".to_string(),
        InsightKind::LockedExcluded => "Locked time".to_string(),
        InsightKind::SleepExcluded => "Sleep time".to_string(),
        InsightKind::UnobservedExcluded => "Tracker off time".to_string(),
        InsightKind::ExcludedImpact => "Not counted time".to_string(),
        InsightKind::FocusAnomaly => "Unusual focus time".to_string(),
        InsightKind::AppAnomaly => "Unusual app time".to_string(),
        InsightKind::HourAnomaly => "Unusual hour".to_string(),
        InsightKind::UnobservedAnomaly => "Tracker off gap".to_string(),
    }
}

fn insight_display_explanation(insight: &Insight) -> String {
    match insight.kind {
        InsightKind::TopApp => {
            "The app with the largest share of focused time in this period.".to_string()
        }
        InsightKind::DayComparison => {
            "Compares today's focus time with yesterday.".to_string()
        }
        InsightKind::PeriodComparison => {
            "Compares this period with the previous matching period.".to_string()
        }
        InsightKind::WorstActiveDay => {
            "Shows the active day with the least focused time in this period.".to_string()
        }
        InsightKind::DeepWorkBlocks => {
            "Counts long focus sessions at or above your long session threshold.".to_string()
        }
        InsightKind::AppSwitchRate => {
            "Counts how often focus moved from one app to another.".to_string()
        }
        InsightKind::FragmentedApp => {
            "Shows the app with the shortest typical focus sessions.".to_string()
        }
        InsightKind::FocusDensity => "Compares focus with the surrounding tracked time.".to_string(),
        InsightKind::AppFocusDensity => {
            "Uses open-time telemetry as a background signal for app focus facts.".to_string()
        }
        InsightKind::EffectiveApps => {
            "Estimates how broadly your focus was spread across apps.".to_string()
        }
        InsightKind::IdleExcluded => "Away time was not counted as focus.".to_string(),
        InsightKind::LockedExcluded => "Locked-screen time was not counted as focus.".to_string(),
        InsightKind::SleepExcluded => "Sleep time was not counted as focus.".to_string(),
        InsightKind::UnobservedExcluded => {
            "Tracker off time was not counted as focus.".to_string()
        }
        InsightKind::ExcludedImpact => {
            "Shows how much time was left out because it was away, locked, sleep, or tracker off time."
                .to_string()
        }
        InsightKind::UnobservedAnomaly => {
            "Flags a tracker off gap that is larger than usual.".to_string()
        }
        _ => insight.explanation.clone(),
    }
}

fn insight_tone_label(tone: InsightTone) -> &'static str {
    match tone {
        InsightTone::Positive => "positive",
        InsightTone::Negative => "negative",
        InsightTone::Neutral => "neutral",
        InsightTone::Info => "info",
        InsightTone::Caution => "caution",
    }
}

fn insight_tone_marker(tone: InsightTone) -> &'static str {
    match tone {
        InsightTone::Positive => "+",
        InsightTone::Negative => "-",
        InsightTone::Neutral => "-",
        InsightTone::Info => "i",
        InsightTone::Caution => "!",
    }
}

fn insight_tone_color(tone: InsightTone, theme: &Theme) -> Color {
    match tone {
        InsightTone::Positive => theme.success,
        InsightTone::Negative => theme.danger,
        InsightTone::Neutral => theme.muted,
        InsightTone::Info => theme.secondary,
        InsightTone::Caution => theme.warn,
    }
}

fn insight_confidence_label(confidence: InsightConfidence) -> &'static str {
    match confidence {
        InsightConfidence::Low => "low confidence",
        InsightConfidence::Medium => "medium confidence",
        InsightConfidence::High => "high confidence",
    }
}

fn insight_support_lines(
    support: &InsightSupport,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    push_optional_metric(
        &mut lines,
        "Period",
        support.period_label.as_deref(),
        width,
        theme.primary,
        theme,
    );
    push_optional_metric(
        &mut lines,
        "App",
        support.app_label.as_deref(),
        width,
        theme.warn,
        theme,
    );
    push_optional_metric(
        &mut lines,
        "Workspace",
        support.workspace.as_deref(),
        width,
        theme.primary,
        theme,
    );
    push_optional_metric(
        &mut lines,
        "Date",
        support.date_label.as_deref().or(support.date.as_deref()),
        width,
        theme.secondary,
        theme,
    );
    push_optional_metric(
        &mut lines,
        "Compare",
        support
            .comparison_label
            .as_deref()
            .or(support.comparison_date.as_deref()),
        width,
        theme.secondary,
        theme,
    );
    push_optional_metric(
        &mut lines,
        "Hour",
        support.hour_label.as_deref(),
        width,
        theme.tertiary,
        theme,
    );
    push_optional_metric(
        &mut lines,
        "Weekday",
        support.weekday_label.as_deref(),
        width,
        theme.tertiary,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Focused",
        support.focused_seconds,
        width,
        theme.warn,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Baseline",
        support.comparison_seconds,
        width,
        theme.muted,
        theme,
    );
    if let Some(delta) = support.delta_seconds {
        lines.push(widgets::metric_line(
            "Delta",
            &format_signed_duration(delta),
            width,
            if delta >= 0 {
                theme.success
            } else {
                theme.danger
            },
            theme,
        ));
    }
    if let Some(share) = support.share {
        lines.push(widgets::metric_line(
            "Share",
            &report::percent(share),
            width,
            theme.tertiary,
            theme,
        ));
    }
    if let Some(app_count) = support.app_count {
        lines.push(widgets::metric_line(
            "Apps",
            &app_count.to_string(),
            width,
            theme.primary,
            theme,
        ));
    }
    if let Some(effective_app_count) = support.effective_app_count {
        lines.push(widgets::metric_line(
            "Focus spread",
            &format!("{effective_app_count:.1} apps"),
            width,
            theme.primary,
            theme,
        ));
    }
    if let Some(block_count) = support.block_count {
        lines.push(widgets::metric_line(
            "Sessions",
            &block_count.to_string(),
            width,
            theme.success,
            theme,
        ));
    }
    if let Some(switch_count) = support.switch_count {
        lines.push(widgets::metric_line(
            "App changes",
            &switch_count.to_string(),
            width,
            theme.warn,
            theme,
        ));
    }
    if let Some(rate_per_hour) = support.rate_per_hour {
        lines.push(widgets::metric_line(
            "Rate",
            &format!("{rate_per_hour:.1}/h"),
            width,
            theme.warn,
            theme,
        ));
    }
    if let Some(current_streak_days) = support.current_streak_days {
        lines.push(widgets::metric_line(
            "Current",
            &format!("{current_streak_days} days"),
            width,
            theme.success,
            theme,
        ));
    }
    if let Some(longest_streak_days) = support.longest_streak_days {
        lines.push(widgets::metric_line(
            "Longest",
            &format!("{longest_streak_days} days"),
            width,
            theme.success,
            theme,
        ));
    }
    push_duration_metric(
        &mut lines,
        "Total",
        support.total_seconds,
        width,
        theme.success,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Typical",
        support.median_seconds,
        width,
        theme.success,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Longest",
        support.longest_seconds,
        width,
        theme.success,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Threshold",
        support.threshold_seconds,
        width,
        theme.dim,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Baseline",
        support.baseline_seconds,
        width,
        theme.dim,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Not counted",
        support.excluded_seconds,
        width,
        theme.warn,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Away",
        support.idle_seconds,
        width,
        theme.tertiary,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Locked",
        support.locked_seconds,
        width,
        theme.danger,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Sleep",
        support.sleep_seconds,
        width,
        theme.tertiary,
        theme,
    );
    push_duration_metric(
        &mut lines,
        "Tracker off",
        support.unobserved_seconds,
        width,
        theme.warn,
        theme,
    );
    lines
}

fn push_optional_metric(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    value: Option<&str>,
    width: usize,
    color: Color,
    theme: &Theme,
) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        lines.push(widgets::metric_line(label, value, width, color, theme));
    }
}

fn push_duration_metric(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    seconds: Option<i64>,
    width: usize,
    color: Color,
    theme: &Theme,
) {
    if let Some(seconds) = seconds {
        lines.push(widgets::metric_line(
            label,
            &report::format_duration(seconds),
            width,
            color,
            theme,
        ));
    }
}

fn format_signed_duration(seconds: i64) -> String {
    if seconds > 0 {
        format!("+{}", report::format_duration(seconds))
    } else if seconds < 0 {
        format!("-{}", report::format_duration(seconds.saturating_abs()))
    } else {
        report::format_duration(0)
    }
}

fn heat_value(cells: &[FocusHeatCell], weekday: u32, hour: u32) -> i64 {
    cells
        .iter()
        .find(|cell| cell.weekday == weekday && cell.hour == hour)
        .map(|cell| cell.focused_seconds)
        .unwrap_or(0)
}

fn hour_totals(cells: &[FocusHeatCell]) -> Vec<(u32, i64)> {
    let mut totals = [0_i64; 24];
    for cell in cells {
        if let Some(total) = totals.get_mut(cell.hour as usize) {
            *total += cell.focused_seconds.max(0);
        }
    }
    let mut rows = totals
        .into_iter()
        .enumerate()
        .map(|(hour, focused_seconds)| (hour as u32, focused_seconds))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows
}

fn hour_label(hour: u32) -> String {
    format!("{:02}:00", hour.min(23))
}

fn weekday_label(weekday: u32) -> &'static str {
    WEEKDAYS.get(weekday as usize).copied().unwrap_or("--")
}

fn workspace_bars<'a>(
    workspaces: &'a [WorkspaceTotals],
    limit: usize,
    theme: &Theme,
) -> Vec<Bar<'a>> {
    workspaces
        .iter()
        .take(limit)
        .enumerate()
        .map(|(index, row)| {
            Bar::with_label(
                widgets::fit_text(&row.workspace, 12),
                row.focused_seconds.max(0) as u64,
            )
            .text_value(widgets::compact_duration(row.focused_seconds))
            .style(Style::default().fg(widgets::rank_color(index, theme)))
            .value_style(Style::default().fg(theme.text))
        })
        .collect()
}

fn app_daily_values(daily_apps: &[AppDayTotals], days: &[DayTotals], app_class: &str) -> Vec<u64> {
    days.iter()
        .map(|day| {
            daily_apps
                .iter()
                .filter(|row| row.app_class == app_class && row.date == day.date)
                .map(|row| row.focused_seconds.max(0) as u64)
                .sum()
        })
        .collect()
}

#[derive(Debug, Clone)]
struct AppDetailFacts {
    typical_hour_label: String,
    longest_block_label: String,
    fragmentation_label: String,
    workspace_label: String,
}

fn app_detail_facts(app: &App, row: &AppTotals) -> AppDetailFacts {
    let focus_intervals = app
        .data()
        .timeline_intervals
        .iter()
        .filter(|interval| {
            interval.kind == IntervalKind::Focused && interval.app_class == row.app_class
        })
        .collect::<Vec<_>>();
    let typical_hour_label = app_typical_hour_label(&focus_intervals);
    let longest_block_seconds = focus_intervals
        .iter()
        .map(|interval| interval.ended_at.saturating_sub(interval.started_at))
        .max()
        .unwrap_or_default();
    let longest_block_label = if longest_block_seconds > 0 {
        report::format_duration(longest_block_seconds)
    } else {
        "none".to_string()
    };
    let block_count = focus_intervals.len();
    let blocks_per_hour = if row.focused_seconds > 0 {
        block_count as f64 / (row.focused_seconds as f64 / 3600.0)
    } else {
        0.0
    };
    let fragmentation_label = if block_count > 0 {
        format!("{block_count} sessions ({blocks_per_hour:.1}/h)")
    } else {
        "none".to_string()
    };
    let workspace_label = app_workspace_label(app, row);

    AppDetailFacts {
        typical_hour_label,
        longest_block_label,
        fragmentation_label,
        workspace_label,
    }
}

fn app_typical_hour_label(intervals: &[&TimelineInterval]) -> String {
    let mut totals = [0_i64; 24];
    for interval in intervals {
        add_hourly_overlap(&mut totals, interval.started_at, interval.ended_at);
    }
    totals
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(&left.0)))
        .filter(|(_, seconds)| **seconds > 0)
        .map(|(hour, seconds)| {
            format!(
                "{} ({})",
                hour_label(hour as u32),
                report::format_duration(*seconds)
            )
        })
        .unwrap_or_else(|| "none".to_string())
}

fn add_hourly_overlap(totals: &mut [i64; 24], started_at: i64, ended_at: i64) {
    let mut cursor = started_at;
    while cursor < ended_at {
        let Some(local) = Local.timestamp_opt(cursor, 0).single() else {
            break;
        };
        let Some(hour_start) = Local
            .with_ymd_and_hms(local.year(), local.month(), local.day(), local.hour(), 0, 0)
            .single()
        else {
            break;
        };
        let next_hour = (hour_start + chrono::Duration::hours(1)).timestamp();
        let segment_end = next_hour.min(ended_at);
        let overlap = segment_end.saturating_sub(cursor);
        if overlap > 0
            && let Some(total) = totals.get_mut(local.hour() as usize)
        {
            *total += overlap;
        }
        cursor = segment_end.max(cursor + 1);
    }
}

fn app_workspace_label(app: &App, row: &AppTotals) -> String {
    let matches = app
        .data()
        .app_workspaces
        .iter()
        .filter(|workspace| workspace.app_class == row.app_class)
        .collect::<Vec<_>>();
    let total = matches
        .iter()
        .map(|workspace| workspace.focused_seconds.max(0))
        .sum::<i64>();
    let Some(best) = matches
        .into_iter()
        .max_by(|left, right| {
            left.focused_seconds
                .cmp(&right.focused_seconds)
                .then_with(|| right.workspace.cmp(&left.workspace))
        })
        .filter(|workspace| workspace.focused_seconds > 0)
    else {
        return "none".to_string();
    };
    let share = widgets::ratio(best.focused_seconds, total.max(1));
    format!(
        "{} ({})",
        widgets::fit_text(&best.workspace, 14).trim_end(),
        report::percent(share)
    )
}

fn timeline_label(timestamp: i64, span_seconds: i64) -> String {
    if span_seconds >= 2 * 86400 {
        return Local
            .timestamp_opt(timestamp, 0)
            .single()
            .map(|time| time.format("%b %-d %H:%M").to_string())
            .unwrap_or_else(|| "--".to_string());
    }

    widgets::format_clock(timestamp)
}

fn interval_time_label(timestamp: i64, span_seconds: i64) -> String {
    if span_seconds >= 2 * 86400 {
        return Local
            .timestamp_opt(timestamp, 0)
            .single()
            .map(|time| time.format("%m/%d %H:%M").to_string())
            .unwrap_or_else(|| "--/-- --:--".to_string());
    }

    widgets::format_clock(timestamp)
}
