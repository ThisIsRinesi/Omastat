use crate::{
    clock,
    config::Config,
    insights::{Insight, InsightCategory, InsightTone},
    report::{self, Lens},
    steam::SteamResolver,
    storage::{AppTotals, DayTotals, RawExportRows, Storage},
};
use anyhow::Result;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataExportScope {
    All,
    Raw,
    Aggregate,
}

#[derive(Debug, Clone)]
pub struct DataExportOptions {
    pub lens: Lens,
    pub offset: i32,
    pub scope: DataExportScope,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataExport {
    pub schema_version: u32,
    pub generated_at: i64,
    pub timezone: String,
    pub query_start_ts: i64,
    pub query_end_ts: i64,
    pub period: report::Period,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<AggregateExport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<RawExportRows>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregateExport {
    pub app_totals: Vec<AppTotals>,
    pub app_breakdown: Vec<report::AppBreakdown>,
    pub daily_totals: Vec<DayTotals>,
    pub insights: Vec<Insight>,
}

pub fn build_data_export(
    storage: &Storage,
    steam: &mut SteamResolver,
    config: &Config,
    options: DataExportOptions,
) -> Result<DataExport> {
    let report =
        report::usage_report_for_period(storage, steam, config, options.lens, options.offset)?;
    let raw = matches!(options.scope, DataExportScope::All | DataExportScope::Raw)
        .then(|| storage.raw_export_between(report.query_start_ts, report.query_end_ts))
        .transpose()?;
    let aggregate = matches!(
        options.scope,
        DataExportScope::All | DataExportScope::Aggregate
    )
    .then(|| AggregateExport {
        app_totals: report.rows.clone(),
        app_breakdown: report.apps.clone(),
        daily_totals: report.daily.clone(),
        insights: report.insights.clone(),
    });

    Ok(DataExport {
        schema_version: 1,
        generated_at: report.generated_at,
        timezone: clock::local_now().offset().to_string(),
        query_start_ts: report.query_start_ts,
        query_end_ts: report.query_end_ts,
        period: report.period,
        aggregate,
        raw,
    })
}

pub fn write_data_export_csv(export: &DataExport, output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir)?;
    fs::write(
        output_dir.join("metadata.json"),
        serde_json::to_string_pretty(&ExportMetadata::from(export))?,
    )?;

    if let Some(aggregate) = &export.aggregate {
        write_csv(output_dir.join("app_totals.csv"), &aggregate.app_totals)?;
        write_csv(
            output_dir.join("app_breakdown.csv"),
            &aggregate.app_breakdown,
        )?;
        write_csv(output_dir.join("daily_totals.csv"), &aggregate.daily_totals)?;
        let insight_rows = aggregate
            .insights
            .iter()
            .map(InsightCsvRow::from)
            .collect::<Vec<_>>();
        write_csv(output_dir.join("insights.csv"), &insight_rows)?;
    }

    if let Some(raw) = &export.raw {
        write_csv(output_dir.join("raw_intervals.csv"), &raw.intervals)?;
        write_csv(
            output_dir.join("raw_session_intervals.csv"),
            &raw.session_intervals,
        )?;
        write_csv(
            output_dir.join("raw_system_intervals.csv"),
            &raw.system_intervals,
        )?;
    }

    Ok(())
}

#[derive(Debug, Serialize)]
struct ExportMetadata {
    schema_version: u32,
    generated_at: i64,
    timezone: String,
    query_start_ts: i64,
    query_end_ts: i64,
    period_label: String,
    period_start_date: Option<String>,
    period_end_date: Option<String>,
}

impl From<&DataExport> for ExportMetadata {
    fn from(export: &DataExport) -> Self {
        Self {
            schema_version: export.schema_version,
            generated_at: export.generated_at,
            timezone: export.timezone.clone(),
            query_start_ts: export.query_start_ts,
            query_end_ts: export.query_end_ts,
            period_label: export.period.label.clone(),
            period_start_date: export.period.start_date.clone(),
            period_end_date: export.period.end_date.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct InsightCsvRow {
    kind: String,
    category: String,
    tone: String,
    confidence: String,
    title: String,
    value: String,
    explanation: String,
    data_points: usize,
    minimum_data_points: usize,
    observed_focus_seconds: i64,
    observed_open_seconds: i64,
    app_class: Option<String>,
    app_label: Option<String>,
    workspace: Option<String>,
    focused_seconds: Option<i64>,
    open_seconds: Option<i64>,
    excluded_seconds: Option<i64>,
    share: Option<f64>,
    routine_evidence: Option<String>,
    matching_dates: Option<String>,
}

impl From<&Insight> for InsightCsvRow {
    fn from(insight: &Insight) -> Self {
        Self {
            kind: serde_json::to_value(insight.kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_string))
                .unwrap_or_else(|| format!("{:?}", insight.kind)),
            category: insight_category_label(insight.category).to_string(),
            tone: insight_tone_label(insight.tone).to_string(),
            confidence: format!("{:?}", insight.confidence).to_lowercase(),
            title: insight.title.clone(),
            value: insight.value.clone(),
            explanation: insight.explanation.clone(),
            data_points: insight.evidence.data_points,
            minimum_data_points: insight.evidence.minimum_data_points,
            observed_focus_seconds: insight.evidence.observed_focus_seconds,
            observed_open_seconds: insight.evidence.observed_open_seconds,
            app_class: insight.supporting.app_class.clone(),
            app_label: insight.supporting.app_label.clone(),
            workspace: insight.supporting.workspace.clone(),
            focused_seconds: insight.supporting.focused_seconds,
            open_seconds: insight.supporting.open_seconds,
            excluded_seconds: insight.supporting.excluded_seconds,
            share: insight.supporting.share,
            routine_evidence: insight
                .supporting
                .routine
                .as_ref()
                .and_then(|r| serde_json::to_string(r).ok()),
            matching_dates: insight
                .supporting
                .matching_dates
                .as_ref()
                .map(|d| d.join(", ")),
        }
    }
}

fn write_csv<T: Serialize>(path: PathBuf, rows: &[T]) -> Result<()> {
    let mut writer = csv::Writer::from_path(path)?;
    for row in rows {
        writer.serialize(row)?;
    }
    writer.flush()?;
    Ok(())
}

fn insight_category_label(category: InsightCategory) -> &'static str {
    match category {
        InsightCategory::Patterns => "Patterns",
        InsightCategory::FocusQuality => "Focus quality",
        InsightCategory::Apps => "Apps",
        InsightCategory::SystemSignals => "System signals",
    }
}

fn insight_tone_label(tone: InsightTone) -> &'static str {
    match tone {
        InsightTone::Positive => "Positive",
        InsightTone::Negative => "Negative",
        InsightTone::Neutral => "Neutral",
        InsightTone::Info => "Info",
        InsightTone::Caution => "Caution",
    }
}

#[cfg(test)]
mod tests {
    use super::{DataExportOptions, DataExportScope, build_data_export, write_data_export_csv};
    use crate::{
        config::Config,
        report::Lens,
        steam::SteamResolver,
        storage::{IntervalKind, Storage},
    };

