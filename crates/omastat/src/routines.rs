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
    insight.title = format!("{label} {when}");
    insight.value = format!("{range} · {count} of {sample} {unit}");
    support.hour_label = Some(range.clone());
    let recent = if routine.status == "recent" {
        "Lately, you've"
    } else {
        "You've"
    };
    let duration = analytics::duration_words(support.baseline_seconds.unwrap_or(0));
    insight.explanation = if routine.timing_basis == "visit-start" {
        format!(
            "{recent} often come back here around this time. A typical visit adds up to about {duration} of use."
        )
    } else {
        format!("{recent} often spent about {duration} here around this time.")
    };
    if let Some((start, end)) = routine.visit_start_window {
        insight.explanation.push_str(&format!(
            " You tend to start between {}.",
            analytics::clock_range(start, end).replace('–', " and ")
        ));
    }
    insight
        .explanation
        .push_str(" Days with too much missing tracking aren't counted.");
    if insight.confidence == InsightConfidence::Low {
        insight
            .explanation
            .push_str(" This is still an early hint.");
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
    fn natural_copy_preserves_the_underlying_routine() {
        let times = (42..56).map(|d| (d, 1230, 30)).collect::<Vec<_>>();
        let mut insight = detect_fixture(&times).remove(0);
        let evidence = insight.evidence.clone();
        let routine = insight.supporting.routine.clone();
        let dates = insight.supporting.matching_dates.clone();
        humanize(&mut insight);
        assert_eq!(insight.title, "Game most evenings");
        assert!(insight.value.contains("PM"));
        assert!(insight.value.contains("14 of 14 days"));
        assert!(insight.explanation.starts_with("Lately, you've"));
        assert_eq!(insight.evidence, evidence);
        assert_eq!(insight.supporting.routine, routine);
        assert_eq!(insight.supporting.matching_dates, dates);
        for jargon in ["foreground", "eligible", "baseline", "recurrence", "median"] {
            assert!(!insight.explanation.contains(jargon));
        }
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
