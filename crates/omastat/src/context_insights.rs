//! Observation-backed context, kept descriptive rather than a productivity score.
use crate::{
    activity::{AnalysisContext, midnight, subtract_windows},
    analytics,
    config::Config,
    identity,
    insights::{
        Insight, InsightCategory, InsightConfidence, InsightEvidence, InsightKind, InsightSupport,
        InsightTone, TrendComparison,
    },
    multitasking::MultitaskingReport,
};
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
struct Block {
    app: String,
    start: i64,
    end: i64,
    date: NaiveDate,
}

fn median(mut values: Vec<i64>) -> i64 {
    values.sort_unstable();
    analytics::median(&values)
}

// Intersect with observation and subtract pauses before constructing stretches.
// Audio changes and window-title changes never create artificial app switches.
fn blocks(context: &AnalysisContext, from: i64, to: i64) -> Result<Vec<Block>> {
    let mut out: Vec<Block> = Vec::new();
    for interval in &context.focus {
        let a = interval.started_at.max(from);
        let b = interval.ended_at.min(to);
        if b <= a {
            continue;
        }
        let first = context
            .observation
            .windows
            .partition_point(|(_, end)| *end <= a);
        for &(x, y) in &context.observation.windows[first..] {
            if x >= b {
                break;
            }
            let x = x.max(a);
            let y = y.min(b);
            let gap = context.pauses.windows.partition_point(|(_, end)| *end <= x);
            for (mut start, end) in subtract_windows(&[(x, y)], &context.pauses.windows[gap..]) {
                while start < end {
                    let date = Local.timestamp_opt(start, 0).single().unwrap().date_naive();
                    let stop = end.min(midnight(date + Duration::days(1))?);
                    let app = identity::canonical_app_class(&interval.app_class);
                    if let Some(last) = out.last_mut()
                        && last.app == app
                        && last.end == start
                        && last.date == date
                    {
                        last.end = stop;
                    } else {
                        out.push(Block {
                            app,
                            start,
                            end: stop,
                            date,
                        });
                    }
                    start = stop;
                }
            }
        }
    }
    Ok(out)
}

fn evidence(points: usize, minimum: usize, seconds: i64) -> InsightEvidence {
    InsightEvidence {
        data_points: points,
        minimum_data_points: minimum,
        observed_focus_seconds: seconds,
        observed_open_seconds: 0,
    }
}

pub(crate) fn history(
    context: &AnalysisContext,
    config: &Config,
    cutoff: NaiveDate,
    selector: Option<(&str, &str)>,
) -> Result<Vec<Insight>> {
    // App transitions cannot be inferred from website totals or browser titles.
    if selector.is_some_and(|(kind, _)| kind != "app") {
        return Ok(Vec::new());
    }
    let from = cutoff - Duration::days(35);
    let blocks = blocks(context, midnight(from)?, midnight(cutoff)?)?;
    let mut out = Vec::new();
    if let Some(trend) = stretch_trend(context, config, &blocks, cutoff, selector)? {
        out.push(trend);
    }
    if let Some(handoff) = handoff(config, &blocks, cutoff, selector) {
        out.push(handoff);
    }
    Ok(out)
}

