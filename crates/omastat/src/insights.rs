use crate::{
    analytics, identity,
    storage::{
        AppTotals, AppWorkspaceTotals, DayTotals, FocusHeatCell, TimelineInterval, WorkspaceTotals,
    },
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct AnalysisPeriod<'a> {
    pub lens: AnalysisLens,
    pub label: &'a str,
    pub start_date: Option<&'a str>,
    pub end_date: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisLens {
    Day,
    Week,
    Month,
    Year,
    Life,
}

#[derive(Debug, Clone)]
pub struct AnalysisComparisonPeriod {
    pub label: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub focused_seconds: i64,
    pub matched_elapsed: bool,
    pub observed_seconds: i64,
    pub elapsed_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct AnalysisInput<'a> {
    pub rows: &'a [AppTotals],
    pub daily: &'a [DayTotals],
    pub heatmap: &'a [FocusHeatCell],
    pub focus_intervals: &'a [TimelineInterval],
    pub workspaces: &'a [WorkspaceTotals],
    pub app_workspaces: &'a [AppWorkspaceTotals],
    pub today_key: &'a str,
    pub selected_day_key: &'a str,
    pub period: AnalysisPeriod<'a>,
    pub previous_period: Option<AnalysisComparisonPeriod>,
    pub total_focused_seconds: i64,
    pub observed_seconds: i64,
    pub elapsed_seconds: i64,
    pub continuity: Option<&'a crate::activity::Coverage>,
    pub pauses: Option<&'a crate::activity::Coverage>,
    pub total_open_seconds: i64,
    pub total_idle_seconds: i64,
    pub total_locked_seconds: i64,
    pub total_sleep_seconds: i64,
    pub total_unobserved_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Insight {
    pub kind: InsightKind,
    pub category: InsightCategory,
    pub tone: InsightTone,
    pub title: String,
    pub value: String,
    pub explanation: String,
    pub confidence: InsightConfidence,
    pub evidence: InsightEvidence,
    pub supporting: InsightSupport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InsightKind {
    TopApp,
    DayComparison,
    SameWeekdayPace,
    UsuallyActiveNow,
    UsualAppNow,
    AppRoutine,
    FocusMomentum,
    PeriodComparison,
    BestDay,
    WorstActiveDay,
    CurrentStreak,
    LongestStreak,
    PeakFocusHour,
    PeakFocusWeekday,
    DeepWorkBlocks,
    AppSwitchRate,
    FragmentedApp,
    FocusDensity,
    AppFocusDensity,
    EffectiveApps,
    StrongestWorkspace,
    WorkspaceAppAffinity,
    IdleExcluded,
    LockedExcluded,
    SleepExcluded,
    UnobservedExcluded,
    ExcludedImpact,
    FocusAnomaly,
    AppAnomaly,
    HourAnomaly,
    UnobservedAnomaly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InsightCategory {
    Patterns,
    FocusQuality,
    Apps,
    SystemSignals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InsightTone {
    Positive,
    Negative,
    Neutral,
    Info,
    Caution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InsightConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InsightEvidence {
    pub data_points: usize,
    pub minimum_data_points: usize,
    pub observed_focus_seconds: i64,
    pub observed_open_seconds: i64,
}

/// Local clock minutes, with an exclusive end that may wrap past midnight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutineEvidence {
    pub cadence: String,
    pub status: String,
    pub start_minute: u32,
    pub end_minute: u32,
    pub timing_basis: String,
    pub eligible_dates: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visit_start_window: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InsightSupport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routine: Option<RoutineEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eligible_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matching_dates: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_start_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_end_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hour: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hour_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekday_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_streak_days: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub longest_streak_days: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub longest_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_per_hour: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_app_count: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sleep_seconds: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unobserved_seconds: Option<i64>,
}

pub fn analyze(input: AnalysisInput<'_>) -> Vec<Insight> {
    let mut out = Vec::new();
    let blocks = focus_blocks(&input);

    if input.total_focused_seconds > 0 {
        push_top_app(&input, &mut out);
        push_day_facts(&input, &mut out);
        push_peak_facts(&input, &mut out);
        push_deep_work_facts(&input, &blocks, &mut out);
        push_switch_facts(&input, &blocks, &mut out);
        push_density_facts(&input, &mut out);
        push_effective_app_fact(&input, &mut out);
        push_workspace_facts(&input, &mut out);
        push_anomaly_facts(&input, &mut out);
    }

    push_day_comparison(&input, &mut out);
    push_period_comparison(&input, &mut out);
    push_system_facts(&input, &mut out);

    for insight in &mut out {
        if matches!(insight.tone, InsightTone::Positive | InsightTone::Negative)
            || (insight.tone == InsightTone::Caution
                && insight.category != InsightCategory::SystemSignals)
        {
            insight.tone = InsightTone::Info;
        }
        let points = match insight.kind {
            InsightKind::DeepWorkBlocks => blocks.len(),
            InsightKind::AppSwitchRate => insight.supporting.switch_count.unwrap_or(0),
            InsightKind::FragmentedApp => insight.supporting.block_count.unwrap_or(0),
            InsightKind::TopApp | InsightKind::AppAnomaly | InsightKind::EffectiveApps => {
                active_app_count(input.rows)
            }
            InsightKind::DayComparison | InsightKind::PeriodComparison => 2,
            InsightKind::FocusAnomaly => insight.evidence.data_points,
            _ => input
                .daily
                .iter()
                .filter(|d| in_period(&input, d) && eligible_day(d))
                .count(),
        };
        insight.evidence.data_points = points;
        if insight.evidence.minimum_data_points > 0 && insight.kind != InsightKind::FocusAnomaly {
            insight.confidence = confidence(points, insight.evidence.minimum_data_points);
        }
    }
    out
}

const DEEP_BLOCK_SECONDS: i64 = analytics::DEEP_BLOCK_SECONDS;
const MIN_APP_DENSITY_OPEN_SECONDS: i64 = 10 * 60;
const MIN_FRAGMENTED_APP_SECONDS: i64 = 15 * 60;
const MIN_AFFINITY_APP_SECONDS: i64 = 20 * 60;
const MIN_AFFINITY_PAIR_SECONDS: i64 = 10 * 60;

#[derive(Debug, Clone)]
struct FocusBlock {
    app_class: String,
    duration_seconds: i64,
}

#[derive(Debug, Clone, Copy)]
struct HourTotal {
    hour: u32,
    focused_seconds: i64,
}

#[derive(Debug, Clone, Copy)]
struct WeekdayTotal {
    weekday: u32,
    focused_seconds: i64,
}

#[derive(Debug, Clone)]
struct FragmentedApp {
    app_class: String,
    focused_seconds: i64,
    block_count: usize,
    rate_per_hour: f64,
}

fn push_top_app(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    let Some(top) = input.rows.iter().find(|row| row.focused_seconds > 0) else {
        return;
    };

    let share = ratio(top.focused_seconds, input.total_focused_seconds.max(1)).clamp(0.0, 1.0);
    let app_label = identity::display_name(&top.app_class);
    out.push(Insight {
        kind: InsightKind::TopApp,
        category: InsightCategory::Apps,
        tone: if share >= 0.75 && active_app_count(input.rows) > 1 {
            InsightTone::Caution
        } else {
            InsightTone::Neutral
        },
        title: "Your most-used app".to_string(),
        value: format!(
            "{} - {} ({})",
            app_label,
            format_duration(top.focused_seconds),
            percent(share)
        ),
        explanation: "You spent more of your app time here than anywhere else.".to_string(),
        confidence: confidence(input.daily.len(), 1),
        evidence: evidence(input, 1),
        supporting: period_support(input.period).with_app(
            &top.app_class,
            &app_label,
            top.focused_seconds,
            Some(top.open_seconds),
            Some(share),
        ),
    });
}

fn push_day_comparison(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    if input.period.lens != AnalysisLens::Day {
        return;
    }

    let Some((comparison_date, yesterday)) = yesterday_total(input.daily, input.selected_day_key)
    else {
        return;
    };
    if !covered_enough(input.observed_seconds, input.elapsed_seconds)
        || !input
            .daily
            .iter()
            .any(|d| d.date == comparison_date && eligible_day(d))
    {
        return;
    }

    let delta = input.total_focused_seconds - yesterday;
    let selected_label = selected_day_label(input.period);
    let comparison_label = if selected_label == "Today" {
        "Yesterday"
    } else {
        "Previous day"
    };
    out.push(Insight {
        kind: InsightKind::DayComparison,
        category: InsightCategory::Patterns,
        tone: comparison_tone(delta),
        title: if selected_label == "Today" {
            "Compared with yesterday".to_string()
        } else {
            "Compared with the day before".to_string()
        },
        value: signed_duration(delta),
        explanation: "How much your app time changed from the day before. Both days had enough tracking to compare."
            .to_string(),
        confidence: confidence(input.daily.len(), 2),
        evidence: evidence(input, 2),
        supporting: period_support(input.period).with_comparison(ComparisonSupport {
            date: input.selected_day_key,
            label: selected_label,
            comparison_date: &comparison_date,
            comparison_label,
            focused_seconds: input.total_focused_seconds,
            comparison_seconds: yesterday,
            delta_seconds: delta,
        }),
    });
}

fn push_period_comparison(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    let (label, _minimum_days, full_explanation, elapsed_explanation) = match input.period.lens {
        AnalysisLens::Week => (
            "week",
            7,
            "How your app time compares with the week before, from Monday through Sunday.",
            "Your app time so far this week, compared with the same point in the week before.",
        ),
        AnalysisLens::Month => (
            "month",
            14,
            "How your app time compares with the month before.",
            "Your app time so far this month, compared with the same amount of time in the month before.",
        ),
        AnalysisLens::Year => (
            "year",
            30,
            "How your app time compares with the year before.",
            "Your app time so far this year, compared with the same amount of time in the year before.",
        ),
        AnalysisLens::Day | AnalysisLens::Life => {
            return;
        }
    };

    let Some(previous) = input.previous_period.as_ref() else {
        return;
    };
    if !covered_enough(input.observed_seconds, input.elapsed_seconds)
        || !covered_enough(previous.observed_seconds, previous.elapsed_seconds)
    {
        return;
    }

    let delta = input.total_focused_seconds - previous.focused_seconds;
    let mut support = period_support(input.period);
    support.date = input.period.start_date.map(str::to_string);
    support.date_label = Some(input.period.label.to_string());
    support.comparison_date = previous.start_date.clone();
    support.comparison_label = Some(previous.label.clone());
    support.focused_seconds = Some(input.total_focused_seconds.max(0));
    support.comparison_seconds = Some(previous.focused_seconds.max(0));
    support.delta_seconds = Some(delta);

    out.push(Insight {
        kind: InsightKind::PeriodComparison,
        category: InsightCategory::Patterns,
        tone: comparison_tone(delta),
        title: format!("Compared with the {label} before"),
        value: signed_duration(delta),
        explanation: if previous.matched_elapsed {
            elapsed_explanation
        } else {
            full_explanation
        }
        .to_string(),
        confidence: confidence(2, 2),
        evidence: evidence(input, 2),
        supporting: support,
    });
}

fn push_day_facts(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    let active_days = input
        .daily
        .iter()
        .filter(|day| day.focused_seconds > 0 && in_period(input, day))
        .collect::<Vec<_>>();

    if let Some(best) = active_days.iter().max_by_key(|day| day.focused_seconds) {
        out.push(Insight {
            kind: InsightKind::BestDay,
            category: InsightCategory::Patterns,
            tone: InsightTone::Positive,
            title: "Your busiest day".to_string(),
            value: format!(
                "{} - {}",
                relative_day_label(best, input.today_key),
                format_duration(best.focused_seconds)
            ),
            explanation: "The day you spent the most time using apps in this period.".to_string(),
            confidence: confidence(input.daily.len(), 1),
            evidence: evidence(input, 1),
            supporting: period_support(input.period).with_day(
                &best.date,
                &relative_day_label(best, input.today_key),
                best.focused_seconds,
            ),
        });
    }

    if active_days.len() >= 2
        && let Some(worst) = active_days.iter().min_by_key(|day| day.focused_seconds)
    {
        out.push(Insight {
            kind: InsightKind::WorstActiveDay,
            category: InsightCategory::Patterns,
            tone: InsightTone::Neutral,
            title: "Your quietest day".to_string(),
            value: format!(
                "{} - {}",
                relative_day_label(worst, input.today_key),
                format_duration(worst.focused_seconds)
            ),
            explanation:
                "The day with the least recorded app use. Days with no recorded use are left out."
                    .to_string(),
            confidence: confidence(input.daily.len(), 2),
            evidence: evidence(input, 2),
            supporting: period_support(input.period).with_day(
                &worst.date,
                &relative_day_label(worst, input.today_key),
                worst.focused_seconds,
            ),
        });
    }
}

fn push_peak_facts(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    if let Some(peak) = peak_hour(input.heatmap) {
        let share = ratio(peak.focused_seconds, input.total_focused_seconds.max(1));
        out.push(Insight {
            kind: InsightKind::PeakFocusHour,
            category: InsightCategory::Patterns,
            tone: InsightTone::Info,
            title: "Your busiest time of day".to_string(),
            value: format!(
                "{} - {}",
                hour_label(peak.hour),
                format_duration(peak.focused_seconds)
            ),
            explanation: "You spent the most time using apps during this hour, adding up the days in this period."
                .to_string(),
            confidence: confidence(input.daily.len(), 2),
            evidence: evidence(input, 2),
            supporting: period_support(input.period).with_hour(
                peak.hour,
                peak.focused_seconds,
                share,
            ),
        });
    }

    if let Some(peak) = peak_weekday(input.heatmap) {
        let share = ratio(peak.focused_seconds, input.total_focused_seconds.max(1));
        out.push(Insight {
            kind: InsightKind::PeakFocusWeekday,
            category: InsightCategory::Patterns,
            tone: InsightTone::Info,
            title: "Your busiest day of the week".to_string(),
            value: format!(
                "{} - {}",
                weekday_label(peak.weekday),
                format_duration(peak.focused_seconds)
            ),
            explanation: "This day of the week had the most app use in total during this period."
                .to_string(),
            confidence: confidence(input.daily.len(), 2),
            evidence: evidence(input, 2),
            supporting: period_support(input.period).with_weekday(
                peak.weekday,
                peak.focused_seconds,
                share,
            ),
        });
    }
}

fn push_deep_work_facts(input: &AnalysisInput<'_>, blocks: &[FocusBlock], out: &mut Vec<Insight>) {
    if blocks.is_empty() {
        return;
    }

    let mut durations = blocks
        .iter()
        .map(|block| block.duration_seconds)
        .filter(|seconds| *seconds > 0)
        .collect::<Vec<_>>();
    durations.sort_unstable();
    let deep_total = durations
        .iter()
        .filter(|seconds| **seconds >= DEEP_BLOCK_SECONDS)
        .sum::<i64>();
    let deep_count = durations
        .iter()
        .filter(|seconds| **seconds >= DEEP_BLOCK_SECONDS)
        .count();
    let longest = durations.last().copied().unwrap_or_default();
    let median = median(&durations);

    out.push(Insight {
        kind: InsightKind::DeepWorkBlocks,
        category: InsightCategory::FocusQuality,
        tone: if deep_count > 0 {
            InsightTone::Positive
        } else {
            InsightTone::Caution
        },
        title: "Time with one app".to_string(),
        value: format!(
            "{} - {} total",
            format_blocks(deep_count),
            format_duration(deep_total)
        ),
        explanation: format!(
            "Times you stayed with one app for at least {}. Your longest stretch was {}; a typical stretch was {}.",
            format_duration(DEEP_BLOCK_SECONDS),
            format_duration(longest),
            format_duration(median)
        ),
        confidence: confidence(blocks.len(), 2),
        evidence: evidence(input, 2),
        supporting: period_support(input.period).with_blocks(
            deep_count,
            deep_total,
            longest,
            median,
            DEEP_BLOCK_SECONDS,
        ),
    });
}

fn push_switch_facts(input: &AnalysisInput<'_>, blocks: &[FocusBlock], out: &mut Vec<Insight>) {
    if input.focus_intervals.len() >= 2 {
        let switch_count = input
            .focus_intervals
            .windows(2)
            .filter(|pair| {
                pair[0].app_class != pair[1].app_class
                    && continuous(input, pair[0].ended_at, pair[1].started_at)
            })
            .count();
        let rate = per_hour(switch_count, input.total_focused_seconds);
        out.push(Insight {
            kind: InsightKind::AppSwitchRate,
            category: InsightCategory::FocusQuality,
            tone: switch_rate_tone(rate),
            title: "Moving between apps".to_string(),
            value: format!("{} switches an hour", format_rate(rate)),
            explanation: "How often you moved to a different app while using your computer. Breaks and gaps in tracking don't count as switches.".to_string(),
            confidence: confidence(input.focus_intervals.len(), 3),
            evidence: evidence(input, 3),
            supporting: period_support(input.period).with_switch_rate(switch_count, rate),
        });
    }

    if let Some(fragmented) = most_fragmented_app(blocks) {
        let app_label = identity::display_name(&fragmented.app_class);
        out.push(Insight {
            kind: InsightKind::FragmentedApp,
            category: InsightCategory::Apps,
            tone: if fragmented.rate_per_hour >= 8.0 {
                InsightTone::Caution
            } else {
                InsightTone::Neutral
            },
            title: "An app you dip in and out of".to_string(),
            value: format!(
                "{} · {} stretches an hour",
                app_label,
                format_rate(fragmented.rate_per_hour)
            ),
            explanation:
                "You returned to this app most often for the amount of time you spent using it."
                    .to_string(),
            confidence: confidence(fragmented.block_count, 3),
            evidence: evidence(input, 3),
            supporting: period_support(input.period).with_fragmented_app(
                &fragmented.app_class,
                &app_label,
                fragmented.focused_seconds,
                fragmented.block_count,
                fragmented.rate_per_hour,
            ),
        });
    }
}

fn push_density_facts(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    if input.total_open_seconds > 0 {
        let density = ratio(input.total_focused_seconds, input.total_open_seconds);
        out.push(Insight {
            kind: InsightKind::FocusDensity,
            category: InsightCategory::FocusQuality,
            tone: density_tone(density),
            title: "Time spent using open apps".to_string(),
            value: percent(density),
            explanation: "The share of all open-app time you spent actually using those apps. Several apps can be open at once."
                .to_string(),
            confidence: confidence(input.daily.len(), 1),
            evidence: evidence(input, 1),
            supporting: period_support(input.period).with_totals(
                input.total_focused_seconds,
                input.total_open_seconds,
                density,
            ),
        });
    }

    let app_densities = input
        .rows
        .iter()
        .filter(|row| row.focused_seconds > 0 && row.open_seconds >= MIN_APP_DENSITY_OPEN_SECONDS)
        .map(|row| (row, ratio(row.focused_seconds, row.open_seconds)))
        .collect::<Vec<_>>();

    if let Some((row, density)) = app_densities.iter().max_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.focused_seconds.cmp(&right.0.focused_seconds))
    }) {
        push_app_density(input, out, "Open and in use", row, *density);
    }

    if app_densities.len() >= 2
        && let Some((row, density)) = app_densities.iter().min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| right.0.focused_seconds.cmp(&left.0.focused_seconds))
        })
        && *density < 0.65
    {
        push_app_density(input, out, "Often open in the background", row, *density);
    }
}

