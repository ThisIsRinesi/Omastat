use crate::{config::Config, identity, steam::SteamResolver};
use anyhow::{Context, Result};
use chrono::{Datelike, Local, TimeZone, Timelike};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
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

#[derive(Debug, Clone, Serialize)]
pub struct DayTotals {
    pub date: String,
    pub label: String,
    pub focused_seconds: i64,
    pub open_seconds: i64,
    pub idle_seconds: i64,
    pub locked_seconds: i64,
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

#[derive(Debug, Clone, Default)]
pub struct StorageStatus {
    pub interval_count: i64,
    pub last_event_at: Option<i64>,
    pub focused_active: i64,
    pub open_active: i64,
    pub idle_active: i64,
    pub locked_active: i64,
}

#[derive(Debug, Clone)]
pub struct ActiveInterval {
    pub id: i64,
    pub kind: IntervalKind,
    pub app_class: String,
    pub window_address: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIntervalKind {
    Idle,
    Locked,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionTotals {
    pub idle_seconds: i64,
    pub locked_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct ActiveSessionInterval {
    pub id: i64,
    pub kind: SessionIntervalKind,
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
        self.conn.execute(
            "UPDATE session_intervals
             SET ended_at = ?1
             WHERE id = ?2 AND ended_at IS NULL",
            params![ended_at, id],
        )?;
        Ok(())
    }

    pub fn close_session_intervals(&self, ended_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE session_intervals SET ended_at = ?1 WHERE ended_at IS NULL",
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
        let mut output = (0..days)
            .map(|offset| {
                let start = range_start + chrono::Duration::days(offset as i64);
                DayTotals {
                    date: start.format("%Y-%m-%d").to_string(),
                    label: start.format("%a").to_string(),
                    focused_seconds: 0,
                    open_seconds: 0,
                    idle_seconds: 0,
                    locked_seconds: 0,
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

        Ok(status)
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

    pub fn focused_app_daily_totals_between(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Vec<AppDayTotals>> {
        if end <= start {
            return Ok(Vec::new());
        }

        let (range_start, days) = local_day_range(start, end)?;
        let boundaries = (0..=days)
            .map(|offset| (range_start + chrono::Duration::days(offset as i64)).timestamp())
            .collect::<Vec<_>>();
        let labels = (0..days)
            .map(|offset| {
                let day = range_start + chrono::Duration::days(offset as i64);
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

        for (_, mut cursor, interval_end) in self.focused_intervals_between(start, end)? {
            while cursor < interval_end {
                let Some(local) = Local.timestamp_opt(cursor, 0).single() else {
                    break;
                };
                let Some(hour_start) = Local
                    .with_ymd_and_hms(local.year(), local.month(), local.day(), local.hour(), 0, 0)
                    .single()
                else {
                    break;
                };
                let next_hour = (hour_start + chrono::Duration::hours(1)).timestamp();
                let segment_end = next_hour.min(interval_end);
                let overlap = segment_end - cursor;
                if overlap > 0 {
                    let key = (local.weekday().num_days_from_monday(), local.hour());
                    *totals.entry(key).or_default() += overlap;
                }
                cursor = segment_end.max(cursor + 1);
            }
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
        Ok(totals)
    }

    pub fn repair_titles(
        &mut self,
        steam: &mut SteamResolver,
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

        let mut title_counts = BTreeMap::<String, i64>::new();
        for (app_class, rows) in self.missing_focused_title_counts()? {
            let app_class = class_update_map
                .get(&app_class)
                .cloned()
                .unwrap_or(app_class);
            *title_counts.entry(app_class).or_default() += rows;
        }

        let title_updates = title_counts
            .into_iter()
            .map(|(app_class, rows)| TitleFill {
                title: identity::display_name(&app_class),
                app_class,
                rows,
            })
            .collect::<Vec<_>>();
        let title_normalizations = self.planned_title_normalizations(&class_update_map)?;
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
            INSERT OR IGNORE INTO schema_migrations(version, applied_at)
                VALUES (2, unixepoch());
            ",
        )?;
        Ok(())
    }

    fn validate_schema(&self) -> Result<()> {
        for statement in [
            "SELECT id, kind, app_class, window_address, title, started_at, ended_at FROM intervals LIMIT 0",
            "SELECT id, kind, source, started_at, ended_at FROM session_intervals LIMIT 0",
        ] {
            self.conn.prepare(statement).with_context(|| {
                format!(
                    "database {} is not initialized or needs migration; start omastatd once with write access before running read-only reports",
                    self.path.display()
                )
            })?;
        }
        Ok(())
    }

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
            WHERE kind = 'focused'
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

fn local_day_range(start: i64, end: i64) -> Result<(chrono::DateTime<Local>, usize)> {
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
    let range_start = Local
        .from_local_datetime(
            &start_date
                .and_hms_opt(0, 0, 0)
                .context("invalid local range start")?,
        )
        .single()
        .context("failed to compute local range start")?;
    let days = (end_date - start_date).num_days().max(0) as usize + 1;
    Ok((range_start, days))
}

#[cfg(test)]
mod tests {
    use super::{IntervalKind, SessionIntervalKind, Storage};
    use crate::config::Config;
    use crate::steam::SteamResolver;
    use chrono::{Datelike, Local, TimeZone};
    use rusqlite::Connection;

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
        let config = Config::default();
        let mut storage = Storage::open(Some(&db), &config).unwrap();

        storage
            .start_interval(
                IntervalKind::Focused,
                "chrome-discord.com__channels_@me-Default",
                None,
                None,
                100,
            )
            .unwrap();
        storage
            .start_interval(
                IntervalKind::Focused,
                "com.mitchellh.ghostty",
                None,
                None,
                100,
            )
            .unwrap();

        let mut steam = SteamResolver::default();
        let repair = storage.repair_titles(&mut steam, false).unwrap();

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
}
