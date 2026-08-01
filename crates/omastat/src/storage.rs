use crate::config::Config;
use anyhow::{Context, Result};
use chrono::{Datelike, Local, TimeZone};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalKind {
    Focused,
    Open,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppTotals {
    pub app_class: String,
    pub focused_seconds: i64,
    pub open_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct DayTotals {
    pub label: String,
    pub focused_seconds: i64,
    pub open_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct TimelineInterval {
    pub kind: IntervalKind,
    pub app_class: String,
    pub started_at: i64,
    pub ended_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct StorageStatus {
    pub interval_count: i64,
    pub last_event_at: Option<i64>,
    pub focused_active: i64,
    pub open_active: i64,
}

#[derive(Debug, Clone)]
pub struct ActiveInterval {
    pub id: i64,
    pub kind: IntervalKind,
    pub app_class: String,
    pub window_address: Option<String>,
}

pub struct Storage {
    conn: Connection,
    path: PathBuf,
}

impl Storage {
    pub fn open(explicit_path: Option<&Path>, _config: &Config) -> Result<Self> {
        let path = explicit_path
            .map(PathBuf::from)
            .unwrap_or_else(default_db_path);
        if explicit_path.is_none() {
            copy_legacy_db_if_needed(&path)?;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let conn = Connection::open(&path)
            .with_context(|| format!("failed to open database {}", path.display()))?;
        let storage = Self { conn, path };
        storage.migrate()?;
        Ok(storage)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn healthcheck(&self) -> Result<()> {
        self.conn.execute_batch("SELECT 1;")?;
        Ok(())
    }

    pub fn start_interval(
        &self,
        kind: IntervalKind,
        app_class: &str,
        window_address: Option<&str>,
        title: Option<&str>,
        started_at: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO intervals (kind, app_class, window_address, title, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![kind.as_str(), app_class, window_address, title, started_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn close_interval(&self, id: i64, ended_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE intervals
             SET ended_at = ?1
             WHERE id = ?2 AND ended_at IS NULL",
            params![ended_at, id],
        )?;
        Ok(())
    }

    pub fn close_open_intervals(&self, ended_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE intervals SET ended_at = ?1 WHERE ended_at IS NULL",
            params![ended_at],
        )?;
        Ok(())
    }

    pub fn unclosed_intervals(&self) -> Result<Vec<ActiveInterval>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, kind, app_class, window_address
            FROM intervals
            WHERE ended_at IS NULL
            ORDER BY started_at ASC, id ASC
            ",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|(id, kind, app_class, window_address)| {
                let kind = IntervalKind::from_str(&kind)
                    .with_context(|| format!("unknown interval kind {kind:?} for row {id}"))?;
                Ok(ActiveInterval {
                    id,
                    kind,
                    app_class,
                    window_address,
                })
            })
            .collect()
    }

    pub fn totals_for_today(&self) -> Result<Vec<AppTotals>> {
        let now = Local::now();
        let start = Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .context("failed to compute local day start")?
            .timestamp();
        self.totals_between(start, now.timestamp())
    }

    pub fn totals_for_week(&self) -> Result<Vec<AppTotals>> {
        let now = Local::now();
        let days_from_monday = now.weekday().num_days_from_monday() as i64;
        let today_start = Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .context("failed to compute local day start")?;
        self.totals_between(
            (today_start - chrono::Duration::days(days_from_monday)).timestamp(),
            now.timestamp(),
        )
    }

    pub fn totals_for_month(&self) -> Result<Vec<AppTotals>> {
        let now = Local::now();
        let start = Local
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()
            .context("failed to compute local month start")?
            .timestamp();
        self.totals_between(start, now.timestamp())
    }

    pub fn totals_for_year(&self) -> Result<Vec<AppTotals>> {
        let now = Local::now();
        let start = Local
            .with_ymd_and_hms(now.year(), 1, 1, 0, 0, 0)
            .single()
            .context("failed to compute local year start")?
            .timestamp();
        self.totals_between(start, now.timestamp())
    }

    pub fn totals_all_time(&self) -> Result<Vec<AppTotals>> {
        let now = Local::now().timestamp();
        let start: i64 = self
            .conn
            .query_row("SELECT MIN(started_at) FROM intervals", [], |row| {
                row.get(0)
            })
            .optional()?
            .flatten()
            .unwrap_or(now);
        self.totals_between(start, now)
    }

    pub fn daily_totals(&self, days: u32) -> Result<Vec<DayTotals>> {
        let now = Local::now();
        let today_start = Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .context("failed to compute local day start")?;
        let days = days.max(1) as usize;
        let range_start = today_start - chrono::Duration::days(days.saturating_sub(1) as i64);
        let query_end = now.timestamp();
        let mut output = (0..days)
            .map(|offset| {
                let start = range_start + chrono::Duration::days(offset as i64);
                DayTotals {
                    label: start.format("%a").to_string(),
                    focused_seconds: 0,
                    open_seconds: 0,
                }
            })
            .collect::<Vec<_>>();
        let boundaries = (0..=days)
            .map(|offset| (range_start + chrono::Duration::days(offset as i64)).timestamp())
            .collect::<Vec<_>>();

        let mut stmt = self.conn.prepare(
            "
            SELECT
                kind,
                MAX(started_at, ?1) AS bounded_start,
                MIN(COALESCE(ended_at, ?2), ?2) AS bounded_end
            FROM intervals
            WHERE started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ",
        )?;
        let intervals = stmt
            .query_map(params![range_start.timestamp(), query_end], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (kind, started_at, ended_at) in intervals {
            for (index, window) in boundaries.windows(2).enumerate() {
                let overlap = ended_at.min(window[1]).min(query_end) - started_at.max(window[0]);
                if overlap <= 0 {
                    continue;
                }
                match kind.as_str() {
                    "focused" => output[index].focused_seconds += overlap,
                    "open" => output[index].open_seconds += overlap,
                    _ => {}
                }
            }
        }

        Ok(output)
    }

    pub fn timeline_for_today(&self) -> Result<Vec<TimelineInterval>> {
        let now = Local::now();
        let start = Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .context("failed to compute local day start")?
            .timestamp();
        self.timeline_between(start, now.timestamp())
    }

    pub fn usage_status(&self) -> Result<StorageStatus> {
        self.conn
            .query_row(
                "
                SELECT
                    COUNT(*),
                    MAX(COALESCE(ended_at, started_at)),
                    SUM(CASE WHEN ended_at IS NULL AND kind = 'focused' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN ended_at IS NULL AND kind = 'open' THEN 1 ELSE 0 END)
                FROM intervals
                ",
                [],
                |row| {
                    Ok(StorageStatus {
                        interval_count: row.get(0)?,
                        last_event_at: row.get(1)?,
                        focused_active: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                        open_active: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    })
                },
            )
            .context("failed to read storage status")
    }

    pub fn total_duration(&self) -> Result<i64> {
        let now = Local::now().timestamp();
        let start: i64 = self
            .conn
            .query_row("SELECT MIN(started_at) FROM intervals", [], |row| {
                row.get(0)
            })
            .optional()?
            .flatten()
            .unwrap_or(now);
        Ok(now.saturating_sub(start))
    }

    pub fn totals_for_date_range(&self, from: &str, to: &str) -> Result<Vec<AppTotals>> {
        let start_date = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d")
            .with_context(|| format!("invalid --from date {from:?}, expected YYYY-MM-DD"))?;
        let end_date = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d")
            .with_context(|| format!("invalid --to date {to:?}, expected YYYY-MM-DD"))?;

        if end_date < start_date {
            anyhow::bail!("--to must be on or after --from");
        }

        let start = Local
            .from_local_datetime(
                &start_date
                    .and_hms_opt(0, 0, 0)
                    .context("invalid start date")?,
            )
            .single()
            .context("failed to compute local range start")?
            .timestamp();
        let end = Local
            .from_local_datetime(
                &end_date
                    .succ_opt()
                    .context("date range end overflow")?
                    .and_hms_opt(0, 0, 0)
                    .context("invalid end date")?,
            )
            .single()
            .context("failed to compute local range end")?
            .timestamp();

        self.totals_between(start, end)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 250;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS intervals (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('focused', 'open')),
                app_class TEXT NOT NULL,
                window_address TEXT,
                title TEXT,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                CHECK (ended_at IS NULL OR ended_at >= started_at)
            );

            CREATE INDEX IF NOT EXISTS idx_intervals_kind_app
                ON intervals(kind, app_class);
            CREATE INDEX IF NOT EXISTS idx_intervals_time
                ON intervals(started_at, ended_at);
            CREATE INDEX IF NOT EXISTS idx_intervals_open
                ON intervals(ended_at)
                WHERE ended_at IS NULL;
            INSERT OR IGNORE INTO schema_migrations(version, applied_at)
                VALUES (1, unixepoch());
            ",
        )?;
        Ok(())
    }

    fn totals_between(&self, start: i64, end: i64) -> Result<Vec<AppTotals>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                app_class,
                SUM(CASE WHEN kind = 'focused' THEN overlap_seconds ELSE 0 END) AS focused_seconds,
                SUM(CASE WHEN kind = 'open' THEN overlap_seconds ELSE 0 END) AS open_seconds
            FROM (
                SELECT
                    app_class,
                    kind,
                    MAX(0, MIN(COALESCE(ended_at, ?2), ?2) - MAX(started_at, ?1)) AS overlap_seconds
                FROM intervals
                WHERE started_at < ?2
                  AND COALESCE(ended_at, ?2) > ?1
            )
            GROUP BY app_class
            HAVING focused_seconds > 0 OR open_seconds > 0
            ORDER BY focused_seconds DESC, open_seconds DESC, app_class ASC
            ",
        )?;

        let rows = stmt
            .query_map(params![start, end], |row| {
                Ok(AppTotals {
                    app_class: row.get(0)?,
                    focused_seconds: row.get(1)?,
                    open_seconds: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn timeline_between(&self, start: i64, end: i64) -> Result<Vec<TimelineInterval>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                kind,
                app_class,
                MAX(started_at, ?1) AS bounded_start,
                MIN(COALESCE(ended_at, ?2), ?2) AS bounded_end
            FROM intervals
            WHERE started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ORDER BY bounded_start ASC, bounded_end ASC, id ASC
            ",
        )?;

        let rows = stmt
            .query_map(params![start, end], |row| {
                let kind = row.get::<_, String>(0)?;
                let kind = IntervalKind::from_str(&kind).ok_or_else(|| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "kind".to_string(),
                        rusqlite::types::Type::Text,
                    )
                })?;
                Ok(TimelineInterval {
                    kind,
                    app_class: row.get(1)?,
                    started_at: row.get(2)?,
                    ended_at: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

impl IntervalKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Focused => "focused",
            Self::Open => "open",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "focused" => Some(Self::Focused),
            "open" => Some(Self::Open),
            _ => None,
        }
    }
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omastat")
        .join("omastat.db")
}

fn legacy_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hours-played")
        .join("hours-played.db")
}

fn copy_legacy_db_if_needed(path: &Path) -> Result<()> {
    let legacy = legacy_db_path();
    if path.exists() || !legacy.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(&legacy, path).with_context(|| {
        format!(
            "failed to copy legacy database {} to {}",
            legacy.display(),
            path.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{IntervalKind, Storage};
    use crate::config::Config;
    use chrono::{Datelike, Local, TimeZone};

    #[test]
    fn aggregates_overlapping_intervals() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let focused = storage
            .start_interval(IntervalKind::Focused, "firefox", None, None, 100)
            .unwrap();
        storage.close_interval(focused, 200).unwrap();

        let open = storage
            .start_interval(IntervalKind::Open, "firefox", None, None, 50)
            .unwrap();
        storage.close_interval(open, 250).unwrap();

        let rows = storage.totals_between(150, 225).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].focused_seconds, 50);
        assert_eq!(rows[0].open_seconds, 75);
    }

    #[test]
    fn includes_unclosed_intervals_until_report_end() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        storage
            .start_interval(IntervalKind::Focused, "code", None, None, 100)
            .unwrap();

        let rows = storage.totals_between(100, 160).unwrap();
        assert_eq!(rows[0].focused_seconds, 60);
    }

    #[test]
    fn daily_totals_bucket_interval_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let now = Local::now();
        let today_start = Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .unwrap();
        let yesterday_start = today_start - chrono::Duration::days(1);
        let started_at = (yesterday_start + chrono::Duration::hours(1)).timestamp();
        let ended_at = (yesterday_start + chrono::Duration::hours(2)).timestamp();

        let focused = storage
            .start_interval(IntervalKind::Focused, "ghostty", None, None, started_at)
            .unwrap();
        storage.close_interval(focused, ended_at).unwrap();

        let days = storage.daily_totals(2).unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].focused_seconds, 3600);
        assert_eq!(days[1].focused_seconds, 0);
    }

    #[test]
    fn daily_totals_bounds_unclosed_today_at_now() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let started_at = Local::now().timestamp() - 60;

        storage
            .start_interval(IntervalKind::Focused, "ghostty", None, None, started_at)
            .unwrap();

        let days = storage.daily_totals(1).unwrap();
        assert!(days[0].focused_seconds >= 60);
        assert!(days[0].focused_seconds < 120);
    }
}