fn push_app_density(
    input: &AnalysisInput<'_>,
    out: &mut Vec<Insight>,
    title: &str,
    row: &AppTotals,
    density: f64,
) {
    let app_label = identity::display_name(&row.app_class);
    out.push(Insight {
        kind: InsightKind::AppFocusDensity,
        category: InsightCategory::FocusQuality,
        tone: density_tone(density),
        title: title.to_string(),
        value: format!("{} - {}", app_label, percent(density)),
        explanation: "How much of the time this app was open you spent actually using it."
            .to_string(),
        confidence: confidence(input.daily.len(), 1),
        evidence: evidence(input, 1),
        supporting: period_support(input.period).with_app(
            &row.app_class,
            &app_label,
            row.focused_seconds,
            Some(row.open_seconds),
            Some(density),
        ),
    });
}

fn push_effective_app_fact(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    let app_count = active_app_count(input.rows);
    if app_count == 0 {
        return;
    }

    let effective = effective_app_count(input.rows, input.total_focused_seconds);
    let top_share = input
        .rows
        .iter()
        .find(|row| row.focused_seconds > 0)
        .map(|row| ratio(row.focused_seconds, input.total_focused_seconds.max(1)))
        .unwrap_or_default();

    out.push(Insight {
        kind: InsightKind::EffectiveApps,
        category: InsightCategory::Apps,
        tone: if app_count > 1 && effective < 1.5 {
            InsightTone::Caution
        } else {
            InsightTone::Info
        },
        title: "How your time is spread".to_string(),
        value: format!("Like {} equally used apps", format_decimal(effective)),
        explanation:
            "A way to describe how evenly you shared your time between apps. A smaller number means more of your time went to just a few apps."
                .to_string(),
        confidence: confidence(app_count, 2),
        evidence: evidence(input, 2),
        supporting: period_support(input.period)
            .with_effective_apps(app_count, effective, top_share),
    });
}

