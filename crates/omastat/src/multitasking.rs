//! Concurrent background audio, kept separate from foreground time.
use crate::{
    activity::Coverage,
    config::Config,
    identity,
    storage::{FocusedIntervalMetadata, Storage},
};
use anyhow::Result;
use chrono::{Datelike, Local, TimeZone, Timelike};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn migrate(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch("CREATE TABLE media_intervals (
        id INTEGER PRIMARY KEY, source TEXT NOT NULL, app_class TEXT NOT NULL,
        label TEXT NOT NULL, started_at INTEGER NOT NULL, ended_at INTEGER,
        last_confirmed_at INTEGER NOT NULL, ttl INTEGER NOT NULL,
        CHECK(last_confirmed_at >= started_at), CHECK(ended_at IS NULL OR ended_at >= started_at));
        CREATE UNIQUE INDEX media_active ON media_intervals(source,app_class,label) WHERE ended_at IS NULL;
        CREATE INDEX media_time ON media_intervals(started_at,ended_at);
        CREATE TABLE media_state(source TEXT PRIMARY KEY, updated_at INTEGER NOT NULL);")
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MultitaskingReport {
    pub total_seconds: i64,
    pub daily: Vec<MultitaskingDay>,
    pub heatmap: Vec<crate::storage::FocusHeatCell>,
    pub sources: Vec<MultitaskingSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<ActivityTimeline>,
}
/// Detailed intervals are included only in day reports, using the same attribution sweep.
#[derive(Debug, Clone, Serialize)]
pub struct ActivityTimeline {
    pub start: i64,
    pub end: i64,
    pub segments: Vec<TimelineSegment>,
}
#[derive(Debug, Clone, Serialize)]
pub struct TimelineSegment {
    pub start: i64,
    pub end: i64,
    pub app_class: String,
    pub label: String,
    pub audio: Vec<TimelineSource>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimelineSource {
    pub app_class: String,
    pub label: String,
    pub attribution: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct MultitaskingDay {
    pub date: String,
    pub seconds: i64,
}
#[derive(Debug, Clone, Serialize)]
pub struct MultitaskingSource {
    pub app_class: String,
    pub label: String,
    pub attribution: String,
    pub seconds: i64,
}
#[derive(Debug, Clone, Serialize)]
pub struct MediaInterval {
    pub source: String,
    pub app_class: String,
    pub label: String,
    pub started_at: i64,
    pub ended_at: i64,
}

impl Storage {
    /// One transaction per snapshot, updating existing intervals instead of inserting samples.
    pub fn record_media_snapshot(
        &mut self,
        source: &str,
        items: &[(String, String)],
        at: i64,
        ttl: i64,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        let previous: Option<i64> = tx
            .query_row(
                "SELECT updated_at FROM media_state WHERE source=?1",
                [source],
                |r| r.get(0),
            )
            .optional()?;
        if previous.is_some_and(|p| at < p) {
            return Ok(());
        }
        tx.execute("INSERT INTO media_state VALUES(?1,?2) ON CONFLICT(source) DO UPDATE SET updated_at=excluded.updated_at", params![source,at])?;
        let active = {
            let mut stmt = tx.prepare("SELECT id,app_class,label,last_confirmed_at,ttl FROM media_intervals WHERE source=?1 AND ended_at IS NULL")?;
            stmt.query_map([source], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut remaining: BTreeSet<_> = items.iter().cloned().collect();
        for (id, app, label, confirmed, old_ttl) in active {
            let key = (app, label);
            if at <= confirmed.saturating_add(old_ttl) && remaining.remove(&key) {
                tx.execute(
                    "UPDATE media_intervals SET last_confirmed_at=?1 WHERE id=?2",
                    params![at, id],
                )?;
            } else {
                tx.execute("UPDATE media_intervals SET ended_at=MAX(started_at,MIN(?1,last_confirmed_at+ttl)) WHERE id=?2",params![at,id])?;
            }
        }
        for (app, label) in remaining {
            tx.execute("INSERT INTO media_intervals(source,app_class,label,started_at,last_confirmed_at,ttl) VALUES(?1,?2,?3,?4,?4,?5)",params![source,app,label,at,ttl.clamp(1,3600)])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn close_media_system(&mut self, at: i64) -> Result<()> {
        self.record_media_snapshot("system", &[], at, 45)
    }

    pub fn media_intervals_between(&self, start: i64, end: i64) -> Result<Vec<MediaInterval>> {
        let mut stmt = self.conn.prepare("SELECT source,app_class,label,MAX(started_at,?1),MIN(COALESCE(ended_at,?2),last_confirmed_at+ttl,?2) FROM media_intervals WHERE started_at<?2 AND COALESCE(ended_at,last_confirmed_at+ttl)>?1 ORDER BY started_at,id")?;
        Ok(stmt
            .query_map(params![start, end], |r| {
                Ok(MediaInterval {
                    source: r.get(0)?,
                    app_class: r.get(1)?,
                    label: r.get(2)?,
                    started_at: r.get(3)?,
                    ended_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(crate) fn multitasking_from_metadata(
        &self,
        start: i64,
        end: i64,
        focus: &[FocusedIntervalMetadata],
        config: &Config,
        include_timeline: bool,
    ) -> Result<MultitaskingReport> {
        let media = self.media_intervals_between(start, end)?;
        if !include_timeline
            && !media
                .iter()
                .any(|m| m.source == "system" && m.ended_at > m.started_at)
        {
            return Ok(MultitaskingReport::default());
        }
        // A single event sweep avoids an SQL overlap join for every focus fragment.
        let mut events = Vec::new();
        for (i, f) in focus.iter().enumerate() {
            let (a, b) = (f.started_at.max(start), f.ended_at.min(end));
            if b > a {
                events.push((a, true, false, i));
                events.push((b, false, false, i));
            }
        }
        for (i, m) in media.iter().enumerate() {
            if m.ended_at > m.started_at {
                events.push((m.started_at, true, true, i));
                events.push((m.ended_at, false, true, i));
            }
        }
        events.sort_unstable();
        let mut active_focus = BTreeSet::<usize>::new();
        let mut active_media = BTreeSet::<usize>::new();
        let mut spans = Vec::new();
        let mut timeline: Vec<TimelineSegment> = Vec::new();
        let mut grouped = BTreeMap::<(String, String, String), i64>::new();
        let mut foreground_apps = vec![None; focus.len()];
        let mut cursor = start;
        for (at, add, is_media, index) in events {
            if at > cursor
                && let Some(&f) = active_focus.last()
            {
                let foreground = foreground_apps[f]
                    .get_or_insert_with(|| identity::canonical_app_class(&focus[f].app_class));
                let mut keys = BTreeSet::<(String, String, String)>::new();
                for &i in &active_media {
                    let m: &MediaInterval = &media[i];
                    if m.source != "system" || m.app_class.eq_ignore_ascii_case(foreground) {
                        continue;
                    }
                    let domains: BTreeSet<_> = if config.privacy.browser_domains {
                        active_media
                            .iter()
                            .filter_map(|&j| {
                                let d = &media[j];
                                (d.source.starts_with("browser:") && d.app_class == m.app_class)
                                    .then_some(d.label.clone())
                            })
                            .collect()
                    } else {
                        BTreeSet::new()
                    };
                    if !domains.is_empty() {
                        for domain in domains {
                            keys.insert((m.app_class.clone(), domain, "domain".into()));
                        }
                    } else {
                        let title = config
                            .capture_titles()
                            .then_some(m.label.as_str())
                            .filter(|s| !s.is_empty());
                        let label = title
                            .map(str::to_owned)
                            .unwrap_or_else(|| identity::display_name(&m.app_class));
                        keys.insert((
                            m.app_class.clone(),
                            label,
                            if title.is_some() { "title" } else { "app" }.into(),
                        ));
                    }
                }
                if !keys.is_empty() {
                    spans.push((cursor, at));
                }
                if include_timeline {
                    let audio: Vec<_> = keys
                        .iter()
                        .map(|(app_class, label, attribution)| TimelineSource {
                            app_class: app_class.clone(),
                            label: label.clone(),
                            attribution: attribution.clone(),
                        })
                        .collect();
                    if let Some(last) = timeline.last_mut().filter(|last| {
                        last.end == cursor && last.app_class == *foreground && last.audio == audio
                    }) {
                        last.end = at;
                    } else {
                        timeline.push(TimelineSegment {
                            start: cursor,
                            end: at,
                            app_class: foreground.clone(),
                            label: identity::display_name(foreground),
                            audio,
                        });
                    }
                }
                for key in keys {
                    // Sweep segments are disjoint and keys are deduplicated per segment.
                    *grouped.entry(key).or_default() += at - cursor;
                }
            }
            let active = if is_media {
                &mut active_media
            } else {
                &mut active_focus
            };
            if add {
                active.insert(index);
            } else {
                active.remove(&index);
            }
            cursor = at;
        }
        let coverage = Coverage::new(spans);
        let mut daily = BTreeMap::new();
        let mut heatmap = BTreeMap::new();
        for &(a, b) in &coverage.windows {
            let mut cursor = a;
            while cursor < b {
                let local = Local
                    .timestamp_opt(cursor, 0)
                    .single()
                    .ok_or_else(|| anyhow::anyhow!("invalid media timestamp"))?;
                let next = (cursor + 3600 - i64::from(local.minute() * 60 + local.second())).min(b);
                *heatmap
                    .entry((local.weekday().num_days_from_monday(), local.hour()))
                    .or_insert(0) += next - cursor;
                cursor = next;
            }
            let mut cursor = a;
            while cursor < b {
                let date = Local
                    .timestamp_opt(cursor, 0)
                    .single()
                    .ok_or_else(|| anyhow::anyhow!("invalid media timestamp"))?
                    .date_naive();
                let next = crate::activity::midnight(
                    date.succ_opt()
                        .ok_or_else(|| anyhow::anyhow!("invalid media date"))?,
                )?
                .min(b);
                *daily.entry(date.to_string()).or_insert(0) += next - cursor;
                cursor = next;
            }
        }
        let mut sources: Vec<_> = grouped
            .into_iter()
            .map(
                |((app_class, label, attribution), seconds)| MultitaskingSource {
                    app_class,
                    label,
                    attribution,
                    seconds,
                },
            )
            .collect();
        sources.sort_by(|a, b| b.seconds.cmp(&a.seconds).then(a.label.cmp(&b.label)));
        Ok(MultitaskingReport {
            total_seconds: coverage.covered(start, end),
            timeline: include_timeline.then_some(ActivityTimeline {
                start,
                end,
                segments: timeline,
            }),
            heatmap: heatmap
                .into_iter()
                .map(
                    |((weekday, hour), focused_seconds)| crate::storage::FocusHeatCell {
                        weekday,
                        hour,
                        focused_seconds,
                    },
                )
                .collect(),
            daily: daily
                .into_iter()
                .map(|(date, seconds)| MultitaskingDay { date, seconds })
                .collect(),
            sources,
        })
    }

    /// The bar only needs a duration: omit domain attribution and calendar rollups.
    pub fn multitasked_seconds_between(&self, start: i64, end: i64) -> Result<i64> {
        let mut stmt = self.conn.prepare(
            "SELECT app_class,MAX(started_at,?1),MIN(COALESCE(ended_at,?2),last_confirmed_at+ttl,?2)
             FROM media_intervals WHERE source='system' AND started_at<?2
             AND COALESCE(ended_at,last_confirmed_at+ttl)>?1")?;
        let audio = stmt
            .query_map(params![start, end], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !audio.iter().any(|(_, a, b)| b > a) {
            return Ok(0);
        }
        let audio_start = audio
            .iter()
            .filter(|(_, a, b)| b > a)
            .map(|(_, a, _)| *a)
            .min()
            .unwrap();
        let audio_end = audio
            .iter()
            .filter(|(_, a, b)| b > a)
            .map(|(_, _, b)| *b)
            .max()
            .unwrap();
        // Only fetch focus that can overlap audio, preserving the full query's
        // ordering and bounds for equal-start/overlapping legacy intervals.
        let mut stmt = self.conn.prepare(
            "SELECT app_class,MAX(started_at,?1) AS a,MIN(COALESCE(ended_at,?2),?2) AS b
             FROM report_intervals WHERE kind='focused' AND started_at<?4
             AND COALESCE(ended_at,?2)>?3 ORDER BY a,b,id",
        )?;
        let focus = stmt
            .query_map(params![start, end, audio_start, audio_end], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        // Intern identities once, so each event only adjusts integer counters.
        let mut apps = BTreeMap::new();
        let mut intern = |name: String| {
            let next = apps.len();
            *apps.entry(name.to_ascii_lowercase()).or_insert(next)
        };
        let focus_apps: Vec<_> = focus
            .iter()
            .map(|f| intern(identity::canonical_app_class(&f.0)))
            .collect();
        let audio_apps: Vec<_> = audio
            .iter()
            .map(|(app, _, _)| intern(app.clone()))
            .collect();
        let mut events = Vec::with_capacity((focus.len() + audio.len()) * 2);
        for (i, f) in focus.iter().enumerate() {
            if f.2 > f.1 {
                events.push((f.1, true, false, i));
                events.push((f.2, false, false, i));
            }
        }
        for (i, (_, a, b)) in audio.iter().enumerate() {
            if b > a {
                events.push((*a, true, true, audio_apps[i]));
                events.push((*b, false, true, audio_apps[i]));
            }
        }
        events.sort_unstable();
        let mut counts = vec![0usize; apps.len()];
        let mut audible = 0usize;
        let mut active_focus = BTreeSet::new();
        let mut cursor = start;
        let mut seconds = 0;
        for (at, add, media, index) in events {
            if at > cursor
                && let Some(&f) = active_focus.last()
                && audible > counts[focus_apps[f]]
            {
                seconds += at - cursor;
            }
            if media {
                if add {
                    counts[index] += 1;
                    audible += 1;
                } else {
                    counts[index] -= 1;
                    audible -= 1;
                }
            } else if add {
                active_focus.insert(index);
            } else {
                active_focus.remove(&index);
            }
            cursor = at;
        }
        Ok(seconds)
    }

    pub fn multitasking_between(
        &self,
        start: i64,
        end: i64,
        config: &Config,
    ) -> Result<MultitaskingReport> {
        // Most historical periods predate media tracking: skip loading focus there.
        let has_media: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM media_intervals WHERE source='system' AND started_at<?2 AND COALESCE(ended_at,last_confirmed_at+ttl)>?1)",
            params![start,end], |r| r.get(0))?;
        if !has_media {
            return Ok(MultitaskingReport::default());
        }
        self.multitasking_from_metadata(
            start,
            end,
            &self.focused_interval_metadata_between(start, end)?,
            config,
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::IntervalKind;
    fn setup() -> (tempfile::TempDir, Storage, Config) {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        (dir, storage, config)
    }
    fn focus(storage: &mut Storage, app: &str, a: i64, b: i64) {
        let id = storage
            .start_interval(IntervalKind::Focused, app, None, None, a)
            .unwrap();
        storage.close_interval(id, b).unwrap();
    }
    #[test]
    fn day_timeline_clips_merges_and_shares_privacy_safe_attribution() {
        let (_dir, mut storage, mut config) = setup();
        config.privacy.browser_domains = true;
        focus(&mut storage, "game", 90, 150);
        focus(&mut storage, "game", 150, 200);
        focus(&mut storage, "zen", 210, 280);
        storage
            .record_media_snapshot(
                "system",
                &[("zen".into(), "private title".into())],
                120,
                200,
            )
            .unwrap();
        storage
            .record_media_snapshot(
                "browser:test",
                &[("zen".into(), "youtube.com".into())],
                130,
                200,
            )
            .unwrap();
        storage.close_media_system(250).unwrap();
        let metadata = storage.focused_interval_metadata_between(100, 260).unwrap();
        let report = storage
            .multitasking_from_metadata(100, 260, &metadata, &config, true)
            .unwrap();
        let timeline = report.timeline.unwrap();
        assert_eq!((timeline.start, timeline.end), (100, 260));
        assert_eq!(
            timeline
                .segments
                .iter()
                .map(|s| (s.start, s.end))
                .collect::<Vec<_>>(),
            vec![(100, 120), (120, 130), (130, 200), (210, 260)]
        );
        assert!(timeline.segments[0].audio.is_empty());
        assert_eq!(timeline.segments[2].audio[0].label, "youtube.com");
        assert!(
            timeline.segments[3].audio.is_empty(),
            "foreground browser audio is not multitasking"
        );
        let overlap: i64 = timeline
            .segments
            .iter()
            .filter(|s| !s.audio.is_empty())
            .map(|s| s.end - s.start)
            .sum();
        assert_eq!(overlap, report.total_seconds);
        config.privacy.browser_domains = false;
        config.privacy.title_capture = crate::config::TitleCapture::Off;
        let private = storage
            .multitasking_from_metadata(100, 260, &metadata, &config, true)
            .unwrap();
        let json = serde_json::to_string(&private.timeline).unwrap();
        assert!(!json.contains("youtube.com") && !json.contains("private title"));
        assert!(
            storage
                .multitasking_from_metadata(100, 260, &metadata, &config, false)
                .unwrap()
                .timeline
                .is_none()
        );
    }

    #[test]
    fn day_timeline_retains_focus_without_audio() {
        let (_dir, mut storage, config) = setup();
        focus(&mut storage, "game", 100, 200);
        let metadata = storage.focused_interval_metadata_between(120, 180).unwrap();
        let report = storage
            .multitasking_from_metadata(120, 180, &metadata, &config, true)
            .unwrap();
        assert_eq!(report.total_seconds, 0);
        let segments = report.timeline.unwrap().segments;
        assert_eq!(segments.len(), 1);
        assert_eq!((segments[0].start, segments[0].end), (120, 180));
        assert!(segments[0].audio.is_empty());
    }

    #[test]
    fn compact_total_matches_attributed_sweep_with_overlaps_and_expiry() {
        let (_dir, mut storage, config) = setup();
        assert_eq!(storage.multitasked_seconds_between(0, 10000).unwrap(), 0);
        let mut seed = 7123u64;
        let mut random = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as i64
        };
        let apps = ["zen", "ZEN", "firefox", "game", "spotify"];
        for i in 0..800 {
            let start = random() % 9000;
            let end = start + random() % 300;
            focus(&mut storage, apps[i % apps.len()], start, end);
        }
        for i in 0..1200 {
            let start = random() % 9000;
            let confirmed = start + random() % 90;
            let end = start + random() % 300;
            let source = if i % 3 == 0 { "browser:zen" } else { "system" };
            storage.conn.execute(
                "INSERT INTO media_intervals(source,app_class,label,started_at,ended_at,last_confirmed_at,ttl) VALUES(?1,?2,?3,?4,?5,?6,90)",
                params![source,apps[i%apps.len()],format!("site-{}",i%7),start,if i>1180 { None } else { Some(end) },confirmed]
            ).unwrap();
        }
        for (start, end) in [
            (0, 10000),
            (100, 100),
            (200, 100),
            (90, 150),
            (2000, 5000),
            (8900, 10000),
        ] {
            let full = storage.multitasking_between(start, end, &config).unwrap();
            assert_eq!(
                storage.multitasked_seconds_between(start, end).unwrap(),
                full.total_seconds,
                "{start}..{end}"
            );
        }
    }

    #[test]
    fn overlapping_sources_are_counted_once_and_focus_is_unchanged() {
        let (_dir, mut storage, config) = setup();
        focus(&mut storage, "deadlock", 100, 3700);
        let sources = vec![("zen".into(), "".into()), ("spotify".into(), "".into())];
        storage
            .record_media_snapshot("system", &sources, 100, 3600)
            .unwrap();
        storage
            .record_media_snapshot("system", &sources, 3700, 3600)
            .unwrap();
        storage
            .record_media_snapshot(
                "browser:zen",
                &[("zen".into(), "youtube.com".into())],
                100,
                3600,
            )
            .unwrap();
        let report = storage.multitasking_between(100, 3700, &config).unwrap();
        assert_eq!(report.total_seconds, 3600);
        assert_eq!(report.daily.iter().map(|d| d.seconds).sum::<i64>(), 3600);
        assert_eq!(report.sources.len(), 2);
        assert!(
            report.sources.iter().any(|s| s.label == "youtube.com"
                && s.attribution == "domain"
                && s.seconds == 3600)
        );
        assert_eq!(
            storage.totals_between(100, 3700).unwrap()[0].focused_seconds,
            3600
        );
        assert_eq!(
            storage
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM media_intervals WHERE source='system'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
    }
    #[test]
    fn same_app_pauses_gaps_and_expiry_do_not_count() {
        let (_dir, mut storage, config) = setup();
        focus(&mut storage, "zen", 100, 120);
        focus(&mut storage, "game", 120, 140);
        focus(&mut storage, "game", 180, 240);
        storage
            .record_media_snapshot("system", &[("zen".into(), "".into())], 100, 90)
            .unwrap();
        let report = storage.multitasking_between(100, 240, &config).unwrap();
        assert_eq!(report.total_seconds, 30); // 120..140 + 180..190
        storage.close_media_system(185).unwrap();
        assert_eq!(
            storage
                .multitasking_between(100, 240, &config)
                .unwrap()
                .total_seconds,
            25
        );
    }
    #[test]
    fn stale_updates_do_not_reopen_audio_and_long_gaps_start_new_intervals() {
        let (_dir, mut storage, config) = setup();
        focus(&mut storage, "game", 100, 400);
        let sources = vec![("zen".into(), "".into())];
        storage
            .record_media_snapshot("system", &sources, 100, 45)
            .unwrap();
        storage.close_media_system(120).unwrap();
        storage
            .record_media_snapshot("system", &sources, 110, 45)
            .unwrap();
        storage
            .record_media_snapshot("system", &sources, 200, 45)
            .unwrap();
        storage
            .record_media_snapshot("system", &sources, 300, 45)
            .unwrap();
        assert_eq!(
            storage
                .multitasking_between(100, 400, &config)
                .unwrap()
                .total_seconds,
            110
        );
    }
    #[test]
    fn disabled_domain_capture_hides_stored_attribution_and_purge_removes_media() {
        let (_dir, mut storage, mut config) = setup();
        focus(&mut storage, "game", 100, 200);
        storage
            .record_media_snapshot("system", &[("zen".into(), "Private video".into())], 100, 90)
            .unwrap();
        storage
            .record_media_snapshot(
                "browser:zen",
                &[("zen".into(), "youtube.com".into())],
                100,
                90,
            )
            .unwrap();
        config.privacy.browser_domains = false;
        let report = storage.multitasking_between(100, 200, &config).unwrap();
        assert_eq!(report.sources[0].label, "Zen Browser");
        assert_eq!(report.sources[0].attribution, "app");
        assert_eq!(
            storage
                .purge_before(None, true, false)
                .unwrap()
                .media_intervals_deleted,
            2
        );
        assert_eq!(
            storage
                .purge_before(None, false, false)
                .unwrap()
                .media_intervals_deleted,
            2
        );
        assert!(
            storage
                .media_intervals_between(100, 200)
                .unwrap()
                .is_empty()
        );
    }
}
