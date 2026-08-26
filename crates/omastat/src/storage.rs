use crate::{clock, config::Config, identity, steam::SteamResolver};
use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate, TimeZone, Timelike};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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

#[derive(Debug, Clone, Serialize)]
pub struct DayTotals {
    pub date: String,
    pub label: String,
    pub focused_seconds: i64,
    pub open_seconds: i64,
    pub elapsed_seconds: i64,
    pub observed_seconds: i64,
    pub idle_seconds: i64,
    pub locked_seconds: i64,
    pub sleep_seconds: i64,
    pub unobserved_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppDayTotals {
    pub date: String,
    pub label: String,
    pub app_class: String,
    pub focused_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FocusHeatCell {
    pub weekday: u32,
    pub hour: u32,
    pub focused_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceTotals {
    pub workspace: String,
    pub focused_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppWorkspaceTotals {
    pub workspace: String,
    pub app_class: String,
    pub focused_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleTotals {
    pub app_class: String,
    pub title: String,
    pub focused_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct TimelineInterval {
    pub kind: IntervalKind,
    pub app_class: String,
    pub started_at: i64,
    pub ended_at: i64,
}

#[derive(Debug, Clone)]
pub struct SystemTimelineInterval {
    pub kind: SystemIntervalKind,
    pub source: Option<String>,
    pub started_at: i64,
    pub ended_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct StorageStatus {
    pub interval_count: i64,
    pub last_event_at: Option<i64>,
    pub focused_active: i64,
    pub open_active: i64,
    pub idle_active: i64,
    pub locked_active: i64,
    pub sleep_active: i64,
    pub daemon_active: i64,
    pub last_heartbeat_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageDiagnostic {
    pub path: PathBuf,
    pub exists: bool,
    pub schema_status: StorageSchemaStatus,
    pub quick_check: StorageQuickCheck,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageSchemaStatus {
    Missing,
    NotInitialized {
        reason: String,
    },
    Current {
        applied_migrations: Vec<i64>,
    },
    NeedsMigration {
        version: i64,
        description: String,
        applied_migrations: Vec<i64>,
    },
    UnknownMigration {
        version: i64,
        applied_migrations: Vec<i64>,
    },
    Invalid {
        error: String,
    },
    Unreadable {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageQuickCheck {
    Ok,
    Problem(String),
    Skipped(String),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ActiveInterval {
    pub id: i64,
    pub kind: IntervalKind,
    pub app_class: String,
    pub window_address: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IntervalMetadata<'a> {
    pub window_address: Option<&'a str>,
    pub title: Option<&'a str>,
    pub workspace: Option<&'a str>,
    pub monitor: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionIntervalKind {
    Idle,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemIntervalKind {
    Sleep,
    Unobserved,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionTotals {
    pub idle_seconds: i64,
    pub locked_seconds: i64,
    pub sleep_seconds: i64,
    pub unobserved_seconds: i64,
}

#[derive(Debug, Clone, Default)]
pub struct FocusedRollups {
    pub focus_intervals: Vec<TimelineInterval>,
    pub daily_apps: Vec<AppDayTotals>,
    pub heatmap: Vec<FocusHeatCell>,
    pub workspaces: Vec<WorkspaceTotals>,
    pub app_workspaces: Vec<AppWorkspaceTotals>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawInterval {
    pub kind: IntervalKind,
    pub app_class: String,
    pub window_address: Option<String>,
    pub title: Option<String>,
    pub workspace: Option<String>,
    pub monitor: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub local_start: String,
    pub local_end: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawSessionInterval {
    pub kind: SessionIntervalKind,
    pub source: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub local_start: String,
    pub local_end: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawSystemInterval {
    pub kind: SystemIntervalKind,
    pub source: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub local_start: String,
    pub local_end: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RawExportRows {
    pub intervals: Vec<RawInterval>,
    pub session_intervals: Vec<RawSessionInterval>,
    pub system_intervals: Vec<RawSystemInterval>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurgeReport {
    pub dry_run: bool,
    pub cutoff_ts: Option<i64>,
    pub cutoff_local: Option<String>,
    pub intervals_deleted: i64,
    pub session_intervals_deleted: i64,
    pub system_intervals_deleted: i64,
    pub daemon_events_deleted: i64,
    pub daemon_runs_deleted: i64,
    pub intervals_trimmed: i64,
    pub session_intervals_trimmed: i64,
    pub system_intervals_trimmed: i64,
    pub vacuumed: bool,
}

#[derive(Debug, Clone)]
pub struct ActiveSessionInterval {
    pub id: i64,
    pub kind: SessionIntervalKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRunStart {
    pub run_id: i64,
    pub recovery: Option<DaemonRecovery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRecovery {
    pub previous_run_id: Option<i64>,
    pub closed_at: i64,
    pub unobserved_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClassRepair {
    pub from: String,
    pub to: String,
    pub rows: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleFill {
    pub app_class: String,
    pub title: String,
    pub rows: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleNormalize {
    pub app_class: String,
    pub from: String,
    pub to: String,
    pub rows: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TitleRepair {
    pub dry_run: bool,
    pub class_updates: Vec<ClassRepair>,
    pub title_updates: Vec<TitleFill>,
    pub title_normalizations: Vec<TitleNormalize>,
    pub rewritten_rows: i64,
    pub filled_titles: i64,
    pub normalized_titles: i64,
}

pub struct Storage {
    conn: Connection,
    path: PathBuf,
}

struct Migration {
    version: i64,
    description: &'static str,
    up: fn(&Transaction<'_>) -> rusqlite::Result<()>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "create app interval tables",
        up: migrate_0001_create_intervals,
    },
    Migration {
        version: 2,
        description: "create session interval tables",
        up: migrate_0002_create_session_intervals,
    },
    Migration {
        version: 3,
        description: "add interval workspace metadata",
        up: migrate_0003_add_interval_workspace,
    },
    Migration {
        version: 4,
        description: "add interval monitor metadata",
        up: migrate_0004_add_interval_monitor,
    },
    Migration {
        version: 5,
        description: "add daemon lifecycle and unobserved intervals",
        up: migrate_0005_add_daemon_lifecycle,
    },
    Migration {
        version: 6,
        description: "record sleep intervals separately from unobserved gaps",
        up: migrate_0006_expand_system_intervals,
    },
    Migration {
        version: 7,
        description: "add report rollup query indexes",
        up: migrate_0007_add_report_indexes,
    },
    Migration {
        version: 8,
        description: "enforce one active interval per tracked state",
        up: migrate_0008_add_active_interval_invariants,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageOpenMode {
    ReadWriteMigrate,
    ReadOnly,
}

impl Storage {
    pub fn open(explicit_path: Option<&Path>, _config: &Config) -> Result<Self> {
        Self::open_with_mode(explicit_path, _config, StorageOpenMode::ReadWriteMigrate)
    }

    pub fn open_read_only(explicit_path: Option<&Path>, config: &Config) -> Result<Self> {
        Self::open_with_mode(explicit_path, config, StorageOpenMode::ReadOnly)
    }

    pub fn diagnose(explicit_path: Option<&Path>) -> StorageDiagnostic {
        let path = explicit_path
            .map(PathBuf::from)
            .unwrap_or_else(|| default_db_path_for_mode(StorageOpenMode::ReadOnly));

        if !path.exists() {
            return StorageDiagnostic {
                path,
                exists: false,
                schema_status: StorageSchemaStatus::Missing,
                quick_check: StorageQuickCheck::Skipped("database file does not exist".to_string()),
            };
        }

        let conn = match Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(conn) => conn,
            Err(error) => {
                return StorageDiagnostic {
                    path,
                    exists: true,
                    schema_status: StorageSchemaStatus::Unreadable {
                        error: format!("{error:#}"),
                    },
                    quick_check: StorageQuickCheck::Skipped(
                        "database could not be opened read-only".to_string(),
                    ),
                };
            }
        };

        StorageDiagnostic {
            path,
            exists: true,
            schema_status: diagnose_schema(&conn),
            quick_check: sqlite_quick_check(&conn),
        }
    }

    pub fn open_with_mode(
        explicit_path: Option<&Path>,
        _config: &Config,
        mode: StorageOpenMode,
    ) -> Result<Self> {
        let path = explicit_path
            .map(PathBuf::from)
            .unwrap_or_else(|| default_db_path_for_mode(mode));

        if mode == StorageOpenMode::ReadOnly {
            if !path.exists() {
                anyhow::bail!(
                    "database {} does not exist; start omastatd once with write access to initialize it",
                    path.display()
                );
            }
            let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .with_context(|| format!("failed to open database {} read-only", path.display()))?;
            let storage = Self { conn, path };
            storage.validate_schema()?;
            return Ok(storage);
        }

        if explicit_path.is_none() {
            copy_legacy_db_if_needed(&path)?;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let conn = Connection::open(&path)
            .with_context(|| format!("failed to open database {}", path.display()))?;
        let mut storage = Self { conn, path };
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
        self.start_interval_with_metadata(
            kind,
            app_class,
            IntervalMetadata {
                window_address,
                title,
                ..IntervalMetadata::default()
            },
            started_at,
        )
    }

    pub fn start_interval_with_metadata(
        &self,
        kind: IntervalKind,
        app_class: &str,
        metadata: IntervalMetadata<'_>,
        started_at: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO intervals (kind, app_class, window_address, title, workspace, monitor, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                kind.as_str(),
                app_class,
                metadata.window_address,
                metadata.title,
                metadata.workspace,
                metadata.monitor,
                started_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn close_interval(&self, id: i64, ended_at: i64) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE intervals
             SET ended_at = ?1
             WHERE id = ?2 AND ended_at IS NULL",
            params![ended_at, id],
        )?;
        ensure_row_was_closeable(&self.conn, "intervals", id, updated)?;
        Ok(())
    }

    pub fn close_open_intervals(&self, ended_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE intervals SET ended_at = ?1 WHERE ended_at IS NULL",
            params![ended_at],
        )?;
        Ok(())
    }

    pub fn start_session_interval(
        &self,
        kind: SessionIntervalKind,
        source: Option<&str>,
        started_at: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO session_intervals (kind, source, started_at)
             VALUES (?1, ?2, ?3)",
            params![kind.as_str(), source, started_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn close_session_interval(&self, id: i64, ended_at: i64) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE session_intervals
             SET ended_at = ?1
             WHERE id = ?2 AND ended_at IS NULL",
            params![ended_at, id],
        )?;
        ensure_row_was_closeable(&self.conn, "session_intervals", id, updated)?;
        Ok(())
    }

    pub fn close_session_intervals(&self, ended_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE session_intervals SET ended_at = ?1 WHERE ended_at IS NULL",
            params![ended_at],
        )?;
        Ok(())
    }

    pub fn close_observed_intervals(&self, ended_at: i64) -> Result<()> {
        self.conn.execute(
            "
            UPDATE intervals
            SET ended_at = MAX(started_at, ?1)
            WHERE ended_at IS NULL
            ",
            params![ended_at],
        )?;
        self.conn.execute(
            "
            UPDATE session_intervals
            SET ended_at = MAX(started_at, ?1)
            WHERE ended_at IS NULL
            ",
            params![ended_at],
        )?;
        Ok(())
    }

    pub fn start_system_interval(
        &self,
        kind: SystemIntervalKind,
        source: Option<&str>,
        started_at: i64,
    ) -> Result<i64> {
        self.conn.execute(
            "
            INSERT INTO unobserved_intervals (kind, source, started_at)
            VALUES (?1, ?2, ?3)
            ",
            params![kind.as_str(), source, started_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn close_system_interval(&self, id: i64, ended_at: i64) -> Result<()> {
        let updated = self.conn.execute(
            "
            UPDATE unobserved_intervals
            SET ended_at = MAX(started_at, ?1)
            WHERE id = ?2
              AND ended_at IS NULL
            ",
            params![ended_at, id],
        )?;
        ensure_row_was_closeable(&self.conn, "unobserved_intervals", id, updated)?;
        Ok(())
    }

    pub fn start_daemon_run(&mut self, now: i64) -> Result<DaemonRunStart> {
        let tx = self.conn.transaction()?;
        let previous_run = latest_daemon_run(&tx)?;
        let stale_boundary = latest_unclosed_interval_start(&tx)?;
        let active_sleep_started_at = earliest_active_system_interval_start(&tx, "sleep")?;
        let previous_unclosed = previous_run
            .as_ref()
            .is_some_and(|run| run.stopped_at.is_none());
        let needs_recovery =
            previous_unclosed || stale_boundary.is_some() || active_sleep_started_at.is_some();

        let recovery = if needs_recovery {
            let previous_run_id = previous_run
                .as_ref()
                .filter(|run| run.stopped_at.is_none())
                .map(|run| run.id);
            let (closed_at, unobserved_seconds) =
                if let Some(sleep_started_at) = active_sleep_started_at {
                    close_unclosed_observed_intervals_tx(&tx, sleep_started_at)?;
                    close_unclosed_system_intervals_tx(&tx, "sleep", now)?;
                    (now, 0)
                } else {
                    let heartbeat_boundary = previous_run
                        .as_ref()
                        .filter(|run| run.stopped_at.is_none())
                        .map(|run| run.last_heartbeat_at);
                    let closed_at = heartbeat_boundary
                        .into_iter()
                        .chain(stale_boundary)
                        .max()
                        .unwrap_or(now)
                        .min(now);
                    close_unclosed_observed_intervals_tx(&tx, closed_at)?;
                    let unobserved_seconds = if now > closed_at {
                        tx.execute(
                            "
                        INSERT INTO unobserved_intervals (kind, source, started_at, ended_at)
                        VALUES ('unobserved', 'daemon-recovery', ?1, ?2)
                        ",
                            params![closed_at, now],
                        )?;
                        now - closed_at
                    } else {
                        0
                    };
                    (closed_at, unobserved_seconds)
                };

            if let Some(run_id) = previous_run_id {
                tx.execute(
                    "
                    UPDATE daemon_runs
                    SET stopped_at = MAX(started_at, ?1),
                        stop_kind = 'recovered'
                    WHERE id = ?2
                      AND stopped_at IS NULL
                    ",
                    params![closed_at, run_id],
                )?;
            }

            Some(DaemonRecovery {
                previous_run_id,
                closed_at,
                unobserved_seconds,
            })
        } else {
            None
        };

        tx.execute(
            "
            INSERT INTO daemon_runs (started_at, last_heartbeat_at)
            VALUES (?1, ?1)
            ",
            params![now],
        )?;
        let run_id = tx.last_insert_rowid();

        if let Some(recovery) = &recovery {
            let detail = match recovery.previous_run_id {
                Some(previous_run_id) => format!(
                    "recovered run {previous_run_id}; closed stale intervals at {}; unobserved {}s",
                    recovery.closed_at, recovery.unobserved_seconds
                ),
                None => format!(
                    "recovered legacy stale intervals at {}; unobserved {}s",
                    recovery.closed_at, recovery.unobserved_seconds
                ),
            };
            insert_daemon_event_tx(&tx, run_id, "recovery", now, Some(&detail))?;
        }
        insert_daemon_event_tx(&tx, run_id, "start", now, None)?;

        tx.commit()?;
        Ok(DaemonRunStart { run_id, recovery })
    }

    pub fn record_daemon_heartbeat(&mut self, run_id: i64, now: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        let updated = tx.execute(
            "
            UPDATE daemon_runs
            SET last_heartbeat_at = MAX(last_heartbeat_at, ?1)
            WHERE id = ?2
              AND stopped_at IS NULL
            ",
            params![now, run_id],
        )?;
        if updated == 0 {
            anyhow::bail!("daemon run {run_id} is not active");
        }
        insert_daemon_event_tx(&tx, run_id, "heartbeat", now, None)?;
        tx.commit()?;
        Ok(())
    }

    pub fn finish_daemon_run(&mut self, run_id: i64, now: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        let updated = tx.execute(
            "
            UPDATE daemon_runs
            SET last_heartbeat_at = MAX(last_heartbeat_at, ?1),
                stopped_at = MAX(started_at, ?1),
                stop_kind = 'clean'
            WHERE id = ?2
              AND stopped_at IS NULL
            ",
            params![now, run_id],
        )?;
        if updated == 0 {
            anyhow::bail!("daemon run {run_id} is not active");
        }
        insert_daemon_event_tx(&tx, run_id, "clean-stop", now, None)?;
        tx.commit()?;
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

    pub fn unclosed_session_intervals(&self) -> Result<Vec<ActiveSessionInterval>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, kind
            FROM session_intervals
            WHERE ended_at IS NULL
            ORDER BY started_at ASC, id ASC
            ",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|(id, kind)| {
                let kind = SessionIntervalKind::from_str(&kind).with_context(|| {
                    format!("unknown session interval kind {kind:?} for row {id}")
                })?;
                Ok(ActiveSessionInterval { id, kind })
            })
            .collect()
    }

    pub fn totals_for_today(&self) -> Result<Vec<AppTotals>> {
        let now = clock::local_now();
        let start = Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .context("failed to compute local day start")?
            .timestamp();
        self.totals_between(start, now.timestamp())
    }

    pub fn totals_for_week(&self) -> Result<Vec<AppTotals>> {
        let now = clock::local_now();
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
        let now = clock::local_now();
        let start = Local
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()
            .context("failed to compute local month start")?
            .timestamp();
        self.totals_between(start, now.timestamp())
    }

    pub fn totals_for_year(&self) -> Result<Vec<AppTotals>> {
        let now = clock::local_now();
        let start = Local
            .with_ymd_and_hms(now.year(), 1, 1, 0, 0, 0)
            .single()
            .context("failed to compute local year start")?
            .timestamp();
        self.totals_between(start, now.timestamp())
    }

    pub fn totals_all_time(&self) -> Result<Vec<AppTotals>> {
        let now = clock::local_now().timestamp();
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
        let now = clock::local_now();
        let today_start = Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .context("failed to compute local day start")?;
        let days = days.max(1) as usize;
        let range_start = today_start - chrono::Duration::days(days.saturating_sub(1) as i64);
        self.daily_totals_from(range_start, days, now.timestamp())
    }

    pub fn daily_totals_for_local_dates(
        &self,
        start_date: chrono::NaiveDate,
        days: usize,
        query_end: i64,
    ) -> Result<Vec<DayTotals>> {
        let range_start = Local
            .from_local_datetime(
                &start_date
                    .and_hms_opt(0, 0, 0)
                    .context("invalid daily totals start date")?,
            )
            .single()
            .context("failed to compute local daily totals start")?;
        self.daily_totals_from(range_start, days.max(1), query_end)
    }

    fn daily_totals_from(
        &self,
        range_start: chrono::DateTime<Local>,
        days: usize,
        query_end: i64,
    ) -> Result<Vec<DayTotals>> {
        let range_start_date = range_start.date_naive();
        let mut output = (0..days)
            .map(|offset| {
                let start = range_start_date + chrono::Duration::days(offset as i64);
                DayTotals {
                    date: start.format("%Y-%m-%d").to_string(),
                    label: start.format("%b %-d").to_string(),
                    focused_seconds: 0,
                    open_seconds: 0,
                    elapsed_seconds: 0,
                    observed_seconds: 0,
                    idle_seconds: 0,
                    locked_seconds: 0,
                    sleep_seconds: 0,
                    unobserved_seconds: 0,
                }
            })
            .collect::<Vec<_>>();
        let boundaries = local_midnight_boundaries(range_start_date, days)?;

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

        let mut stmt = self.conn.prepare(
            "
            SELECT
                kind,
                MAX(started_at, ?1) AS bounded_start,
                MIN(COALESCE(ended_at, ?2), ?2) AS bounded_end
            FROM session_intervals
            WHERE started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ",
        )?;
        let session_intervals = stmt
            .query_map(params![range_start.timestamp(), query_end], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (kind, started_at, ended_at) in session_intervals {
            for (index, window) in boundaries.windows(2).enumerate() {
                let overlap = ended_at.min(window[1]).min(query_end) - started_at.max(window[0]);
                if overlap <= 0 {
                    continue;
                }
                match kind.as_str() {
                    "idle" => output[index].idle_seconds += overlap,
                    "locked" => output[index].locked_seconds += overlap,
                    _ => {}
                }
            }
        }

        let mut stmt = self.conn.prepare(
            "
            SELECT
                kind,
                MAX(started_at, ?1) AS bounded_start,
                MIN(COALESCE(ended_at, ?2), ?2) AS bounded_end
            FROM unobserved_intervals
            WHERE started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ",
        )?;
        let unobserved_intervals = stmt
            .query_map(params![range_start.timestamp(), query_end], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (kind, started_at, ended_at) in unobserved_intervals {
            for (index, window) in boundaries.windows(2).enumerate() {
                let overlap = ended_at.min(window[1]).min(query_end) - started_at.max(window[0]);
                if overlap > 0 {
                    match kind.as_str() {
                        "sleep" => output[index].sleep_seconds += overlap,
                        "unobserved" => output[index].unobserved_seconds += overlap,
                        _ => {}
                    }
                }
            }
        }

        for (index, window) in boundaries.windows(2).enumerate() {
            let elapsed = query_end.min(window[1]) - window[0];
            output[index].elapsed_seconds = elapsed.max(0);
            output[index].observed_seconds = output[index]
                .elapsed_seconds
                .saturating_sub(output[index].unobserved_seconds.max(0));
        }

        Ok(output)
    }

    pub fn timeline_for_today(&self) -> Result<Vec<TimelineInterval>> {
        let now = clock::local_now();
        let start = Local
            .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
            .single()
            .context("failed to compute local day start")?
            .timestamp();
        self.timeline_between(start, now.timestamp())
    }

    pub fn usage_status(&self) -> Result<StorageStatus> {
        let mut status = self
            .conn
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
                        idle_active: 0,
                        locked_active: 0,
                        sleep_active: 0,
                        daemon_active: 0,
                        last_heartbeat_at: None,
                    })
                },
            )
            .context("failed to read storage status")?;

        self.conn
            .query_row(
                "
                SELECT
                    SUM(CASE WHEN ended_at IS NULL AND kind = 'idle' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN ended_at IS NULL AND kind = 'locked' THEN 1 ELSE 0 END)
                FROM session_intervals
                ",
                [],
                |row| {
                    status.idle_active = row.get::<_, Option<i64>>(0)?.unwrap_or(0);
                    status.locked_active = row.get::<_, Option<i64>>(1)?.unwrap_or(0);
                    Ok(())
                },
            )
            .context("failed to read session storage status")?;

        let session_last_event_at = self
            .conn
            .query_row(
                "SELECT MAX(COALESCE(ended_at, started_at)) FROM session_intervals",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .context("failed to read session event status")?;
        status.last_event_at = latest_timestamp(status.last_event_at, session_last_event_at);

        self.conn
            .query_row(
                "
                SELECT
                    SUM(CASE WHEN ended_at IS NULL AND kind = 'sleep' THEN 1 ELSE 0 END),
                    MAX(COALESCE(ended_at, started_at))
                FROM unobserved_intervals
                ",
                [],
                |row| {
                    status.sleep_active = row.get::<_, Option<i64>>(0)?.unwrap_or(0);
                    let last_system_event_at = row.get::<_, Option<i64>>(1)?;
                    status.last_event_at =
                        latest_timestamp(status.last_event_at, last_system_event_at);
                    Ok(())
                },
            )
            .context("failed to read system interval status")?;

        let daemon_last_event_at = self
            .conn
            .query_row("SELECT MAX(occurred_at) FROM daemon_events", [], |row| {
                row.get::<_, Option<i64>>(0)
            })
            .context("failed to read daemon event status")?;
        status.last_event_at = latest_timestamp(status.last_event_at, daemon_last_event_at);

        self.conn
            .query_row(
                "
                SELECT COUNT(*), MAX(last_heartbeat_at)
                FROM daemon_runs
                WHERE stopped_at IS NULL
                ",
                [],
                |row| {
                    status.daemon_active = row.get(0)?;
                    status.last_heartbeat_at = row.get(1)?;
                    Ok(())
                },
            )
            .context("failed to read daemon storage status")?;

        Ok(status)
    }

    pub fn total_duration(&self) -> Result<i64> {
        let now = clock::local_now().timestamp();
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

    pub fn focused_title_totals_between(
        &self,
        start: i64,
        end: i64,
        limit: usize,
    ) -> Result<Vec<TitleTotals>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                app_class,
                title,
                SUM(overlap_seconds) AS focused_seconds
            FROM (
                SELECT
                    app_class,
                    title,
                    MAX(0, MIN(COALESCE(ended_at, ?2), ?2) - MAX(started_at, ?1)) AS overlap_seconds
                FROM intervals
                WHERE kind = 'focused'
                  AND title IS NOT NULL
                  AND trim(title) <> ''
                  AND started_at < ?2
                  AND COALESCE(ended_at, ?2) > ?1
            )
            WHERE overlap_seconds > 0
            GROUP BY app_class, title
            ORDER BY focused_seconds DESC, app_class ASC, title ASC
            LIMIT ?3
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end, limit.max(1) as i64], |row| {
                Ok(TitleTotals {
                    app_class: row.get(0)?,
                    title: row.get(1)?,
                    focused_seconds: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn focused_title_totals_by_app_between(
        &self,
        start: i64,
        end: i64,
        limit_per_app: usize,
    ) -> Result<Vec<TitleTotals>> {
        let mut stmt = self.conn.prepare(
            "
            WITH title_totals AS (
                SELECT
                    app_class,
                    title,
                    SUM(overlap_seconds) AS focused_seconds
                FROM (
                    SELECT
                        app_class,
                        title,
                        MAX(0, MIN(COALESCE(ended_at, ?2), ?2) - MAX(started_at, ?1)) AS overlap_seconds
                    FROM intervals
                    WHERE kind = 'focused'
                      AND title IS NOT NULL
                      AND trim(title) <> ''
                      AND started_at < ?2
                      AND COALESCE(ended_at, ?2) > ?1
                )
                WHERE overlap_seconds > 0
                GROUP BY app_class, title
            ),
            ranked AS (
                SELECT
                    app_class,
                    title,
                    focused_seconds,
                    ROW_NUMBER() OVER (
                        PARTITION BY app_class
                        ORDER BY focused_seconds DESC, title ASC
                    ) AS title_rank
                FROM title_totals
            )
            SELECT app_class, title, focused_seconds
            FROM ranked
            WHERE title_rank <= ?3
            ORDER BY app_class ASC, focused_seconds DESC, title ASC
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end, limit_per_app.max(1) as i64], |row| {
                Ok(TitleTotals {
                    app_class: row.get(0)?,
                    title: row.get(1)?,
                    focused_seconds: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn focused_app_daily_totals_between(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<AppDayTotals>> {
        if end <= start {
            return Ok(Vec::new());
        }

        let (range_start_date, days) = local_day_range(start, end)?;
        let boundaries = local_midnight_boundaries(range_start_date, days)?;
        let labels = (0..days)
            .map(|offset| {
                let day = range_start_date + chrono::Duration::days(offset as i64);
                (
                    day.format("%Y-%m-%d").to_string(),
                    day.format("%b %-d").to_string(),
                )
            })
            .collect::<Vec<_>>();
        let mut totals = BTreeMap::<(usize, String), i64>::new();

        for (app_class, started_at, ended_at) in self.focused_intervals_between(start, end)? {
            for (index, window) in boundaries.windows(2).enumerate() {
                let overlap = ended_at.min(window[1]).min(end) - started_at.max(window[0]);
                if overlap > 0 {
                    *totals.entry((index, app_class.clone())).or_default() += overlap;
                }
            }
        }

        let mut rows = totals
            .into_iter()
            .map(|((index, app_class), focused_seconds)| AppDayTotals {
                date: labels[index].0.clone(),
                label: labels[index].1.clone(),
                app_class,
                focused_seconds,
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            left.date
                .cmp(&right.date)
                .then_with(|| right.focused_seconds.cmp(&left.focused_seconds))
                .then_with(|| left.app_class.cmp(&right.app_class))
        });
        Ok(rows)
    }

    pub fn focused_timeline_between(&self, start: i64, end: i64) -> Result<Vec<TimelineInterval>> {
        Ok(self
            .focused_intervals_between(start, end)?
            .into_iter()
            .map(|(app_class, started_at, ended_at)| TimelineInterval {
                kind: IntervalKind::Focused,
                app_class,
                started_at,
                ended_at,
            })
            .collect())
    }

    pub fn focused_rollups_between(
        &self,
        start: i64,
        end: i64,
        workspace_limit: usize,
        app_workspace_limit: usize,
    ) -> Result<FocusedRollups> {
        let mut heatmap = BTreeMap::<(u32, u32), i64>::new();
        for weekday in 0..7 {
            for hour in 0..24 {
                heatmap.insert((weekday, hour), 0);
            }
        }

        if end <= start {
            return Ok(FocusedRollups {
                heatmap: heatmap
                    .into_iter()
                    .map(|((weekday, hour), focused_seconds)| FocusHeatCell {
                        weekday,
                        hour,
                        focused_seconds,
                    })
                    .collect(),
                ..FocusedRollups::default()
            });
        }

        let (range_start_date, days) = local_day_range(start, end)?;
        let boundaries = local_midnight_boundaries(range_start_date, days)?;
        let labels = (0..days)
            .map(|offset| {
                let day = range_start_date + chrono::Duration::days(offset as i64);
                (
                    day.format("%Y-%m-%d").to_string(),
                    day.format("%b %-d").to_string(),
                )
            })
            .collect::<Vec<_>>();
        let mut daily_apps = BTreeMap::<(usize, String), i64>::new();
        let mut workspaces = BTreeMap::<String, i64>::new();
        let mut app_workspaces = BTreeMap::<(String, String), i64>::new();
        let mut focus_intervals = Vec::new();

        for interval in self.focused_interval_metadata_between(start, end)? {
            let duration = interval.ended_at.saturating_sub(interval.started_at);
            if duration <= 0 {
                continue;
            }

            focus_intervals.push(TimelineInterval {
                kind: IntervalKind::Focused,
                app_class: interval.app_class.clone(),
                started_at: interval.started_at,
                ended_at: interval.ended_at,
            });
            add_daily_app_rollup(
                &mut daily_apps,
                &boundaries,
                &interval.app_class,
                interval.started_at,
                interval.ended_at,
                start,
                end,
            );
            add_hourly_focus_rollup(&mut heatmap, interval.started_at, interval.ended_at);

            if let Some(workspace) = interval
                .workspace
                .as_deref()
                .map(str::trim)
                .filter(|workspace| !workspace.is_empty())
            {
                *workspaces.entry(workspace.to_string()).or_default() += duration;
                *app_workspaces
                    .entry((workspace.to_string(), interval.app_class.clone()))
                    .or_default() += duration;
            }
        }

        let mut daily_apps = daily_apps
            .into_iter()
            .map(|((index, app_class), focused_seconds)| AppDayTotals {
                date: labels[index].0.clone(),
                label: labels[index].1.clone(),
                app_class,
                focused_seconds,
            })
            .collect::<Vec<_>>();
        daily_apps.sort_by(|left, right| {
            left.date
                .cmp(&right.date)
                .then_with(|| right.focused_seconds.cmp(&left.focused_seconds))
                .then_with(|| left.app_class.cmp(&right.app_class))
        });

        let mut workspaces = workspaces
            .into_iter()
            .map(|(workspace, focused_seconds)| WorkspaceTotals {
                workspace,
                focused_seconds,
            })
            .collect::<Vec<_>>();
        workspaces.sort_by(|left, right| {
            right
                .focused_seconds
                .cmp(&left.focused_seconds)
                .then_with(|| left.workspace.cmp(&right.workspace))
        });
        workspaces.truncate(workspace_limit.max(1));

        let mut app_workspaces = app_workspaces
            .into_iter()
            .map(
                |((workspace, app_class), focused_seconds)| AppWorkspaceTotals {
                    workspace,
                    app_class,
                    focused_seconds,
                },
            )
            .collect::<Vec<_>>();
        app_workspaces.sort_by(|left, right| {
            right
                .focused_seconds
                .cmp(&left.focused_seconds)
                .then_with(|| left.workspace.cmp(&right.workspace))
                .then_with(|| left.app_class.cmp(&right.app_class))
        });
        app_workspaces.truncate(app_workspace_limit.max(1));

        Ok(FocusedRollups {
            focus_intervals,
            daily_apps,
            heatmap: heatmap
                .into_iter()
                .map(|((weekday, hour), focused_seconds)| FocusHeatCell {
                    weekday,
                    hour,
                    focused_seconds,
                })
                .collect(),
            workspaces,
            app_workspaces,
        })
    }

    pub fn focus_heatmap_between(&self, start: i64, end: i64) -> Result<Vec<FocusHeatCell>> {
        let mut totals = BTreeMap::<(u32, u32), i64>::new();
        for weekday in 0..7 {
            for hour in 0..24 {
                totals.insert((weekday, hour), 0);
            }
        }

        if end <= start {
            return Ok(totals
                .into_iter()
                .map(|((weekday, hour), focused_seconds)| FocusHeatCell {
                    weekday,
                    hour,
                    focused_seconds,
                })
                .collect());
        }

        for (_, cursor, interval_end) in self.focused_intervals_between(start, end)? {
            add_hourly_focus_rollup(&mut totals, cursor, interval_end);
        }

        Ok(totals
            .into_iter()
            .map(|((weekday, hour), focused_seconds)| FocusHeatCell {
                weekday,
                hour,
                focused_seconds,
            })
            .collect())
    }

    pub fn focused_workspace_totals_between(
        &self,
        start: i64,
        end: i64,
        limit: usize,
    ) -> Result<Vec<WorkspaceTotals>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                workspace,
                SUM(overlap_seconds) AS focused_seconds
            FROM (
                SELECT
                    trim(workspace) AS workspace,
                    MAX(0, MIN(COALESCE(ended_at, ?2), ?2) - MAX(started_at, ?1)) AS overlap_seconds
                FROM intervals
                WHERE kind = 'focused'
                  AND workspace IS NOT NULL
                  AND trim(workspace) <> ''
                  AND started_at < ?2
                  AND COALESCE(ended_at, ?2) > ?1
            )
            WHERE overlap_seconds > 0
            GROUP BY workspace
            ORDER BY focused_seconds DESC, workspace ASC
            LIMIT ?3
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end, limit.max(1) as i64], |row| {
                Ok(WorkspaceTotals {
                    workspace: row.get(0)?,
                    focused_seconds: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn focused_app_workspace_totals_between(
        &self,
        start: i64,
        end: i64,
        limit: usize,
    ) -> Result<Vec<AppWorkspaceTotals>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                workspace,
                app_class,
                SUM(overlap_seconds) AS focused_seconds
            FROM (
                SELECT
                    trim(workspace) AS workspace,
                    app_class,
                    MAX(0, MIN(COALESCE(ended_at, ?2), ?2) - MAX(started_at, ?1)) AS overlap_seconds
                FROM intervals
                WHERE kind = 'focused'
                  AND workspace IS NOT NULL
                  AND trim(workspace) <> ''
                  AND started_at < ?2
                  AND COALESCE(ended_at, ?2) > ?1
            )
            WHERE overlap_seconds > 0
            GROUP BY workspace, app_class
            ORDER BY focused_seconds DESC, workspace ASC, app_class ASC
            LIMIT ?3
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end, limit.max(1) as i64], |row| {
                Ok(AppWorkspaceTotals {
                    workspace: row.get(0)?,
                    app_class: row.get(1)?,
                    focused_seconds: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn session_totals_between(&self, start: i64, end: i64) -> Result<SessionTotals> {
        let mut totals = SessionTotals::default();
        if end <= start {
            return Ok(totals);
        }

        let mut stmt = self.conn.prepare(
            "
            SELECT kind, SUM(overlap_seconds)
            FROM (
                SELECT
                    kind,
                    MAX(0, MIN(COALESCE(ended_at, ?2), ?2) - MAX(started_at, ?1)) AS overlap_seconds
                FROM session_intervals
                WHERE started_at < ?2
                  AND COALESCE(ended_at, ?2) > ?1
            )
            WHERE overlap_seconds > 0
            GROUP BY kind
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        for (kind, seconds) in rows {
            match kind.as_str() {
                "idle" => totals.idle_seconds += seconds,
                "locked" => totals.locked_seconds += seconds,
                _ => {}
            }
        }

        let mut stmt = self.conn.prepare(
            "
            SELECT kind, SUM(overlap_seconds)
            FROM (
                SELECT
                    kind,
                    MAX(0, MIN(COALESCE(ended_at, ?2), ?2) - MAX(started_at, ?1)) AS overlap_seconds
                FROM unobserved_intervals
                WHERE started_at < ?2
                  AND COALESCE(ended_at, ?2) > ?1
            )
            WHERE overlap_seconds > 0
            GROUP BY kind
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (kind, seconds) in rows {
            match kind.as_str() {
                "sleep" => totals.sleep_seconds += seconds,
                "unobserved" => totals.unobserved_seconds += seconds,
                _ => {}
            }
        }
        Ok(totals)
    }

    pub fn repair_titles(
        &mut self,
        steam: &mut SteamResolver,
        config: &Config,
        dry_run: bool,
    ) -> Result<TitleRepair> {
        let class_counts = self.app_class_counts()?;
        let mut class_update_map = HashMap::new();
        let mut class_updates = Vec::new();

        for (from, rows) in class_counts {
            let to = identity::canonical_app_class(&steam.resolve_class(&from));
            if to != from {
                class_update_map.insert(from.clone(), to.clone());
                class_updates.push(ClassRepair { from, to, rows });
            }
        }

        let title_updates = if config.capture_titles() {
            let mut title_counts = BTreeMap::<String, i64>::new();
            for (app_class, rows) in self.missing_focused_title_counts()? {
                let app_class = class_update_map
                    .get(&app_class)
                    .cloned()
                    .unwrap_or(app_class);
                *title_counts.entry(app_class).or_default() += rows;
            }

            title_counts
                .into_iter()
                .filter_map(|(app_class, rows)| {
                    let title = identity::display_name(&app_class);
                    config
                        .title_allowed(&app_class, &title)
                        .then_some(TitleFill {
                            title,
                            app_class,
                            rows,
                        })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let title_normalizations = if config.capture_titles() {
            self.planned_title_normalizations(&class_update_map, config)?
        } else {
            Vec::new()
        };
        let planned_rewritten_rows = class_updates.iter().map(|update| update.rows).sum();
        let planned_filled_titles = title_updates.iter().map(|update| update.rows).sum();
        let planned_normalized_titles = title_normalizations.iter().map(|update| update.rows).sum();

        if dry_run {
            return Ok(TitleRepair {
                dry_run,
                class_updates,
                title_updates,
                title_normalizations,
                rewritten_rows: planned_rewritten_rows,
                filled_titles: planned_filled_titles,
                normalized_titles: planned_normalized_titles,
            });
        }

        let tx = self.conn.transaction()?;
        let mut rewritten_rows = 0;
        for update in &class_updates {
            rewritten_rows += tx.execute(
                "UPDATE intervals SET app_class = ?1 WHERE app_class = ?2",
                params![update.to, update.from],
            )? as i64;
        }

        let mut filled_titles = 0;
        for update in &title_updates {
            filled_titles += tx.execute(
                "
                UPDATE intervals
                SET title = ?1
                WHERE kind = 'focused'
                  AND app_class = ?2
                  AND (title IS NULL OR trim(title) = '')
                ",
                params![update.title, update.app_class],
            )? as i64;
        }
        let mut normalized_titles = 0;
        for update in &title_normalizations {
            normalized_titles += tx.execute(
                "
                UPDATE intervals
                SET title = ?1
                WHERE kind = 'focused'
                  AND app_class = ?2
                  AND title = ?3
                ",
                params![update.to, update.app_class, update.from],
            )? as i64;
        }
        tx.commit()?;

        Ok(TitleRepair {
            dry_run,
            class_updates,
            title_updates,
            title_normalizations,
            rewritten_rows,
            filled_titles,
            normalized_titles,
        })
    }

    fn migrate(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 250;
            PRAGMA foreign_keys = ON;
            ",
        )?;
        self.ensure_migration_table()?;

        let mut applied = self.applied_migration_versions().with_context(|| {
            format!(
                "failed to read schema migrations for database {}",
                self.path.display()
            )
        })?;
        self.validate_known_migrations(&applied)?;

        for migration in MIGRATIONS {
            if applied.contains(&migration.version) {
                continue;
            }

            let tx = self.conn.transaction().with_context(|| {
                format!(
                    "failed to start migration {} ({})",
                    migration.version, migration.description
                )
            })?;
            (migration.up)(&tx).with_context(|| {
                format!(
                    "failed to apply migration {} ({})",
                    migration.version, migration.description
                )
            })?;
            tx.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, unixepoch())",
                params![migration.version],
            )
            .with_context(|| {
                format!(
                    "failed to record migration {} ({})",
                    migration.version, migration.description
                )
            })?;
            tx.commit().with_context(|| {
                format!(
                    "failed to commit migration {} ({})",
                    migration.version, migration.description
                )
            })?;
            applied.insert(migration.version);
        }

        Ok(())
    }

    fn ensure_migration_table(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    fn validate_schema(&self) -> Result<()> {
        let applied = self.applied_migration_versions().with_context(|| {
            format!(
                "database {} is not initialized or needs migration; start omastatd once with write access before running read-only reports",
                self.path.display()
            )
        })?;
        self.validate_known_migrations(&applied)?;
        if let Some(migration) = missing_migration(&applied) {
            anyhow::bail!(
                "database {} is not initialized or needs migration (missing migration {}: {}); start omastatd once with write access before running read-only reports",
                self.path.display(),
                migration.version,
                migration.description
            );
        }

        validate_required_schema(&self.conn).with_context(|| {
            format!(
                "database {} is not initialized or needs migration; start omastatd once with write access before running read-only reports",
                self.path.display()
            )
        })?;
        Ok(())
    }

    fn applied_migration_versions(&self) -> Result<BTreeSet<i64>> {
        read_applied_migration_versions(&self.conn)
    }

    fn validate_known_migrations(&self, applied: &BTreeSet<i64>) -> Result<()> {
        if let Some(version) = unknown_migration_version(applied) {
            anyhow::bail!(
                "database {} has unknown schema migration {}; update Omastat before using this database",
                self.path.display(),
                version
            );
        }
        Ok(())
    }
}

struct DaemonRunRow {
    id: i64,
    last_heartbeat_at: i64,
    stopped_at: Option<i64>,
}

fn latest_daemon_run(tx: &Transaction<'_>) -> rusqlite::Result<Option<DaemonRunRow>> {
    tx.query_row(
        "
        SELECT id, last_heartbeat_at, stopped_at
        FROM daemon_runs
        ORDER BY id DESC
        LIMIT 1
        ",
        [],
        |row| {
            Ok(DaemonRunRow {
                id: row.get(0)?,
                last_heartbeat_at: row.get(1)?,
                stopped_at: row.get(2)?,
            })
        },
    )
    .optional()
}

fn latest_unclosed_interval_start(tx: &Transaction<'_>) -> rusqlite::Result<Option<i64>> {
    tx.query_row(
        "
        SELECT MAX(started_at)
        FROM (
            SELECT started_at FROM intervals WHERE ended_at IS NULL
            UNION ALL
            SELECT started_at FROM session_intervals WHERE ended_at IS NULL
        )
        ",
        [],
        |row| row.get(0),
    )
}

fn earliest_active_system_interval_start(
    tx: &Transaction<'_>,
    kind: &str,
) -> rusqlite::Result<Option<i64>> {
    tx.query_row(
        "
        SELECT MIN(started_at)
        FROM unobserved_intervals
        WHERE kind = ?1
          AND ended_at IS NULL
        ",
        params![kind],
        |row| row.get(0),
    )
}

fn close_unclosed_observed_intervals_tx(
    tx: &Transaction<'_>,
    closed_at: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        "
        UPDATE intervals
        SET ended_at = MAX(started_at, ?1)
        WHERE ended_at IS NULL
        ",
        params![closed_at],
    )?;
    tx.execute(
        "
        UPDATE session_intervals
        SET ended_at = MAX(started_at, ?1)
        WHERE ended_at IS NULL
        ",
        params![closed_at],
    )?;
    Ok(())
}

fn close_unclosed_system_intervals_tx(
    tx: &Transaction<'_>,
    kind: &str,
    closed_at: i64,
) -> rusqlite::Result<()> {
    tx.execute(
        "
        UPDATE unobserved_intervals
        SET ended_at = MAX(started_at, ?2)
        WHERE kind = ?1
          AND ended_at IS NULL
        ",
        params![kind, closed_at],
    )?;
    Ok(())
}

fn insert_daemon_event_tx(
    tx: &Transaction<'_>,
    run_id: i64,
    kind: &str,
    occurred_at: i64,
    detail: Option<&str>,
) -> rusqlite::Result<()> {
    tx.execute(
        "
        INSERT INTO daemon_events (run_id, kind, occurred_at, detail)
        VALUES (?1, ?2, ?3, ?4)
        ",
        params![run_id, kind, occurred_at, detail],
    )?;
    Ok(())
}

fn diagnose_schema(conn: &Connection) -> StorageSchemaStatus {
    match table_exists(conn, "schema_migrations") {
        Ok(true) => {}
        Ok(false) => {
            return StorageSchemaStatus::NotInitialized {
                reason: "schema_migrations table is missing".to_string(),
            };
        }
        Err(error) => {
            return StorageSchemaStatus::Invalid {
                error: format!("{error:#}"),
            };
        }
    }

    let applied = match read_applied_migration_versions(conn) {
        Ok(applied) => applied,
        Err(error) => {
            return StorageSchemaStatus::Invalid {
                error: format!("{error:#}"),
            };
        }
    };
    let applied_migrations = applied.iter().copied().collect::<Vec<_>>();

    if let Some(version) = unknown_migration_version(&applied) {
        return StorageSchemaStatus::UnknownMigration {
            version,
            applied_migrations,
        };
    }

    if let Some(migration) = missing_migration(&applied) {
        return StorageSchemaStatus::NeedsMigration {
            version: migration.version,
            description: migration.description.to_string(),
            applied_migrations,
        };
    }

    if let Err(error) = validate_required_schema(conn) {
        return StorageSchemaStatus::Invalid {
            error: format!("{error:#}"),
        };
    }

    StorageSchemaStatus::Current { applied_migrations }
}

fn sqlite_quick_check(conn: &Connection) -> StorageQuickCheck {
    match conn.query_row("PRAGMA quick_check(1)", [], |row| row.get::<_, String>(0)) {
        Ok(value) if value.eq_ignore_ascii_case("ok") => StorageQuickCheck::Ok,
        Ok(value) => StorageQuickCheck::Problem(value),
        Err(error) => StorageQuickCheck::Error(format!("{error:#}")),
    }
}

fn table_exists(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "
        SELECT EXISTS(
            SELECT 1
            FROM sqlite_schema
            WHERE type = 'table'
              AND name = ?1
        )
        ",
        params![table],
        |row| Ok(row.get::<_, i64>(0)? != 0),
    )
}

fn validate_required_schema(conn: &Connection) -> rusqlite::Result<()> {
    for statement in [
        "SELECT id, kind, app_class, window_address, title, workspace, monitor, started_at, ended_at FROM intervals LIMIT 0",
        "SELECT id, kind, source, started_at, ended_at FROM session_intervals LIMIT 0",
        "SELECT id, started_at, last_heartbeat_at, stopped_at, stop_kind FROM daemon_runs LIMIT 0",
        "SELECT id, run_id, kind, occurred_at, detail FROM daemon_events LIMIT 0",
        "SELECT id, kind, source, started_at, ended_at FROM unobserved_intervals LIMIT 0",
    ] {
        conn.prepare(statement)?;
    }
    for index in [
        "idx_intervals_one_active_focused",
        "idx_intervals_one_active_open_per_app",
        "idx_session_intervals_one_active_kind",
        "idx_unobserved_intervals_one_active_kind",
        "idx_daemon_runs_one_active",
    ] {
        if !index_exists(conn, index)? {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "missing required index {index}"
            )));
        }
    }
    Ok(())
}

fn index_exists(conn: &Connection, index: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "
        SELECT EXISTS(
            SELECT 1
            FROM sqlite_schema
            WHERE type = 'index'
              AND name = ?1
        )
        ",
        params![index],
        |row| Ok(row.get::<_, i64>(0)? != 0),
    )
}

fn read_applied_migration_versions(conn: &Connection) -> Result<BTreeSet<i64>> {
    let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version ASC")?;
    let versions = stmt
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    Ok(versions)
}

fn migrate_0001_create_intervals(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
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
        ",
    )
}

fn migrate_0002_create_session_intervals(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS session_intervals (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('idle', 'locked')),
            source TEXT,
            started_at INTEGER NOT NULL,
            ended_at INTEGER,
            CHECK (ended_at IS NULL OR ended_at >= started_at)
        );

        CREATE INDEX IF NOT EXISTS idx_session_intervals_kind
            ON session_intervals(kind);
        CREATE INDEX IF NOT EXISTS idx_session_intervals_time
            ON session_intervals(started_at, ended_at);
        CREATE INDEX IF NOT EXISTS idx_session_intervals_open
            ON session_intervals(ended_at)
            WHERE ended_at IS NULL;
        ",
    )
}

fn migrate_0003_add_interval_workspace(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    add_column_if_missing(tx, "intervals", "workspace", "TEXT")
}

fn migrate_0004_add_interval_monitor(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    add_column_if_missing(tx, "intervals", "monitor", "TEXT")
}

fn migrate_0005_add_daemon_lifecycle(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS daemon_runs (
            id INTEGER PRIMARY KEY,
            started_at INTEGER NOT NULL,
            last_heartbeat_at INTEGER NOT NULL,
            stopped_at INTEGER,
            stop_kind TEXT CHECK (stop_kind IS NULL OR stop_kind IN ('clean', 'recovered')),
            CHECK (last_heartbeat_at >= started_at),
            CHECK (stopped_at IS NULL OR stopped_at >= started_at)
        );

        CREATE TABLE IF NOT EXISTS daemon_events (
            id INTEGER PRIMARY KEY,
            run_id INTEGER,
            kind TEXT NOT NULL CHECK (kind IN ('start', 'heartbeat', 'clean-stop', 'recovery')),
            occurred_at INTEGER NOT NULL,
            detail TEXT
        );

        CREATE TABLE IF NOT EXISTS unobserved_intervals (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('sleep', 'unobserved')),
            source TEXT,
            started_at INTEGER NOT NULL,
            ended_at INTEGER,
            CHECK (ended_at IS NULL OR ended_at >= started_at)
        );

        CREATE INDEX IF NOT EXISTS idx_daemon_runs_active
            ON daemon_runs(stopped_at)
            WHERE stopped_at IS NULL;
        CREATE INDEX IF NOT EXISTS idx_daemon_events_time
            ON daemon_events(occurred_at);
        CREATE INDEX IF NOT EXISTS idx_daemon_events_kind
            ON daemon_events(kind);
        CREATE INDEX IF NOT EXISTS idx_unobserved_intervals_time
            ON unobserved_intervals(started_at, ended_at);
        CREATE INDEX IF NOT EXISTS idx_unobserved_intervals_open
            ON unobserved_intervals(kind, ended_at)
            WHERE ended_at IS NULL;
        ",
    )
}

fn migrate_0006_expand_system_intervals(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        ALTER TABLE unobserved_intervals RENAME TO unobserved_intervals_old;

        CREATE TABLE unobserved_intervals (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('sleep', 'unobserved')),
            source TEXT,
            started_at INTEGER NOT NULL,
            ended_at INTEGER,
            CHECK (ended_at IS NULL OR ended_at >= started_at)
        );

        INSERT INTO unobserved_intervals (id, kind, source, started_at, ended_at)
        SELECT id, kind, source, started_at, ended_at
        FROM unobserved_intervals_old
        WHERE kind IN ('sleep', 'unobserved');

        DROP TABLE unobserved_intervals_old;

        CREATE INDEX IF NOT EXISTS idx_unobserved_intervals_time
            ON unobserved_intervals(started_at, ended_at);
        CREATE INDEX IF NOT EXISTS idx_unobserved_intervals_open
            ON unobserved_intervals(kind, ended_at)
            WHERE ended_at IS NULL;
        ",
    )
}

fn migrate_0007_add_report_indexes(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        CREATE INDEX IF NOT EXISTS idx_intervals_kind_time
            ON intervals(kind, started_at, ended_at);
        CREATE INDEX IF NOT EXISTS idx_intervals_kind_app_time
            ON intervals(kind, app_class, started_at, ended_at);
        CREATE INDEX IF NOT EXISTS idx_intervals_focused_workspace_time
            ON intervals(workspace, started_at, ended_at)
            WHERE kind = 'focused' AND workspace IS NOT NULL AND trim(workspace) <> '';
        CREATE INDEX IF NOT EXISTS idx_session_intervals_kind_time
            ON session_intervals(kind, started_at, ended_at);
        CREATE INDEX IF NOT EXISTS idx_unobserved_intervals_kind_time
            ON unobserved_intervals(kind, started_at, ended_at);
        ",
    )
}

fn migrate_0008_add_active_interval_invariants(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute_batch(
        "
        UPDATE intervals
        SET ended_at = started_at
        WHERE ended_at IS NULL
          AND id IN (
            SELECT id
            FROM (
                SELECT
                    id,
                    ROW_NUMBER() OVER (
                        PARTITION BY
                            CASE
                                WHEN kind = 'open' THEN 'open:' || app_class
                                ELSE kind
                            END
                        ORDER BY started_at ASC, id ASC
                    ) AS duplicate_rank
                FROM intervals
                WHERE ended_at IS NULL
            )
            WHERE duplicate_rank > 1
          );

        UPDATE session_intervals
        SET ended_at = started_at
        WHERE ended_at IS NULL
          AND id IN (
            SELECT id
            FROM (
                SELECT
                    id,
                    ROW_NUMBER() OVER (
                        PARTITION BY kind
                        ORDER BY started_at ASC, id ASC
                    ) AS duplicate_rank
                FROM session_intervals
                WHERE ended_at IS NULL
            )
            WHERE duplicate_rank > 1
          );

        UPDATE unobserved_intervals
        SET ended_at = started_at
        WHERE ended_at IS NULL
          AND id IN (
            SELECT id
            FROM (
                SELECT
                    id,
                    ROW_NUMBER() OVER (
                        PARTITION BY kind
                        ORDER BY started_at ASC, id ASC
                    ) AS duplicate_rank
                FROM unobserved_intervals
                WHERE ended_at IS NULL
            )
            WHERE duplicate_rank > 1
          );

        UPDATE daemon_runs
        SET stopped_at = MAX(started_at, last_heartbeat_at),
            stop_kind = 'recovered'
        WHERE stopped_at IS NULL
          AND id NOT IN (
              SELECT id
              FROM daemon_runs
              WHERE stopped_at IS NULL
              ORDER BY id DESC
              LIMIT 1
          );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_intervals_one_active_focused
            ON intervals(kind)
            WHERE kind = 'focused' AND ended_at IS NULL;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_intervals_one_active_open_per_app
            ON intervals(kind, app_class)
            WHERE kind = 'open' AND ended_at IS NULL;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_session_intervals_one_active_kind
            ON session_intervals(kind)
            WHERE ended_at IS NULL;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_unobserved_intervals_one_active_kind
            ON unobserved_intervals(kind)
            WHERE ended_at IS NULL;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_daemon_runs_one_active
            ON daemon_runs((1))
            WHERE stopped_at IS NULL;
        ",
    )
}

fn add_column_if_missing(
    tx: &Transaction<'_>,
    table: &str,
    column: &str,
    column_type: &str,
) -> rusqlite::Result<()> {
    if column_exists(tx, table, column)? {
        return Ok(());
    }
    tx.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {column_type}"),
        [],
    )?;
    Ok(())
}

fn column_exists(tx: &Transaction<'_>, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut stmt = tx.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(columns.iter().any(|name| name == column))
}

fn missing_migration(applied: &BTreeSet<i64>) -> Option<&'static Migration> {
    MIGRATIONS
        .iter()
        .find(|migration| !applied.contains(&migration.version))
}

fn unknown_migration_version(applied: &BTreeSet<i64>) -> Option<i64> {
    applied.iter().copied().find(|version| {
        !MIGRATIONS
            .iter()
            .any(|migration| migration.version == *version)
    })
}

#[derive(Debug, Clone)]
struct FocusedIntervalMetadata {
    app_class: String,
    workspace: Option<String>,
    started_at: i64,
    ended_at: i64,
}

impl Storage {
    fn app_class_counts(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT app_class, COUNT(*)
            FROM intervals
            GROUP BY app_class
            ORDER BY app_class ASC
            ",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn missing_focused_title_counts(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT app_class, COUNT(*)
            FROM intervals
            WHERE kind = 'focused'
              AND (title IS NULL OR trim(title) = '')
            GROUP BY app_class
            ORDER BY app_class ASC
            ",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn focused_intervals_between(&self, start: i64, end: i64) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                app_class,
                MAX(started_at, ?1) AS bounded_start,
                MIN(COALESCE(ended_at, ?2), ?2) AS bounded_end
            FROM intervals
            WHERE kind = 'focused'
              AND started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ORDER BY bounded_start ASC, bounded_end ASC, id ASC
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn raw_intervals_between(&self, start: i64, end: i64) -> Result<Vec<RawInterval>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                kind,
                app_class,
                window_address,
                title,
                workspace,
                monitor,
                started_at,
                ended_at
            FROM intervals
            WHERE started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ORDER BY started_at ASC, COALESCE(ended_at, ?2) ASC, id ASC
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
                let started_at = row.get::<_, i64>(6)?;
                let ended_at = row.get::<_, Option<i64>>(7)?;
                Ok(RawInterval {
                    kind,
                    app_class: row.get(1)?,
                    window_address: row.get(2)?,
                    title: row.get(3)?,
                    workspace: row.get(4)?,
                    monitor: row.get(5)?,
                    started_at,
                    ended_at,
                    local_start: local_timestamp(started_at),
                    local_end: ended_at.map(local_timestamp),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn raw_session_intervals_between(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<RawSessionInterval>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT kind, source, started_at, ended_at
            FROM session_intervals
            WHERE started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ORDER BY started_at ASC, COALESCE(ended_at, ?2) ASC, id ASC
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end], |row| {
                let kind = row.get::<_, String>(0)?;
                let kind = SessionIntervalKind::from_str(&kind).ok_or_else(|| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "kind".to_string(),
                        rusqlite::types::Type::Text,
                    )
                })?;
                let started_at = row.get::<_, i64>(2)?;
                let ended_at = row.get::<_, Option<i64>>(3)?;
                Ok(RawSessionInterval {
                    kind,
                    source: row.get(1)?,
                    started_at,
                    ended_at,
                    local_start: local_timestamp(started_at),
                    local_end: ended_at.map(local_timestamp),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn raw_system_intervals_between(&self, start: i64, end: i64) -> Result<Vec<RawSystemInterval>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT kind, source, started_at, ended_at
            FROM unobserved_intervals
            WHERE started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ORDER BY started_at ASC, COALESCE(ended_at, ?2) ASC, id ASC
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end], |row| {
                let kind = row.get::<_, String>(0)?;
                let kind = SystemIntervalKind::from_str(&kind).ok_or_else(|| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "kind".to_string(),
                        rusqlite::types::Type::Text,
                    )
                })?;
                let started_at = row.get::<_, i64>(2)?;
                let ended_at = row.get::<_, Option<i64>>(3)?;
                Ok(RawSystemInterval {
                    kind,
                    source: row.get(1)?,
                    started_at,
                    ended_at,
                    local_start: local_timestamp(started_at),
                    local_end: ended_at.map(local_timestamp),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn focused_interval_metadata_between(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<FocusedIntervalMetadata>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                app_class,
                workspace,
                MAX(started_at, ?1) AS bounded_start,
                MIN(COALESCE(ended_at, ?2), ?2) AS bounded_end
            FROM intervals
            WHERE kind = 'focused'
              AND started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ORDER BY bounded_start ASC, bounded_end ASC, id ASC
            ",
        )?;
        let rows = stmt
            .query_map(params![start, end], |row| {
                Ok(FocusedIntervalMetadata {
                    app_class: row.get(0)?,
                    workspace: row.get(1)?,
                    started_at: row.get(2)?,
                    ended_at: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn focused_title_counts(&self) -> Result<Vec<(String, String, i64)>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT app_class, title, COUNT(*)
            FROM intervals
            WHERE kind = 'focused'
              AND title IS NOT NULL
              AND trim(title) <> ''
            GROUP BY app_class, title
            ORDER BY app_class ASC, title ASC
            ",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn planned_title_normalizations(
        &self,
        class_update_map: &HashMap<String, String>,
        config: &Config,
    ) -> Result<Vec<TitleNormalize>> {
        let mut planned = BTreeMap::<(String, String, String), i64>::new();
        for (app_class, title, rows) in self.focused_title_counts()? {
            let canonical_app_class = class_update_map
                .get(&app_class)
                .cloned()
                .unwrap_or(app_class);
            let Some(cleaned) = identity::clean_window_title(&title, &canonical_app_class) else {
                continue;
            };
            if cleaned == title {
                continue;
            }
            if !config.title_allowed(&canonical_app_class, &cleaned) {
                continue;
            }
            *planned
                .entry((canonical_app_class, title, cleaned))
                .or_default() += rows;
        }

        Ok(planned
            .into_iter()
            .map(|((app_class, from, to), rows)| TitleNormalize {
                app_class,
                from,
                to,
                rows,
            })
            .collect())
    }

    #[cfg(test)]
    pub(crate) fn focused_titles_for_tests(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT title
            FROM intervals
            WHERE kind = 'focused' AND title IS NOT NULL
            ORDER BY id ASC
            ",
        )?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn totals_between(&self, start: i64, end: i64) -> Result<Vec<AppTotals>> {
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

    pub fn timeline_between(&self, start: i64, end: i64) -> Result<Vec<TimelineInterval>> {
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

    pub fn system_timeline_between(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<SystemTimelineInterval>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                kind,
                source,
                MAX(started_at, ?1) AS bounded_start,
                MIN(COALESCE(ended_at, ?2), ?2) AS bounded_end
            FROM unobserved_intervals
            WHERE started_at < ?2
              AND COALESCE(ended_at, ?2) > ?1
            ORDER BY bounded_start ASC, bounded_end ASC, id ASC
            ",
        )?;

        let rows = stmt
            .query_map(params![start, end], |row| {
                let kind = row.get::<_, String>(0)?;
                let kind = SystemIntervalKind::from_str(&kind).ok_or_else(|| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "kind".to_string(),
                        rusqlite::types::Type::Text,
                    )
                })?;
                Ok(SystemTimelineInterval {
                    kind,
                    source: row.get(1)?,
                    started_at: row.get(2)?,
                    ended_at: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn raw_export_between(&self, start: i64, end: i64) -> Result<RawExportRows> {
        Ok(RawExportRows {
            intervals: self.raw_intervals_between(start, end)?,
            session_intervals: self.raw_session_intervals_between(start, end)?,
            system_intervals: self.raw_system_intervals_between(start, end)?,
        })
    }

    pub fn purge_before(
        &mut self,
        cutoff_ts: Option<i64>,
        dry_run: bool,
        vacuum: bool,
    ) -> Result<PurgeReport> {
        let cutoff_local = cutoff_ts.map(local_timestamp);
        let mut report = PurgeReport {
            dry_run,
            cutoff_ts,
            cutoff_local,
            intervals_deleted: 0,
            session_intervals_deleted: 0,
            system_intervals_deleted: 0,
            daemon_events_deleted: 0,
            daemon_runs_deleted: 0,
            intervals_trimmed: 0,
            session_intervals_trimmed: 0,
            system_intervals_trimmed: 0,
            vacuumed: false,
        };

        if dry_run {
            report.intervals_deleted = purge_delete_count(
                &self.conn,
                "intervals",
                cutoff_ts,
                "ended_at IS NOT NULL AND ended_at <= ?1",
            )?;
            report.session_intervals_deleted = purge_delete_count(
                &self.conn,
                "session_intervals",
                cutoff_ts,
                "ended_at IS NOT NULL AND ended_at <= ?1",
            )?;
            report.system_intervals_deleted = purge_delete_count(
                &self.conn,
                "unobserved_intervals",
                cutoff_ts,
                "ended_at IS NOT NULL AND ended_at <= ?1",
            )?;
            report.daemon_events_deleted =
                purge_delete_count(&self.conn, "daemon_events", cutoff_ts, "occurred_at < ?1")?;
            report.daemon_runs_deleted = purge_delete_count(
                &self.conn,
                "daemon_runs",
                cutoff_ts,
                "stopped_at IS NOT NULL AND stopped_at <= ?1",
            )?;
            report.intervals_trimmed = purge_trim_count(&self.conn, "intervals", cutoff_ts)?;
            report.session_intervals_trimmed =
                purge_trim_count(&self.conn, "session_intervals", cutoff_ts)?;
            report.system_intervals_trimmed =
                purge_trim_count(&self.conn, "unobserved_intervals", cutoff_ts)?;
            return Ok(report);
        }

        {
            let tx = self.conn.transaction()?;
            match cutoff_ts {
                Some(cutoff) => {
                    report.intervals_deleted = tx.execute(
                        "DELETE FROM intervals WHERE ended_at IS NOT NULL AND ended_at <= ?1",
                        params![cutoff],
                    )? as i64;
                    report.session_intervals_deleted = tx.execute(
                        "DELETE FROM session_intervals WHERE ended_at IS NOT NULL AND ended_at <= ?1",
                        params![cutoff],
                    )? as i64;
                    report.system_intervals_deleted = tx.execute(
                        "DELETE FROM unobserved_intervals WHERE ended_at IS NOT NULL AND ended_at <= ?1",
                        params![cutoff],
                    )? as i64;
                    report.daemon_events_deleted = tx.execute(
                        "DELETE FROM daemon_events WHERE occurred_at < ?1",
                        params![cutoff],
                    )? as i64;
                    report.daemon_runs_deleted = tx.execute(
                        "DELETE FROM daemon_runs WHERE stopped_at IS NOT NULL AND stopped_at <= ?1",
                        params![cutoff],
                    )? as i64;
                    report.intervals_trimmed = trim_table_start(&tx, "intervals", cutoff)?;
                    report.session_intervals_trimmed =
                        trim_table_start(&tx, "session_intervals", cutoff)?;
                    report.system_intervals_trimmed =
                        trim_table_start(&tx, "unobserved_intervals", cutoff)?;
                }
                None => {
                    report.intervals_deleted = tx.execute("DELETE FROM intervals", [])? as i64;
                    report.session_intervals_deleted =
                        tx.execute("DELETE FROM session_intervals", [])? as i64;
                    report.system_intervals_deleted =
                        tx.execute("DELETE FROM unobserved_intervals", [])? as i64;
                    report.daemon_events_deleted =
                        tx.execute("DELETE FROM daemon_events", [])? as i64;
                    report.daemon_runs_deleted = tx.execute("DELETE FROM daemon_runs", [])? as i64;
                }
            }
            tx.commit()?;
        }

        if vacuum {
            self.conn.execute_batch("VACUUM")?;
            report.vacuumed = true;
        }

        Ok(report)
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

impl SessionIntervalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Locked => "locked",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "idle" => Some(Self::Idle),
            "locked" => Some(Self::Locked),
            _ => None,
        }
    }
}

impl SystemIntervalKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Sleep => "sleep",
            Self::Unobserved => "unobserved",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "sleep" => Some(Self::Sleep),
            "unobserved" => Some(Self::Unobserved),
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

fn default_db_path_for_mode(mode: StorageOpenMode) -> PathBuf {
    let path = default_db_path();
    if mode == StorageOpenMode::ReadOnly && !path.exists() {
        let legacy = legacy_db_path();
        if legacy.exists() {
            return legacy;
        }
    }
    path
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

fn latest_timestamp(left: Option<i64>, right: Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn ensure_row_was_closeable(conn: &Connection, table: &str, id: i64, updated: usize) -> Result<()> {
    if updated > 0 {
        return Ok(());
    }

    let exists = conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = ?1)"),
        params![id],
        |row| Ok(row.get::<_, i64>(0)? != 0),
    )?;
    if exists {
        anyhow::bail!("{table} row {id} is already closed");
    }
    anyhow::bail!("{table} row {id} does not exist")
}

fn local_timestamp(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| time.format("%Y-%m-%d %H:%M:%S %:z").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn purge_delete_count(
    conn: &Connection,
    table: &str,
    cutoff_ts: Option<i64>,
    predicate: &str,
) -> Result<i64> {
    let sql = match cutoff_ts {
        Some(_) => format!("SELECT COUNT(*) FROM {table} WHERE {predicate}"),
        None => format!("SELECT COUNT(*) FROM {table}"),
    };
    let count = match cutoff_ts {
        Some(cutoff) => conn.query_row(&sql, params![cutoff], |row| row.get(0))?,
        None => conn.query_row(&sql, [], |row| row.get(0))?,
    };
    Ok(count)
}

fn purge_trim_count(conn: &Connection, table: &str, cutoff_ts: Option<i64>) -> Result<i64> {
    let Some(cutoff) = cutoff_ts else {
        return Ok(0);
    };
    let sql = format!(
        "SELECT COUNT(*) FROM {table} WHERE started_at < ?1 AND (ended_at IS NULL OR ended_at > ?1)"
    );
    Ok(conn.query_row(&sql, params![cutoff], |row| row.get(0))?)
}

fn trim_table_start(tx: &Transaction<'_>, table: &str, cutoff: i64) -> rusqlite::Result<i64> {
    let sql = format!(
        "UPDATE {table} SET started_at = ?1 WHERE started_at < ?1 AND (ended_at IS NULL OR ended_at > ?1)"
    );
    tx.execute(&sql, params![cutoff]).map(|rows| rows as i64)
}

fn local_day_range(start: i64, end: i64) -> Result<(NaiveDate, usize)> {
    let start_local = Local
        .timestamp_opt(start, 0)
        .single()
        .context("failed to compute local start timestamp")?;
    let end_local = Local
        .timestamp_opt(end.saturating_sub(1).max(start), 0)
        .single()
        .context("failed to compute local end timestamp")?;
    let start_date = start_local.date_naive();
    let end_date = end_local.date_naive();
    let days = (end_date - start_date).num_days().max(0) as usize + 1;
    Ok((start_date, days))
}

fn add_daily_app_rollup(
    totals: &mut BTreeMap<(usize, String), i64>,
    boundaries: &[i64],
    app_class: &str,
    started_at: i64,
    ended_at: i64,
    start: i64,
    end: i64,
) {
    for (index, window) in boundaries.windows(2).enumerate() {
        let overlap = ended_at.min(window[1]).min(end) - started_at.max(window[0]).max(start);
        if overlap > 0 {
            *totals.entry((index, app_class.to_string())).or_default() += overlap;
        }
    }
}

fn add_hourly_focus_rollup(totals: &mut BTreeMap<(u32, u32), i64>, started_at: i64, ended_at: i64) {
    let mut cursor = started_at;
    while cursor < ended_at {
        let Some(key) = local_weekday_hour(cursor) else {
            break;
        };
        let segment_end = next_local_hour_change(cursor, ended_at, key);
        let overlap = segment_end - cursor;
        if overlap > 0 {
            *totals.entry(key).or_default() += overlap;
        }
        cursor = segment_end.max(cursor + 1);
    }
}

fn local_midnight_boundaries(start_date: NaiveDate, days: usize) -> Result<Vec<i64>> {
    (0..=days)
        .map(|offset| local_midnight_timestamp(start_date + chrono::Duration::days(offset as i64)))
        .collect()
}

fn local_midnight_timestamp(date: NaiveDate) -> Result<i64> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .context("invalid local midnight")?;
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(time) => Ok(time.timestamp()),
        chrono::LocalResult::Ambiguous(earliest, _) => Ok(earliest.timestamp()),
        chrono::LocalResult::None => {
            let mut candidate = naive;
            for _ in 0..180 {
                candidate += chrono::Duration::minutes(1);
                match Local.from_local_datetime(&candidate) {
                    chrono::LocalResult::Single(time) => return Ok(time.timestamp()),
                    chrono::LocalResult::Ambiguous(earliest, _) => return Ok(earliest.timestamp()),
                    chrono::LocalResult::None => {}
                }
            }
            anyhow::bail!("failed to compute local midnight")
        }
    }
}

fn local_weekday_hour(timestamp: i64) -> Option<(u32, u32)> {
    let local = Local.timestamp_opt(timestamp, 0).single()?;
    Some((local.weekday().num_days_from_monday(), local.hour()))
}

fn next_local_hour_change(cursor: i64, ended_at: i64, key: (u32, u32)) -> i64 {
    let search_end = ended_at.min(cursor.saturating_add(3 * 60 * 60 + 1));
    if search_end <= cursor + 1 {
        return search_end;
    }
    if local_weekday_hour(search_end.saturating_sub(1)) == Some(key) {
        return search_end;
    }

    let mut low = cursor + 1;
    let mut high = search_end;
    while low < high {
        let mid = low + (high - low) / 2;
        if local_weekday_hour(mid) == Some(key) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

#[cfg(test)]
mod tests {
    use super::{
        DaemonRecovery, IntervalKind, IntervalMetadata, SessionIntervalKind, Storage,
        StorageQuickCheck, StorageSchemaStatus, SystemIntervalKind, index_exists,
    };
    use crate::config::{Config, TitleCapture};
    use crate::steam::SteamResolver;
    use chrono::{Datelike, Local, TimeZone};
    use rusqlite::{Connection, params};
    use std::time::{Duration as StdDuration, Instant};

    fn migration_versions(conn: &Connection) -> Vec<i64> {
        conn.prepare("SELECT version FROM schema_migrations ORDER BY version ASC")
            .unwrap()
            .query_map([], |row| row.get::<_, i64>(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    fn table_columns(conn: &Connection, table: &str) -> Vec<String> {
        conn.prepare(&format!("PRAGMA table_info({table})"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    fn explain_plan(conn: &Connection, sql: &str) -> Vec<String> {
        conn.prepare(sql)
            .unwrap()
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
    }

    fn create_legacy_usage_schema(conn: &Connection) {
        conn.execute_batch(
            "
            CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );
            INSERT INTO schema_migrations(version, applied_at)
                VALUES (1, 1), (2, 2);

            CREATE TABLE intervals (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('focused', 'open')),
                app_class TEXT NOT NULL,
                window_address TEXT,
                title TEXT,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                CHECK (ended_at IS NULL OR ended_at >= started_at)
            );
            CREATE INDEX idx_intervals_kind_app
                ON intervals(kind, app_class);
            CREATE INDEX idx_intervals_time
                ON intervals(started_at, ended_at);
            CREATE INDEX idx_intervals_open
                ON intervals(ended_at)
                WHERE ended_at IS NULL;

            CREATE TABLE session_intervals (
                id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL CHECK (kind IN ('idle', 'locked')),
                source TEXT,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                CHECK (ended_at IS NULL OR ended_at >= started_at)
            );
            CREATE INDEX idx_session_intervals_kind
                ON session_intervals(kind);
            CREATE INDEX idx_session_intervals_time
                ON session_intervals(started_at, ended_at);
            CREATE INDEX idx_session_intervals_open
                ON session_intervals(ended_at)
                WHERE ended_at IS NULL;
            ",
        )
        .unwrap();
    }

    #[test]
    fn fresh_database_records_full_migration_history() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        assert_eq!(
            migration_versions(&storage.conn),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
        let interval_columns = table_columns(&storage.conn, "intervals");
        assert!(interval_columns.iter().any(|column| column == "workspace"));
        assert!(interval_columns.iter().any(|column| column == "monitor"));
        for index in [
            "idx_intervals_one_active_focused",
            "idx_intervals_one_active_open_per_app",
            "idx_session_intervals_one_active_kind",
            "idx_unobserved_intervals_one_active_kind",
            "idx_daemon_runs_one_active",
        ] {
            assert!(index_exists(&storage.conn, index).unwrap(), "{index}");
        }
    }

    #[test]
    fn active_interval_constraints_reject_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let mut storage = Storage::open(Some(&db), &config).unwrap();

        let focused = storage
            .start_interval(IntervalKind::Focused, "firefox", None, None, 100)
            .unwrap();
        assert!(
            storage
                .start_interval(IntervalKind::Focused, "code", None, None, 110)
                .is_err()
        );
        storage.close_interval(focused, 120).unwrap();
        storage
            .start_interval(IntervalKind::Focused, "code", None, None, 130)
            .unwrap();

        storage
            .start_interval(IntervalKind::Open, "firefox", None, None, 100)
            .unwrap();
        assert!(
            storage
                .start_interval(IntervalKind::Open, "firefox", None, None, 110)
                .is_err()
        );
        storage
            .start_interval(IntervalKind::Open, "code", None, None, 120)
            .unwrap();

        storage
            .start_session_interval(SessionIntervalKind::Idle, Some("test"), 100)
            .unwrap();
        assert!(
            storage
                .start_session_interval(SessionIntervalKind::Idle, Some("test"), 110)
                .is_err()
        );

        storage
            .start_system_interval(SystemIntervalKind::Sleep, Some("test"), 100)
            .unwrap();
        assert!(
            storage
                .start_system_interval(SystemIntervalKind::Sleep, Some("test"), 110)
                .is_err()
        );

        let first = storage.start_daemon_run(100).unwrap();
        let second = storage.start_daemon_run(200).unwrap();
        assert_eq!(second.recovery.unwrap().previous_run_id, Some(first.run_id));
        let active_runs: i64 = storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM daemon_runs WHERE stopped_at IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active_runs, 1);
    }

    #[test]
    fn migrates_legacy_database_without_workspace_monitor_columns() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        {
            let conn = Connection::open(&db).unwrap();
            create_legacy_usage_schema(&conn);
            conn.execute(
                "
                INSERT INTO intervals(kind, app_class, started_at, ended_at)
                VALUES ('focused', 'firefox', 100, 160)
                ",
                [],
            )
            .unwrap();
        }

        let storage = Storage::open(Some(&db), &config).unwrap();

        assert_eq!(
            migration_versions(&storage.conn),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
        let interval_columns = table_columns(&storage.conn, "intervals");
        assert!(interval_columns.iter().any(|column| column == "workspace"));
        assert!(interval_columns.iter().any(|column| column == "monitor"));
        let rows = storage.totals_between(100, 200).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].focused_seconds, 60);
    }

    #[test]
    fn records_workspace_monitor_migrations_when_columns_already_exist() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        {
            let conn = Connection::open(&db).unwrap();
            create_legacy_usage_schema(&conn);
            conn.execute_batch(
                "
                ALTER TABLE intervals ADD COLUMN workspace TEXT;
                ALTER TABLE intervals ADD COLUMN monitor TEXT;
                ",
            )
            .unwrap();
        }

        let storage = Storage::open(Some(&db), &config).unwrap();

        assert_eq!(
            migration_versions(&storage.conn),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
    }

    #[test]
    fn migrates_version_five_unobserved_table_for_sleep_intervals() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        {
            let conn = Connection::open(&db).unwrap();
            create_legacy_usage_schema(&conn);
            conn.execute_batch(
                "
                ALTER TABLE intervals ADD COLUMN workspace TEXT;
                ALTER TABLE intervals ADD COLUMN monitor TEXT;
                INSERT INTO schema_migrations(version, applied_at)
                    VALUES (3, 3), (4, 4), (5, 5);

                CREATE TABLE daemon_runs (
                    id INTEGER PRIMARY KEY,
                    started_at INTEGER NOT NULL,
                    last_heartbeat_at INTEGER NOT NULL,
                    stopped_at INTEGER,
                    stop_kind TEXT CHECK (stop_kind IS NULL OR stop_kind IN ('clean', 'recovered')),
                    CHECK (last_heartbeat_at >= started_at),
                    CHECK (stopped_at IS NULL OR stopped_at >= started_at)
                );
                CREATE TABLE daemon_events (
                    id INTEGER PRIMARY KEY,
                    run_id INTEGER,
                    kind TEXT NOT NULL CHECK (kind IN ('start', 'heartbeat', 'clean-stop', 'recovery')),
                    occurred_at INTEGER NOT NULL,
                    detail TEXT
                );
                CREATE TABLE unobserved_intervals (
                    id INTEGER PRIMARY KEY,
                    kind TEXT NOT NULL CHECK (kind IN ('unobserved')),
                    source TEXT,
                    started_at INTEGER NOT NULL,
                    ended_at INTEGER NOT NULL,
                    CHECK (ended_at >= started_at)
                );
                INSERT INTO unobserved_intervals(kind, source, started_at, ended_at)
                    VALUES ('unobserved', 'test', 100, 160);
                ",
            )
            .unwrap();
        }

        let storage = Storage::open(Some(&db), &config).unwrap();

        assert_eq!(
            migration_versions(&storage.conn),
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );
        let sleep = storage
            .start_system_interval(SystemIntervalKind::Sleep, Some("test"), 200)
            .unwrap();
        storage.close_system_interval(sleep, 260).unwrap();
        let totals = storage.session_totals_between(0, 300).unwrap();
        assert_eq!(totals.unobserved_seconds, 60);
        assert_eq!(totals.sleep_seconds, 60);
    }

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
    fn timeline_between_bounds_open_and_focused_intervals() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let focused = storage
            .start_interval(IntervalKind::Focused, "ghostty", None, None, 100)
            .unwrap();
        storage.close_interval(focused, 220).unwrap();
        let open = storage
            .start_interval(IntervalKind::Open, "firefox", None, None, 140)
            .unwrap();
        storage.close_interval(open, 260).unwrap();

        let rows = storage.timeline_between(150, 225).unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, IntervalKind::Focused);
        assert_eq!(rows[0].started_at, 150);
        assert_eq!(rows[0].ended_at, 220);
        assert_eq!(rows[1].kind, IntervalKind::Open);
        assert_eq!(rows[1].started_at, 150);
        assert_eq!(rows[1].ended_at, 225);
    }

    #[test]
    fn title_totals_by_app_keeps_each_apps_top_titles() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let ghostty_alpha = storage
            .start_interval(
                IntervalKind::Focused,
                "ghostty",
                Some("0x1"),
                Some("alpha"),
                100,
            )
            .unwrap();
        storage.close_interval(ghostty_alpha, 160).unwrap();
        let ghostty_beta = storage
            .start_interval(
                IntervalKind::Focused,
                "ghostty",
                Some("0x2"),
                Some("beta"),
                170,
            )
            .unwrap();
        storage.close_interval(ghostty_beta, 220).unwrap();
        let firefox_docs = storage
            .start_interval(
                IntervalKind::Focused,
                "firefox",
                Some("0x3"),
                Some("docs"),
                110,
            )
            .unwrap();
        storage.close_interval(firefox_docs, 140).unwrap();

        let rows = storage
            .focused_title_totals_by_app_between(100, 240, 1)
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].app_class, "firefox");
        assert_eq!(rows[0].title, "docs");
        assert_eq!(rows[0].focused_seconds, 30);
        assert_eq!(rows[1].app_class, "ghostty");
        assert_eq!(rows[1].title, "alpha");
        assert_eq!(rows[1].focused_seconds, 60);
    }

    #[test]
    fn workspace_totals_use_focused_interval_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let code = storage
            .start_interval_with_metadata(
                IntervalKind::Focused,
                "ghostty",
                IntervalMetadata {
                    window_address: Some("0x1"),
                    workspace: Some("code"),
                    monitor: Some("0"),
                    ..IntervalMetadata::default()
                },
                100,
            )
            .unwrap();
        storage.close_interval(code, 220).unwrap();
        let chat = storage
            .start_interval_with_metadata(
                IntervalKind::Focused,
                "vesktop",
                IntervalMetadata {
                    window_address: Some("0x2"),
                    workspace: Some("chat"),
                    monitor: Some("0"),
                    ..IntervalMetadata::default()
                },
                150,
            )
            .unwrap();
        storage.close_interval(chat, 190).unwrap();
        let legacy = storage
            .start_interval(IntervalKind::Focused, "firefox", None, None, 100)
            .unwrap();
        storage.close_interval(legacy, 300).unwrap();

        let rows = storage
            .focused_workspace_totals_between(120, 240, 8)
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].workspace, "code");
        assert_eq!(rows[0].focused_seconds, 100);
        assert_eq!(rows[1].workspace, "chat");
        assert_eq!(rows[1].focused_seconds, 40);
    }

    #[test]
    fn app_workspace_totals_keep_app_affinity_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let ghostty_code = storage
            .start_interval_with_metadata(
                IntervalKind::Focused,
                "ghostty",
                IntervalMetadata {
                    window_address: Some("0x1"),
                    workspace: Some("code"),
                    monitor: Some("0"),
                    ..IntervalMetadata::default()
                },
                100,
            )
            .unwrap();
        storage.close_interval(ghostty_code, 220).unwrap();
        let ghostty_chat = storage
            .start_interval_with_metadata(
                IntervalKind::Focused,
                "ghostty",
                IntervalMetadata {
                    window_address: Some("0x2"),
                    workspace: Some("chat"),
                    monitor: Some("0"),
                    ..IntervalMetadata::default()
                },
                200,
            )
            .unwrap();
        storage.close_interval(ghostty_chat, 260).unwrap();
        let firefox_code = storage
            .start_interval_with_metadata(
                IntervalKind::Focused,
                "firefox",
                IntervalMetadata {
                    window_address: Some("0x3"),
                    workspace: Some("code"),
                    monitor: Some("0"),
                    ..IntervalMetadata::default()
                },
                120,
            )
            .unwrap();
        storage.close_interval(firefox_code, 180).unwrap();
        let legacy = storage
            .start_interval(IntervalKind::Focused, "legacy", None, None, 100)
            .unwrap();
        storage.close_interval(legacy, 300).unwrap();

        let rows = storage
            .focused_app_workspace_totals_between(150, 240, 8)
            .unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].workspace, "code");
        assert_eq!(rows[0].app_class, "ghostty");
        assert_eq!(rows[0].focused_seconds, 70);
        assert_eq!(rows[1].workspace, "chat");
        assert_eq!(rows[1].app_class, "ghostty");
        assert_eq!(rows[1].focused_seconds, 40);
        assert_eq!(rows[2].workspace, "code");
        assert_eq!(rows[2].app_class, "firefox");
        assert_eq!(rows[2].focused_seconds, 30);
    }

    #[test]
    fn focused_rollups_handle_high_volume_with_one_bounded_scan() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        for index in 0..12_000 {
            let started_at = 1_700_000_000 + index * 120;
            let app_class = format!("app-{}", index % 24);
            let workspace = format!("ws-{}", index % 6);
            let id = storage
                .start_interval_with_metadata(
                    IntervalKind::Focused,
                    &app_class,
                    IntervalMetadata {
                        workspace: Some(&workspace),
                        ..IntervalMetadata::default()
                    },
                    started_at,
                )
                .unwrap();
            storage.close_interval(id, started_at + 90).unwrap();
        }

        let started = Instant::now();
        let rollups = storage
            .focused_rollups_between(1_700_000_000, 1_700_000_000 + 12_000 * 120, 8, 64)
            .unwrap();

        assert!(started.elapsed() < StdDuration::from_secs(5));
        assert_eq!(rollups.focus_intervals.len(), 12_000);
        assert!(!rollups.daily_apps.is_empty());
        assert_eq!(rollups.heatmap.len(), 7 * 24);
        assert_eq!(rollups.workspaces.len(), 6);
        assert!(!rollups.app_workspaces.is_empty());
    }

    #[test]
    fn report_indexes_are_available_to_bounded_interval_queries() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let plan = explain_plan(
            &storage.conn,
            "
            EXPLAIN QUERY PLAN
            SELECT app_class
            FROM intervals
            WHERE kind = 'focused'
              AND started_at < 200
              AND COALESCE(ended_at, 200) > 100
            ",
        );

        assert!(
            plan.iter()
                .any(|detail| detail.contains("idx_intervals_kind_time")),
            "{plan:?}"
        );
    }

    #[test]
    fn raw_export_includes_local_timestamps_and_system_gaps() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let focused = storage
            .start_interval_with_metadata(
                IntervalKind::Focused,
                "ghostty",
                IntervalMetadata {
                    title: Some("Plan"),
                    workspace: Some("code"),
                    ..IntervalMetadata::default()
                },
                100,
            )
            .unwrap();
        storage.close_interval(focused, 200).unwrap();
        let idle = storage
            .start_session_interval(SessionIntervalKind::Idle, Some("test"), 150)
            .unwrap();
        storage.close_session_interval(idle, 180).unwrap();
        let sleep = storage
            .start_system_interval(SystemIntervalKind::Sleep, Some("logind"), 160)
            .unwrap();
        storage.close_system_interval(sleep, 190).unwrap();

        let raw = storage.raw_export_between(0, 300).unwrap();

        assert_eq!(raw.intervals.len(), 1);
        assert!(!raw.intervals[0].local_start.is_empty());
        assert_eq!(raw.session_intervals.len(), 1);
        assert_eq!(raw.system_intervals.len(), 1);
        assert_eq!(raw.system_intervals[0].kind, SystemIntervalKind::Sleep);
    }

    #[test]
    fn purge_before_deletes_old_rows_and_trims_overlaps() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let mut storage = Storage::open(Some(&db), &config).unwrap();

        let old = storage
            .start_interval(IntervalKind::Focused, "old", None, None, 100)
            .unwrap();
        storage.close_interval(old, 150).unwrap();
        let overlap = storage
            .start_interval(IntervalKind::Focused, "overlap", None, None, 100)
            .unwrap();
        storage.close_interval(overlap, 300).unwrap();
        let system = storage
            .start_system_interval(SystemIntervalKind::Unobserved, Some("test"), 100)
            .unwrap();
        storage.close_system_interval(system, 150).unwrap();

        let report = storage.purge_before(Some(200), false, false).unwrap();

        assert_eq!(report.intervals_deleted, 1);
        assert_eq!(report.intervals_trimmed, 1);
        assert_eq!(report.system_intervals_deleted, 1);
        let rows = storage.totals_between(0, 400).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].app_class, "overlap");
        assert_eq!(rows[0].focused_seconds, 100);
    }

    #[test]
    fn read_only_open_reads_existing_schema_without_migration() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        {
            let storage = Storage::open(Some(&db), &config).unwrap();
            let focused = storage
                .start_interval(IntervalKind::Focused, "firefox", None, None, 100)
                .unwrap();
            storage.close_interval(focused, 160).unwrap();
        }

        let storage = Storage::open_read_only(Some(&db), &config).unwrap();
        let rows = storage.totals_between(100, 200).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].focused_seconds, 60);
        assert!(
            storage
                .start_interval(IntervalKind::Focused, "firefox", None, None, 200)
                .is_err()
        );
    }

    #[test]
    fn read_only_open_requires_existing_database() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("missing.db");
        let config = Config::default();

        let error = match Storage::open_read_only(Some(&db), &config) {
            Ok(_) => panic!("read-only open unexpectedly succeeded"),
            Err(error) => error,
        };
        let message = format!("{error:#}");

        assert!(message.contains("does not exist"), "{message}");
    }

    #[test]
    fn read_only_open_reports_unmigrated_schema() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("empty.db");
        let config = Config::default();
        drop(Connection::open(&db).unwrap());

        let error = match Storage::open_read_only(Some(&db), &config) {
            Ok(_) => panic!("read-only open unexpectedly succeeded"),
            Err(error) => error,
        };
        let message = format!("{error:#}");

        assert!(message.contains("needs migration"), "{message}");
    }

    #[test]
    fn read_only_open_reports_schema_with_unrecorded_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("legacy-columns.db");
        let config = Config::default();
        {
            let conn = Connection::open(&db).unwrap();
            create_legacy_usage_schema(&conn);
            conn.execute_batch(
                "
                ALTER TABLE intervals ADD COLUMN workspace TEXT;
                ALTER TABLE intervals ADD COLUMN monitor TEXT;
                ",
            )
            .unwrap();
        }

        let error = match Storage::open_read_only(Some(&db), &config) {
            Ok(_) => panic!("read-only open unexpectedly succeeded"),
            Err(error) => error,
        };
        let message = format!("{error:#}");

        assert!(message.contains("needs migration"), "{message}");
        assert!(message.contains("missing migration 3"), "{message}");
    }

    #[test]
    fn diagnose_reports_missing_database_without_creating_it() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("missing.db");

        let diagnostic = Storage::diagnose(Some(&db));

        assert_eq!(diagnostic.path, db);
        assert!(!diagnostic.exists);
        assert!(matches!(
            diagnostic.schema_status,
            StorageSchemaStatus::Missing
        ));
        assert!(matches!(
            diagnostic.quick_check,
            StorageQuickCheck::Skipped(_)
        ));
        assert!(!diagnostic.path.exists());
    }

    #[test]
    fn diagnose_reports_empty_database_as_not_initialized() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("empty.db");
        drop(Connection::open(&db).unwrap());

        let diagnostic = Storage::diagnose(Some(&db));

        assert!(diagnostic.exists);
        assert!(matches!(
            diagnostic.schema_status,
            StorageSchemaStatus::NotInitialized { .. }
        ));
        assert_eq!(diagnostic.quick_check, StorageQuickCheck::Ok);
    }

    #[test]
    fn diagnose_reports_current_schema() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("current.db");
        let config = Config::default();
        drop(Storage::open(Some(&db), &config).unwrap());

        let diagnostic = Storage::diagnose(Some(&db));

        assert!(diagnostic.exists);
        assert_eq!(diagnostic.quick_check, StorageQuickCheck::Ok);
        assert_eq!(
            diagnostic.schema_status,
            StorageSchemaStatus::Current {
                applied_migrations: vec![1, 2, 3, 4, 5, 6, 7, 8]
            }
        );
    }

    #[test]
    fn diagnose_reports_migration_need() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("legacy.db");
        {
            let conn = Connection::open(&db).unwrap();
            create_legacy_usage_schema(&conn);
        }

        let diagnostic = Storage::diagnose(Some(&db));

        assert!(diagnostic.exists);
        assert_eq!(
            diagnostic.schema_status,
            StorageSchemaStatus::NeedsMigration {
                version: 3,
                description: "add interval workspace metadata".to_string(),
                applied_migrations: vec![1, 2]
            }
        );
    }

    #[test]
    fn daemon_lifecycle_records_start_heartbeat_and_clean_stop() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let mut storage = Storage::open(Some(&db), &config).unwrap();

        let started = storage.start_daemon_run(100).unwrap();
        assert_eq!(started.recovery, None);
        storage
            .record_daemon_heartbeat(started.run_id, 130)
            .unwrap();
        storage.finish_daemon_run(started.run_id, 160).unwrap();

        let events = storage
            .conn
            .prepare("SELECT kind, occurred_at FROM daemon_events ORDER BY id ASC")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            events,
            vec![
                ("start".to_string(), 100),
                ("heartbeat".to_string(), 130),
                ("clean-stop".to_string(), 160)
            ]
        );

        let run = storage
            .conn
            .query_row(
                "SELECT last_heartbeat_at, stopped_at, stop_kind FROM daemon_runs WHERE id = ?1",
                params![started.run_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(run, (160, Some(160), Some("clean".to_string())));
    }

    #[test]
    fn daemon_recovery_closes_stale_intervals_at_last_observed_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let mut storage = Storage::open(Some(&db), &config).unwrap();

        let first = storage.start_daemon_run(100).unwrap();
        storage.record_daemon_heartbeat(first.run_id, 150).unwrap();
        let open = storage
            .start_interval(IntervalKind::Open, "code", None, None, 110)
            .unwrap();
        let idle = storage
            .start_session_interval(SessionIntervalKind::Idle, Some("test"), 140)
            .unwrap();
        let focused = storage
            .start_interval(IntervalKind::Focused, "code", None, None, 170)
            .unwrap();

        let second = storage.start_daemon_run(400).unwrap();

        assert_ne!(second.run_id, first.run_id);
        assert_eq!(
            second.recovery,
            Some(DaemonRecovery {
                previous_run_id: Some(first.run_id),
                closed_at: 170,
                unobserved_seconds: 230,
            })
        );
        assert_eq!(
            storage
                .conn
                .query_row(
                    "SELECT ended_at FROM intervals WHERE id = ?1",
                    params![open],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            170
        );
        assert_eq!(
            storage
                .conn
                .query_row(
                    "SELECT ended_at FROM intervals WHERE id = ?1",
                    params![focused],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            170
        );
        assert_eq!(
            storage
                .conn
                .query_row(
                    "SELECT ended_at FROM session_intervals WHERE id = ?1",
                    params![idle],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            170
        );

        let rows = storage.totals_between(100, 400).unwrap();
        assert_eq!(rows[0].open_seconds, 60);
        assert_eq!(rows[0].focused_seconds, 0);
        let totals = storage.session_totals_between(100, 400).unwrap();
        assert_eq!(totals.idle_seconds, 30);
        assert_eq!(totals.unobserved_seconds, 230);
    }

    #[test]
    fn daemon_recovery_closes_active_sleep_as_sleep_not_unobserved() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let mut storage = Storage::open(Some(&db), &config).unwrap();

        let first = storage.start_daemon_run(100).unwrap();
        storage.record_daemon_heartbeat(first.run_id, 150).unwrap();
        let sleep = storage
            .start_system_interval(SystemIntervalKind::Sleep, Some("logind"), 180)
            .unwrap();

        let second = storage.start_daemon_run(400).unwrap();

        assert_eq!(
            second.recovery,
            Some(DaemonRecovery {
                previous_run_id: Some(first.run_id),
                closed_at: 400,
                unobserved_seconds: 0,
            })
        );
        assert_eq!(
            storage
                .conn
                .query_row(
                    "SELECT ended_at FROM unobserved_intervals WHERE id = ?1",
                    params![sleep],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            400
        );
        let totals = storage.session_totals_between(100, 450).unwrap();
        assert_eq!(totals.sleep_seconds, 220);
        assert_eq!(totals.unobserved_seconds, 0);
    }

    #[test]
    fn system_timeline_between_returns_bounded_sleep_and_offline_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let sleep = storage
            .start_system_interval(SystemIntervalKind::Sleep, Some("logind"), 100)
            .unwrap();
        storage.close_system_interval(sleep, 220).unwrap();
        let unobserved = storage
            .start_system_interval(SystemIntervalKind::Unobserved, Some("daemon-recovery"), 260)
            .unwrap();
        storage.close_system_interval(unobserved, 420).unwrap();

        let rows = storage.system_timeline_between(150, 360).unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, SystemIntervalKind::Sleep);
        assert_eq!(rows[0].source.as_deref(), Some("logind"));
        assert_eq!(rows[0].started_at, 150);
        assert_eq!(rows[0].ended_at, 220);
        assert_eq!(rows[1].kind, SystemIntervalKind::Unobserved);
        assert_eq!(rows[1].source.as_deref(), Some("daemon-recovery"));
        assert_eq!(rows[1].started_at, 260);
        assert_eq!(rows[1].ended_at, 360);
    }

    #[test]
    fn daily_totals_include_unobserved_time() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let date = Local::now().date_naive();
        let day_start = Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap();
        let started_at = (day_start + chrono::Duration::hours(1)).timestamp();
        let ended_at = (day_start + chrono::Duration::hours(2)).timestamp();

        storage
            .conn
            .execute(
                "
                INSERT INTO unobserved_intervals(kind, source, started_at, ended_at)
                VALUES ('unobserved', 'test', ?1, ?2)
                ",
                params![started_at, ended_at],
            )
            .unwrap();
        let sleep = storage
            .start_system_interval(SystemIntervalKind::Sleep, Some("test"), ended_at)
            .unwrap();
        storage
            .close_system_interval(sleep, ended_at + 30 * 60)
            .unwrap();

        let days = storage
            .daily_totals_for_local_dates(date, 1, ended_at)
            .unwrap();
        assert_eq!(days[0].elapsed_seconds, 2 * 3600);
        assert_eq!(days[0].observed_seconds, 3600);
        assert_eq!(days[0].unobserved_seconds, 3600);
        assert_eq!(days[0].sleep_seconds, 0);
        let days = storage
            .daily_totals_for_local_dates(date, 1, ended_at + 30 * 60)
            .unwrap();
        assert_eq!(days[0].elapsed_seconds, 2 * 3600 + 30 * 60);
        assert_eq!(days[0].observed_seconds, 3600 + 30 * 60);
        assert_eq!(days[0].unobserved_seconds, 3600);
        assert_eq!(days[0].sleep_seconds, 1800);
        let totals = storage
            .session_totals_between(started_at, ended_at + 30 * 60)
            .unwrap();
        assert_eq!(totals.unobserved_seconds, 3600);
        assert_eq!(totals.sleep_seconds, 1800);
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

    #[test]
    fn daily_totals_include_session_idle_time() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let date = Local::now().date_naive();
        let day_start = Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap();
        let started_at = (day_start + chrono::Duration::hours(1)).timestamp();
        let ended_at = (day_start + chrono::Duration::hours(2)).timestamp();

        let idle = storage
            .start_session_interval(SessionIntervalKind::Idle, Some("test"), started_at)
            .unwrap();
        storage.close_session_interval(idle, ended_at).unwrap();

        let days = storage
            .daily_totals_for_local_dates(date, 1, ended_at)
            .unwrap();
        assert_eq!(days[0].idle_seconds, 3600);
        assert_eq!(days[0].locked_seconds, 0);
    }

    #[test]
    fn aggregates_session_idle_and_locked_intervals() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();

        let idle = storage
            .start_session_interval(SessionIntervalKind::Idle, Some("test"), 100)
            .unwrap();
        storage.close_session_interval(idle, 200).unwrap();
        let locked = storage
            .start_session_interval(SessionIntervalKind::Locked, Some("test"), 150)
            .unwrap();
        storage.close_session_interval(locked, 250).unwrap();

        let totals = storage.session_totals_between(125, 225).unwrap();
        assert_eq!(totals.idle_seconds, 75);
        assert_eq!(totals.locked_seconds, 75);
    }

    #[test]
    fn repairs_existing_classes_and_missing_titles() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let mut config = Config::default();
        config.privacy.title_capture = TitleCapture::All;
        let mut storage = Storage::open(Some(&db), &config).unwrap();

        let discord = storage
            .start_interval(
                IntervalKind::Focused,
                "chrome-discord.com__channels_@me-Default",
                None,
                None,
                100,
            )
            .unwrap();
        storage.close_interval(discord, 120).unwrap();
        let ghostty = storage
            .start_interval(
                IntervalKind::Focused,
                "com.mitchellh.ghostty",
                None,
                None,
                130,
            )
            .unwrap();
        storage.close_interval(ghostty, 150).unwrap();

        let mut steam = SteamResolver::default();
        let repair = storage.repair_titles(&mut steam, &config, false).unwrap();

        assert_eq!(repair.rewritten_rows, 1);
        assert_eq!(repair.filled_titles, 2);

        let rows = storage
            .conn
            .prepare(
                "
                SELECT app_class, title
                FROM intervals
                WHERE kind = 'focused'
                ORDER BY app_class ASC
                ",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            rows,
            vec![
                ("com.mitchellh.ghostty".to_string(), "Ghostty".to_string()),
                ("discord".to_string(), "Discord".to_string())
            ]
        );
    }

    #[test]
    fn repair_titles_does_not_fill_titles_when_capture_is_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let mut storage = Storage::open(Some(&db), &config).unwrap();

        let interval = storage
            .start_interval(
                IntervalKind::Focused,
                "chrome-discord.com__channels_@me-Default",
                None,
                None,
                100,
            )
            .unwrap();
        storage.close_interval(interval, 120).unwrap();

        let mut steam = SteamResolver::default();
        let repair = storage.repair_titles(&mut steam, &config, false).unwrap();

        assert_eq!(repair.rewritten_rows, 1);
        assert_eq!(repair.filled_titles, 0);
        let (app_class, title) = storage
            .conn
            .query_row(
                "SELECT app_class, title FROM intervals WHERE kind = 'focused'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .unwrap();

        assert_eq!(app_class, "discord");
        assert_eq!(title, None);
    }
}
