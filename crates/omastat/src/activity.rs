//! Foreground visits and observation-backed analytics shared by report consumers.
use crate::{
    analytics,
    config::Config,
    identity,
    insights::{
        Insight, InsightCategory, InsightConfidence, InsightEvidence, InsightKind, InsightSupport,
        InsightTone,
    },
    steam::SteamResolver,
    storage::{
        BrowserDomainTotals, FocusHeatCell, FocusedIntervalMetadata, Storage, TimelineInterval,
    },
};
use anyhow::Result;
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Default)]
pub struct ActivityStats {
    pub kind: String,
    pub key: String,
    pub label: String,
    pub focused_seconds: i64,
    pub days_used: usize,
    pub visits: usize,
    pub median_visit_seconds: i64,
}
#[derive(Debug, Clone, Serialize, Default)]
pub struct ActivityDay {
    pub date: String,
    pub label: String,
    pub focused_seconds: i64,
    pub observed_seconds: i64,
}
#[derive(Debug, Clone, Serialize, Default)]
pub struct ActivityAnalytics {
    pub baseline_start: String,
    pub baseline_end: String,
    pub eligible_days: usize,
    pub activities: Vec<ActivityStats>,
    pub insights: Vec<Insight>,
    pub daily: Vec<ActivityDay>,
    pub heatmap: Vec<FocusHeatCell>,
    pub browser_domains_enabled: bool,
    pub unattributed_browser_seconds: i64,
}
#[derive(Clone)]
pub(crate) struct Slice {
    pub start: i64,
    pub end: i64,
}

pub fn midnight(date: NaiveDate) -> Result<i64> {
    let mut naive = date.and_hms_opt(0, 0, 0).unwrap();
    for _ in 0..180 {
        if let Some(time) = Local.from_local_datetime(&naive).earliest() {
            return Ok(time.timestamp());
        }
        naive += Duration::minutes(1);
    }
    anyhow::bail!("No local midnight for {date}")
}

/// Union intervals with cumulative lengths: overlap queries are O(log N).
#[derive(Debug, Default)]
pub struct Coverage {
    pub windows: Vec<(i64, i64)>,
    prefix: Vec<i64>,
}
impl Coverage {
    pub fn new(windows: Vec<(i64, i64)>) -> Self {
        let windows = crate::storage::merge_windows(windows);
        let mut prefix = vec![0];
        for &(a, b) in &windows {
            prefix.push(prefix.last().unwrap() + b - a);
        }
        Self { windows, prefix }
    }
    fn through(&self, at: i64) -> i64 {
        let index = self.windows.partition_point(|(_, b)| *b <= at);
        self.prefix.get(index).copied().unwrap_or(0)
            + self
                .windows
                .get(index)
                .map_or(0, |(a, b)| (at.min(*b) - a).max(0))
    }
    pub fn covered(&self, a: i64, b: i64) -> i64 {
        if b <= a {
            0
        } else {
            self.through(b) - self.through(a)
        }
    }
    pub fn eligible(&self, a: i64, b: i64) -> bool {
        b > a && self.covered(a, b) * 10 >= (b - a) * 9
    }
}
pub(crate) fn subtract_windows(spans: &[(i64, i64)], gaps: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut output = Vec::new();
    let mut index = 0;
    for &(a, b) in spans {
        let mut cursor = a;
        while index < gaps.len() && gaps[index].1 <= a {
            index += 1;
        }
        let mut j = index;
        while j < gaps.len() && gaps[j].0 < b {
            if gaps[j].0 > cursor {
                output.push((cursor, gaps[j].0.min(b)));
            }
            cursor = cursor.max(gaps[j].1).min(b);
            j += 1;
        }
        if cursor < b {
            output.push((cursor, b));
        }
    }
    output
}
fn slots(start: i64, end: i64, mut consume: impl FnMut(String, u32, u32, i64)) {
    let mut cursor = start;
    while cursor < end {
        let Some(local) = Local.timestamp_opt(cursor, 0).single() else {
            break;
        };
        let next = (cursor + 3600 - i64::from(local.minute() * 60 + local.second())).min(end);
        consume(
            local.format("%Y-%m-%d").to_string(),
            local.weekday().num_days_from_monday(),
            local.hour(),
            next - cursor,
        );
        cursor = next;
    }
}

