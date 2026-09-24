use super::Storage;
use crate::config::Config;
use anyhow::Result;
use rusqlite::OptionalExtension;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TrackingStatus {
    pub checked_at: i64,
    pub tracker: &'static str,
    pub last_heartbeat_at: Option<i64>,
    pub browser: &'static str,
    pub last_browser_report_at: Option<i64>,
}

impl Storage {
    /// Read current health without scanning activity history or running analytics.
    pub fn tracking_status(&self, config: &Config, now: i64) -> Result<TrackingStatus> {
        let run: Option<(i64, Option<i64>)> = self
            .conn
            .query_row(
                "SELECT last_heartbeat_at, stopped_at FROM daemon_runs ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (gap, sleep, locked, idle): (bool, bool, bool, bool) = self.conn.query_row(
            "SELECT
                EXISTS(SELECT 1 FROM unobserved_intervals WHERE ended_at IS NULL AND kind = 'unobserved'),
                EXISTS(SELECT 1 FROM unobserved_intervals WHERE ended_at IS NULL AND kind = 'sleep'),
                EXISTS(SELECT 1 FROM session_intervals WHERE ended_at IS NULL AND kind = 'locked'),
                EXISTS(SELECT 1 FROM session_intervals WHERE ended_at IS NULL AND kind = 'idle')",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let grace = i64::try_from(config.tracking.heartbeat_seconds.max(15).saturating_mul(3))
            .unwrap_or(i64::MAX);
        let tracker = match run {
            None => "never-seen",
            Some((_, Some(_))) => "stopped",
            Some((heartbeat, None)) if now.saturating_sub(heartbeat) > grace => "stale",
            _ if gap => "disconnected",
            _ if sleep => "sleeping",
            _ if locked => "locked",
            _ if idle => "idle",
            _ => "reporting",
        };
        let last_browser_report_at: Option<i64> = self.conn.query_row(
            "SELECT MAX(updated_at) FROM browser_activity_state",
            [],
            |row| row.get(0),
        )?;
        let browser = match last_browser_report_at {
            _ if !config.privacy.browser_domains => "disabled",
            None => "never-seen",
            Some(at) if now.saturating_sub(at) > 90 => "stale",
            _ => "reporting",
        };
        Ok(TrackingStatus {
            checked_at: now,
            tracker,
            last_heartbeat_at: run.map(|(at, _)| at),
            browser,
            last_browser_report_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{SessionIntervalKind, StorageOpenMode};

    #[test]
    fn distinguishes_idle_stale_stopped_and_browser_silence() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.db");
        let mut config = Config::default();
        let mut storage =
            Storage::open_with_mode(Some(&path), &config, StorageOpenMode::ReadWriteMigrate)
                .unwrap();
        let status = storage.tracking_status(&config, 1000).unwrap();
        assert_eq!(status.tracker, "never-seen");
        assert_eq!(status.browser, "never-seen");
        let run = storage.start_daemon_run(1000).unwrap();
        assert_eq!(
            storage.tracking_status(&config, 1000).unwrap().tracker,
            "reporting"
        );
        storage
            .start_session_interval(SessionIntervalKind::Idle, None, 1001)
            .unwrap();
        assert_eq!(
            storage.tracking_status(&config, 1001).unwrap().tracker,
            "idle"
        );
        let locked = storage
            .start_session_interval(SessionIntervalKind::Locked, None, 1001)
            .unwrap();
        assert_eq!(
            storage.tracking_status(&config, 1001).unwrap().tracker,
            "locked"
        );
        storage.close_session_interval(locked, 1001).unwrap();
        let grace = config.tracking.heartbeat_seconds.max(15) * 3;
        assert_eq!(
            storage
                .tracking_status(&config, 1000 + grace as i64)
                .unwrap()
                .tracker,
            "idle"
        );
        assert_eq!(
            storage
                .tracking_status(&config, 1001 + grace as i64)
                .unwrap()
                .tracker,
            "stale"
        );
        storage.finish_daemon_run(run.run_id, 1002).unwrap();
        let status = storage.tracking_status(&config, 1003).unwrap();
        assert_eq!(status.tracker, "stopped");
        assert_eq!(status.last_heartbeat_at, Some(1002));
        // Clear-domain messages also demonstrate a working browser connection.
        storage
            .record_browser_state("omastat-firefox", "firefox", None, 1000)
            .unwrap();
        assert_eq!(
            storage.tracking_status(&config, 1090).unwrap().browser,
            "reporting"
        );
        assert_eq!(
            storage.tracking_status(&config, 1091).unwrap().browser,
            "stale"
        );
        config.privacy.browser_domains = false;
        assert_eq!(
            storage.tracking_status(&config, 1000).unwrap().browser,
            "disabled"
        );
        drop(storage);
        let readonly =
            Storage::open_with_mode(Some(&path), &config, StorageOpenMode::ReadOnly).unwrap();
        assert_eq!(
            readonly.tracking_status(&config, 1003).unwrap().tracker,
            "stopped"
        );
    }

    #[test]
    fn fresh_heartbeat_does_not_hide_a_desktop_outage() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("test.db");
        let config = Config::default();
        let mut storage =
            Storage::open_with_mode(Some(&path), &config, StorageOpenMode::ReadWriteMigrate)
                .unwrap();
        let run = storage.start_daemon_run(1000).unwrap();
        let gap = storage.begin_observation_gap(1001).unwrap();
        storage.record_daemon_heartbeat(run.run_id, 1002).unwrap();
        assert_eq!(
            storage.tracking_status(&config, 1002).unwrap().tracker,
            "disconnected"
        );
        storage.close_system_interval(gap, 1003).unwrap();
        assert_eq!(
            storage.tracking_status(&config, 1003).unwrap().tracker,
            "reporting"
        );
    }
}