    #[test]
    fn builds_json_data_export_with_raw_and_aggregate_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let focused = storage
            .start_interval(IntervalKind::Focused, "firefox", None, Some("Docs"), 100)
            .unwrap();
        storage.close_interval(focused, 220).unwrap();

        let mut steam = SteamResolver::default();
        let export = build_data_export(
            &storage,
            &mut steam,
            &config,
            DataExportOptions {
                lens: Lens::Life,
                offset: 0,
                scope: DataExportScope::All,
            },
        )
        .unwrap();

        assert!(export.aggregate.as_ref().unwrap().app_totals.len() == 1);
        assert!(export.raw.as_ref().unwrap().intervals.len() == 1);
        let json = serde_json::to_value(&export).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert!(json["raw"]["intervals"][0]["local_start"].is_string());
    }

    #[test]
    fn writes_csv_data_export_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let focused = storage
            .start_interval(IntervalKind::Focused, "firefox", None, None, 100)
            .unwrap();
        storage.close_interval(focused, 200).unwrap();

        let mut steam = SteamResolver::default();
        let export = build_data_export(
            &storage,
            &mut steam,
            &config,
            DataExportOptions {
                lens: Lens::Life,
                offset: 0,
                scope: DataExportScope::All,
            },
        )
        .unwrap();
        let out = dir.path().join("csv");
        write_data_export_csv(&export, &out).unwrap();

        assert!(out.join("metadata.json").exists());
        assert!(out.join("app_totals.csv").exists());
        assert!(out.join("raw_intervals.csv").exists());
    }
}
