use chrono::{Duration, Local};
use omastat::{
    activity::midnight,
    config::Config,
    report::{self, Lens},
    steam::SteamResolver,
    storage::{IntervalKind, Storage},
};

#[test]
fn recent_game_routine_reaches_overview_and_detail_with_identical_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let mut storage = Storage::open(Some(&dir.path().join("routine.db")), &config).unwrap();
    let today = Local::now().date_naive();
    let from = midnight(today - Duration::days(56)).unwrap();
    let end = midnight(today).unwrap();
    let run = storage.start_daemon_run(from).unwrap();
    storage.finish_daemon_run(run.run_id, end).unwrap();
    for (ago, minute) in [
        (13, 1257),
        (11, 1233),
        (10, 1276),
        (8, 1270),
        (7, 1215),
        (6, 1283),
        (5, 1265),
        (4, 1262),
        (2, 1181),
        (1, 1196),
    ] {
        let start = midnight(today - Duration::days(ago)).unwrap() + minute * 60;
        let id = storage
            .start_interval(IntervalKind::Focused, "Slay the Spire 2", None, None, start)
            .unwrap();
        storage.close_interval(id, start + 4500).unwrap();
    }
    let mut reference = None;
    for lens in [Lens::Day, Lens::Week, Lens::Month, Lens::Year, Lens::Life] {
        let overview = report::usage_report_for_period(
            &storage,
            &mut SteamResolver::default(),
            &config,
            lens,
            0,
        )
        .unwrap();
        let detail = report::activity_detail(
            &storage,
            &mut SteamResolver::default(),
            &config,
            lens,
            0,
            "app",
            "Slay the Spire 2",
        )
        .unwrap();
        let insight = overview
            .insights
            .iter()
            .find(|i| i.supporting.activity_key.as_deref() == Some("Slay the Spire 2"))
            .unwrap();
        assert_eq!(insight, &detail.insights[0]);
        assert_eq!(insight.supporting.occurrence_count, Some(10));
        assert_eq!(insight.supporting.eligible_count, Some(14));
        if let Some(previous) = &reference {
            assert_eq!(previous, insight);
        } else {
            reference = Some(insight.clone());
        }
        let detail_total: i64 = detail.daily.iter().map(|d| d.focused_seconds).sum();
        assert_eq!(detail_total, overview.total_focused_seconds);
        assert!(overview.activity_analytics.daily.is_empty());
        assert!(overview.activity_analytics.heatmap.is_empty());
    }
}

#[test]
fn overview_diversifies_before_limiting_routines() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::default();
    let mut storage = Storage::open(Some(&dir.path().join("diverse.db")), &config).unwrap();
    let today = Local::now().date_naive();
    let from = midnight(today - Duration::days(56)).unwrap();
    let end = midnight(today).unwrap();
    let run = storage.start_daemon_run(from).unwrap();
    storage.finish_daemon_run(run.run_id, end).unwrap();
    for day in 1..=56 {
        for app in 0..14 {
            let start = midnight(today - Duration::days(day)).unwrap() + app * 3600 + 600;
            let id = storage
                .start_interval(
                    IntervalKind::Focused,
                    &format!("app-{app:02}"),
                    None,
                    None,
                    start,
                )
                .unwrap();
            storage.close_interval(id, start + 600).unwrap();
        }
    }
    let overview = report::usage_report_for_period(
        &storage,
        &mut SteamResolver::default(),
        &config,
        Lens::Week,
        0,
    )
    .unwrap();
    let routines = &overview.activity_analytics.insights;
    assert_eq!(routines.len(), 12);
    let keys = routines
        .iter()
        .map(|i| i.supporting.activity_key.as_ref().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys.len(), 12);
}