fn stretch_trend(
    context: &AnalysisContext,
    config: &Config,
    blocks: &[Block],
    cutoff: NaiveDate,
    selector: Option<(&str, &str)>,
) -> Result<Option<Insight>> {
    let mut days = BTreeMap::<NaiveDate, Vec<&Block>>::new();
    for b in blocks {
        if selector.is_none_or(|(_, key)| key == b.app) {
            days.entry(b.date).or_default().push(b);
        }
    }
    let mut samples = BTreeMap::new();
    for (date, list) in days {
        let total: i64 = list.iter().map(|b| b.end - b.start).sum();
        if list.len() < 5
            || total < 1800
            || context
                .observation
                .covered(midnight(date)?, midnight(date + Duration::days(1))?)
                < 4 * 3600
            || !context
                .observation
                .eligible(list[0].start, list.last().unwrap().end)
        {
            continue;
        }
        samples.insert(date, median(list.iter().map(|b| b.end - b.start).collect()));
    }
    let recent_start = cutoff - Duration::days(7);
    let mut pairs = Vec::new();
    let mut baseline_dates = BTreeSet::new();
    let mut noise = Vec::new();
    for (&date, &value) in samples.range(recent_start..) {
        let baseline: Vec<_> = samples
            .range(..recent_start)
            .filter(|(d, _)| d.weekday() == date.weekday())
            .collect();
        if baseline.len() < 2 {
            continue;
        }
        let expected = median(baseline.iter().map(|(_, n)| **n).collect());
        noise.extend(baseline.iter().map(|(_, n)| (**n - expected).abs()));
        baseline_dates.extend(baseline.iter().map(|(d, _)| d.to_string()));
        pairs.push((date, value, expected));
    }
    if pairs.len() < 4 || baseline_dates.len() < 8 {
        return Ok(None);
    }
    let current = median(pairs.iter().map(|(_, n, _)| *n).collect());
    let baseline = median(pairs.iter().map(|(_, _, n)| *n).collect());
    let delta = current - baseline;
    let threshold = 60.max(baseline / 4).max(median(noise).saturating_mul(2));
    let agreeing = pairs
        .iter()
        .filter(|(_, n, old)| (*n - *old).signum() == delta.signum())
        .count();
    if delta.abs() < threshold || delta == 0 || agreeing * 4 < pairs.len() * 3 {
        return Ok(None);
    }
    let direction = if delta > 0 { "longer" } else { "shorter" };
    let label = selector.map(|(_, key)| config.app_label(key, || identity::display_name(key)));
    let subject = label.as_deref().unwrap_or("Your app");
    let current_dates: Vec<_> = pairs.iter().map(|(d, _, _)| d.to_string()).collect();
    let comparison = TrendComparison {
        current_seconds: current,
        baseline_seconds: baseline,
        current_dates: current_dates.clone(),
        baseline_dates: baseline_dates.into_iter().collect(),
    };
    Ok(Some(Insight {
        kind: InsightKind::StretchTrend, category: InsightCategory::Patterns, tone: InsightTone::Info,
        title: format!("{subject} stretches have been {direction}"),
        value: format!("{} → {}", analytics::format_duration(baseline), analytics::format_duration(current)),
        explanation: format!("Across {} comparable days in the last week, the middle uninterrupted stretch was {}, versus {} on matching weekdays in the prior four weeks.", pairs.len(), analytics::format_duration(current), analytics::format_duration(baseline)),
        confidence: InsightConfidence::Medium,
        evidence: evidence(pairs.len() + comparison.baseline_dates.len(), 12, blocks.iter().filter(|b| selector.is_none_or(|(_, key)| key == b.app)).map(|b| b.end-b.start).sum()),
        supporting: InsightSupport {
            activity_kind: selector.map(|_| "app".into()), activity_key: selector.map(|(_, key)| key.into()), app_label: label,
            period_start_date: Some((cutoff - Duration::days(35)).to_string()), period_end_date: Some((cutoff - Duration::days(1)).to_string()),
            matching_dates: Some(current_dates), comparison: Some(comparison),
            method: Some("Completed local days only. Each day needs five stretches, 30 minutes of focused use, four hours of observation, and 90% coverage between its first and last recorded stretch. Recent days are matched to at least two earlier instances of the same weekday. At least four recent and eight earlier days must qualify. A change must reach one minute, 25% of the baseline and twice the baseline's median absolute deviation; at least 75% of paired days must agree. Pauses and missing observation break stretches.".into()),
            ..Default::default()
        },
    }))
}

