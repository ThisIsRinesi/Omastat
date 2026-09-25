//! Bounded local-clock recurrence. Bitsets count dates, never focus fragments.
use crate::{
    activity::{Coverage, Slice, Visit, midnight},
    analytics,
    insights::{
        Insight, InsightCategory, InsightConfidence, InsightEvidence, InsightKind, InsightSupport,
        InsightTone, RoutineEvidence,
    },
};
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};
use std::collections::BTreeSet;

const DAYS: usize = 56;
const BINS: usize = 96;
const SIZE: usize = DAYS * BINS;
const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

struct Window {
    start: usize,
    width: usize,
    app: u64,
    domain: u64,
}
pub(crate) struct Calendar {
    start: NaiveDate,
    end: NaiveDate,
    from: i64,
    to: i64,
    windows: Vec<Window>,
    masks: [u64; 10],
    dates: Vec<String>,
}
fn buckets(start: NaiveDate, from: i64, to: i64, mut consume: impl FnMut(usize, i64)) {
    let mut cursor = from;
    while cursor < to {
        let Some(local) = Local.timestamp_opt(cursor, 0).single() else {
            break;
        };
        let day = (local.date_naive() - start).num_days();
        let bin = local.hour() as usize * 4 + local.minute() as usize / 15;
        let next = (cursor + 900 - i64::from(local.minute() % 15 * 60 + local.second())).min(to);
        if (0..DAYS as i64).contains(&day) {
            consume(day as usize * BINS + bin, next - cursor);
        }
        cursor = next;
    }
}
fn sum(bins: &[i64], day: usize, start: usize, width: usize) -> i64 {
    let a = day * BINS + start;
    let b = (a + width).min(bins.len());
    bins[a..b].iter().sum()
}
impl Calendar {
    pub fn new(end: NaiveDate, app: &Coverage, domain: &Coverage) -> Result<Self> {
        let start = end - Duration::days(DAYS as i64);
        let from = midnight(start)?;
        let to = midnight(end)?;
        let mut elapsed = vec![0; SIZE];
        let mut app_bins = vec![0; SIZE];
        let mut domain_bins = vec![0; SIZE];
        buckets(start, from, to, |i, s| elapsed[i] += s);
        for (coverage, bins) in [(app, &mut app_bins), (domain, &mut domain_bins)] {
            for &(a, b) in &coverage.windows {
                buckets(start, a.max(from), b.min(to), |i, s| bins[i] += s);
            }
        }
        let mut windows = Vec::new();
        for width in [4, 8] {
            for slot in 0..BINS {
                let mut w = Window {
                    start: slot,
                    width,
                    app: 0,
                    domain: 0,
                };
                for day in 0..DAYS {
                    let a = day * BINS + slot;
                    let b = a + width;
                    // Missing spring-forward bins and windows extending beyond the cutoff are not evidence.
                    if b > SIZE || elapsed[a..b].contains(&0) {
                        continue;
                    }
                    let length = sum(&elapsed, day, slot, width);
                    if sum(&app_bins, day, slot, width) * 10 >= length * 9 {
                        w.app |= 1 << day;
                    }
                    if sum(&domain_bins, day, slot, width) * 10 >= length * 9 {
                        w.domain |= 1 << day;
                    }
                }
                windows.push(w);
            }
        }
        let mut masks = [0; 10];
        let mut dates = Vec::new();
        for day in 0..DAYS {
            let date = start + Duration::days(day as i64);
            let weekday = date.weekday().num_days_from_monday() as usize;
            masks[0] |= 1 << day;
            masks[if weekday < 5 { 1 } else { 2 }] |= 1 << day;
            masks[3 + weekday] |= 1 << day;
            dates.push(date.to_string());
        }
        Ok(Self {
            start,
            end,
            from,
            to,
            windows,
            masks,
            dates,
        })
    }
    fn dates(&self, mask: u64) -> Vec<String> {
        (0..DAYS)
            .filter(|d| mask & (1 << d) != 0)
            .map(|d| self.dates[d].clone())
            .collect()
    }
}
fn cadence(index: usize) -> String {
    match index {
        0 => "everyday".into(),
        1 => "weekdays".into(),
        2 => "weekends".into(),
        _ => WEEKDAYS[index - 3].to_lowercase(),
    }
}
fn cadence_label(index: usize) -> String {
    match index {
        0 => "across the week".into(),
        1 => "on weekdays".into(),
        2 => "on weekends".into(),
        _ => format!("on {}s", WEEKDAYS[index - 3]),
    }
}
fn time(minute: u32) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}
fn confidence(count: usize, mask: u64, share: f64) -> InsightConfidence {
    let span = if mask == 0 {
        0
    } else {
        63 - mask.leading_zeros() - mask.trailing_zeros()
    };
    if count >= 14 && span >= 27 && share >= 0.75 {
        InsightConfidence::High
    } else if count >= 7 {
        InsightConfidence::Medium
    } else {
        InsightConfidence::Low
    }
}
fn confidence_rank(c: InsightConfidence) -> u8 {
    match c {
        InsightConfidence::High => 2,
        InsightConfidence::Medium => 1,
        InsightConfidence::Low => 0,
    }
}
pub(crate) fn rank(items: &mut [Insight]) {
    items.sort_by(|a, b| {
        confidence_rank(b.confidence)
            .cmp(&confidence_rank(a.confidence))
            .then(
                b.supporting
                    .occurrence_count
                    .cmp(&a.supporting.occurrence_count),
            )
            .then(
                b.supporting
                    .share
                    .unwrap_or(0.0)
                    .total_cmp(&a.supporting.share.unwrap_or(0.0)),
            )
            .then_with(|| width(a).cmp(&width(b)))
            .then(a.title.cmp(&b.title))
            .then(a.value.cmp(&b.value))
    });
}
fn width(i: &Insight) -> u32 {
    i.supporting
        .routine
        .as_ref()
        .map_or(1440, |r| (r.end_minute + 1440 - r.start_minute) % 1440)
}
fn overlaps(a: &RoutineEvidence, b: &RoutineEvidence) -> bool {
    let wa = (a.end_minute + 1440 - a.start_minute) % 1440;
    let wb = (b.end_minute + 1440 - b.start_minute) % 1440;
    (b.start_minute + 1440 - a.start_minute) % 1440 <= wa
        || (a.start_minute + 1440 - b.start_minute) % 1440 <= wb
}
fn related(a: &str, b: &str) -> bool {
    a == b
        || a == "everyday"
        || b == "everyday"
        || (a == "weekdays" && !["weekends", "saturday", "sunday"].contains(&b))
        || (b == "weekdays" && !["weekends", "saturday", "sunday"].contains(&a))
        || (a == "weekends" && ["saturday", "sunday"].contains(&b))
        || (b == "weekends" && ["saturday", "sunday"].contains(&a))
}