fn push_workspace_facts(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    if let Some(workspace) = input
        .workspaces
        .iter()
        .find(|workspace| workspace.focused_seconds > 0)
    {
        let share = ratio(
            workspace.focused_seconds,
            input.total_focused_seconds.max(1),
        );
        out.push(Insight {
            kind: InsightKind::StrongestWorkspace,
            category: InsightCategory::Patterns,
            tone: InsightTone::Info,
            title: "Your most-used workspace".to_string(),
            value: format!(
                "{} - {} ({})",
                workspace.workspace,
                format_duration(workspace.focused_seconds),
                percent(share)
            ),
            explanation: "You spent the most app time on this workspace.".to_string(),
            confidence: confidence(input.workspaces.len(), 1),
            evidence: evidence(input, 1),
            supporting: period_support(input.period).with_workspace(
                &workspace.workspace,
                None,
                None,
                workspace.focused_seconds,
                share,
            ),
        });
    }

    let app_totals = input
        .rows
        .iter()
        .map(|row| (row.app_class.as_str(), row.focused_seconds.max(0)))
        .collect::<BTreeMap<_, _>>();
    let affinity = input
        .app_workspaces
        .iter()
        .filter_map(|row| {
            let app_total = *app_totals.get(row.app_class.as_str())?;
            if app_total < MIN_AFFINITY_APP_SECONDS
                || row.focused_seconds < MIN_AFFINITY_PAIR_SECONDS
            {
                return None;
            }
            Some((row, ratio(row.focused_seconds, app_total)))
        })
        .max_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.focused_seconds.cmp(&right.0.focused_seconds))
        });

    if let Some((row, affinity)) = affinity {
        let app_label = identity::display_name(&row.app_class);
        out.push(Insight {
            kind: InsightKind::WorkspaceAppAffinity,
            category: InsightCategory::Apps,
            tone: InsightTone::Info,
            title: "Where you use this app".to_string(),
            value: format!("{} on {} - {}", app_label, row.workspace, percent(affinity)),
            explanation: "This is the workspace you used most for this app. The percentage shows how much of its use happened there."
                .to_string(),
            confidence: confidence(input.app_workspaces.len(), 2),
            evidence: evidence(input, 2),
            supporting: period_support(input.period).with_workspace(
                &row.workspace,
                Some(&row.app_class),
                Some(&app_label),
                row.focused_seconds,
                affinity,
            ),
        });
    }
}

fn push_anomaly_facts(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    if let Some(insight) = focus_anomaly(input) {
        out.push(insight);
    }
    if let Some(insight) = app_anomaly(input) {
        out.push(insight);
    }
    if let Some(insight) = hour_anomaly(input) {
        out.push(insight);
    }
}

fn push_system_facts(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    if let Some(insight) = unobserved_anomaly(input) {
        out.push(insight);
    }
    if input.total_idle_seconds > 0 {
        out.push(system_signal(
            input,
            InsightKind::IdleExcluded,
            "Idle time",
            "Time when your computer was idle. It isn't counted as app use.",
            input.total_idle_seconds,
            InsightTone::Info,
        ));
    }

    if input.total_locked_seconds > 0 {
        out.push(system_signal(
            input,
            InsightKind::LockedExcluded,
            "Screen locked",
            "Time when your screen was locked. It isn't counted as app use.",
            input.total_locked_seconds,
            InsightTone::Info,
        ));
    }

    if input.total_sleep_seconds > 0 {
        out.push(system_signal(
            input,
            InsightKind::SleepExcluded,
            "Computer asleep",
            "Time when your computer was asleep. It isn't counted as app use.",
            input.total_sleep_seconds,
            InsightTone::Info,
        ));
    }

    if input.total_unobserved_seconds > 0 {
        out.push(system_signal(
            input,
            InsightKind::UnobservedExcluded,
            "Time without tracking",
            "The tracker wasn't recording during this time, so we don't guess which apps you used.",
            input.total_unobserved_seconds,
            InsightTone::Caution,
        ));
    }

    let excluded = input
        .total_idle_seconds
        .saturating_add(input.total_locked_seconds)
        .saturating_add(input.total_sleep_seconds)
        .saturating_add(input.total_unobserved_seconds);
    if excluded <= 0 {
        return;
    }

    let impact = ratio(excluded, input.elapsed_seconds);
    out.push(Insight {
        kind: InsightKind::ExcludedImpact,
        category: InsightCategory::SystemSignals,
        tone: if impact >= 0.25 {
            InsightTone::Caution
        } else {
            InsightTone::Info
        },
        title: "Time outside your app totals".to_string(),
        value: format!("{} ({})", format_duration(excluded), percent(impact)),
        explanation: "Time when your computer was idle, locked, asleep, or not being tracked. It isn't included in your app totals."
            .to_string(),
        confidence: InsightConfidence::High,
        evidence: evidence(input, 0),
        supporting: period_support(input.period).with_system_breakdown(
            input.total_idle_seconds,
            input.total_locked_seconds,
            input.total_sleep_seconds,
            input.total_unobserved_seconds,
            excluded,
            impact,
        ),
    });
}