/// One bounded telemetry load for routines, visit statistics, rollups and live pace.
pub(crate) struct AnalysisContext {
    pub metadata: Vec<FocusedIntervalMetadata>,
    pub focus: Vec<TimelineInterval>,
    pub domains: Vec<(String, TimelineInterval)>,
    pub observation: Coverage,
    pub domain_observation: Coverage,
    pub pauses: Coverage,
    pub scan_start: i64,
}
impl AnalysisContext {
    pub fn load(
        storage: &Storage,
        config: &Config,
        steam: &mut SteamResolver,
        start: i64,
        end: i64,
        baseline_end: NaiveDate,
    ) -> Result<Self> {
        let scan_start = start
            .min(midnight(baseline_end - Duration::weeks(8))?)
            .saturating_sub(300);
        let metadata = storage.focused_interval_metadata_between(scan_start, end)?;
        let focus = metadata
            .iter()
            .map(|i| TimelineInterval {
                kind: crate::storage::IntervalKind::Focused,
                app_class: steam.resolve_class(&i.app_class),
                started_at: i.started_at,
                ended_at: i.ended_at,
            })
            .collect::<Vec<_>>();
        let domains = if config.privacy.browser_domains {
            storage.browser_focused_intervals(scan_start, end)?
        } else {
            Vec::new()
        };
        let attributed = crate::storage::merge_windows(
            domains
                .iter()
                .map(|(_, i)| (i.started_at, i.ended_at))
                .collect(),
        );
        let browsers = crate::storage::merge_windows(
            focus
                .iter()
                .filter(|i| crate::browser::is_browser_class(&i.app_class))
                .map(|i| (i.started_at, i.ended_at))
                .collect(),
        );
        let unknown = subtract_windows(&browsers, &attributed);
        let observation = Coverage::new(storage.observation_windows(scan_start, end)?);
        let domain_observation = Coverage::new(subtract_windows(&observation.windows, &unknown));
        let pauses = Coverage::new(storage.pause_windows(scan_start, end)?);
        Ok(Self {
            metadata,
            focus,
            domains,
            observation,
            domain_observation,
            pauses,
            scan_start,
        })
    }
    pub fn domain_totals(&self, start: i64, end: i64) -> Vec<BrowserDomainTotals> {
        let mut totals = BTreeMap::new();
        for (app, i) in &self.domains {
            let amount = (i.ended_at.min(end) - i.started_at.max(start)).max(0);
            if amount > 0 {
                *totals
                    .entry((app.clone(), i.app_class.clone()))
                    .or_insert(0) += amount;
            }
        }
        let mut rows = totals
            .into_iter()
            .map(
                |((app_class, domain), focused_seconds)| BrowserDomainTotals {
                    app_class,
                    domain,
                    focused_seconds,
                },
            )
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| {
            a.app_class
                .cmp(&b.app_class)
                .then(b.focused_seconds.cmp(&a.focused_seconds))
                .then(a.domain.cmp(&b.domain))
        });
        rows
    }
}

pub fn analyze(
    storage: &Storage,
    config: &Config,
    steam: &mut SteamResolver,
    start: i64,
    end: i64,
    baseline_end: NaiveDate,
    selector: Option<(&str, &str)>,
) -> Result<ActivityAnalytics> {
    let context = AnalysisContext::load(storage, config, steam, start, end, baseline_end)?;
    analyze_context(&context, config, start, end, baseline_end, selector, true)
}

