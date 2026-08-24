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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InsightSupport {
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
    let blocks = focus_blocks(input.focus_intervals);

    if input.total_focused_seconds > 0 {
        push_top_app(&input, &mut out);
        push_day_comparison(&input, &mut out);
        push_period_comparison(&input, &mut out);
        push_day_facts(&input, &mut out);
        push_peak_facts(&input, &mut out);
        push_deep_work_facts(&input, &blocks, &mut out);
        push_switch_facts(&input, &blocks, &mut out);
        push_density_facts(&input, &mut out);
        push_effective_app_fact(&input, &mut out);
        push_workspace_facts(&input, &mut out);
        push_anomaly_facts(&input, &mut out);
    }

    push_system_facts(&input, &mut out);

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
        title: "Top app share".to_string(),
        value: format!(
            "{} - {} ({})",
            app_label,
            format_duration(top.focused_seconds),
            percent(share)
        ),
        explanation: "The app with the largest share of focused time in this period.".to_string(),
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
    if yesterday <= 0 {
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
            "vs yesterday".to_string()
        } else {
            "vs previous day".to_string()
        },
        value: signed_duration(delta),
        explanation: "Compares focused time for the selected day with the previous local day."
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
    let (label, minimum_days, explanation) = match input.period.lens {
        AnalysisLens::Week => (
            "week",
            7,
            "Compares this week with the previous local Monday-through-Sunday week.",
        ),
        AnalysisLens::Month => (
            "month",
            14,
            "Compares this month with the previous local calendar month.",
        ),
        AnalysisLens::Year => (
            "year",
            30,
            "Compares this year with the previous local calendar year.",
        ),
        AnalysisLens::Day | AnalysisLens::Life => {
            return;
        }
    };

    let Some(previous) = input.previous_period.as_ref() else {
        return;
    };
    if previous.focused_seconds <= 0 {
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
        title: format!("vs previous {label}"),
        value: signed_duration(delta),
        explanation: explanation.to_string(),
        confidence: confidence(input.daily.len(), minimum_days),
        evidence: evidence(input, minimum_days),
        supporting: support,
    });
}