fn handoff(
    config: &Config,
    blocks: &[Block],
    cutoff: NaiveDate,
    selector: Option<(&str, &str)>,
) -> Option<Insight> {
    let first = cutoff - Duration::days(28);
    let mut pairs = BTreeMap::<(&str, &str), (usize, BTreeSet<NaiveDate>, i64)>::new();
    for adjacent in blocks.windows(2) {
        let (a, b) = (&adjacent[0], &adjacent[1]);
        if a.date < first
            || a.date != b.date
            || a.end != b.start
            || a.app == b.app
            || a.end - a.start < 30
            || b.end - b.start < 30
            || selector.is_some_and(|(_, key)| key != a.app && key != b.app)
        {
            continue;
        }
        let entry = pairs.entry((&a.app, &b.app)).or_default();
        entry.0 += 1;
        entry.1.insert(a.date);
        entry.2 += b.end - b.start;
    }
    let ((from, to), (count, days, seconds)) = pairs
        .into_iter()
        .filter(|(_, (count, days, _))| *count >= 6 && days.len() >= 3)
        .max_by(|a, b| {
            a.1.1
                .len()
                .cmp(&b.1.1.len())
                .then(a.1.0.cmp(&b.1.0))
                .then(b.0.cmp(&a.0))
        })?;
    let from_label = config.app_label(from, || identity::display_name(from));
    let to_label = config.app_label(to, || identity::display_name(to));
    Some(Insight {
        kind: InsightKind::AppHandoff, category: InsightCategory::Patterns, tone: InsightTone::Info,
        title: format!("From {from_label} to {to_label}"), value: format!("{count} transitions · {} days", days.len()),
        explanation: format!("You moved directly from {from_label} to {to_label} {count} times across {} recorded days in the last four weeks. Both stays lasted at least 30 seconds.", days.len()),
        confidence: if days.len() >= 7 { InsightConfidence::High } else { InsightConfidence::Medium },
        evidence: evidence(days.len(), 3, seconds),
        supporting: InsightSupport {
            activity_kind: Some("app".into()), activity_key: Some(to.into()), app_label: Some(to_label),
            occurrence_count: Some(count), matching_dates: Some(days.iter().map(ToString::to_string).collect()),
            period_start_date: Some(first.to_string()), period_end_date: Some((cutoff-Duration::days(1)).to_string()),
            method: Some("Only directly adjacent, observed app stretches count. Each side must last at least 30 seconds. Pauses, observation gaps, midnight and same-app title changes do not count as handoffs. At least six transitions on three distinct completed days are required; only the strongest pair is shown.".into()),
            ..Default::default()
        },
    })
}