pub(crate) fn analyze_context(
    context: &AnalysisContext,
    config: &Config,
    start: i64,
    end: i64,
    baseline_end: NaiveDate,
    selector: Option<(&str, &str)>,
    charts: bool,
) -> Result<ActivityAnalytics> {
    let baseline_start = baseline_end - Duration::weeks(8);
    let mut grouped = BTreeMap::<(&str, String), Vec<Slice>>::new();
    for i in &context.focus {
        if selector.is_none_or(|(kind, key)| kind == "app" && key == i.app_class) {
            grouped
                .entry(("app", i.app_class.clone()))
                .or_default()
                .push(Slice {
                    start: i.started_at,
                    end: i.ended_at,
                });
        }
    }
    for (_, i) in &context.domains {
        if selector.is_none_or(|(kind, key)| kind == "domain" && key == i.app_class) {
            grouped
                .entry(("domain", i.app_class.clone()))
                .or_default()
                .push(Slice {
                    start: i.started_at,
                    end: i.ended_at,
                });
        }
    }
    let attributed: i64 = context
        .domains
        .iter()
        .map(|(_, i)| (i.ended_at.min(end) - i.started_at.max(start)).max(0))
        .sum();
    let browser_seconds: i64 = context
        .focus
        .iter()
        .filter(|i| crate::browser::is_browser_class(&i.app_class))
        .map(|i| (i.ended_at.min(end) - i.started_at.max(start)).max(0))
        .sum();
    let mut output = ActivityAnalytics {
        baseline_start: baseline_start.to_string(),
        baseline_end: (baseline_end - Duration::days(1)).to_string(),
        browser_domains_enabled: config.privacy.browser_domains,
        unattributed_browser_seconds: (browser_seconds - attributed).max(0),
        ..Default::default()
    };
    let grid = crate::routines::Calendar::new(
        baseline_end,
        &context.observation,
        &context.domain_observation,
    )?;
    for n in 0..56 {
        let date = baseline_start + Duration::days(n);
        if context
            .observation
            .eligible(midnight(date)?, midnight(date + Duration::days(1))?)
        {
            output.eligible_days += 1;
        }
    }
    let mut daily = BTreeMap::<String, i64>::new();
    let mut heat = BTreeMap::<(u32, u32), i64>::new();
    for ((kind, key), mut intervals) in grouped {
        intervals.sort_by_key(|i| i.start);
        let label = if kind == "app" {
            config.app_label(&key, || identity::display_name(&key))
        } else {
            key.clone()
        };
        let coverage = if kind == "domain" {
            &context.domain_observation
        } else {
            &context.observation
        };
        let visits = visits(&intervals, coverage, &context.pauses, context.scan_start);
        let mut days = BTreeSet::new();
        let mut seconds = 0;
        for i in &intervals {
            let a = i.start.max(start);
            let b = i.end.min(end);
            if b > a {
                seconds += b - a;
                slots(a, b, |date, w, h, amount| {
                    days.insert(date.clone());
                    if charts && (selector.is_some() || kind == "app") {
                        *daily.entry(date).or_default() += amount;
                        *heat.entry((w, h)).or_default() += amount;
                    }
                });
            }
        }
        if seconds > 0 || selector.is_some() {
            let mut durations = visit_amounts(&visits, &intervals, start, end);
            durations.sort_unstable();
            output.activities.push(ActivityStats {
                kind: kind.into(),
                key: key.clone(),
                label: label.clone(),
                focused_seconds: seconds,
                days_used: days.len(),
                visits: durations.len(),
                median_visit_seconds: analytics::median(&durations),
            });
        }
        output.insights.extend(crate::routines::detect(
            &grid, kind, &key, &label, &intervals, &visits,
        ));
    }
    output.activities.sort_by(|a, b| {
        b.focused_seconds
            .cmp(&a.focused_seconds)
            .then(a.key.cmp(&b.key))
    });
    crate::routines::rank(&mut output.insights);
    if selector.is_none() {
        let mut seen = BTreeSet::new();
        let mut first = Vec::new();
        let mut rest = Vec::new();
        for i in output.insights {
            if seen.insert((
                i.supporting.activity_kind.clone(),
                i.supporting.activity_key.clone(),
            )) {
                first.push(i)
            } else {
                rest.push(i)
            }
        }
        first.extend(rest);
        first.truncate(12);
        output.insights = first;
    }
    if charts && end > start {
        let first = Local.timestamp_opt(start, 0).single().unwrap().date_naive();
        let last = Local
            .timestamp_opt(end - 1, 0)
            .single()
            .unwrap()
            .date_naive();
        for n in 0..=(last - first).num_days() {
            let date = first + Duration::days(n);
            let key = date.to_string();
            output.daily.push(ActivityDay {
                date: key.clone(),
                label: date.format("%b %-d").to_string(),
                focused_seconds: *daily.get(&key).unwrap_or(&0),
                observed_seconds: context.observation.covered(
                    midnight(date)?.max(start),
                    midnight(date + Duration::days(1))?.min(end),
                ),
            });
        }
        output.heatmap = (0..7)
            .flat_map(|w| (0..24).map(move |h| (w, h)))
            .map(|(weekday, hour)| FocusHeatCell {
                weekday,
                hour,
                focused_seconds: *heat.get(&(weekday, hour)).unwrap_or(&0),
            })
            .collect();
    }
    for insight in &mut output.insights {
        crate::routines::humanize(insight);
    }
    Ok(output)
}

