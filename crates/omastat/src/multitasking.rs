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
    ) -> Result<MultitaskingReport> {
        let media = self.media_intervals_between(start, end)?;
        if !media
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
        let mut grouped = BTreeMap::<(String, String, String), Vec<(i64, i64)>>::new();
        let mut cursor = start;
        for (at, add, is_media, index) in events {
            if at > cursor
                && let Some(&f) = active_focus.last()
            {
                let foreground: &FocusedIntervalMetadata = &focus[f];
                let foreground = identity::canonical_app_class(&foreground.app_class);
                let mut keys = BTreeSet::new();
                for &i in &active_media {
                    let m: &MediaInterval = &media[i];
                    if m.source != "system" || m.app_class.eq_ignore_ascii_case(&foreground) {
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
                for key in keys {
                    grouped.entry(key).or_default().push((cursor, at));
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
                |((app_class, label, attribution), spans)| MultitaskingSource {
                    app_class,
                    label,
                    attribution,
                    seconds: Coverage::new(spans).covered(start, end),
                },
            )
            .collect();
        sources.sort_by(|a, b| b.seconds.cmp(&a.seconds).then(a.label.cmp(&b.label)));
        Ok(MultitaskingReport {
            total_seconds: coverage.covered(start, end),
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