fn typical_visit(visits: &[(usize, i64)], window: &Window, matching: u64) -> i64 {
    let mut durations = Vec::new();
    for day in 0..DAYS {
        if matching & (1 << day) == 0 {
            continue;
        }
        let start = day * BINS + window.start;
        let first = visits.partition_point(|(bin, _)| *bin < start);
        let last = visits.partition_point(|(bin, _)| *bin < start + window.width);
        durations.extend(visits[first..last].iter().map(|(_, seconds)| *seconds));
    }
    durations.sort_unstable();
    analytics::median(&durations)
}

/// A suggestion is based on observed visit starts, never merely on time spent open.
pub(crate) fn predict(
    calendar: &Calendar,
    kind: &str,
    key: &str,
    label: &str,
    visits: &[Visit],
    now: i64,
) -> Vec<Insight> {
    let Some(local_now) = Local.timestamp_opt(now, 0).single() else {
        return Vec::new();
    };
    let today = local_now.date_naive();
    if today != calendar.end {
        return Vec::new();
    }
    let mut starts = vec![Vec::<i64>::new(); SIZE];
    for visit in visits {
        if !visit.known_start
            || visit.seconds < 300
            || visit.start < calendar.from
            || visit.start >= calendar.to
        {
            continue;
        }
        let Some(at) = Local.timestamp_opt(visit.start, 0).single() else {
            continue;
        };
        let day = (at.date_naive() - calendar.start).num_days();
        if !(0..DAYS as i64).contains(&day) {
            continue;
        }
        let bin = at.hour() as usize * 4 + at.minute() as usize / 15;
        starts[day as usize * BINS + bin].push(visit.start);
    }
    let mut candidates = Vec::new();
    // Only a one-hour window can justify an imminent start.
    for window in calendar.windows.iter().filter(|w| w.width == 4) {
        let eligible = if kind == "domain" {
            window.domain
        } else {
            window.app
        };
        let start_minute = window.start as u32 * 15;
        let anchor_day = if local_now.hour() == 0 && start_minute >= 23 * 60 {
            today - Duration::days(1)
        } else if local_now.hour() == 23 && start_minute < 60 {
            today + Duration::days(1)
        } else {
            today
        };
        let weekday = anchor_day.weekday().num_days_from_monday() as usize;
        let naive_start = anchor_day
            .and_hms_opt(start_minute / 60, start_minute % 60, 0)
            .unwrap();
        let naive_end = naive_start + Duration::hours(1);
        // Ambiguous or missing local times should not become a precise prediction.
        let (Some(begin), Some(end)) = (
            Local.from_local_datetime(&naive_start).single(),
            Local.from_local_datetime(&naive_end).single(),
        ) else {
            continue;
        };
        if now < begin.timestamp() - 1800 || now >= end.timestamp() {
            continue;
        }
        if visits.iter().any(|v| {
            v.start <= now && v.end > begin.timestamp() - 1800 && v.start < end.timestamp()
        }) {
            continue;
        }
        for cohort in [3 + weekday, if weekday < 5 { 1 } else { 2 }, 0] {
            let cohort_mask = eligible & calendar.masks[cohort];
            for (recent, mask) in [(false, cohort_mask), (true, cohort_mask & (!0u64 << 42))] {
                if recent && cohort >= 3 {
                    continue;
                }
                let sample = mask.count_ones() as usize;
                let minimum = if cohort >= 3 { 5 } else { 7 };
                if sample < minimum {
                    continue;
                }
                let mut matches = 0u64;
                let mut stamps = Vec::new();
                for day in 0..DAYS {
                    if mask & (1u64 << day) == 0 {
                        continue;
                    }
                    let found = (window.start..window.start + window.width)
                        .flat_map(|bin| starts[day * BINS + bin].iter().copied())
                        .collect::<Vec<_>>();
                    if !found.is_empty() {
                        matches |= 1u64 << day;
                        stamps.extend(found);
                    }
                }
                let count = matches.count_ones() as usize;
                let required = if cohort >= 3 { 4 } else { 5 };
                if count < required || count * 100 < sample * if recent { 60 } else { 70 } {
                    continue;
                }
                let all_starts: usize = (0..DAYS)
                    .filter(|day| mask & (1u64 << day) != 0)
                    .map(|day| {
                        starts[day * BINS..(day + 1) * BINS]
                            .iter()
                            .map(Vec::len)
                            .sum::<usize>()
                    })
                    .sum();
                if stamps.len() * 48 < all_starts * 3 {
                    continue;
                }
                if !recent {
                    let weeks = (0..DAYS)
                        .filter(|d| matches & (1u64 << d) != 0)
                        .map(|d| d / 7)
                        .collect::<BTreeSet<_>>();
                    if weeks.len() < 3 {
                        continue;
                    }
                }
                let last_three = (0..DAYS)
                    .rev()
                    .filter(|d| mask & (1u64 << d) != 0)
                    .take(3)
                    .collect::<Vec<_>>();
                if last_three
                    .iter()
                    .filter(|d| matches & (1u64 << **d) != 0)
                    .count()
                    < 2
                {
                    continue;
                }
                let minutes = stamps
                    .iter()
                    .filter_map(|s| Local.timestamp_opt(*s, 0).single())
                    .map(|at| {
                        let minute = at.hour() * 60 + at.minute();
                        if minute < start_minute {
                            minute + 1440
                        } else {
                            minute
                        }
                    })
                    .collect::<Vec<_>>();
                let mut minutes = minutes;
                minutes.sort_unstable();
                let middle = minutes[minutes.len() / 2] % 1440;
                let rounded = ((middle + 7) / 15 * 15) % 1440;
                let time_label = analytics::clock_label(rounded);
                let display_time = if minutes.last().unwrap() - minutes[0] <= 30 {
                    format!("Around {time_label}")
                } else {
                    analytics::clock_range(start_minute, start_minute + 60)
                };
                let expected_day = if middle < start_minute {
                    anchor_day + Duration::days(1)
                } else {
                    anchor_day
                };
                let expected = expected_day
                    .and_hms_opt(middle / 60, middle % 60, 0)
                    .and_then(|naive| Local.from_local_datetime(&naive).single())
                    .map(|at| at.timestamp())
                    .unwrap_or(begin.timestamp());
                let status = if recent { "recent" } else { "established" };
                let eligible_dates = calendar.dates(mask);
                candidates.push(Insight {
                    kind: InsightKind::UpcomingActivity,
                    category: InsightCategory::Patterns,
                    tone: InsightTone::Info,
                    title: format!("{label} might be coming up"),
                    value: display_time,
                    explanation: format!("{} started {label} around this time on {count} of {sample} tracked {}.", if recent { "Recently, you" } else { "You" }, if cohort >= 3 { "weeks" } else { "days" }),
                    confidence: if recent { InsightConfidence::Low } else { InsightConfidence::High },
                    evidence: InsightEvidence { data_points: sample, minimum_data_points: minimum, observed_focus_seconds: 0, observed_open_seconds: 0 },
                    supporting: InsightSupport {
                        prediction_id: Some(format!("{kind}:{key}:{anchor_day}:{start_minute}")),
                        generated_at: Some(now),
                        activity_kind: Some(kind.into()), activity_key: Some(key.into()), app_label: Some(label.into()),
                        occurrence_count: Some(count), eligible_count: Some(sample), matching_dates: Some(calendar.dates(matches)),
                        period_start_date: eligible_dates.first().cloned(), period_end_date: eligible_dates.last().cloned(),
                        display_until: Some(end.timestamp()), expected_start: Some(expected),
                        routine: Some(RoutineEvidence { cadence: cadence(cohort), status: status.into(), start_minute,
                            end_minute: (start_minute + 60) % 1440, timing_basis: "visit-start".into(), eligible_dates,
                            visit_start_window: None }),
                        method: Some("Known foreground visits of at least five minutes, on days with at least 90% tracking of this hour. The displayed frequency counts tracked opportunities, not the probability of opening the app today. Recent matching starts must remain present.".into()),
                        ..Default::default()
                    },
                });
                break;
            }
        }
    }
    candidates.sort_by(|a, b| {
        let ar = a
            .supporting
            .routine
            .as_ref()
            .is_some_and(|r| r.status == "recent");
        let br = b
            .supporting
            .routine
            .as_ref()
            .is_some_and(|r| r.status == "recent");
        ar.cmp(&br)
            .then(
                b.supporting
                    .occurrence_count
                    .cmp(&a.supporting.occurrence_count),
            )
            .then_with(|| {
                let distance = |item: &Insight| {
                    let routine = item.supporting.routine.as_ref().unwrap();
                    let median = item
                        .supporting
                        .expected_start
                        .and_then(|ts| Local.timestamp_opt(ts, 0).single())
                        .map(|at| at.hour() * 60 + at.minute())
                        .unwrap_or(routine.start_minute);
                    let midpoint = (routine.start_minute + 30) % 1440;
                    let gap = (i64::from(midpoint) - i64::from(median)).abs();
                    gap.min(1440 - gap)
                };
                distance(a).cmp(&distance(b))
            })
            .then(
                a.supporting
                    .expected_start
                    .cmp(&b.supporting.expected_start),
            )
    });
    candidates.truncate(1);
    candidates
}