pub(crate) struct Visit {
    pub start: i64,
    pub end: i64,
    pub seconds: i64,
    pub known_start: bool,
    pub first: usize,
    pub last: usize,
}
fn visits(
    intervals: &[Slice],
    observation: &Coverage,
    pauses: &Coverage,
    scan_start: i64,
) -> Vec<Visit> {
    let mut visits: Vec<Visit> = Vec::new();
    for (index, i) in intervals.iter().enumerate() {
        if let Some(last) = visits.last_mut() {
            let gap = i.start - last.end;
            if gap <= 300
                && observation.covered(last.end, i.start) == gap.max(0)
                && pauses.covered(last.end, i.start) == 0
            {
                last.seconds += (i.end - i.start.max(last.end)).max(0);
                last.end = last.end.max(i.end);
                last.last = index;
                continue;
            }
        }
        visits.push(Visit {
            start: i.start,
            end: i.end,
            seconds: (i.end - i.start).max(0),
            known_start: i.start > scan_start && observation.covered(i.start - 300, i.start) == 300,
            first: index,
            last: index,
        });
    }
    visits
}
fn visit_amounts(visits: &[Visit], intervals: &[Slice], start: i64, end: i64) -> Vec<i64> {
    visits
        .iter()
        .filter_map(|v| {
            let seconds = intervals[v.first..=v.last]
                .iter()
                .map(|i| (i.end.min(end) - i.start.max(start)).max(0))
                .sum::<i64>();
            (seconds > 0).then_some(seconds)
        })
        .collect()
}
#[cfg(test)]
fn visit_durations(
    intervals: &[Slice],
    observation: &[(i64, i64)],
    pauses: &[(i64, i64)],
    start: i64,
    end: i64,
) -> Vec<i64> {
    visit_amounts(
        &visits(
            intervals,
            &Coverage::new(observation.to_vec()),
            &Coverage::new(pauses.to_vec()),
            i64::MIN,
        ),
        intervals,
        start,
        end,
    )
}