pub(crate) fn audio(
    report: &MultitaskingReport,
    focused: i64,
    config: &Config,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Option<Insight> {
    let source = report.sources.first()?;
    if focused < 1800 || source.seconds < 600 || source.seconds * 2 < report.total_seconds {
        return None;
    }
    let domain = source.attribution == "domain";
    let label = if domain {
        source.label.clone()
    } else {
        config.app_label(&source.app_class, || source.label.clone())
    };
    let share = (source.seconds as f64 / focused as f64).clamp(0.0, 1.0);
    Some(Insight {
        kind: InsightKind::AudioCompanion, category: InsightCategory::Patterns, tone: InsightTone::Info,
        title: format!("{label}, in the background"), value: format!("{} · {:.0}% of focused time", analytics::format_duration(source.seconds), share*100.0),
        explanation: format!("{label} played in the background for {} while you used other apps in this period. That playback overlaps your focused time.", analytics::format_duration(source.seconds)),
        confidence: InsightConfidence::Medium,
        evidence: evidence(report.daily.len(), 1, focused),
        supporting: InsightSupport {
            activity_kind: Some(if domain { "domain" } else { "app" }.into()),
            activity_key: Some(if domain { source.label.clone() } else { source.app_class.clone() }), app_label: Some(label),
            focused_seconds: Some(focused), total_seconds: Some(source.seconds), share: Some(share),
            period_start_date: start_date.map(str::to_owned), period_end_date: end_date.map(str::to_owned),
            method: Some("A description of this report period, not a long-term habit. Requires 30 minutes of focused activity and at least ten minutes from one source covering half of recorded audio overlap. Simultaneous streams from that source are unioned. Audio comes from live playback snapshots and, when enabled, audible website domains. Missing audio history is never interpreted as silence.".into()),
            ..Default::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        activity::Coverage,
        multitasking::{MultitaskingDay, MultitaskingSource},
        storage::{IntervalKind, TimelineInterval},
    };

    fn fixture(recent: i64) -> (AnalysisContext, NaiveDate) {
        let cutoff = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        let mut context = AnalysisContext {
            metadata: vec![],
            focus: vec![],
            domains: vec![],
            observation: Coverage::new(vec![(
                midnight(cutoff - Duration::days(35)).unwrap(),
                midnight(cutoff).unwrap(),
            )]),
            domain_observation: Coverage::new(vec![]),
            pauses: Coverage::new(vec![]),
            scan_start: midnight(cutoff - Duration::days(35)).unwrap(),
        };
        for n in 0..35 {
            let date = cutoff - Duration::days(35 - n);
            let duration = if n >= 28 { recent } else { 600 };
            let mut at = midnight(date).unwrap() + 9 * 3600;
            for i in 0..12 {
                context.focus.push(TimelineInterval {
                    kind: IntervalKind::Focused,
                    app_class: if i % 2 == 0 { "editor" } else { "browser" }.into(),
                    started_at: at,
                    ended_at: at + duration,
                });
                at += duration;
            }
        }
        (context, cutoff)
    }
    fn trend(context: &AnalysisContext, cutoff: NaiveDate) -> Option<Insight> {
        history(context, &Config::default(), cutoff, None)
            .unwrap()
            .into_iter()
            .find(|i| i.kind == InsightKind::StretchTrend)
    }
    #[test]
    fn matched_weekday_change_has_inspectable_before_after_evidence() {
        let (context, cutoff) = fixture(1200);
        let finding = trend(&context, cutoff).unwrap();
        let comparison = finding.supporting.comparison.unwrap();
        assert_eq!(comparison.current_seconds, 1200);
        assert_eq!(comparison.baseline_seconds, 600);
        assert_eq!(comparison.current_dates.len(), 7);
        assert_eq!(comparison.baseline_dates.len(), 28);
        assert_eq!(finding.tone, InsightTone::Info);
        assert!(finding.supporting.method.unwrap().contains("90%"));
        let selected = history(
            &context,
            &Config::default(),
            cutoff,
            Some(("app", "editor")),
        )
        .unwrap();
        assert!(selected.iter().any(|i| i.kind == InsightKind::StretchTrend
            && i.supporting.activity_key.as_deref() == Some("editor")));
        assert!(
            history(
                &context,
                &Config::default(),
                cutoff,
                Some(("domain", "example.com"))
            )
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn missing_tracking_pauses_and_thin_history_do_not_make_a_trend() {
        let (mut context, cutoff) = fixture(1200);
        let recent = midnight(cutoff - Duration::days(7)).unwrap();
        context.focus.retain(|i| {
            i.started_at < recent || i.started_at >= midnight(cutoff - Duration::days(3)).unwrap()
        });
        assert!(trend(&context, cutoff).is_none());
        let (mut context, cutoff) = fixture(1200);
        context.observation = Coverage::new(vec![]);
        assert!(
            history(&context, &Config::default(), cutoff, None)
                .unwrap()
                .is_empty()
        );
        let (mut context, cutoff) = fixture(1200);
        context.pauses = Coverage::new(context.observation.windows.clone());
        assert!(
            history(&context, &Config::default(), cutoff, None)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn one_unusual_day_does_not_become_a_change() {
        let (mut context, cutoff) = fixture(600);
        let day = midnight(cutoff - Duration::days(1)).unwrap();
        for i in &mut context.focus {
            if i.started_at >= day {
                i.started_at = day + (i.started_at - day) * 2;
                i.ended_at = day + (i.ended_at - day) * 2;
            }
        }
        assert!(trend(&context, cutoff).is_none());
    }
    #[test]
    fn inconsistent_recent_days_and_noisy_baselines_are_suppressed() {
        let (mut context, cutoff) = fixture(1200);
        for i in &mut context.focus {
            let date = Local
                .timestamp_opt(i.started_at, 0)
                .single()
                .unwrap()
                .date_naive();
            let anchor = midnight(date).unwrap() + 9 * 3600;
            if date >= cutoff - Duration::days(4) {
                i.started_at = anchor + (i.started_at - anchor) / 4;
                i.ended_at = anchor + (i.ended_at - anchor) / 4;
            }
        }
        assert!(trend(&context, cutoff).is_none());
        let (mut context, cutoff) = fixture(1200);
        for i in &mut context.focus {
            let date = Local
                .timestamp_opt(i.started_at, 0)
                .single()
                .unwrap()
                .date_naive();
            let n = (date - (cutoff - Duration::days(35))).num_days();
            if n < 28 {
                let seconds = [300, 900, 2100, 600][n as usize / 7];
                let anchor = midnight(date).unwrap() + 9 * 3600;
                i.started_at = anchor + (i.started_at - anchor) * seconds / 600;
                i.ended_at = anchor + (i.ended_at - anchor) * seconds / 600;
            }
        }
        assert!(trend(&context, cutoff).is_none());
    }
    #[test]
    fn handoffs_need_repetition_across_days_and_do_not_bridge_gaps() {
        let (mut context, cutoff) = fixture(600);
        let output = history(&context, &Config::default(), cutoff, None).unwrap();
        let finding = output
            .iter()
            .find(|i| i.kind == InsightKind::AppHandoff)
            .unwrap();
        assert_eq!(
            finding.supporting.matching_dates.as_ref().unwrap().len(),
            28
        );
        assert!(finding.supporting.occurrence_count.unwrap() >= 6);
        for (n, i) in context.focus.iter_mut().enumerate() {
            i.started_at += n as i64;
            i.ended_at += n as i64;
        }
        assert!(
            !history(&context, &Config::default(), cutoff, None)
                .unwrap()
                .iter()
                .any(|i| i.kind == InsightKind::AppHandoff)
        );
        let (mut context, cutoff) = fixture(600);
        context
            .focus
            .retain(|i| i.started_at >= midnight(cutoff - Duration::days(1)).unwrap());
        assert!(
            !history(&context, &Config::default(), cutoff, None)
                .unwrap()
                .iter()
                .any(|i| i.kind == InsightKind::AppHandoff)
        );
    }
    #[test]
    fn title_fragments_merge_and_future_records_do_not_affect_history() {
        let (mut context, cutoff) = fixture(1200);
        let expected = history(&context, &Config::default(), cutoff, None).unwrap();
        context.focus = context
            .focus
            .iter()
            .flat_map(|i| {
                let mut a = i.clone();
                let mut b = i.clone();
                a.ended_at = (i.started_at + i.ended_at) / 2;
                b.started_at = a.ended_at;
                [a, b]
            })
            .collect();
        assert_eq!(
            expected,
            history(&context, &Config::default(), cutoff, None).unwrap()
        );
        let at = midnight(cutoff).unwrap() + 3600;
        context.focus.push(TimelineInterval {
            kind: IntervalKind::Focused,
            app_class: "future-private-app".into(),
            started_at: at,
            ended_at: at + 3600,
        });
        assert_eq!(
            expected,
            history(&context, &Config::default(), cutoff, None).unwrap()
        );
    }
    #[test]
    fn audio_uses_resolved_domains_and_reports_overlap_without_inference() {
        let mut report = MultitaskingReport {
            total_seconds: 1800,
            sources: vec![MultitaskingSource {
                app_class: "zen".into(),
                label: "youtube.com".into(),
                attribution: "domain".into(),
                seconds: 1200,
            }],
            daily: vec![MultitaskingDay {
                date: "2026-09-20".into(),
                seconds: 1800,
            }],
            ..Default::default()
        };
        let finding = audio(
            &report,
            3600,
            &Config::default(),
            Some("2026-09-20"),
            Some("2026-09-20"),
        )
        .unwrap();
        assert_eq!(finding.supporting.activity_kind.as_deref(), Some("domain"));
        assert_eq!(
            finding.supporting.activity_key.as_deref(),
            Some("youtube.com")
        );
        assert!(
            finding
                .supporting
                .method
                .unwrap()
                .contains("never interpreted as silence")
        );
        assert!(audio(&report, 1200, &Config::default(), None, None).is_none());
        report.sources[0].seconds = 599;
        assert!(audio(&report, 3600, &Config::default(), None, None).is_none());
        assert!(
            audio(
                &MultitaskingReport::default(),
                3600,
                &Config::default(),
                None,
                None
            )
            .is_none()
        );
    }
}