pub(crate) fn detect(
    calendar: &Calendar,
    kind: &str,
    key: &str,
    label: &str,
    intervals: &[Slice],
    visits: &[Visit],
) -> Vec<Insight> {
    let mut usage = vec![0; SIZE];
    let mut starts = vec![0; SIZE];
    let mut start_visits = Vec::new();
    for i in intervals {
        buckets(
            calendar.start,
            i.start.max(calendar.from),
            i.end.min(calendar.to),
            |bin, s| usage[bin] += s,
        );
    }
    for v in visits {
        if !v.known_start || v.start < calendar.from || v.start >= calendar.to {
            continue;
        }
        // Foreground evidence is clipped at the historical cutoff even if the visit continues.
        let seconds = intervals[v.first..=v.last]
            .iter()
            .map(|i| (i.end.min(calendar.to) - i.start).max(0))
            .sum::<i64>();
        if seconds < 300 {
            continue;
        }
        buckets(calendar.start, v.start, v.start + 1, |bin, _| {
            starts[bin] += seconds;
            start_visits.push((bin, seconds));
        });
    }
    // Repeated DST hours may put local bins out of timestamp order.
    start_visits.sort_unstable_by_key(|(bin, _)| *bin);
    let mut candidates = Vec::new();
    for (basis, bins) in [("usage", &usage), ("visit-start", &starts)] {
        for w in &calendar.windows {
            let eligible = if kind == "domain" { w.domain } else { w.app };
            let amounts = (0..DAYS)
                .map(|d| sum(bins, d, w.start, w.width))
                .collect::<Vec<_>>();
            let matches = amounts.iter().enumerate().fold(0u64, |mask, (d, s)| {
                if *s >= 300 { mask | (1 << d) } else { mask }
            });
            for cohort in 0..10 {
                for length in [56, 14] {
                    if cohort >= 3 && length == 14 {
                        continue;
                    }
                    let recent_mask = (!0u64) << (DAYS - length);
                    let available = eligible & calendar.masks[cohort] & recent_mask;
                    // A newly installed tracker cannot establish a long-term habit from recent data alone.
                    if cohort < 3 && length == 56 && available & ((1u64 << 42) - 1) == 0 {
                        continue;
                    }
                    let matching = matches & available;
                    let count = matching.count_ones() as usize;
                    let sample = available.count_ones() as usize;
                    let minimum = if cohort >= 3 { 3 } else { 5 };
                    if count < minimum || (cohort < 3 && sample < 7) || count * 10 < sample * 6 {
                        continue;
                    }
                    // A time window must concentrate activity, rather than describe an all-day app arbitrarily.
                    let total: i64 = (0..DAYS)
                        .filter(|d| available & (1 << d) != 0)
                        .map(|d| bins[d * BINS..(d + 1) * BINS].iter().sum::<i64>())
                        .sum();
                    let in_window: i64 = (0..DAYS)
                        .filter(|d| available & (1 << d) != 0)
                        .map(|d| amounts[d])
                        .sum();
                    if in_window * BINS as i64 * 2 < total * w.width as i64 * 3 {
                        continue;
                    }
                    let mut observed = (0..DAYS)
                        .filter(|d| matching & (1 << d) != 0)
                        .map(|d| amounts[d])
                        .collect::<Vec<_>>();
                    observed.sort_unstable();
                    let typical = if basis == "visit-start" {
                        typical_visit(&start_visits, w, matching)
                    } else {
                        analytics::median(&observed)
                    };
                    let share = count as f64 / sample as f64;
                    let start_minute = (w.start * 15) as u32;
                    let end_minute = ((w.start + w.width) * 15 % 1440) as u32;
                    let recent = length == 14;
                    let baseline_start = (calendar.end - Duration::days(length as i64)).to_string();
                    let baseline_end = (calendar.end - Duration::days(1)).to_string();
                    let description = if basis == "usage" {
                        format!(
                            "Typical foreground use within this window: {}.",
                            analytics::format_duration(typical)
                        )
                    } else {
                        format!(
                            "Foreground visits commonly started in this window; typical recorded visit duration: {}.",
                            analytics::format_duration(typical)
                        )
                    };
                    let supporting = InsightSupport {
                        app_class: (kind == "app").then(|| key.into()),
                        app_label: Some(label.into()),
                        activity_kind: Some(kind.into()),
                        activity_key: Some(key.into()),
                        weekday: (cohort >= 3).then(|| (cohort - 3) as u32),
                        weekday_label: (cohort >= 3).then(|| WEEKDAYS[cohort - 3].into()),
                        hour: Some(start_minute / 60),
                        hour_label: Some(format!("{}–{}", time(start_minute), time(end_minute))),
                        baseline_seconds: Some(typical),
                        share: Some(share),
                        period_start_date: Some(baseline_start.clone()),
                        period_end_date: Some(baseline_end.clone()),
                        occurrence_count: Some(count),
                        eligible_count: Some(sample),
                        matching_dates: Some(calendar.dates(matching)),
                        routine: Some(RoutineEvidence {
                            cadence: cadence(cohort),
                            status: if recent { "recent" } else { "established" }.into(),
                            start_minute,
                            end_minute,
                            timing_basis: basis.into(),
                            eligible_dates: calendar.dates(available),
                            visit_start_window: None,
                        }),
                        ..Default::default()
                    };
                    let qualification = if recent {
                        " This is a recent pattern, not an established long-term routine."
                    } else {
                        ""
                    };
                    candidates.push(Insight {
                        kind: InsightKind::AppRoutine,
                        category: InsightCategory::Patterns,
                        tone: InsightTone::Info,
                        title: format!("{label} {}{}", if recent { "recently " } else { "" }, cadence_label(cohort)),
                        value: format!("{}–{} · {count} of {sample} observed {}", time(start_minute), time(end_minute), if cohort >= 3 { "weeks" } else { "days" }),
                        explanation: format!("{description} Based on {baseline_start} through {baseline_end}; only windows with at least 90% recorded coverage are included.{qualification}"),
                        confidence: confidence(count, matching, share),
                        evidence: InsightEvidence {
                            data_points: sample,
                            minimum_data_points: if cohort >= 3 { 3 } else { 7 },
                            observed_focus_seconds: observed.iter().sum(),
                            observed_open_seconds: 0,
                        },
                        supporting,
                    });
                    // Prefer the longer qualifying baseline for this same claim.
                    break;
                }
            }
        }
    }
    rank(&mut candidates);
    let mut chosen: Vec<Insight> = Vec::new();
    for candidate in candidates {
        let r = candidate.supporting.routine.as_ref().unwrap();
        if let Some(previous) = chosen.iter_mut().find(|i| {
            let p = i.supporting.routine.as_ref().unwrap();
            overlaps(p, r) && related(&p.cadence, &r.cadence)
        }) {
            let p = previous.supporting.routine.as_mut().unwrap();
            // Combine only claims supported by precisely the same dates and baseline.
            if p.timing_basis != r.timing_basis
                && p.timing_basis != "usage-and-visit-start"
                && p.cadence == r.cadence
                && previous.supporting.matching_dates == candidate.supporting.matching_dates
                && p.eligible_dates == r.eligible_dates
                && previous.supporting.period_start_date == candidate.supporting.period_start_date
            {
                p.visit_start_window = Some(if r.timing_basis == "visit-start" {
                    (r.start_minute, r.end_minute)
                } else {
                    (p.start_minute, p.end_minute)
                });
                if r.timing_basis == "usage" {
                    p.start_minute = r.start_minute;
                    p.end_minute = r.end_minute;
                    previous.value = candidate.value.clone();
                    previous.evidence = candidate.evidence.clone();
                    previous.supporting.baseline_seconds = candidate.supporting.baseline_seconds;
                    previous.supporting.hour = candidate.supporting.hour;
                    previous.supporting.hour_label = candidate.supporting.hour_label.clone();
                }
                p.timing_basis = "usage-and-visit-start".into();
                let (a, b) = p.visit_start_window.unwrap();
                previous.explanation = format!(
                    "Regular foreground use and visit starts on the same matching dates. Visit starts: {}–{}. {}",
                    time(a),
                    time(b),
                    previous.explanation
                );
            }
            continue;
        }
        if chosen.len() < 3 {
            chosen.push(candidate);
        }
    }
    chosen
}