fn system_signal(
    input: &AnalysisInput<'_>,
    kind: InsightKind,
    title: &str,
    explanation: &str,
    seconds: i64,
    tone: InsightTone,
) -> Insight {
    Insight {
        kind,
        category: InsightCategory::SystemSignals,
        tone,
        title: title.to_string(),
        value: format!("{} not counted", format_duration(seconds)),
        explanation: explanation.to_string(),
        confidence: InsightConfidence::High,
        evidence: evidence(input, 0),
        supporting: period_support(input.period).with_excluded(seconds),
    }
}

fn evidence(input: &AnalysisInput<'_>, minimum_data_points: usize) -> InsightEvidence {
    InsightEvidence {
        data_points: input.daily.len(),
        minimum_data_points,
        observed_focus_seconds: input.total_focused_seconds.max(0),
        observed_open_seconds: input.total_open_seconds.max(0),
    }
}

fn confidence(data_points: usize, minimum_data_points: usize) -> InsightConfidence {
    if data_points < minimum_data_points {
        InsightConfidence::Low
    } else if data_points >= minimum_data_points.saturating_mul(3).max(3) {
        InsightConfidence::High
    } else {
        InsightConfidence::Medium
    }
}

fn period_support(period: AnalysisPeriod<'_>) -> InsightSupport {
    InsightSupport {
        period_label: Some(period.label.to_string()),
        period_start_date: period.start_date.map(str::to_string),
        period_end_date: period.end_date.map(str::to_string),
        ..InsightSupport::default()
    }
}

impl InsightSupport {
    fn with_app(
        mut self,
        app_class: &str,
        app_label: &str,
        focused_seconds: i64,
        open_seconds: Option<i64>,
        share: Option<f64>,
    ) -> Self {
        self.app_class = Some(app_class.to_string());
        self.app_label = Some(app_label.to_string());
        self.focused_seconds = Some(focused_seconds.max(0));
        self.open_seconds = open_seconds.map(|seconds| seconds.max(0));
        self.share = share.map(|value| value.clamp(0.0, 1.0));
        self
    }

    fn with_day(mut self, date: &str, label: &str, focused_seconds: i64) -> Self {
        self.date = Some(date.to_string());
        self.date_label = Some(label.to_string());
        self.focused_seconds = Some(focused_seconds.max(0));
        self
    }

    fn with_comparison(mut self, comparison: ComparisonSupport<'_>) -> Self {
        self.date = Some(comparison.date.to_string());
        self.date_label = Some(comparison.label.to_string());
        self.comparison_date = Some(comparison.comparison_date.to_string());
        self.comparison_label = Some(comparison.comparison_label.to_string());
        self.focused_seconds = Some(comparison.focused_seconds.max(0));
        self.comparison_seconds = Some(comparison.comparison_seconds.max(0));
        self.delta_seconds = Some(comparison.delta_seconds);
        self
    }

    fn with_totals(mut self, focused_seconds: i64, open_seconds: i64, share: f64) -> Self {
        self.focused_seconds = Some(focused_seconds.max(0));
        self.open_seconds = Some(open_seconds.max(0));
        self.share = Some(share.clamp(0.0, 1.0));
        self
    }

    fn with_excluded(mut self, seconds: i64) -> Self {
        self.excluded_seconds = Some(seconds.max(0));
        self
    }

    fn with_hour(mut self, hour: u32, focused_seconds: i64, share: f64) -> Self {
        self.hour = Some(hour);
        self.hour_label = Some(hour_label(hour));
        self.focused_seconds = Some(focused_seconds.max(0));
        self.share = Some(share.clamp(0.0, 1.0));
        self
    }

    fn with_weekday(mut self, weekday: u32, focused_seconds: i64, share: f64) -> Self {
        self.weekday = Some(weekday);
        self.weekday_label = Some(weekday_label(weekday).to_string());
        self.focused_seconds = Some(focused_seconds.max(0));
        self.share = Some(share.clamp(0.0, 1.0));
        self
    }

    fn with_blocks(
        mut self,
        block_count: usize,
        total_seconds: i64,
        longest_seconds: i64,
        median_seconds: i64,
        threshold_seconds: i64,
    ) -> Self {
        self.block_count = Some(block_count);
        self.total_seconds = Some(total_seconds.max(0));
        self.longest_seconds = Some(longest_seconds.max(0));
        self.median_seconds = Some(median_seconds.max(0));
        self.threshold_seconds = Some(threshold_seconds.max(0));
        self
    }

    fn with_switch_rate(mut self, switch_count: usize, rate_per_hour: f64) -> Self {
        self.switch_count = Some(switch_count);
        self.rate_per_hour = Some(rate_per_hour.max(0.0));
        self
    }

    fn with_fragmented_app(
        mut self,
        app_class: &str,
        app_label: &str,
        focused_seconds: i64,
        block_count: usize,
        rate_per_hour: f64,
    ) -> Self {
        self.app_class = Some(app_class.to_string());
        self.app_label = Some(app_label.to_string());
        self.focused_seconds = Some(focused_seconds.max(0));
        self.block_count = Some(block_count);
        self.rate_per_hour = Some(rate_per_hour.max(0.0));
        self
    }

    fn with_effective_apps(
        mut self,
        app_count: usize,
        effective_app_count: f64,
        top_app_share: f64,
    ) -> Self {
        self.app_count = Some(app_count);
        self.effective_app_count = Some(effective_app_count.max(0.0));
        self.share = Some(top_app_share.clamp(0.0, 1.0));
        self
    }

    fn with_workspace(
        mut self,
        workspace: &str,
        app_class: Option<&str>,
        app_label: Option<&str>,
        focused_seconds: i64,
        share: f64,
    ) -> Self {
        self.workspace = Some(workspace.to_string());
        self.app_class = app_class.map(str::to_string);
        self.app_label = app_label.map(str::to_string);
        self.focused_seconds = Some(focused_seconds.max(0));
        self.share = Some(share.clamp(0.0, 1.0));
        self
    }

    fn with_system_breakdown(
        mut self,
        idle_seconds: i64,
        locked_seconds: i64,
        sleep_seconds: i64,
        unobserved_seconds: i64,
        excluded_seconds: i64,
        share: f64,
    ) -> Self {
        self.idle_seconds = Some(idle_seconds.max(0));
        self.locked_seconds = Some(locked_seconds.max(0));
        self.sleep_seconds = Some(sleep_seconds.max(0));
        self.unobserved_seconds = Some(unobserved_seconds.max(0));
        self.excluded_seconds = Some(excluded_seconds.max(0));
        self.share = Some(share.clamp(0.0, 1.0));
        self
    }
}

struct ComparisonSupport<'a> {
    date: &'a str,
    label: &'a str,
    comparison_date: &'a str,
    comparison_label: &'a str,
    focused_seconds: i64,
    comparison_seconds: i64,
    delta_seconds: i64,
}

fn yesterday_total(daily: &[DayTotals], today_key: &str) -> Option<(String, i64)> {
    let today = NaiveDate::parse_from_str(today_key, "%Y-%m-%d").ok()?;
    let yesterday = today.pred_opt()?.format("%Y-%m-%d").to_string();
    daily
        .iter()
        .find(|day| day.date == yesterday)
        .map(|day| (yesterday, day.focused_seconds))
}