fn push_day_facts(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    let active_days = input
        .daily
        .iter()
        .filter(|day| day.focused_seconds > 0)
        .collect::<Vec<_>>();

    if let Some(best) = active_days.iter().max_by_key(|day| day.focused_seconds) {
        out.push(Insight {
            kind: InsightKind::BestDay,
            category: InsightCategory::Patterns,
            tone: InsightTone::Positive,
            title: "Best day".to_string(),
            value: format!(
                "{} - {}",
                relative_day_label(best, input.today_key),
                format_duration(best.focused_seconds)
            ),
            explanation: "The highest-focus day visible in the loaded period history.".to_string(),
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
            title: "Lightest active day".to_string(),
            value: format!(
                "{} - {}",
                relative_day_label(worst, input.today_key),
                format_duration(worst.focused_seconds)
            ),
            explanation: "The lowest-focus day that still had tracked focus in this period."
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

    if input.daily.len() >= 2 {
        let current = current_active_streak(input.daily);
        out.push(Insight {
            kind: InsightKind::CurrentStreak,
            category: InsightCategory::Patterns,
            tone: if current > 0 {
                InsightTone::Positive
            } else {
                InsightTone::Neutral
            },
            title: "Current streak".to_string(),
            value: format_days(current),
            explanation: "Consecutive focused days ending at the latest loaded local day."
                .to_string(),
            confidence: confidence(input.daily.len(), 2),
            evidence: evidence(input, 2),
            supporting: period_support(input.period).with_streak(current, None),
        });
    }

    let longest = longest_active_streak(input.daily);
    if longest > 0 && input.daily.len() >= 2 {
        out.push(Insight {
            kind: InsightKind::LongestStreak,
            category: InsightCategory::Patterns,
            tone: InsightTone::Positive,
            title: "Longest streak".to_string(),
            value: format_days(longest),
            explanation: "Longest run of consecutive local days with any focused time.".to_string(),
            confidence: confidence(input.daily.len(), 2),
            evidence: evidence(input, 2),
            supporting: period_support(input.period).with_streak(0, Some(longest)),
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
            title: "Peak focus hour".to_string(),
            value: format!(
                "{} - {}",
                hour_label(peak.hour),
                format_duration(peak.focused_seconds)
            ),
            explanation: "The local clock hour with the most focused time in this period."
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
            title: "Peak focus weekday".to_string(),
            value: format!(
                "{} - {}",
                weekday_label(peak.weekday),
                format_duration(peak.focused_seconds)
            ),
            explanation: "The weekday with the most focused time in this period.".to_string(),
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
        title: "Deep work blocks".to_string(),
        value: format!(
            "{} - {} total",
            format_blocks(deep_count),
            format_duration(deep_total)
        ),
        explanation: format!(
            "Counts focused blocks at or above {}; longest block is {}, median block is {}.",
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
            .filter(|pair| pair[0].app_class != pair[1].app_class)
            .count();
        let rate = per_hour(switch_count, input.total_focused_seconds);
        out.push(Insight {
            kind: InsightKind::AppSwitchRate,
            category: InsightCategory::FocusQuality,
            tone: switch_rate_tone(rate),
            title: "App switches".to_string(),
            value: format!("{} switches/hour", format_rate(rate)),
            explanation: "Counts focused app changes normalized by focused hours.".to_string(),
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
            title: "Most fragmented app".to_string(),
            value: format!(
                "{} - {} blocks/hour",
                app_label,
                format_rate(fragmented.rate_per_hour)
            ),
            explanation: "The app split across the most focus blocks per focused hour.".to_string(),
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
            title: "Focus density".to_string(),
            value: percent(density),
            explanation: "Focused time divided by open app time for the selected period."
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
        push_app_density(input, out, "Densest app", row, *density);
    }

    if app_densities.len() >= 2
        && let Some((row, density)) = app_densities.iter().min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| right.0.focused_seconds.cmp(&left.0.focused_seconds))
        })
        && *density < 0.65
    {
        push_app_density(input, out, "Lowest-density app", row, *density);
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
        explanation: "Focused time divided by open time for this app.".to_string(),
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
        title: "Effective app count".to_string(),
        value: format!("{} effective apps", format_decimal(effective)),
        explanation:
            "Shannon effective count; lower values mean focus is concentrated in fewer apps."
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
            title: "Strongest workspace".to_string(),
            value: format!(
                "{} - {} ({})",
                workspace.workspace,
                format_duration(workspace.focused_seconds),
                percent(share)
            ),
            explanation: "The workspace with the largest amount of focused time.".to_string(),
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
            title: "Workspace/app affinity".to_string(),
            value: format!("{} on {} - {}", app_label, row.workspace, percent(affinity)),
            explanation: "The strongest workspace association for an app with enough focused time."
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
    if let Some(insight) = unobserved_anomaly(input) {
        out.push(insight);
    }
}

fn push_system_facts(input: &AnalysisInput<'_>, out: &mut Vec<Insight>) {
    if input.total_idle_seconds > 0 {
        out.push(system_signal(
            input,
            InsightKind::IdleExcluded,
            "Idle",
            "Session idle time was excluded from focused time.",
            input.total_idle_seconds,
            InsightTone::Info,
        ));
    }

    if input.total_locked_seconds > 0 {
        out.push(system_signal(
            input,
            InsightKind::LockedExcluded,
            "Locked",
            "Session locked time was excluded from focused time.",
            input.total_locked_seconds,
            InsightTone::Info,
        ));
    }

    if input.total_sleep_seconds > 0 {
        out.push(system_signal(
            input,
            InsightKind::SleepExcluded,
            "Sleep",
            "System sleep was excluded from focused time.",
            input.total_sleep_seconds,
            InsightTone::Info,
        ));
    }

    if input.total_unobserved_seconds > 0 {
        out.push(system_signal(
            input,
            InsightKind::UnobservedExcluded,
            "Unobserved",
            "Daemon-offline time was excluded instead of being counted as active focus.",
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

    let impact = ratio(
        excluded,
        input.total_focused_seconds.saturating_add(excluded).max(1),
    );
    out.push(Insight {
        kind: InsightKind::ExcludedImpact,
        category: InsightCategory::SystemSignals,
        tone: if impact >= 0.25 {
            InsightTone::Caution
        } else {
            InsightTone::Info
        },
        title: "Excluded time impact".to_string(),
        value: format!("{} ({})", format_duration(excluded), percent(impact)),
        explanation: "Idle, locked, sleep, and daemon-offline time excluded from focus totals."
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
        value: format!("{} excluded", format_duration(seconds)),
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

    fn with_streak(
        mut self,
        current_streak_days: usize,
        longest_streak_days: Option<usize>,
    ) -> Self {
        if current_streak_days > 0 || longest_streak_days.is_none() {
            self.current_streak_days = Some(current_streak_days);
        }
        self.longest_streak_days = longest_streak_days;
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

fn focus_blocks(intervals: &[TimelineInterval]) -> Vec<FocusBlock> {
    intervals
        .iter()
        .filter_map(|interval| {
            let duration = interval.ended_at.saturating_sub(interval.started_at);
            (duration > 0).then(|| FocusBlock {
                app_class: interval.app_class.clone(),
                duration_seconds: duration,
            })
        })
        .collect()
}

fn median(sorted: &[i64]) -> i64 {
    analytics::median(sorted)
}

fn current_active_streak(days: &[DayTotals]) -> usize {
    days.iter()
        .rev()
        .take_while(|day| day.focused_seconds > 0)
        .count()
}

fn longest_active_streak(days: &[DayTotals]) -> usize {
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
    analytics::signed_duration(seconds)
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
    if input.daily.len() < 5 {
        return None;
    }

    let (best_index, best) = input
        .daily
        .iter()
        .enumerate()
        .max_by_key(|(_, day)| day.focused_seconds)?;
    if best.focused_seconds < 60 * 60 {
        return None;
    }

    let baseline = input
        .daily
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != best_index)
        .map(|(_, day)| day.focused_seconds.max(0))
        .collect::<Vec<_>>();
    if baseline.len() < 4 {
        return None;
    }

    let mean = mean_seconds(&baseline);
    let deviation = std_dev_seconds(&baseline, mean);
    let threshold = (deviation * 2.0).round().max(30.0 * 60.0) as i64;
    if best.focused_seconds < mean.round() as i64 + threshold {
        return None;
    }

    let mut support = period_support(input.period).with_day(
        &best.date,
        &relative_day_label(best, input.today_key),
        best.focused_seconds,
    );
    support.baseline_seconds = Some(mean.round().max(0.0) as i64);
    support.threshold_seconds = Some(threshold);

    Some(Insight {
        kind: InsightKind::FocusAnomaly,
        category: InsightCategory::Patterns,
        tone: InsightTone::Info,
        title: "Unusual focus spike".to_string(),
        value: format!(
            "{} - {}",
            relative_day_label(best, input.today_key),
            format_duration(best.focused_seconds)
        ),
        explanation: "This day is well above the recent daily focus baseline.".to_string(),
        confidence: confidence(input.daily.len(), 5),
        evidence: evidence(input, 5),
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
        title: "App concentration".to_string(),
        value: format!("{} held {}", app_label, percent(share)),
        explanation: "Focused time is unusually concentrated in one app for this period."
            .to_string(),
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
        title: "Hour concentration".to_string(),
        value: format!("{} held {}", hour_label(peak.hour), percent(share)),
        explanation: "A large share of focus landed in one local clock hour.".to_string(),
        confidence: confidence(input.daily.len(), 2),
        evidence: evidence(input, 2),
        supporting: period_support(input.period).with_hour(peak.hour, peak.focused_seconds, share),
    })
}

fn unobserved_anomaly(input: &AnalysisInput<'_>) -> Option<Insight> {
    if input.total_unobserved_seconds < 30 * 60 {
        return None;
    }

    let signal_total = input
        .total_focused_seconds
        .saturating_add(input.total_idle_seconds)
        .saturating_add(input.total_locked_seconds)
        .saturating_add(input.total_sleep_seconds)
        .saturating_add(input.total_unobserved_seconds);
    let share = ratio(input.total_unobserved_seconds, signal_total.max(1));
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
        title: "Unobserved gap anomaly".to_string(),
        value: format!(
            "{} unobserved ({})",
            format_duration(input.total_unobserved_seconds),
            percent(share)
        ),
        explanation: "Daemon-offline time is large enough to affect confidence in this period."
            .to_string(),
        confidence: InsightConfidence::High,
        evidence: evidence(input, 0),
        supporting: support,
    })
}

fn active_app_count(rows: &[AppTotals]) -> usize {
    rows.iter().filter(|row| row.focused_seconds > 0).count()
}

fn mean_seconds(values: &[i64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<i64>().max(0) as f64 / values.len() as f64
}

fn std_dev_seconds(values: &[i64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let variance = values
        .iter()
        .map(|value| {
            let delta = (*value).max(0) as f64 - mean;
            delta.powi(2)
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
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

fn format_days(days: usize) -> String {
    format!("{days}d")
}

fn format_blocks(blocks: usize) -> String {
    if blocks == 1 {
        "1 block".to_string()
    } else {
        format!("{blocks} blocks")
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
        assert_eq!(top.title, "Top app share");
        assert_eq!(
            top.supporting.app_class.as_deref(),
            Some("com.mitchellh.ghostty")
        );
        assert_eq!(top.supporting.app_label.as_deref(), Some("Ghostty"));
        assert_eq!(top.supporting.focused_seconds, Some(3600));

        let json = serde_json::to_value(top).unwrap();
        assert_eq!(json["kind"], "top-app");
        assert_eq!(json["category"], "apps");
        assert_eq!(json["tone"], "caution");
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
        assert_eq!(insights[0].value, "30m excluded");
        assert_eq!(insights[1].kind, InsightKind::UnobservedExcluded);
        assert_eq!(insights[1].tone, InsightTone::Caution);
        assert_eq!(insights[2].kind, InsightKind::ExcludedImpact);
        assert_eq!(insights[2].supporting.excluded_seconds, Some(2700));
    }

    #[test]
    fn negative_comparison_uses_negative_tone() {
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

        assert_eq!(comparison.tone, InsightTone::Negative);
        assert_eq!(comparison.value, "-15m");
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

        assert_eq!(comparison.title, "vs previous day");
        assert_eq!(comparison.value, "+10m");
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
        assert!(kinds.contains(&InsightKind::CurrentStreak));
        assert!(kinds.contains(&InsightKind::LongestStreak));
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
        });

        let insights = analyze(analysis);
        let trend = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::PeriodComparison)
            .unwrap();

        assert_eq!(trend.title, "vs previous week");
        assert_eq!(trend.value, "+1h");
        assert_eq!(trend.tone, InsightTone::Positive);
        assert_eq!(trend.supporting.comparison_seconds, Some(3600));
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
        });

        let insights = analyze(analysis);
        let trend = insights
            .iter()
            .find(|insight| insight.kind == InsightKind::PeriodComparison)
            .unwrap();

        assert_eq!(trend.title, "vs previous month");
        assert_eq!(trend.value, "+2h");
        assert_eq!(trend.tone, InsightTone::Positive);
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