/// Describe only selected findings, so copy changes cannot change ranking or evidence.
pub(crate) fn humanize(insight: &mut Insight) {
    let title_variant = crate::insights::wording_variant(insight, "routine-title", 4);
    let copy_variant = crate::insights::wording_variant(insight, "routine-copy", 5);
    let support = &mut insight.supporting;
    let Some(routine) = &support.routine else {
        return;
    };
    let label = support.app_label.as_deref().unwrap_or("This activity");
    let width = (routine.end_minute + 1440 - routine.start_minute) % 1440;
    let middle = (routine.start_minute + width / 2) % 1440;
    let part = match middle / 60 {
        0..=5 | 22..=23 => "nights",
        6..=11 => "mornings",
        12..=16 => "afternoons",
        _ => "evenings",
    };
    let when = match routine.cadence.as_str() {
        "everyday" => format!("most {part}"),
        "weekdays" => format!("on weekday {part}"),
        "weekends" => format!("on weekend {part}"),
        _ => format!(
            "on {} {part}",
            support.weekday_label.as_deref().unwrap_or("these")
        ),
    };
    let count = support.occurrence_count.unwrap_or(0);
    let sample = support.eligible_count.unwrap_or(0);
    let unit = if support.weekday.is_some() {
        "weeks"
    } else {
        "days"
    };
    let range = analytics::clock_range(routine.start_minute, routine.end_minute);
    let recent = routine.status == "recent";
    let tentative = insight.confidence == InsightConfidence::Low;
    insight.title = if tentative {
        [
            format!("A possible rhythm with {label}"),
            format!("{label} might be finding a place"),
            format!("An early pattern with {label}"),
            format!("A hint of a {label} routine"),
        ][title_variant]
            .clone()
    } else {
        [
            format!("{label}, {when}"),
            format!("You return to {label} {when}"),
            format!("{label} keeps showing up {when}"),
            format!("There's time for {label} {when}"),
        ][title_variant]
            .clone()
    };
    insight.value = format!("{range} · {count} of {sample} {unit}");
    support.hour_label = Some(range.clone());
    let duration = analytics::duration_words(support.baseline_seconds.unwrap_or(0));
    let lead = if recent {
        [
            "Lately, ",
            "Over the past two weeks, ",
            "Recently, ",
            "These past two weeks, ",
            "In your recent activity, ",
        ][copy_variant]
    } else {
        ["", "Looking back, ", "", "Over this stretch, ", ""][copy_variant]
    };
    let when = if routine.cadence == "everyday" {
        format!("in the {part}")
    } else {
        when
    };
    let frequency = if tentative { "sometimes" } else { "often" };
    let visit_start = routine.timing_basis == "visit-start";
    let phrases = if visit_start {
        [
            format!(
                "{lead}you've {frequency} dropped in {when}. A typical visit adds up to about {duration} of use."
            ),
            format!(
                "{lead}your visits have {frequency} begun around this time {when}, with about {duration} of use per visit."
            ),
            format!(
                "{lead}a typical visit starting around this time includes about {duration} of use. You've {frequency} come back {when}."
            ),
            format!(
                "{lead}this has {frequency} been a time to come back {when}. Your usual visit adds up to about {duration}."
            ),
            format!(
                "{lead}visits beginning {when} have {frequency} clustered around this time, with about {duration} of use per visit."
            ),
        ]
    } else {
        [
            format!(
                "{lead}you've {frequency} returned {when}. On days with this pattern, your use between {range} typically totals about {duration} across visits."
            ),
            format!(
                "{lead}this has {frequency} been part of the day {when}. Your use between {range} typically adds up to about {duration} across visits on matching days."
            ),
            format!(
                "{lead}your use between {range} typically totals about {duration} across visits on days with this pattern. You've {frequency} found your way here {when}."
            ),
            format!(
                "{lead}you've {frequency} spent time here {when}. Across visits between {range}, a matching day typically adds up to about {duration} of use."
            ),
            format!(
                "{lead}on days with this pattern {when}, your use typically totals about {duration} across visits between {range}."
            ),
        ]
    };
    insight.explanation = phrases[copy_variant].clone();
    if let Some(first) = insight.explanation.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    if tentative {
        insight
            .explanation
            .push_str(" This may be a pattern taking shape.");
    }
    if visit_start && support.baseline_seconds.unwrap_or(0) <= 600 {
        insight
            .explanation
            .push_str(" These tend to be brief visits.");
    }
    if let Some((start, end)) = routine.visit_start_window {
        insight.explanation.push_str(&format!(
            " You tend to start between {}.",
            analytics::clock_range(start, end).replace('–', " and ")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(
        times: &[(usize, u32, u32)],
        coverage: Option<Vec<(i64, i64)>>,
    ) -> (Calendar, Vec<Slice>, Vec<Visit>) {
        let end = NaiveDate::from_ymd_opt(2026, 9, 5).unwrap();
        let start = end - Duration::days(56);
        let a = midnight(start).unwrap();
        let b = midnight(end).unwrap();
        let coverage = Coverage::new(coverage.unwrap_or_else(|| vec![(a, b)]));
        let calendar = Calendar::new(end, &coverage, &coverage).unwrap();
        let mut intervals = Vec::new();
        let mut visits = Vec::new();
        for &(day, minute, length) in times {
            let start = midnight(start + Duration::days(day as i64)).unwrap() + minute as i64 * 60;
            let end = start + length as i64 * 60;
            let index = intervals.len();
            intervals.push(Slice { start, end });
            visits.push(Visit {
                start,
                end,
                seconds: end - start,
                known_start: true,
                first: index,
                last: index,
            });
        }
        (calendar, intervals, visits)
    }
    fn detect_fixture(times: &[(usize, u32, u32)]) -> Vec<Insight> {
        let (c, s, v) = fixture(times, None);
        detect(&c, "app", "game", "Game", &s, &v)
    }
    #[test]
    fn upcoming_visit_needs_repeated_starts_and_disappears_after_use() {
        let end = Local::now().date_naive();
        let first = end - Duration::days(56);
        let from = midnight(first).unwrap();
        let today = midnight(end).unwrap();
        let coverage = Coverage::new(vec![(from, today)]);
        let calendar = Calendar::new(end, &coverage, &coverage).unwrap();
        let now = today + 19 * 3600 + 45 * 60;
        let mut visits = (0..56)
            .map(|day| {
                let start = midnight(first + Duration::days(day)).unwrap() + 20 * 3600;
                Visit {
                    start,
                    end: start + 1200,
                    seconds: 1200,
                    known_start: true,
                    first: day as usize,
                    last: day as usize,
                }
            })
            .collect::<Vec<_>>();
        let found = predict(&calendar, "app", "spire", "Slay the Spire 2", &visits, now);
        assert!(!found.is_empty());
        assert_eq!(found[0].kind, InsightKind::UpcomingActivity);
        assert_eq!(
            found[0].supporting.routine.as_ref().unwrap().status,
            "established"
        );
        assert!(found[0].supporting.display_until.unwrap() > now);
        visits.push(Visit {
            start: now - 120,
            end: now + 900,
            seconds: 1020,
            known_start: true,
            first: 56,
            last: 56,
        });
        assert!(predict(&calendar, "app", "spire", "Slay the Spire 2", &visits, now).is_empty());
        visits.pop();
        assert!(
            predict(
                &calendar,
                "app",
                "spire",
                "Slay the Spire 2",
                &visits,
                today + 22 * 3600
            )
            .is_empty()
        );
        for visit in &mut visits {
            visit.known_start = false;
        }
        assert!(predict(&calendar, "app", "spire", "Slay the Spire 2", &visits, now).is_empty());
    }
    #[test]
    fn recent_pattern_is_tentative_and_unobserved_days_do_not_count_as_misses() {
        let end = Local::now().date_naive();
        let first = end - Duration::days(56);
        let today = midnight(end).unwrap();
        let observed = Coverage::new(vec![(midnight(end - Duration::days(14)).unwrap(), today)]);
        let calendar = Calendar::new(end, &observed, &observed).unwrap();
        let visits = (42..56)
            .map(|day| {
                let start = midnight(first + Duration::days(day)).unwrap() + 20 * 3600;
                Visit {
                    start,
                    end: start + 900,
                    seconds: 900,
                    known_start: true,
                    first: day as usize,
                    last: day as usize,
                }
            })
            .collect::<Vec<_>>();
        let result = predict(
            &calendar,
            "app",
            "spire",
            "Spire",
            &visits,
            today + 19 * 3600 + 45 * 60,
        );
        assert_eq!(result[0].confidence, InsightConfidence::Low);
        assert_eq!(result[0].supporting.eligible_count, Some(14));
        assert_eq!(
            result[0].supporting.routine.as_ref().unwrap().status,
            "recent"
        );
    }
    #[test]
    fn walk_forward_replay_uses_only_prior_observed_starts() {
        let first = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        let mut hits = 0;
        for day in 1..56 {
            let cutoff = first + Duration::days(day);
            let coverage =
                Coverage::new(vec![(midnight(first).unwrap(), midnight(cutoff).unwrap())]);
            let calendar = Calendar::new(cutoff, &coverage, &coverage).unwrap();
            let visits = (0..day)
                .map(|d| {
                    let start = midnight(first + Duration::days(d)).unwrap() + 20 * 3600;
                    Visit {
                        start,
                        end: start + 1200,
                        seconds: 1200,
                        known_start: true,
                        first: d as usize,
                        last: d as usize,
                    }
                })
                .collect::<Vec<_>>();
            let now = midnight(cutoff).unwrap() + 19 * 3600 + 45 * 60;
            let result = predict(&calendar, "app", "spire", "Spire", &visits, now);
            if day < 7 {
                assert!(result.is_empty(), "thin history should abstain");
            }
            if day >= 42 {
                assert_eq!(result.len(), 1);
                assert!(result[0].supporting.expected_start.unwrap() > now);
                hits += 1;
            }
        }
        assert_eq!(hits, 14);
    }
    #[test]
    fn natural_copy_preserves_the_underlying_routine() {
        let times = (42..56).map(|d| (d, 1230, 30)).collect::<Vec<_>>();
        let mut insight = detect_fixture(&times).remove(0);
        let evidence = insight.evidence.clone();
        let routine = insight.supporting.routine.clone();
        let dates = insight.supporting.matching_dates.clone();
        humanize(&mut insight);
        assert!(insight.title.contains("Game"));
        assert!(insight.title.contains("most evenings"));
        assert!(insight.value.contains("PM"));
        assert!(insight.value.contains("14 of 14 days"));
        assert!(insight.explanation.contains("evenings"));
        assert_eq!(insight.evidence, evidence);
        assert_eq!(insight.supporting.routine, routine);
        assert_eq!(insight.supporting.matching_dates, dates);
        for jargon in ["foreground", "eligible", "baseline", "recurrence", "median"] {
            assert!(!insight.explanation.contains(jargon));
        }
    }

    #[test]
    fn routine_wording_varies_without_changing_or_randomizing_facts() {
        let times = (42..56).map(|d| (d, 1230, 30)).collect::<Vec<_>>();
        let mut original = detect_fixture(&times).remove(0);
        original.supporting.baseline_seconds = Some(1800);
        let mut titles = std::collections::BTreeSet::new();
        let mut explanations = std::collections::BTreeSet::new();
        for n in 0..32 {
            let mut insight = original.clone();
            insight.supporting.activity_key = Some(format!("activity-{n}"));
            humanize(&mut insight);
            let rendered = insight.clone();
            humanize(&mut insight);
            assert_eq!(rendered, insight);
            assert_eq!(insight.evidence, original.evidence);
            assert_eq!(
                insight.supporting.matching_dates,
                original.supporting.matching_dates
            );
            assert_eq!(insight.supporting.routine, original.supporting.routine);
            assert!(insight.explanation.contains("30 minutes"));
            if insight.supporting.routine.as_ref().unwrap().timing_basis != "visit-start" {
                assert!(insight.explanation.to_lowercase().contains("across visits"));
                assert!(
                    insight
                        .explanation
                        .contains(insight.supporting.hour_label.as_ref().unwrap())
                );
            }
            titles.insert(insight.title);
            explanations.insert(insight.explanation);
        }
        assert_eq!(titles.len(), 4);
        assert_eq!(explanations.len(), 5);
    }

    #[test]
    fn tentative_short_visits_keep_their_qualifications_and_cadence() {
        let times = (42..56).map(|d| (d, 1230, 30)).collect::<Vec<_>>();
        let mut insight = detect_fixture(&times).remove(0);
        insight.confidence = InsightConfidence::Low;
        insight.supporting.baseline_seconds = Some(300);
        insight.supporting.weekday_label = Some("Monday".into());
        insight.supporting.weekday = Some(0);
        let routine = insight.supporting.routine.as_mut().unwrap();
        routine.cadence = "weekly".into();
        routine.timing_basis = "visit-start".into();
        routine.visit_start_window = None;
        humanize(&mut insight);
        assert!(insight.explanation.contains("Monday evenings"));
        assert!(insight.explanation.contains("5 minutes"));
        assert!(insight.explanation.contains("may be a pattern"));
        assert!(insight.explanation.contains("brief visits"));
        assert!(!insight.explanation.contains("often"));
    }

    #[test]
    fn typical_start_duration_counts_individual_visits_and_wraps_midnight() {
        let window = Window {
            start: 94,
            width: 8,
            app: 0,
            domain: 0,
        };
        let visits = vec![
            (94, 600),
            (97, 1200),
            (96 + 94, 1800),
            (192 + 1, 2400),
            (192 + 10, 9999),
        ];
        assert_eq!(typical_visit(&visits, &window, 0b11), 1500);
        assert_eq!(typical_visit(&visits, &window, 0b01), 900);
    }
    #[test]
    fn recent_evening_routine_survives_weekday_and_hour_jitter() {
        let times = [
            (43, 1257, 75),
            (45, 1233, 70),
            (46, 1276, 69),
            (48, 1270, 74),
            (49, 1215, 115),
            (50, 1283, 76),
            (51, 1265, 71),
            (52, 1262, 56),
            (54, 1181, 75),
            (55, 1196, 127),
        ];
        let results = detect_fixture(&times);
        let routine = results
            .iter()
            .find(|r| r.supporting.routine.as_ref().unwrap().cadence == "everyday")
            .expect("nightly routine");
        let r = routine.supporting.routine.as_ref().unwrap();
        assert_eq!(r.status, "recent");
        assert_eq!(routine.supporting.occurrence_count, Some(10));
        assert_eq!(routine.supporting.eligible_count, Some(14));
        assert_eq!(routine.confidence, InsightConfidence::Medium);
        assert!((19 * 60..=22 * 60).contains(&r.start_minute));
        assert_eq!(
            routine.supporting.matching_dates.as_ref().unwrap().len(),
            10
        );
        assert!(results.len() <= 3);
    }
    #[test]
    fn start_evidence_excludes_censored_visits() {
        let times = (0..56).map(|d| (d, 1200, 60)).collect::<Vec<_>>();
        let (c, s, mut v) = fixture(&times, None);
        for visit in &mut v {
            visit.known_start = false;
        }
        let result = detect(&c, "app", "game", "Game", &s, &v);
        assert!(!result.is_empty());
        assert!(
            result
                .iter()
                .all(|i| i.supporting.routine.as_ref().unwrap().timing_basis == "usage")
        );
    }
    #[test]
    fn established_routines_prefer_long_baseline_and_merge_evidence() {
        let times = (0..56).map(|d| (d, 1200, 30)).collect::<Vec<_>>();
        let results = detect_fixture(&times);
        assert_eq!(results.len(), 1);
        let i = &results[0];
        let r = i.supporting.routine.as_ref().unwrap();
        assert_eq!(i.supporting.occurrence_count, Some(56));
        assert_eq!(r.status, "established");
        assert_eq!(i.confidence, InsightConfidence::High);
        assert_eq!(r.timing_basis, "usage-and-visit-start");
        assert!(r.visit_start_window.is_some());
    }
    #[test]
    fn weekly_habits_remain_weekly() {
        let times = (0..8).map(|w| (w * 7, 1200, 30)).collect::<Vec<_>>();
        let results = detect_fixture(&times);
        assert!(!results.is_empty());
        assert!(results.iter().all(|r| r.supporting.weekday.is_some()));
        assert_eq!(results[0].supporting.occurrence_count, Some(8));
    }
    #[test]
    fn random_timing_and_all_day_foreground_do_not_invent_specific_hours() {
        let random = (0..56)
            .map(|d| (d, ((d * 317) % 1440) as u32, 20))
            .collect::<Vec<_>>();
        assert!(detect_fixture(&random).is_empty());
        let all_day = (0..56).map(|d| (d, 0, 1440)).collect::<Vec<_>>();
        let (c, s, _) = fixture(&all_day, None);
        assert!(detect(&c, "app", "always", "Always", &s, &[]).is_empty());
    }
    #[test]
    fn midnight_windows_count_one_anchor_date_and_never_use_future() {
        let times = (42..55)
            .map(|d| (d, if d % 2 == 0 { 1430 } else { 1415 }, 45))
            .collect::<Vec<_>>();
        let (c, mut s, mut v) = fixture(&times, None);
        let before = detect(&c, "app", "night", "Night", &s, &v);
        assert!(!before.is_empty());
        let index = s.len();
        s.push(Slice {
            start: c.to + 1200,
            end: c.to + 2400,
        });
        v.push(Visit {
            start: c.to + 1200,
            end: c.to + 2400,
            seconds: 1200,
            known_start: true,
            first: index,
            last: index,
        });
        assert_eq!(before, detect(&c, "app", "night", "Night", &s, &v));
        let r = before[0].supporting.routine.as_ref().unwrap();
        assert!(r.start_minute >= 22 * 60);
        assert!(
            before[0]
                .supporting
                .matching_dates
                .as_ref()
                .unwrap()
                .windows(2)
                .all(|p| p[0] != p[1])
        );
    }
    #[test]
    fn sparse_recent_evidence_and_discontinued_habits_are_not_new_habits() {
        assert!(
            detect_fixture(&[
                (50, 1200, 30),
                (51, 1200, 30),
                (52, 1200, 30),
                (53, 1200, 30)
            ])
            .is_empty()
        );
        let times = (0..14).map(|d| (d, 1200, 30)).collect::<Vec<_>>();
        assert!(detect_fixture(&times).is_empty());
    }
}
