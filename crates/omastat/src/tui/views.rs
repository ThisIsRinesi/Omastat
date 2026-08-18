use super::{
    app::{App, View},
    theme::Theme,
    widgets,
};
use crate::{
    report::{self, Lens},
    storage::{AppDayTotals, AppTotals, DayTotals, FocusHeatCell, IntervalKind, TimelineInterval},
};
use chrono::Local;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{
        Axis, Bar, BarChart, Block, Cell, Chart, Clear, Dataset, Gauge, GraphType,
        HighlightSpacing, LineGauge, Paragraph, Row, Sparkline, Table, Wrap, canvas,
    },
};

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const HEAT_CHARS: [char; 8] = [' ', '·', '░', '▒', '▓', '█', '▇', '▉'];

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
        Local::now().format("%H:%M").to_string()
    } else {
        Local::now().format("%H:%M:%S").to_string()
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
        View::Overview => "Tab view  h/l lens  [/] period  p trends  ? help  q quit",
        View::Apps => "j/k select  PgUp/PgDn jump  h/l lens  [/] period  ? help  q quit",
        View::Timeline => "j/k highlight app  h/l lens  [/] period  ? help  q quit",
        View::System => "Tab view  h/l lens  r refresh  ? help  q quit",
    };
    let status = format!(
        "[{} / {} / {period_hint}]",
        app.view().label(),
        app.lens().label()
    );
    let right = format!("5s auto {status}");
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
        let [kpis, flow, apps, trends] = *Layout::vertical([
            Constraint::Length(4),
            Constraint::Length(7),
            Constraint::Length(6),
            Constraint::Min(3),
        ])
        .split(area) else {
            return;
        };
        render_kpis(frame, kpis, app, theme);
        render_focus_sparkline(frame, flow, app, theme);
        render_app_share(frame, apps, app, theme);
        render_insights(frame, trends, app, theme);
        return;
    }

    let [kpis, body] = *Layout::vertical([Constraint::Length(4), Constraint::Min(10)]).split(area)
    else {
        return;
    };
    let [left, right] =
        *Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).split(body)
    else {
        return;
    };
    let [flow, lower_left] =
        *Layout::vertical([Constraint::Percentage(58), Constraint::Percentage(42)]).split(left)
    else {
        return;
    };
    let [apps, lenses] =
        *Layout::vertical([Constraint::Percentage(62), Constraint::Percentage(38)]).split(right)
    else {
        return;
    };
    let [heat, insights] =
        *Layout::horizontal([Constraint::Percentage(54), Constraint::Percentage(46)])
            .split(lower_left)
    else {
        return;
    };

    render_kpis(frame, kpis, app, theme);
    render_focus_chart(frame, flow, app, theme);
    render_heatmap(frame, heat, app, theme);
    if app.show_trends() {
        render_insights(frame, insights, app, theme);
    } else {
        render_density(frame, insights, app, theme);
    }
    render_app_share(frame, apps, app, theme);
    render_lens_totals(frame, lenses, app, theme);
}

fn render_kpis(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let report = app.report();
    let density = widgets::ratio(report.total_focused_seconds, report.total_open_seconds);
    let app_count = report
        .rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .count();
    let chunks = Layout::horizontal([
        Constraint::Percentage(28),
        Constraint::Percentage(24),
        Constraint::Percentage(18),
        Constraint::Percentage(30),
    ])
    .spacing(1)
    .split(area);
    render_kpi(
        frame,
        chunks[0],
        "Focused",
        &report::format_duration(report.total_focused_seconds),
        "active work",
        theme.warn,
        theme,
    );
    render_kpi(
        frame,
        chunks[1],
        "Open",
        &report::format_duration(report.total_open_seconds),
        "visible apps",
        theme.secondary,
        theme,
    );
    render_kpi(
        frame,
        chunks[2],
        "Density",
        &report::percent(density),
        "focus/open",
        widgets::density_color(density, theme),
        theme,
    );
    render_kpi(
        frame,
        chunks[3],
        "Coverage",
        &format!("{app_count} apps"),
        &app.report().period.label,
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
    let block = widgets::panel("Focus Flow", theme, theme.success);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.report().daily.is_empty() {
        widgets::render_empty(frame, inner, "No daily focus yet", theme);
        return;
    }
    let focused = daily_points(&app.report().daily, |day| day.focused_seconds);
    let open = daily_points(&app.report().daily, |day| day.open_seconds);
    let max = app
        .report()
        .daily
        .iter()
        .flat_map(|day| [day.focused_seconds, day.open_seconds])
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let labels = chart_labels(&app.report().daily);
    let datasets = vec![
        Dataset::default()
            .name("focus")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Area)
            .style(Style::default().fg(theme.warn))
            .data(&focused)
            .fill_to_y(0.0),
        Dataset::default()
            .name("open")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.secondary))
            .data(&open),
    ];
    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .style(Style::default().fg(theme.dim).bg(theme.panel))
                .bounds([0.0, focused.len().saturating_sub(1).max(1) as f64])
                .labels(labels),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(theme.dim).bg(theme.panel))
                .bounds([0.0, max])
                .labels([
                    Line::from("0"),
                    Line::from(widgets::compact_duration(max as i64)),
                ]),
        )
        .legend_position(None)
        .style(Style::default().bg(theme.panel));
    frame.render_widget(chart, inner);
}

