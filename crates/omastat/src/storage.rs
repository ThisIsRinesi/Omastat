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
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS intervals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
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
}

impl IntervalKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Focused => "focused",
            Self::Open => "open",
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
}
