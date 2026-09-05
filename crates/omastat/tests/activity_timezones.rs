use chrono::{DateTime, NaiveDate};
use omastat::{
    config::Config,
    storage::{IntervalKind, Storage},
};
use serde_json::Value;
use std::process::Command;

#[test]
fn local_day_and_hour_totals_reconcile_through_dst_transitions() {
    for (date, from, to) in [
        (
            "2026-03-08",
            "2026-03-08T01:30:00-05:00",
            "2026-03-08T04:30:00-04:00",
        ),
        (
            "2025-11-02",
            "2025-11-02T01:30:00-04:00",
            "2025-11-02T02:30:00-05:00",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("dst.db");
        let storage = Storage::open(Some(&db), &Config::default()).unwrap();
        let from = DateTime::parse_from_rfc3339(from).unwrap().timestamp();
        let to = DateTime::parse_from_rfc3339(to).unwrap().timestamp();
        let id = storage
            .start_interval(IntervalKind::Focused, "editor", None, None, from)
            .unwrap();
        storage.close_interval(id, to).unwrap();
        // Read the target timezone's current date from the CLI itself, avoiding process-global TZ mutation.
        let current = Command::new(env!("CARGO_BIN_EXE_omastat"))
            .env("TZ", "America/New_York")
            .args([
                "--database",
                db.to_str().unwrap(),
                "widget-summary",
                "--lens",
                "day",
            ])
            .output()
            .unwrap();
        assert!(
            current.status.success(),
            "{}",
            String::from_utf8_lossy(&current.stderr)
        );
        let current: Value = serde_json::from_slice(&current.stdout).unwrap();
        let today =
            NaiveDate::parse_from_str(current["today_key"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        let date = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        let offset = (date - today).num_days().to_string();
        let output = Command::new(env!("CARGO_BIN_EXE_omastat"))
            .env("TZ", "America/New_York")
            .args([
                "--database",
                db.to_str().unwrap(),
                "activity-detail",
                "--lens",
                "day",
                "--offset",
                &offset,
                "--app",
                "editor",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["activities"][0]["focused_seconds"], 7200);
        assert_eq!(report["daily"].as_array().unwrap().len(), 1);
        assert_eq!(report["daily"][0]["focused_seconds"], 7200);
        assert_eq!(
            report["heatmap"]
                .as_array()
                .unwrap()
                .iter()
                .map(|h| h["focused_seconds"].as_i64().unwrap())
                .sum::<i64>(),
            7200
        );
    }
}

#[test]
fn evening_routines_keep_local_times_and_distinct_dates_across_dst() {
    for (end, transition, before_offset, after_offset) in [
        ("2026-03-16", "2026-03-08", "-05:00", "-04:00"),
        ("2025-11-10", "2025-11-02", "-04:00", "-05:00"),
    ] {
        let end = NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap();
        let transition = NaiveDate::parse_from_str(transition, "%Y-%m-%d").unwrap();
        let timestamp = |date: NaiveDate, hour: u32| {
            DateTime::parse_from_rfc3339(&format!(
                "{date}T{hour:02}:00:00{}",
                if date < transition {
                    before_offset
                } else {
                    after_offset
                }
            ))
            .unwrap()
            .timestamp()
        };
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("routines.db");
        let config = Config::default();
        let mut storage = Storage::open(Some(&db), &config).unwrap();
        let from = end - chrono::Duration::days(14);
        let run = storage.start_daemon_run(timestamp(from, 0)).unwrap();
        storage
            .finish_daemon_run(run.run_id, timestamp(end, 0))
            .unwrap();
        for n in 1..=14 {
            let start = timestamp(end - chrono::Duration::days(n), 20);
            let id = storage
                .start_interval(IntervalKind::Focused, "evening", None, None, start)
                .unwrap();
            storage.close_interval(id, start + 1800).unwrap();
        }
        let cli = |args: &[&str]| {
            let output = Command::new(env!("CARGO_BIN_EXE_omastat"))
                .env("TZ", "America/New_York")
                .args(["--database", db.to_str().unwrap()])
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            serde_json::from_slice::<Value>(&output.stdout).unwrap()
        };
        let current = cli(&["widget-summary"]);
        let today =
            NaiveDate::parse_from_str(current["today_key"].as_str().unwrap(), "%Y-%m-%d").unwrap();
        let offset = (end - chrono::Duration::days(1) - today)
            .num_days()
            .to_string();
        let detail = cli(&[
            "activity-detail",
            "--lens",
            "day",
            "--offset",
            &offset,
            "--app",
            "evening",
        ]);
        let routine = &detail["insights"][0];
        assert_eq!(routine["supporting"]["occurrence_count"], 14);
        assert_eq!(routine["supporting"]["eligible_count"], 14);
        let window = &routine["supporting"]["routine"];
        let start = window["start_minute"].as_u64().unwrap();
        let end = window["end_minute"].as_u64().unwrap();
        assert!(start <= 20 * 60 && end >= 20 * 60);
        assert_eq!(
            routine["supporting"]["matching_dates"]
                .as_array()
                .unwrap()
                .len(),
            14
        );
    }
}