fn relative_day_label(day: &DayTotals, today_key: &str) -> String {
    if day.date == today_key {
        return "Today".to_string();
    }

    if let Ok(today) = NaiveDate::parse_from_str(today_key, "%Y-%m-%d")
        && today
            .pred_opt()
            .is_some_and(|yesterday| day.date == yesterday.format("%Y-%m-%d").to_string())
    {
        return "Yesterday".to_string();
    }

    day.label.clone()
}

fn selected_day_label(period: AnalysisPeriod<'_>) -> &'static str {
    if period.label == "Today" {
        "Today"
    } else if period.label == "Yesterday" {
        "Yesterday"
    } else {
        "Selected day"
    }
}

fn continuous(input: &AnalysisInput<'_>, a: i64, b: i64) -> bool {
    b == a
        && input.continuity.is_none_or(|c| c.covered(a, b) == b - a)
        && input.pauses.is_none_or(|c| c.covered(a, b) == 0)
}
fn focus_blocks(input: &AnalysisInput<'_>) -> Vec<FocusBlock> {
    let mut blocks: Vec<FocusBlock> = Vec::new();
    let mut previous_end = None;
    for i in input.focus_intervals {
        let duration = (i.ended_at - i.started_at).max(0);
        if duration == 0 {
            continue;
        }
        if let Some(last) = blocks.last_mut()
            && last.app_class == i.app_class
            && previous_end.is_some_and(|end| continuous(input, end, i.started_at))
        {
            last.duration_seconds += duration;
        } else {
            blocks.push(FocusBlock {
                app_class: i.app_class.clone(),
                duration_seconds: duration,
            });
        }
        previous_end = Some(i.ended_at);
    }
    blocks
}
fn covered_enough(observed: i64, elapsed: i64) -> bool {
    elapsed > 0 && observed * 10 >= elapsed * 9
}
fn eligible_day(day: &DayTotals) -> bool {
    covered_enough(day.observed_seconds, day.elapsed_seconds)
}
fn in_period(input: &AnalysisInput<'_>, day: &DayTotals) -> bool {
    input
        .period
        .start_date
        .is_none_or(|a| day.date.as_str() >= a)
        && input.period.end_date.is_none_or(|b| day.date.as_str() <= b)
}

fn median(sorted: &[i64]) -> i64 {
    analytics::median(sorted)
}

fn peak_hour(cells: &[FocusHeatCell]) -> Option<HourTotal> {
    analytics::peak_hour(cells).map(|peak| HourTotal {
        hour: peak.hour,
        focused_seconds: peak.focused_seconds,
    })
}

fn peak_weekday(cells: &[FocusHeatCell]) -> Option<WeekdayTotal> {
    analytics::peak_weekday(cells).map(|peak| WeekdayTotal {
        weekday: peak.weekday,
        focused_seconds: peak.focused_seconds,
    })
}

fn effective_app_count(rows: &[AppTotals], total_focused_seconds: i64) -> f64 {
    analytics::effective_app_count(rows, total_focused_seconds)
}

fn signed_duration(seconds: i64) -> String {
    match seconds.cmp(&0) {
        std::cmp::Ordering::Equal => "No change".into(),
        std::cmp::Ordering::Greater => format!("{} more", format_duration(seconds)),
        std::cmp::Ordering::Less => format!("{} less", format_duration(seconds.abs())),
    }
}

fn format_duration(seconds: i64) -> String {
    analytics::format_duration(seconds)
}

fn percent(value: f64) -> String {
    analytics::percent(value)
}

fn ratio(value: i64, total: i64) -> f64 {
    analytics::ratio(value, total)
}

fn most_fragmented_app(blocks: &[FocusBlock]) -> Option<FragmentedApp> {
    let mut apps = BTreeMap::<String, (usize, i64)>::new();
    for block in blocks {
        let entry = apps.entry(block.app_class.clone()).or_default();
        entry.0 += 1;
        entry.1 += block.duration_seconds.max(0);
    }

    apps.into_iter()
        .filter_map(|(app_class, (block_count, focused_seconds))| {
            if block_count < 2 || focused_seconds < MIN_FRAGMENTED_APP_SECONDS {
                return None;
            }
            Some(FragmentedApp {
                app_class,
                focused_seconds,
                block_count,
                rate_per_hour: per_hour(block_count, focused_seconds),
            })
        })
        .max_by(|left, right| {
            left.rate_per_hour
                .total_cmp(&right.rate_per_hour)
                .then_with(|| left.block_count.cmp(&right.block_count))
                .then_with(|| left.focused_seconds.cmp(&right.focused_seconds))
        })
}

fn focus_anomaly(input: &AnalysisInput<'_>) -> Option<Insight> {
    let days = input
        .daily
        .iter()
        .filter(|d| in_period(input, d) && d.date.as_str() < input.today_key && eligible_day(d))
        .collect::<Vec<_>>();
    let best = *days.iter().max_by_key(|d| d.focused_seconds)?;
    if best.focused_seconds < 3600 {
        return None;
    }
    let baseline_days = days
        .iter()
        .filter(|d| d.date != best.date)
        .collect::<Vec<_>>();
    if baseline_days.len() < 7 {
        return None;
    }
    let mut baseline = baseline_days
        .iter()
        .map(|d| d.focused_seconds)
        .collect::<Vec<_>>();
    baseline.sort_unstable();
    let median = analytics::median(&baseline);
    let mut deviations = baseline
        .iter()
        .map(|s| (s - median).abs())
        .collect::<Vec<_>>();
    deviations.sort_unstable();
    let threshold = (3 * analytics::median(&deviations)).max(1800);
    if best.focused_seconds <= median + threshold {
        return None;
    }
    let mut support = period_support(input.period).with_day(
        &best.date,
        &relative_day_label(best, input.today_key),
        best.focused_seconds,
    );
    support.baseline_seconds = Some(median);
    support.threshold_seconds = Some(threshold);
    support.matching_dates = Some(baseline_days.iter().map(|d| d.date.clone()).collect());
    Some(Insight {
        kind: InsightKind::FocusAnomaly,
        category: InsightCategory::Patterns,
        tone: InsightTone::Info,
        title: "More app time than usual".into(),
        value: format!("{} - {}", relative_day_label(best, input.today_key), format_duration(best.focused_seconds)),
        explanation: "You spent noticeably more time using apps on this day than on a typical day in this period. The comparison uses completed days with enough tracking.".into(),
        confidence: confidence(baseline.len(), 7),
        evidence: InsightEvidence {
            data_points: baseline.len(), minimum_data_points: 7,
            observed_focus_seconds: baseline.iter().sum(), observed_open_seconds: 0,
        },
        supporting: support,
    })
}

fn app_anomaly(input: &AnalysisInput<'_>) -> Option<Insight> {
    let app_count = active_app_count(input.rows);
    if app_count < 2 || input.total_focused_seconds < 60 * 60 {
        return None;
    }

    let top = input.rows.iter().find(|row| row.focused_seconds > 0)?;
    let share = ratio(top.focused_seconds, input.total_focused_seconds.max(1));
    if share < 0.75 {
        return None;
    }

    let app_label = identity::display_name(&top.app_class);
    Some(Insight {
        kind: InsightKind::AppAnomaly,
        category: InsightCategory::Apps,
        tone: InsightTone::Caution,
        title: "One app took most of your time".to_string(),
        value: format!("{} · {} of your app time", app_label, percent(share)),
        explanation: "At least three quarters of your app time went to this one app.".to_string(),
        confidence: confidence(app_count, 2),
        evidence: evidence(input, 2),
        supporting: period_support(input.period).with_app(
            &top.app_class,
            &app_label,
            top.focused_seconds,
            Some(top.open_seconds),
            Some(share),
        ),
    })
}

fn hour_anomaly(input: &AnalysisInput<'_>) -> Option<Insight> {
    if input.total_focused_seconds < 60 * 60 {
        return None;
    }

    let peak = peak_hour(input.heatmap)?;
    let share = ratio(peak.focused_seconds, input.total_focused_seconds.max(1));
    if share < 0.45 {
        return None;
    }

    Some(Insight {
        kind: InsightKind::HourAnomaly,
        category: InsightCategory::Patterns,
        tone: InsightTone::Info,
        title: "A lot happened in one hour".to_string(),
        value: format!("{} · {} of your app time", hour_label(peak.hour), percent(share)),
        explanation: "A large share of your app use fell in this hour of the day, adding up the days in this period.".to_string(),
        confidence: confidence(input.daily.len(), 2),
        evidence: evidence(input, 2),
        supporting: period_support(input.period).with_hour(peak.hour, peak.focused_seconds, share),
    })
}

fn unobserved_anomaly(input: &AnalysisInput<'_>) -> Option<Insight> {
    if input.total_unobserved_seconds < 30 * 60 {
        return None;
    }

    let share = ratio(input.total_unobserved_seconds, input.elapsed_seconds);
    if share < 0.10 {
        return None;
    }

    let mut support = period_support(input.period).with_excluded(input.total_unobserved_seconds);
    support.share = Some(share);
    support.unobserved_seconds = Some(input.total_unobserved_seconds.max(0));

    Some(Insight {
        kind: InsightKind::UnobservedAnomaly,
        category: InsightCategory::SystemSignals,
        tone: InsightTone::Caution,
        title: "Gaps in tracking".to_string(),
        value: format!(
            "{} without tracking ({})",
            format_duration(input.total_unobserved_seconds),
            percent(share)
        ),
        explanation:
            "The tracker missed enough time that this period may not tell the whole story."
                .to_string(),
        confidence: InsightConfidence::High,
        evidence: evidence(input, 0),
        supporting: support,
    })
}

fn active_app_count(rows: &[AppTotals]) -> usize {
    rows.iter().filter(|row| row.focused_seconds > 0).count()
}

fn comparison_tone(delta_seconds: i64) -> InsightTone {
    match delta_seconds.cmp(&0) {
        std::cmp::Ordering::Greater => InsightTone::Positive,
        std::cmp::Ordering::Less => InsightTone::Negative,
        std::cmp::Ordering::Equal => InsightTone::Neutral,
    }
}

fn density_tone(density: f64) -> InsightTone {
    if density >= 0.65 {
        InsightTone::Positive
    } else if density >= 0.35 {
        InsightTone::Neutral
    } else {
        InsightTone::Caution
    }
}

fn switch_rate_tone(rate: f64) -> InsightTone {
    if rate <= 4.0 {
        InsightTone::Positive
    } else if rate <= 12.0 {
        InsightTone::Neutral
    } else {
        InsightTone::Caution
    }
}

fn per_hour(count: usize, focused_seconds: i64) -> f64 {
    if focused_seconds <= 0 {
        0.0
    } else {
        count as f64 / (focused_seconds as f64 / 3600.0)
    }
}

fn hour_label(hour: u32) -> String {
    let hour = hour % 24;
    let suffix = if hour < 12 { "AM" } else { "PM" };
    let hour12 = match hour % 12 {
        0 => 12,
        value => value,
    };
    format!("{hour12} {suffix}")
}

fn weekday_label(weekday: u32) -> &'static str {
    match weekday {
        0 => "Mon",
        1 => "Tue",
        2 => "Wed",
        3 => "Thu",
        4 => "Fri",
        5 => "Sat",
        6 => "Sun",
        _ => "Unknown",
    }
}