pub fn same_time_comparison(storage: &Storage, now: i64) -> Result<Option<Insight>> {
    let today = Local.timestamp_opt(now, 0).single().unwrap().date_naive();
    let context = AnalysisContext::load(
        storage,
        &Config::default(),
        &mut SteamResolver::default(),
        midnight(today)?,
        now,
        today,
    )?;
    same_time_from_context(&context, now)
}
pub(crate) fn same_time_from_context(
    context: &AnalysisContext,
    now: i64,
) -> Result<Option<Insight>> {
    let local = Local.timestamp_opt(now, 0).single().unwrap();
    let today = local.date_naive();
    let start = midnight(today)?;
    let windows = &context.observation;
    if !windows.eligible(start, now) {
        return Ok(None);
    }
    let focus = Coverage::new(
        context
            .focus
            .iter()
            .map(|i| (i.started_at, i.ended_at))
            .collect(),
    );
    let mut samples = Vec::new();
    let mut dates = Vec::new();
    for week in 1..=8 {
        let date = today - Duration::weeks(week);
        let a = midnight(date)?;
        let Some(cutoff) = Local
            .from_local_datetime(&date.and_time(local.time()))
            .earliest()
        else {
            continue;
        };
        let b = cutoff.timestamp();
        if windows.eligible(a, b) {
            samples.push(focus.covered(a, b));
            dates.push(date.to_string());
        }
    }
    if samples.len() < 3 {
        return Ok(None);
    }
    samples.sort_unstable();
    let baseline = analytics::median(&samples);
    let seconds = focus.covered(start, now);
    let delta = seconds - baseline;
    Ok(Some(Insight {
        kind: InsightKind::SameWeekdayPace,
        category: InsightCategory::Patterns,
        tone: InsightTone::Info,
        title: "Your day so far".into(),
        value: if delta == 0 {
            "About the same as usual".into()
        } else {
            format!(
                "{} {} than usual",
                analytics::format_duration(delta.abs()),
                if delta >= 0 { "more" } else { "less" }
            )
        },
        explanation: format!(
            "Compared with a typical {} at this time, using {} past {}s with enough tracking. Days when you didn't use any apps count too.",
            local.format("%A"),
            samples.len(),
            local.format("%A")
        ),
        confidence: if samples.len() >= 6 {
            InsightConfidence::High
        } else {
            InsightConfidence::Medium
        },
        evidence: InsightEvidence {
            data_points: samples.len(),
            minimum_data_points: 3,
            observed_focus_seconds: seconds,
            observed_open_seconds: 0,
        },
        supporting: InsightSupport {
            focused_seconds: Some(seconds),
            baseline_seconds: Some(baseline),
            delta_seconds: Some(delta),
            matching_dates: Some(dates),
            ..Default::default()
        },
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{IntervalKind, SessionIntervalKind};
    fn fixture() -> (tempfile::TempDir, Storage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &Config::default()).unwrap();
        (dir, storage)
    }
    fn focus(storage: &Storage, app: &str, a: i64, b: i64) {
        let id = storage
            .start_interval(IntervalKind::Focused, app, None, None, a)
            .unwrap();
        storage.close_interval(id, b).unwrap();
    }
    #[test]
    fn website_patterns_exclude_hours_without_domain_capture() {
        let (_dir, mut storage) = fixture();
        let end = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
        for week in (1..=8).rev() {
            let a = midnight(end - Duration::weeks(week)).unwrap() + 10 * 3600;
            let run = storage.start_daemon_run(a).unwrap();
            focus(&storage, "zen", a, a + 3600);
            if week <= 3 {
                for offset in (0..3600).step_by(30) {
                    storage
                        .record_browser_domain("zen", "zen", "example.com", a + offset)
                        .unwrap();
                }
                storage
                    .record_browser_state("zen", "zen", None, a + 3600)
                    .unwrap();
            }
            storage.finish_daemon_run(run.run_id, a + 3600).unwrap();
        }
        let result = analyze(
            &storage,
            &Config::default(),
            &mut SteamResolver::default(),
            midnight(end).unwrap(),
            midnight(end + Duration::days(1)).unwrap(),
            end,
            Some(("domain", "example.com")),
        )
        .unwrap();
        let routine = &result.insights[0];
        assert_eq!(routine.supporting.occurrence_count, Some(3));
        assert_eq!(routine.supporting.eligible_count, Some(3));
    }

    #[test]
    fn visits_group_brief_returns_but_never_count_time_away() {
        let intervals = vec![
            Slice { start: 0, end: 60 },
            Slice {
                start: 360,
                end: 420,
            },
            Slice {
                start: 721,
                end: 781,
            },
        ];
        assert_eq!(
            visit_durations(&intervals, &[(0, 1000)], &[], 0, 1000),
            vec![120, 60]
        );
        assert_eq!(
            visit_durations(&intervals, &[(0, 1000)], &[(100, 200)], 0, 1000),
            vec![60, 60, 60]
        );
        assert_eq!(
            visit_durations(&intervals, &[(0, 60), (360, 1000)], &[], 0, 1000),
            vec![60, 60, 60]
        );
        assert_eq!(
            visit_durations(&intervals, &[(0, 1000)], &[], 30, 400),
            vec![70]
        );
    }
    #[test]
    fn coverage_distinguishes_recorded_inactivity_and_clean_shutdown_gaps() {
        let (_dir, mut storage) = fixture();
        let run = storage.start_daemon_run(100).unwrap();
        focus(&storage, "editor", 110, 130);
        storage.finish_daemon_run(run.run_id, 200).unwrap();
        let second = storage.start_daemon_run(300).unwrap();
        storage.finish_daemon_run(second.run_id, 400).unwrap();
        assert_eq!(storage.observed_seconds_between(0, 500).unwrap(), 200);
    }
    #[test]
    fn dead_daemon_does_not_extend_unfinished_focus() {
        let (_dir, mut storage) = fixture();
        let run = storage.start_daemon_run(100).unwrap();
        storage
            .start_interval(IntervalKind::Focused, "editor", None, None, 100)
            .unwrap();
        storage.record_daemon_heartbeat(run.run_id, 160).unwrap();
        assert_eq!(
            storage.totals_between(100, 10000).unwrap()[0].focused_seconds,
            60
        );
        assert_eq!(
            storage
                .focused_rollups_between(100, 10000, 8, 64)
                .unwrap()
                .focus_intervals[0]
                .ended_at,
            160
        );
        assert_eq!(storage.observed_seconds_between(100, 10000).unwrap(), 60);
    }
    #[test]
    fn browser_clear_expiry_and_delayed_messages_do_not_misattribute() {
        let (_dir, mut storage) = fixture();
        focus(&storage, "zen", 100, 500);
        storage
            .record_browser_domain("a", "zen", "example.com", 100)
            .unwrap();
        storage
            .record_browser_domain("a", "zen", "example.com", 130)
            .unwrap();
        storage.record_browser_state("a", "zen", None, 150).unwrap();
        storage
            .record_browser_domain("a", "zen", "late.example", 140)
            .unwrap();
        storage
            .record_browser_domain("a", "zen", "next.example", 300)
            .unwrap();
        let rows = storage.browser_domain_totals_between(100, 500, 20).unwrap();
        assert_eq!(
            rows.iter()
                .find(|r| r.domain == "example.com")
                .unwrap()
                .focused_seconds,
            50
        );
        assert_eq!(
            rows.iter()
                .find(|r| r.domain == "next.example")
                .unwrap()
                .focused_seconds,
            90
        );
        assert!(!rows.iter().any(|r| r.domain == "late.example"));
        storage
            .record_browser_domain("a", "zen", "next.example", 450)
            .unwrap();
        let rows = storage.browser_domain_totals_between(100, 500, 20).unwrap();
        assert_eq!(
            rows.iter()
                .find(|r| r.domain == "next.example")
                .unwrap()
                .focused_seconds,
            140
        );
    }
    #[test]
    fn overlapping_browser_sources_and_app_time_reconcile() {
        let (_dir, mut storage) = fixture();
        focus(&storage, "zen", 100, 250);
        storage
            .record_browser_domain("a", "zen", "first.example", 100)
            .unwrap();
        storage
            .record_browser_domain("b", "zen", "second.example", 150)
            .unwrap();
        storage
            .record_browser_domain("b", "zen", "second.example", 200)
            .unwrap();
        let total: i64 = storage
            .browser_domain_totals_between(100, 250, 20)
            .unwrap()
            .iter()
            .map(|r| r.focused_seconds)
            .sum();
        assert_eq!(total, 150);
    }
    #[test]
    fn routines_include_early_morning_and_exclude_unknown_weeks() {
        let (_dir, mut storage) = fixture();
        let end = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
        for weeks in 1..=4 {
            let date = end - Duration::weeks(weeks);
            let a = midnight(date).unwrap() + 2 * 3600;
            let run = storage.start_daemon_run(a).unwrap();
            focus(&storage, "editor", a, a + 1800);
            storage.finish_daemon_run(run.run_id, a + 3600).unwrap();
        }
        let result = analyze(
            &storage,
            &Config::default(),
            &mut SteamResolver::default(),
            midnight(end).unwrap(),
            midnight(end + Duration::days(1)).unwrap(),
            end,
            None,
        )
        .unwrap();
        let routine = result
            .insights
            .iter()
            .find(|i| i.supporting.hour == Some(2))
            .unwrap();
        assert_eq!(routine.supporting.occurrence_count, Some(4));
        assert_eq!(routine.supporting.eligible_count, Some(4));
        assert_eq!(routine.supporting.matching_dates.as_ref().unwrap().len(), 4);
    }
    #[test]
    fn observed_zero_use_weeks_lower_recurrence_and_future_data_is_excluded() {
        let (_dir, mut storage) = fixture();
        let end = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
        for weeks in (1..=8).rev() {
            let a = midnight(end - Duration::weeks(weeks)).unwrap() + 10 * 3600;
            let run = storage.start_daemon_run(a).unwrap();
            if weeks <= 3 {
                focus(&storage, "editor", a, a + 1800);
            }
            storage.finish_daemon_run(run.run_id, a + 3600).unwrap();
        }
        focus(
            &storage,
            "future",
            midnight(end + Duration::days(7)).unwrap(),
            midnight(end + Duration::days(7)).unwrap() + 1800,
        );
        let result = analyze(
            &storage,
            &Config::default(),
            &mut SteamResolver::default(),
            midnight(end).unwrap(),
            midnight(end + Duration::days(1)).unwrap(),
            end,
            None,
        )
        .unwrap();
        assert!(result.insights.is_empty());
        assert!(!result.activities.iter().any(|a| a.key == "future"));
    }
    #[test]
    fn same_time_baseline_ignores_afternoon_usage() {
        let (_dir, mut storage) = fixture();
        let date = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
        for week in (0..=4).rev() {
            let a = midnight(date - Duration::weeks(week)).unwrap();
            let run = storage.start_daemon_run(a).unwrap();
            focus(&storage, "editor", a + 9 * 3600, a + 10 * 3600);
            if week > 0 {
                focus(&storage, "editor", a + 14 * 3600, a + 18 * 3600);
            }
            storage
                .finish_daemon_run(run.run_id, a + 24 * 3600)
                .unwrap();
        }
        let insight = same_time_comparison(&storage, midnight(date).unwrap() + 12 * 3600)
            .unwrap()
            .unwrap();
        assert_eq!(insight.supporting.baseline_seconds, Some(3600));
        assert_eq!(insight.supporting.delta_seconds, Some(0));
    }
    #[test]
    fn selected_activity_stats_reconcile_across_midnight_and_pause() {
        let (_dir, mut storage) = fixture();
        let date = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
        let a = midnight(date).unwrap();
        let run = storage.start_daemon_run(a - 3600).unwrap();
        focus(&storage, "editor", a - 60, a + 60);
        focus(&storage, "editor", a + 180, a + 240);
        let pause = storage
            .start_session_interval(SessionIntervalKind::Idle, Some("test"), a + 240)
            .unwrap();
        storage.close_session_interval(pause, a + 300).unwrap();
        focus(&storage, "editor", a + 300, a + 360);
        storage.finish_daemon_run(run.run_id, a + 3600).unwrap();
        let result = analyze(
            &storage,
            &Config::default(),
            &mut SteamResolver::default(),
            a,
            a + 3600,
            date,
            Some(("app", "editor")),
        )
        .unwrap();
        let stats = &result.activities[0];
        assert_eq!(stats.focused_seconds, 180);
        assert_eq!(stats.visits, 2);
        assert_eq!(stats.days_used, 1);
        assert_eq!(
            result.daily.iter().map(|d| d.focused_seconds).sum::<i64>(),
            180
        );
        assert_eq!(
            result
                .heatmap
                .iter()
                .map(|d| d.focused_seconds)
                .sum::<i64>(),
            180
        );
    }
    #[test]
    fn lifetime_starts_with_retained_data_and_handles_empty_database() {
        let (_dir, storage) = fixture();
        let empty = crate::report::widget_summary_for_period(
            &storage,
            &mut SteamResolver::default(),
            &Config::default(),
            crate::report::Lens::Life,
            0,
        )
        .unwrap();
        assert_eq!(empty.total_elapsed_seconds, 0);
        focus(
            &storage,
            "editor",
            crate::clock::unix_now() - 3600,
            crate::clock::unix_now() - 1800,
        );
        let report = crate::report::widget_summary_for_period(
            &storage,
            &mut SteamResolver::default(),
            &Config::default(),
            crate::report::Lens::Life,
            0,
        )
        .unwrap();
        assert!(report.total_elapsed_seconds >= 3600 && report.total_elapsed_seconds < 3610);
        assert_eq!(report.total_observed_seconds, 1800);
    }
}