fn render_focus_sparkline(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Focus Spark", theme, theme.success);
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
    frame.render_widget(
        Paragraph::new(format!("{first} -> {last}"))
            .style(Style::default().fg(theme.dim).bg(theme.panel))
            .alignment(Alignment::Center),
        caption,
    );
}

fn render_app_share(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("App Share", theme, theme.warn);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.report().apps.is_empty() {
        widgets::render_empty(frame, inner, "No focused app time for this lens", theme);
        return;
    }
    if inner.height < 7 {
        frame.render_widget(
            Paragraph::new(vec![widgets::app_share_line(
                &app.report().apps,
                inner.width as usize,
                theme,
            )])
            .style(Style::default().bg(theme.panel)),
            inner,
        );
        return;
    }
    let bars = app
        .report()
        .apps
        .iter()
        .enumerate()
        .take(inner.height.saturating_sub(1) as usize)
        .map(|(index, row)| {
            Bar::with_label(
                widgets::fit_text(&row.label, 16),
                row.focused_seconds.max(0) as u64,
            )
            .text_value(report::format_duration(row.focused_seconds))
            .style(Style::default().fg(widgets::rank_color(index, theme)))
            .value_style(Style::default().fg(theme.text))
        })
        .collect::<Vec<_>>();
    let chart = BarChart::horizontal(bars)
        .bar_style(Style::default().fg(theme.warn).bg(theme.panel))
        .value_style(Style::default().fg(theme.text).bg(theme.panel))
        .label_style(Style::default().fg(theme.muted).bg(theme.panel))
        .style(Style::default().bg(theme.panel));
    frame.render_widget(chart, inner);
}

