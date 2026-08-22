use super::theme::Theme;
use crate::report::{AppBreakdown, Lens};
use chrono::{Local, TimeZone};
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

pub(super) fn panel(title: &str, theme: &Theme, accent: Color) -> Block<'static> {
    Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(accent).bg(theme.panel))
        .style(Style::default().bg(theme.panel))
}

pub(super) fn fill_area(frame: &mut Frame<'_>, area: Rect, color: Color) {
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

pub(super) fn render_empty(frame: &mut Frame<'_>, area: Rect, message: &str, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(theme.muted).bg(theme.panel))
            .wrap(Wrap { trim: true }),
        area,
    );
}

pub(super) fn fit_text(value: &str, width: usize) -> String {
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

pub(super) fn compact_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if hours > 0 {
        format!("{hours}h")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{seconds}s")
    }
}

pub(super) fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        (numerator.max(0) as f64 / denominator as f64).clamp(0.0, 1.0)
    }
}

pub(super) fn lens_color(lens: Lens, theme: &Theme) -> Color {
    match lens {
        Lens::Day => theme.primary,
        Lens::Week => theme.success,
        Lens::Month => theme.warn,
        Lens::Year => theme.tertiary,
        Lens::Life => theme.secondary,
    }
}

pub(super) fn rank_color(index: usize, theme: &Theme) -> Color {
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

pub(super) fn density_color(value: f64, theme: &Theme) -> Color {
    if value >= 0.72 {
        theme.warn
    } else if value >= 0.42 {
        theme.success
    } else if value > 0.0 {
        theme.secondary
    } else {
        theme.dim
    }
}

pub(super) fn metric_line(
    label: &str,
    value: &str,
    width: usize,
    color: Color,
    theme: &Theme,
) -> Line<'static> {
    let label_width = width.min(14);
    Line::from(vec![
        Span::styled(
            fit_text(label, label_width),
            Style::default().fg(theme.dim).bg(theme.panel),
        ),
        Span::styled(" ", Style::default().bg(theme.panel)),
        Span::styled(
            fit_text(value, width.saturating_sub(label_width + 1)),
            Style::default().fg(color).bg(theme.panel),
        ),
    ])
}

pub(super) fn pill(label: &str, selected: bool, color: Color, theme: &Theme) -> Span<'static> {
    Span::styled(
        format!(" {label} "),
        Style::default()
            .fg(if selected { theme.bg } else { color })
            .bg(if selected { color } else { theme.bg })
            .add_modifier(if selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    )
}

pub(super) fn format_clock(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_else(|| "--:--".to_string())
}

pub(super) fn app_share_line(apps: &[AppBreakdown], width: usize, theme: &Theme) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }
    if apps.is_empty() {
        return Line::from(Span::styled(
            " ".repeat(width),
            Style::default().bg(theme.panel),
        ));
    }

    let mut remaining = width;
    let mut spans = Vec::new();
    for (index, app) in apps.iter().enumerate() {
        let last = index + 1 == apps.len();
        let len = if last {
            remaining
        } else {
            ((app.share * width as f64).round() as usize)
                .clamp(1, remaining.saturating_sub(apps.len() - index - 1))
        };
        remaining = remaining.saturating_sub(len);
        spans.push(Span::styled(
            "█".repeat(len),
            Style::default()
                .fg(rank_color(index, theme))
                .bg(theme.panel),
        ));
    }
    if remaining > 0 {
        spans.push(Span::styled(
            "░".repeat(remaining),
            Style::default().fg(theme.dim).bg(theme.panel),
        ));
    }
    Line::from(spans)
}