fn format_blocks(blocks: usize) -> String {
    if blocks == 1 {
        "1 stretch".to_string()
    } else {
        format!("{blocks} stretches")
    }
}

fn format_rate(value: f64) -> String {
    format!("{:.1}", value.max(0.0))
}

fn format_decimal(value: f64) -> String {
    if value >= 10.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.1}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::IntervalKind;

    #[test]
    fn comparisons_require_coverage_and_include_zero_use() {
        let rows = Vec::new();
        let mut daily = vec![
            day("2026-01-13", "Yesterday", 0),
            day("2026-01-14", "Today", 0),
        ];
        let result = analyze(input(&rows, &daily, 0, 0, 0, 0));
        assert_eq!(
            result
                .iter()
                .find(|i| i.kind == InsightKind::DayComparison)
                .unwrap()
                .supporting
                .delta_seconds,
            Some(0)
        );
        daily[0].observed_seconds = 100;
        assert!(
            analyze(input(&rows, &daily, 0, 0, 0, 0))
                .iter()
                .all(|i| i.kind != InsightKind::DayComparison)
        );
        daily[0].observed_seconds = 86400;
        let mut partial = input(&rows, &daily, 0, 0, 0, 0);
        partial.observed_seconds = 100;
        assert!(
            analyze(partial)
                .iter()
                .all(|i| i.kind != InsightKind::DayComparison)
        );
        let mut period = input(&rows, &daily, 0, 0, 0, 0);
        period.period.lens = AnalysisLens::Week;
        period.previous_period = Some(AnalysisComparisonPeriod {
            label: "Prior week".into(),
            start_date: None,
            end_date: None,
            focused_seconds: 0,
            matched_elapsed: true,
            observed_seconds: 86400,
            elapsed_seconds: 86400,
        });
        assert!(
            analyze(period.clone())
                .iter()
                .any(|i| i.kind == InsightKind::PeriodComparison)
        );
        period.previous_period.as_mut().unwrap().observed_seconds = 100;
        assert!(
            analyze(period)
                .iter()
                .all(|i| i.kind != InsightKind::PeriodComparison)
        );
    }

    #[test]
    fn robust_anomaly_excludes_partial_missing_and_out_of_period_days() {
        let rows = vec![AppTotals {
            app_class: "editor".into(),
            focused_seconds: 36000,
            open_seconds: 36000,
        }];
        let mut daily = (1..=9)
            .map(|d| day(&format!("2026-01-{d:02}"), "Day", 3600))
            .collect::<Vec<_>>();
        daily[8].focused_seconds = 10800;
        let mut context = input(&rows, &daily, 36000, 36000, 0, 0);
        context.period.lens = AnalysisLens::Month;
        context.period.start_date = Some("2026-01-01");
        context.period.end_date = Some("2026-01-31");
        let result = analyze(context.clone());
        let spike = result
            .iter()
            .find(|i| i.kind == InsightKind::FocusAnomaly)
            .unwrap();
        assert_eq!(spike.supporting.baseline_seconds, Some(3600));
        assert_eq!(spike.evidence.data_points, 8);
        assert_eq!(spike.supporting.threshold_seconds, Some(1800));
        context.today_key = "2026-01-09";
        assert!(
            analyze(context.clone())
                .iter()
                .all(|i| i.kind != InsightKind::FocusAnomaly)
        );
        context.today_key = "2026-01-14";
        context.period.end_date = Some("2026-01-08");
        assert!(
            analyze(context)
                .iter()
                .all(|i| i.kind != InsightKind::FocusAnomaly)
        );
        daily[0].observed_seconds = 0;
        daily[1].observed_seconds = 0;
        let mut context = input(&rows, &daily, 36000, 36000, 0, 0);
        context.period.start_date = None;
        context.period.end_date = None;
        assert!(
            analyze(context)
                .iter()
                .all(|i| i.kind != InsightKind::FocusAnomaly)
        );
    }

    #[test]
    fn title_fragments_merge_but_pauses_and_recording_gaps_break_blocks_and_switches() {
        let intervals = vec![
            focus("editor", 0, 900),
            focus("editor", 900, 1800),
            focus("browser", 1800, 2400),
            focus("editor", 3000, 3600),
            focus("browser", 4000, 4600),
        ];
        let coverage = crate::activity::Coverage::new(vec![(0, 3600), (4000, 4600)]);
        let pauses = crate::activity::Coverage::new(vec![(2400, 3000)]);
        let mut context = input_with_context(&[], &[], &[], &intervals, &[], &[]);
        context.continuity = Some(&coverage);
        context.pauses = Some(&pauses);
        let blocks = focus_blocks(&context);
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].duration_seconds, 1800);
        let mut result = Vec::new();
        push_switch_facts(&context, &blocks, &mut result);
        assert_eq!(
            result
                .iter()
                .find(|i| i.kind == InsightKind::AppSwitchRate)
                .unwrap()
                .supporting
                .switch_count,
            Some(1)
        );
        assert_eq!(analytics::app_switch_count(&intervals), 1);
        assert_eq!(analytics::focus_block_stats(&intervals).count, 4);
    }

    #[test]
    fn additive_routine_evidence_preserves_old_json() {
        let old = r#"{"app_class":"game","hour":20,"occurrence_count":3}"#;
        let support: InsightSupport = serde_json::from_str(old).unwrap();
        assert!(support.routine.is_none());
        assert!(
            serde_json::to_value(support)
                .unwrap()
                .get("routine")
                .is_none()
        );
    }

    #[test]
    fn missing_observation_fraction_uses_elapsed_time_even_without_focus() {
        let mut context = input(&[], &[], 0, 0, 0, 3600);
        context.elapsed_seconds = 36000;
        context.observed_seconds = 32400;
        let result = analyze(context);
        let gap = result
            .iter()
            .find(|i| i.kind == InsightKind::UnobservedAnomaly)
            .unwrap();
        assert_eq!(gap.supporting.share, Some(0.1));
    }
    #[test]
    fn emits_structured_top_app_insight() {
        let rows = vec![
            AppTotals {
                app_class: "com.mitchellh.ghostty".to_string(),
                focused_seconds: 3600,
                open_seconds: 7200,
            },
            AppTotals {
                app_class: "discord".to_string(),
                focused_seconds: 900,
                open_seconds: 3600,
            },
        ];
        let daily = vec![
            day("2026-01-13", "Jan 13", 1800),
            day("2026-01-14", "Jan 14", 4500),
        ];

        let insights = analyze(input(&rows, &daily, 4500, 10_800, 0, 0));
        let top = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::TopApp)
            .unwrap();

        assert_eq!(top.category, InsightCategory::Apps);
        assert_eq!(top.title, "Your most-used app");
        assert_eq!(
            top.supporting.app_class.as_deref(),
            Some("com.mitchellh.ghostty")
        );
        assert_eq!(top.supporting.app_label.as_deref(), Some("Ghostty"));
        assert_eq!(top.supporting.focused_seconds, Some(3600));

        let json = serde_json::to_value(top).unwrap();
        assert_eq!(json["kind"], "top-app");
        assert_eq!(json["category"], "apps");
        assert_eq!(json["tone"], "info");
        assert!(json.get("label").is_none());
    }

    #[test]
    fn gates_day_comparison_without_yesterday() {
        let rows = vec![AppTotals {
            app_class: "firefox".to_string(),
            focused_seconds: 3600,
            open_seconds: 3600,
        }];
        let daily = vec![day("2026-01-14", "Jan 14", 3600)];

        let insights = analyze(input(&rows, &daily, 3600, 3600, 0, 0));

        assert!(
            insights
                .iter()
                .all(|insight| insight.kind != InsightKind::DayComparison)
        );
    }

    #[test]
    fn emits_system_signals_without_focus() {
        let rows = Vec::new();
        let daily = vec![day("2026-01-14", "Jan 14", 0)];

        let insights = analyze(input(&rows, &daily, 0, 0, 1800, 900));

        assert_eq!(insights.len(), 3);
        assert_eq!(insights[0].kind, InsightKind::SleepExcluded);
        assert_eq!(insights[0].value, "30m not counted");
        assert_eq!(insights[1].kind, InsightKind::UnobservedExcluded);
        assert_eq!(insights[1].tone, InsightTone::Caution);
        assert_eq!(insights[2].kind, InsightKind::ExcludedImpact);
        assert_eq!(insights[2].supporting.excluded_seconds, Some(2700));
    }

    #[test]
    fn negative_comparison_is_descriptive() {
        let rows = vec![AppTotals {
            app_class: "firefox".to_string(),
            focused_seconds: 900,
            open_seconds: 3600,
        }];
        let daily = vec![
            day("2026-01-13", "Jan 13", 1800),
            day("2026-01-14", "Jan 14", 900),
        ];

        let insights = analyze(input(&rows, &daily, 900, 3600, 0, 0));
        let comparison = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::DayComparison)
            .unwrap();

        assert_eq!(comparison.tone, InsightTone::Info);
        assert_eq!(comparison.value, "15m less");
        assert_eq!(comparison.supporting.comparison_seconds, Some(1800));
        assert_eq!(comparison.supporting.delta_seconds, Some(-900));
    }

    #[test]
    fn historical_day_comparison_uses_selected_day_not_real_today() {
        let rows = vec![AppTotals {
            app_class: "firefox".to_string(),
            focused_seconds: 2400,
            open_seconds: 3600,
        }];
        let daily = vec![
            day("2026-01-11", "Jan 11", 1800),
            day("2026-01-12", "Jan 12", 2400),
        ];
        let mut input = input(&rows, &daily, 2400, 3600, 0, 0);
        input.today_key = "2026-01-14";
        input.selected_day_key = "2026-01-12";
        input.period = AnalysisPeriod {
            lens: AnalysisLens::Day,
            label: "Jan 12",
            start_date: Some("2026-01-12"),
            end_date: Some("2026-01-12"),
        };

        let insights = analyze(input);
        let comparison = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::DayComparison)
            .unwrap();

        assert_eq!(comparison.title, "Compared with the day before");
        assert_eq!(comparison.value, "10m more");
        assert_eq!(comparison.supporting.date.as_deref(), Some("2026-01-12"));
        assert_eq!(
            comparison.supporting.comparison_date.as_deref(),
            Some("2026-01-11")
        );
        assert_eq!(
            comparison.supporting.comparison_label.as_deref(),
            Some("Previous day")
        );
    }

    #[test]
    fn emits_first_pass_pattern_quality_app_and_workspace_facts() {
        let rows = vec![
            AppTotals {
                app_class: "ghostty".to_string(),
                focused_seconds: 5400,
                open_seconds: 7200,
            },
            AppTotals {
                app_class: "firefox".to_string(),
                focused_seconds: 1800,
                open_seconds: 5400,
            },
        ];
        let daily = vec![
            day("2026-01-08", "Thu", 600),
            day("2026-01-09", "Fri", 1200),
            day("2026-01-10", "Sat", 0),
            day("2026-01-11", "Sun", 1800),
            day("2026-01-12", "Mon", 900),
            day("2026-01-13", "Tue", 1500),
            day("2026-01-14", "Wed", 1200),
        ];
        let heatmap = vec![heat(2, 9, 3600), heat(2, 10, 1800), heat(1, 9, 1800)];
        let focus_intervals = vec![
            focus("ghostty", 0, 1800),
            focus("firefox", 1800, 2400),
            focus("ghostty", 2400, 4500),
            focus("firefox", 4500, 5400),
            focus("ghostty", 5400, 7200),
        ];
        let workspaces = vec![workspace("code", 5400), workspace("web", 1800)];
        let app_workspaces = vec![
            app_workspace("code", "ghostty", 5400),
            app_workspace("web", "firefox", 1800),
        ];

        let insights = analyze(input_with_context(
            &rows,
            &daily,
            &heatmap,
            &focus_intervals,
            &workspaces,
            &app_workspaces,
        ));

        let kinds = insights
            .iter()
            .map(|insight| insight.kind)
            .collect::<Vec<_>>();
        assert!(kinds.contains(&InsightKind::PeakFocusHour));
        assert!(kinds.contains(&InsightKind::PeakFocusWeekday));
        assert!(!kinds.contains(&InsightKind::CurrentStreak));
        assert!(!kinds.contains(&InsightKind::LongestStreak));
        assert!(kinds.contains(&InsightKind::DeepWorkBlocks));
        assert!(kinds.contains(&InsightKind::AppSwitchRate));
        assert!(kinds.contains(&InsightKind::FragmentedApp));
        assert!(kinds.contains(&InsightKind::AppFocusDensity));
        assert!(kinds.contains(&InsightKind::EffectiveApps));
        assert!(kinds.contains(&InsightKind::StrongestWorkspace));
        assert!(kinds.contains(&InsightKind::WorkspaceAppAffinity));

        let deep = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::DeepWorkBlocks)
            .unwrap();
        assert_eq!(deep.supporting.block_count, Some(3));
        assert_eq!(deep.supporting.longest_seconds, Some(2100));
        assert_eq!(deep.supporting.median_seconds, Some(1800));

        let switch_rate = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::AppSwitchRate)
            .unwrap();
        assert_eq!(switch_rate.supporting.switch_count, Some(4));

        let affinity = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::WorkspaceAppAffinity)
            .unwrap();
        assert_eq!(affinity.supporting.workspace.as_deref(), Some("code"));
        assert_eq!(affinity.supporting.app_class.as_deref(), Some("ghostty"));
        assert_eq!(affinity.supporting.share, Some(1.0));
    }

    #[test]
    fn emits_week_vs_previous_week_trend() {
        let rows = vec![AppTotals {
            app_class: "ghostty".to_string(),
            focused_seconds: 7200,
            open_seconds: 9000,
        }];
        let daily = vec![
            day("2026-01-12", "Mon", 1800),
            day("2026-01-13", "Tue", 2400),
            day("2026-01-14", "Wed", 3000),
        ];
        let mut analysis = input(&rows, &daily, 7200, 9000, 0, 0);
        analysis.period = AnalysisPeriod {
            lens: AnalysisLens::Week,
            label: "Week of Jan 12, 2026",
            start_date: Some("2026-01-12"),
            end_date: Some("2026-01-18"),
        };
        analysis.previous_period = Some(AnalysisComparisonPeriod {
            label: "Week of Jan 5, 2026".to_string(),
            start_date: Some("2026-01-05".to_string()),
            end_date: Some("2026-01-11".to_string()),
            focused_seconds: 3600,
            matched_elapsed: false,
            observed_seconds: 86400,
            elapsed_seconds: 86400,
        });

        let insights = analyze(analysis);
        let trend = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::PeriodComparison)
            .unwrap();

        assert_eq!(trend.title, "Compared with the week before");
        assert_eq!(trend.value, "1h more");
        assert_eq!(trend.tone, InsightTone::Info);
        assert_eq!(trend.supporting.comparison_seconds, Some(3600));
    }

    #[test]
    fn elapsed_current_period_comparison_says_so_far() {
        let rows = vec![AppTotals {
            app_class: "ghostty".to_string(),
            focused_seconds: 7200,
            open_seconds: 9000,
        }];
        let daily = vec![
            day("2026-01-12", "Jan 12", 1800),
            day("2026-01-13", "Jan 13", 2400),
            day("2026-01-14", "Jan 14", 3000),
        ];
        let mut analysis = input(&rows, &daily, 7200, 9000, 0, 0);
        analysis.period = AnalysisPeriod {
            lens: AnalysisLens::Week,
            label: "Week of Jan 12, 2026",
            start_date: Some("2026-01-12"),
            end_date: Some("2026-01-18"),
        };
        analysis.previous_period = Some(AnalysisComparisonPeriod {
            label: "Week of Jan 5, 2026".to_string(),
            start_date: Some("2026-01-05".to_string()),
            end_date: Some("2026-01-11".to_string()),
            focused_seconds: 3600,
            matched_elapsed: true,
            observed_seconds: 86400,
            elapsed_seconds: 86400,
        });

        let insights = analyze(analysis);
        let trend = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::PeriodComparison)
            .unwrap();

        assert!(trend.explanation.contains("so far"));
    }

    #[test]
    fn emits_month_vs_previous_month_trend() {
        let rows = vec![AppTotals {
            app_class: "ghostty".to_string(),
            focused_seconds: 14_400,
            open_seconds: 18_000,
        }];
        let daily = vec![
            day("2026-01-12", "Mon", 3600),
            day("2026-01-13", "Tue", 3600),
            day("2026-01-14", "Wed", 7200),
        ];
        let mut analysis = input(&rows, &daily, 14_400, 18_000, 0, 0);
        analysis.period = AnalysisPeriod {
            lens: AnalysisLens::Month,
            label: "January 2026",
            start_date: Some("2026-01-01"),
            end_date: Some("2026-01-31"),
        };
        analysis.previous_period = Some(AnalysisComparisonPeriod {
            label: "December 2025".to_string(),
            start_date: Some("2025-12-01".to_string()),
            end_date: Some("2025-12-31".to_string()),
            focused_seconds: 7200,
            matched_elapsed: false,
            observed_seconds: 86400,
            elapsed_seconds: 86400,
        });

        let insights = analyze(analysis);
        let trend = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::PeriodComparison)
            .unwrap();

        assert_eq!(trend.title, "Compared with the month before");
        assert_eq!(trend.value, "2h more");
        assert_eq!(trend.tone, InsightTone::Info);
        assert_eq!(
            trend.supporting.comparison_label.as_deref(),
            Some("December 2025")
        );
    }

    #[test]
    fn emits_idle_locked_and_unobserved_impact() {
        let rows = vec![AppTotals {
            app_class: "firefox".to_string(),
            focused_seconds: 3600,
            open_seconds: 7200,
        }];
        let daily = vec![day("2026-01-14", "Jan 14", 3600)];
        let mut analysis = input(&rows, &daily, 3600, 7200, 0, 3600);
        analysis.total_idle_seconds = 900;
        analysis.total_locked_seconds = 300;
        analysis.elapsed_seconds = 12000;
        analysis.observed_seconds = 8400;

        let insights = analyze(analysis);

        assert!(
            insights
                .iter()
                .any(|insight| insight.kind == InsightKind::IdleExcluded)
        );
        assert!(
            insights
                .iter()
                .any(|insight| insight.kind == InsightKind::LockedExcluded)
        );
        assert!(
            insights
                .iter()
                .any(|insight| insight.kind == InsightKind::UnobservedAnomaly)
        );
        let impact = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::ExcludedImpact)
            .unwrap();
        assert_eq!(impact.supporting.idle_seconds, Some(900));
        assert_eq!(impact.supporting.locked_seconds, Some(300));
        assert_eq!(impact.supporting.unobserved_seconds, Some(3600));
        assert_eq!(impact.supporting.excluded_seconds, Some(4800));
    }

    fn input<'a>(
        rows: &'a [AppTotals],
        daily: &'a [DayTotals],
        focused: i64,
        open: i64,
        sleep: i64,
        unobserved: i64,
    ) -> AnalysisInput<'a> {
        AnalysisInput {
            rows,
            daily,
            heatmap: &[],
            focus_intervals: &[],
            workspaces: &[],
            app_workspaces: &[],
            today_key: "2026-01-14",
            selected_day_key: "2026-01-14",
            period: AnalysisPeriod {
                lens: AnalysisLens::Day,
                label: "Today",
                start_date: Some("2026-01-14"),
                end_date: Some("2026-01-14"),
            },
            previous_period: None,
            total_focused_seconds: focused,
            observed_seconds: 86400,
            elapsed_seconds: 86400,
            continuity: None,
            pauses: None,
            total_open_seconds: open,
            total_idle_seconds: 0,
            total_locked_seconds: 0,
            total_sleep_seconds: sleep,
            total_unobserved_seconds: unobserved,
        }
    }

    fn input_with_context<'a>(
        rows: &'a [AppTotals],
        daily: &'a [DayTotals],
        heatmap: &'a [FocusHeatCell],
        focus_intervals: &'a [TimelineInterval],
        workspaces: &'a [WorkspaceTotals],
        app_workspaces: &'a [AppWorkspaceTotals],
    ) -> AnalysisInput<'a> {
        AnalysisInput {
            rows,
            daily,
            heatmap,
            focus_intervals,
            workspaces,
            app_workspaces,
            today_key: "2026-01-14",
            selected_day_key: "2026-01-14",
            period: AnalysisPeriod {
                lens: AnalysisLens::Day,
                label: "Today",
                start_date: Some("2026-01-14"),
                end_date: Some("2026-01-14"),
            },
            previous_period: None,
            total_focused_seconds: rows.iter().map(|row| row.focused_seconds.max(0)).sum(),
            observed_seconds: 86400,
            elapsed_seconds: 86400,
            continuity: None,
            pauses: None,
            total_open_seconds: rows.iter().map(|row| row.open_seconds.max(0)).sum(),
            total_idle_seconds: 0,
            total_locked_seconds: 0,
            total_sleep_seconds: 0,
            total_unobserved_seconds: 0,
        }
    }

    fn day(date: &str, label: &str, focused_seconds: i64) -> DayTotals {
        DayTotals {
            date: date.to_string(),
            label: label.to_string(),
            focused_seconds,
            open_seconds: focused_seconds,
            elapsed_seconds: 86400,
            observed_seconds: 86400,
            idle_seconds: 0,
            locked_seconds: 0,
            sleep_seconds: 0,
            unobserved_seconds: 0,
        }
    }

    fn heat(weekday: u32, hour: u32, focused_seconds: i64) -> FocusHeatCell {
        FocusHeatCell {
            weekday,
            hour,
            focused_seconds,
        }
    }

    fn focus(app_class: &str, started_at: i64, ended_at: i64) -> TimelineInterval {
        TimelineInterval {
            kind: IntervalKind::Focused,
            app_class: app_class.to_string(),
            started_at,
            ended_at,
        }
    }

    fn workspace(workspace: &str, focused_seconds: i64) -> WorkspaceTotals {
        WorkspaceTotals {
            workspace: workspace.to_string(),
            focused_seconds,
        }
    }

    fn app_workspace(workspace: &str, app_class: &str, focused_seconds: i64) -> AppWorkspaceTotals {
        AppWorkspaceTotals {
            workspace: workspace.to_string(),
            app_class: app_class.to_string(),
            focused_seconds,
        }
    }
}