fn render_heatmap(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Focus Heat", theme, theme.tertiary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 32 || inner.height < 5 {
        widgets::render_empty(frame, inner, "Need more space for heatmap", theme);
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
    let mut rows = Vec::new();
    rows.push(Line::from(vec![
        Span::styled("    ", Style::default().bg(theme.panel)),
        Span::styled(
            "00 03 06 09 12 15 18 21",
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
    ]));
    for weekday in 0..7 {
        let mut spans = vec![Span::styled(
            format!("{:<3} ", WEEKDAYS[weekday as usize]),
            Style::default().fg(theme.dim).bg(theme.panel),
        )];
        for hour in 0..24 {
            let seconds = heat_value(&app.data().heatmap, weekday, hour);
            let bucket = ((seconds as f64 / max as f64) * (HEAT_CHARS.len() as f64 - 1.0))
                .round()
                .clamp(0.0, HEAT_CHARS.len() as f64 - 1.0) as usize;
            spans.push(Span::styled(
                HEAT_CHARS[bucket].to_string(),
                Style::default()
                    .fg(if seconds > 0 {
                        widgets::density_color(seconds as f64 / max as f64, theme)
                    } else {
                        theme.dim
                    })
                    .bg(theme.panel),
            ));
        }
        rows.push(Line::from(spans));
    }
    frame.render_widget(
        Paragraph::new(rows)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_insights(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Trends", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = if app.report().insights.is_empty() {
        vec![Line::from(Span::styled(
            "No trends yet for this lens",
            Style::default().fg(theme.muted).bg(theme.panel),
        ))]
    } else {
        app.report()
            .insights
            .iter()
            .map(|row| {
                widgets::metric_line(
                    &row.label,
                    &row.value,
                    inner.width as usize,
                    theme.text,
                    theme,
                )
            })
            .collect::<Vec<_>>()
    };
    if lines.len() < inner.height as usize {
        lines.push(Line::from(Span::styled(
            "press p for density card",
            Style::default().fg(theme.dim).bg(theme.panel),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn render_density(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Density", theme, theme.success);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let report = app.report();
    let density = widgets::ratio(report.total_focused_seconds, report.total_open_seconds);
    let gauge = Gauge::default()
        .ratio(density)
        .label(report::percent(density))
        .gauge_style(
            Style::default()
                .fg(widgets::density_color(density, theme))
                .bg(theme.panel)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(theme.panel));
    let [gauge_area, note] =
        *Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(inner)
    else {
        return;
    };
    frame.render_widget(gauge, gauge_area);
    frame.render_widget(
        Paragraph::new("focused time divided by open time")
            .style(Style::default().fg(theme.dim).bg(theme.panel))
            .alignment(Alignment::Center),
        note,
    );
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
    let total = app.report().total_focused_seconds.max(1);
    let rows = rows_data
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let density = widgets::ratio(row.focused_seconds, row.open_seconds);
            Row::new(vec![
                Cell::from(format!("{:>2}", index + 1)).style(Style::default().fg(theme.dim)),
                Cell::from(widgets::app_label(&row.app_class)),
                Cell::from(report::format_duration(row.focused_seconds))
                    .style(Style::default().fg(theme.warn)),
                Cell::from(report::percent(widgets::ratio(row.focused_seconds, total)))
                    .style(Style::default().fg(theme.tertiary)),
                Cell::from(report::percent(density))
                    .style(Style::default().fg(widgets::density_color(density, theme))),
                Cell::from(report::format_duration(row.open_seconds))
                    .style(Style::default().fg(theme.secondary)),
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
            Constraint::Length(7),
            Constraint::Length(9),
        ],
    )
    .header(
        Row::new(["#", "Application", "Focused", "Share", "Dense", "Open"])
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
}

fn render_app_detail(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Selected App", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(row) = app.selected_row() else {
        widgets::render_empty(frame, inner, "No selected app", theme);
        return;
    };
    let density = widgets::ratio(row.focused_seconds, row.open_seconds);
    let [summary, spark, titles] = *Layout::vertical([
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Min(5),
    ])
    .split(inner) else {
        return;
    };
    let lines = vec![
        Line::from(Span::styled(
            widgets::fit_text(&widgets::app_label(&row.app_class), summary.width as usize),
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
            "Open",
            &report::format_duration(row.open_seconds),
            summary.width as usize,
            theme.secondary,
            theme,
        ),
        widgets::metric_line(
            "Density",
            &report::percent(density),
            summary.width as usize,
            widgets::density_color(density, theme),
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
    let block = widgets::panel("Top Titles", theme, theme.tertiary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines = app
        .data()
        .titles
        .iter()
        .filter(|title| title.app_class == selected.app_class)
        .take(inner.height as usize)
        .map(|title| {
            widgets::metric_line(
                &widgets::fit_text(&title.title, 18),
                &report::format_duration(title.focused_seconds),
                inner.width as usize,
                theme.text,
                theme,
            )
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No focused titles for this app",
            Style::default().fg(theme.muted).bg(theme.panel),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme.panel))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

pub(super) fn render_timeline(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    let [canvas_area, table_area] =
        *Layout::vertical([Constraint::Length(10), Constraint::Min(8)]).split(area)
    else {
        return;
    };
    render_activity_canvas(frame, canvas_area, app, theme);
    render_interval_table(frame, table_area, app, theme);
}

fn render_activity_canvas(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Activity Canvas", theme, theme.primary);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some((start, end)) = widgets::today_bounds() else {
        widgets::render_empty(frame, inner, "Timeline unavailable", theme);
        return;
    };
    if app.data().today_intervals.is_empty() || end <= start {
        widgets::render_empty(frame, inner, "No intervals recorded today", theme);
        return;
    }
    let selected = app.selected_row().map(|row| row.app_class.as_str());
    let rows = app.rows().to_vec();
    let intervals = app.data().today_intervals.clone();
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
        });
    frame.render_widget(canvas, inner);

    let labels = vec![
        Line::from(Span::styled(
            format!("{} start", widgets::format_clock(start)),
            Style::default().fg(theme.dim).bg(theme.panel),
        )),
        Line::from(Span::styled(
            format!("now {}", widgets::format_clock(end)),
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

fn render_interval_table(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    let block = widgets::panel("Intervals", theme, theme.secondary);
    let mut intervals = app.data().today_intervals.clone();
    intervals.sort_by_key(|interval| interval.started_at);
    let rows = intervals
        .into_iter()
        .rev()
        .map(|interval| interval_row(interval, app.rows(), theme))
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

fn interval_row(interval: TimelineInterval, rows: &[AppTotals], theme: &Theme) -> Row<'static> {
    let rank = rows
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
            widgets::format_clock(interval.started_at),
            widgets::format_clock(interval.ended_at)
        ))
        .style(Style::default().fg(theme.dim)),
        Cell::from(kind).style(Style::default().fg(color)),
        Cell::from(widgets::app_label(&interval.app_class)),
        Cell::from(widgets::compact_duration(
            interval.ended_at.saturating_sub(interval.started_at),
        ))
        .style(Style::default().fg(theme.muted)),
    ])
    .style(Style::default().fg(theme.text).bg(theme.panel))
}

pub(super) fn render_system(frame: &mut Frame<'_>, area: Rect, app: &App, theme: &Theme) {
    widgets::fill_area(frame, area, theme.bg);
    let [health, lenses] =
        *Layout::horizontal([Constraint::Percentage(54), Constraint::Percentage(46)]).split(area)
    else {
        return;
    };
    render_system_health(frame, health, app, theme);
    render_lens_totals(frame, lenses, app, theme);
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
    ];
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(theme.panel)),
        status,
    );

    let active_total = app.health().storage.focused_active
        + app.health().storage.open_active
        + app.health().storage.idle_active
        + app.health().storage.locked_active;
    let focus_ratio = widgets::ratio(
        app.report().total_focused_seconds,
        app.report().total_open_seconds,
    );
    let gauge_lines = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(2),
    ])
    .split(gauges);
    frame.render_widget(
        LineGauge::default()
            .label(format!("density {}", report::percent(focus_ratio)))
            .ratio(focus_ratio)
            .filled_style(Style::default().fg(widgets::density_color(focus_ratio, theme)))
            .unfilled_style(Style::default().fg(theme.dim))
            .style(Style::default().bg(theme.panel)),
        gauge_lines[0],
    );
    frame.render_widget(
        LineGauge::default()
            .label(format!("active intervals {active_total}"))
            .ratio(widgets::ratio(active_total, 4))
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
            "Idle",
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
            Span::styled("  move selected app", Style::default().fg(theme.text)),
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
                "  toggle overview trends card",
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

fn daily_points<F>(days: &[DayTotals], value: F) -> Vec<(f64, f64)>
where
    F: Fn(&DayTotals) -> i64,
{
    days.iter()
        .enumerate()
        .map(|(index, day)| (index as f64, value(day).max(0) as f64))
        .collect()
}

fn chart_labels(days: &[DayTotals]) -> Vec<Line<'static>> {
    if days.is_empty() {
        return Vec::new();
    }
    let first = days
        .first()
        .map(|day| day.label.clone())
        .unwrap_or_default();
    let last = days.last().map(|day| day.label.clone()).unwrap_or_default();
    if days.len() > 2 {
        let middle = days[days.len() / 2].label.clone();
        vec![Line::from(first), Line::from(middle), Line::from(last)]
    } else {
        vec![Line::from(first), Line::from(last)]
    }
}

fn heat_value(cells: &[FocusHeatCell], weekday: u32, hour: u32) -> i64 {
    cells
        .iter()
        .find(|cell| cell.weekday == weekday && cell.hour == hour)
        .map(|cell| cell.focused_seconds)
        .unwrap_or(0)
}

fn app_daily_values(daily_apps: &[AppDayTotals], days: &[DayTotals], app_class: &str) -> Vec<u64> {
    days.iter()
        .map(|day| {
            daily_apps
                .iter()
                .find(|row| row.app_class == app_class && row.date == day.date)
                .map(|row| row.focused_seconds.max(0) as u64)
                .unwrap_or(0)
        })
        .collect()
}
