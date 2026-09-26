use crate::storage::{AppTotals, DayTotals, FocusHeatCell, TimelineInterval};

pub const DEEP_BLOCK_SECONDS: i64 = 25 * 60;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HourTotal {
    pub hour: u32,
    pub focused_seconds: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WeekdayTotal {
    pub weekday: u32,
    pub focused_seconds: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FocusBlockStats {
    pub count: usize,
    pub total_seconds: i64,
    pub average_seconds: i64,
    pub median_seconds: i64,
    pub longest_seconds: i64,
    pub deep_count: usize,
    pub deep_seconds: i64,
}

pub fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    let rest = minutes % 60;
    if rest == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {rest}m")
    }
}

pub fn percent(value: f64) -> String {
    format!("{:.0}%", value.clamp(0.0, 1.0) * 100.0)
}

pub fn signed_duration(seconds: i64) -> String {
    let sign = if seconds < 0 { "-" } else { "+" };
    format!("{sign}{}", format_duration(seconds.abs()))
}

pub fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        value.max(0) as f64 / total as f64
    }
}

pub fn average(total: i64, count: usize) -> i64 {
    if count == 0 {
        0
    } else {
        total.max(0) / count as i64
    }
}

pub fn median(sorted: &[i64]) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let mid = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2
    } else {
        sorted[mid]
    }
}

pub fn active_day_count(days: &[DayTotals]) -> usize {
    days.iter().filter(|day| day.focused_seconds > 0).count()
}

pub fn longest_active_streak(days: &[DayTotals]) -> usize {
    let mut current = 0;
    let mut best = 0;
    for day in days {
        if day.focused_seconds > 0 {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best
}

pub fn app_switch_count(intervals: &[TimelineInterval]) -> usize {
    intervals
        .windows(2)
        .filter(|pair| {
            pair[0].app_class != pair[1].app_class && pair[0].ended_at == pair[1].started_at
        })
        .count()
}

pub fn focus_block_stats(intervals: &[TimelineInterval]) -> FocusBlockStats {
    let mut durations = focus_block_durations(intervals);
    let count = durations.len();
    let total_seconds = durations.iter().sum::<i64>();
    durations.sort_unstable();
    let average_seconds = average(total_seconds, count);
    let median_seconds = median(&durations);
    let longest_seconds = durations.last().copied().unwrap_or_default();
    let deep_count = durations
        .iter()
        .filter(|seconds| **seconds >= DEEP_BLOCK_SECONDS)
        .count();
    let deep_seconds = durations
        .iter()
        .filter(|seconds| **seconds >= DEEP_BLOCK_SECONDS)
        .sum::<i64>();

    FocusBlockStats {
        count,
        total_seconds,
        average_seconds,
        median_seconds,
        longest_seconds,
        deep_count,
        deep_seconds,
    }
}

pub fn focus_block_durations(intervals: &[TimelineInterval]) -> Vec<i64> {
    let mut durations: Vec<i64> = Vec::new();
    let mut previous: Option<&TimelineInterval> = None;
    for interval in intervals {
        let seconds = (interval.ended_at - interval.started_at).max(0);
        if seconds == 0 {
            continue;
        }
        if previous
            .is_some_and(|p| p.app_class == interval.app_class && p.ended_at == interval.started_at)
        {
            *durations.last_mut().unwrap() += seconds;
        } else {
            durations.push(seconds);
        }
        previous = Some(interval);
    }
    durations
}

pub fn hour_totals(cells: &[FocusHeatCell]) -> Vec<(u32, i64)> {
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

pub fn peak_hour(cells: &[FocusHeatCell]) -> Option<HourTotal> {
    hour_totals(cells)
        .into_iter()
        .find(|(_, seconds)| *seconds > 0)
        .map(|(hour, focused_seconds)| HourTotal {
            hour,
            focused_seconds,
        })
}

pub fn peak_weekday(cells: &[FocusHeatCell]) -> Option<WeekdayTotal> {
    let mut weekdays = [0_i64; 7];
    for cell in cells {
        if let Some(total) = weekdays.get_mut(cell.weekday as usize) {
            *total += cell.focused_seconds.max(0);
        }
    }
    weekdays
        .into_iter()
        .enumerate()
        .max_by_key(|(_, seconds)| *seconds)
        .filter(|(_, seconds)| *seconds > 0)
        .map(|(weekday, focused_seconds)| WeekdayTotal {
            weekday: weekday as u32,
            focused_seconds,
        })
}

pub fn effective_app_count(rows: &[AppTotals], total_focused_seconds: i64) -> f64 {
    if total_focused_seconds <= 0 {
        return 0.0;
    }

    let entropy = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .map(|row| row.focused_seconds as f64 / total_focused_seconds as f64)
        .filter(|share| *share > 0.0)
        .map(|share| -share * share.ln())
        .sum::<f64>();
    entropy.exp()
}

/// Human-readable local clock labels; stored timestamps and clock minutes stay unchanged.
pub fn clock_label(minute: u32) -> String {
    let minute = minute % 1440;
    let hour = minute / 60;
    let hour12 = if hour.is_multiple_of(12) {
        12
    } else {
        hour % 12
    };
    let suffix = if hour < 12 { "AM" } else { "PM" };
    if minute.is_multiple_of(60) {
        format!("{hour12} {suffix}")
    } else {
        format!("{hour12}:{:02} {suffix}", minute % 60)
    }
}

pub fn clock_range(start: u32, end: u32) -> String {
    let first = clock_label(start);
    let last = clock_label(end);
    if (start % 1440 < 720) == (end % 1440 < 720) {
        format!("{}–{last}", first.rsplit_once(' ').unwrap().0)
    } else {
        format!("{first}–{last}")
    }
}

pub fn duration_words(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    if minutes == 0 {
        return "less than a minute".into();
    }
    let hours = minutes / 60;
    let rest = minutes % 60;
    let minute_label = if rest == 1 { "minute" } else { "minutes" };
    if hours == 0 {
        return format!("{rest} {minute_label}");
    }
    let hour_label = if hours == 1 { "hour" } else { "hours" };
    if rest == 0 {
        format!("{hours} {hour_label}")
    } else {
        format!("{hours} {hour_label} and {rest} {minute_label}")
    }
}

#[cfg(test)]
mod wording_tests {
    use super::*;
    #[test]
    fn readable_times_cover_noon_midnight_and_uneven_hours() {
        assert_eq!(clock_label(0), "12 AM");
        assert_eq!(clock_label(720), "12 PM");
        assert_eq!(clock_range(1230, 1290), "8:30–9:30 PM");
        assert_eq!(clock_range(1410, 60), "11:30 PM–1 AM");
        assert_eq!(duration_words(3600), "1 hour");
        assert_eq!(duration_words(3660), "1 hour and 1 minute");
        assert_eq!(duration_words(90), "1 minute");
    }
}
