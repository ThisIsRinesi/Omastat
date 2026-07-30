use crate::{
    config::Config,
    hyprland::{self, Event, EventStream, Snapshot, Window},
    storage::{IntervalKind, Storage},
};
use anyhow::Result;
use std::collections::HashMap;
use tokio::time::{self, Duration};
use tracing::{debug, info, warn};

pub struct Tracker {
    storage: Storage,
    config: Config,
    state: TrackerState,
}

#[derive(Default)]
struct TrackerState {
    windows: HashMap<String, Window>,
    app_open_counts: HashMap<String, usize>,
    open_interval_ids: HashMap<String, i64>,
    focused: Option<FocusedInterval>,
}

struct FocusedInterval {
    address: String,
    app_class: String,
    interval_id: i64,
}

impl Tracker {
    pub fn new(storage: Storage, config: Config) -> Self {
        Self {
            storage,
            config,
            state: TrackerState::default(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let now = unix_now();
        self.storage.close_open_intervals(now)?;
        self.reconcile().await?;

        let mut reconnect_backoff = Duration::from_secs(1);
        let mut reconcile_timer = time::interval(Duration::from_secs(
            self.config.tracking.reconcile_seconds.max(30),
        ));

        loop {
            let mut stream = match EventStream::connect().await {
                Ok(stream) => {
                    reconnect_backoff = Duration::from_secs(1);
                    stream
                }
                Err(error) => {
                    warn!("failed to connect Hyprland event stream: {error:#}");
                    time::sleep(reconnect_backoff).await;
                    reconnect_backoff = (reconnect_backoff * 2).min(Duration::from_secs(30));
                    continue;
                }
            };

            info!("tracking Hyprland usage");
            loop {
                tokio::select! {
                    event = stream.next_event() => {
                        match event {
                            Ok(Some(event)) => self.apply_event(event).await?,
                            Ok(None) => {
                                warn!("Hyprland event stream closed");
                                self.reconcile().await?;
                                break;
                            }
                            Err(error) => {
                                warn!("Hyprland event stream error: {error:#}");
                                self.reconcile().await?;
                                break;
                            }
                        }
                    }
                    _ = reconcile_timer.tick() => {
                        self.reconcile().await?;
                    }
                    _ = tokio::signal::ctrl_c() => {
                        self.shutdown()?;
                        return Ok(());
                    }
                    _ = terminate_signal() => {
                        self.shutdown()?;
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn apply_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::WindowOpened(window) => self.open_window(window)?,
            Event::WindowClosed { address } => self.close_window(&address)?,
            Event::FocusChanged { address } => {
                if let Some(address) = address {
                    if !self.state.windows.contains_key(&address) {
                        self.reconcile().await?;
                    }
                    self.focus_window(Some(&address))?;
                } else {
                    self.focus_window(None)?;
                }
            }
            Event::Unknown => {}
        }
        Ok(())
    }

    async fn reconcile(&mut self) -> Result<()> {
        match hyprland::snapshot().await {
            Ok(snapshot) => self.apply_snapshot(snapshot),
            Err(error) => {
                warn!("snapshot reconciliation failed: {error:#}");
                Ok(())
            }
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) -> Result<()> {
        let live_addresses = snapshot
            .windows
            .iter()
            .map(|window| window.address.clone())
            .collect::<std::collections::HashSet<_>>();

        for address in self.state.windows.keys().cloned().collect::<Vec<_>>() {
            if !live_addresses.contains(&address) {
                self.close_window(&address)?;
            }
        }

        for window in snapshot.windows {
            if !self.state.windows.contains_key(&window.address) {
                self.open_window(window)?;
            } else {
                self.state.windows.insert(window.address.clone(), window);
            }
        }

        self.focus_window(snapshot.active_address.as_deref())?;
        Ok(())
    }

    fn open_window(&mut self, window: Window) -> Result<()> {
        if self.state.windows.contains_key(&window.address) {
            self.close_window(&window.address)?;
        }

        let app_class = window.class.clone();
        debug!("window opened: {} {}", app_class, window.address);
        self.state.windows.insert(window.address.clone(), window);

        let count = self
            .state
            .app_open_counts
            .entry(app_class.clone())
            .or_default();
        *count += 1;
        if *count == 1 {
            let id = self.storage.start_interval(
                IntervalKind::Open,
                &app_class,
                None,
                None,
                unix_now(),
            )?;
            self.state.open_interval_ids.insert(app_class, id);
        }
        Ok(())
    }

    fn close_window(&mut self, address: &str) -> Result<()> {
        let Some(window) = self.state.windows.remove(address) else {
            return Ok(());
        };

        if self
            .state
            .focused
            .as_ref()
            .is_some_and(|focused| focused.address == address)
        {
            self.focus_window(None)?;
        }

        let app_class = window.class;
        if let Some(count) = self.state.app_open_counts.get_mut(&app_class) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.state.app_open_counts.remove(&app_class);
                if let Some(interval_id) = self.state.open_interval_ids.remove(&app_class) {
                    self.storage.close_interval(interval_id, unix_now())?;
                }
            }
        }
        Ok(())
    }

    fn focus_window(&mut self, address: Option<&str>) -> Result<()> {
        let now = unix_now();

        if self
            .state
            .focused
            .as_ref()
            .is_some_and(|focused| Some(focused.address.as_str()) == address)
        {
            return Ok(());
        }

        if let Some(previous) = self.state.focused.take() {
            self.storage.close_interval(previous.interval_id, now)?;
        }

        let Some(address) = address else {
            return Ok(());
        };
        let Some(window) = self.state.windows.get(address) else {
            return Ok(());
        };

        let title = self
            .config
            .capture_titles()
            .then_some(window.title.as_deref())
            .flatten();
        let interval_id = self.storage.start_interval(
            IntervalKind::Focused,
            &window.class,
            Some(&window.address),
            title,
            now,
        )?;

        self.state.focused = Some(FocusedInterval {
            address: window.address.clone(),
            app_class: window.class.clone(),
            interval_id,
        });
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        let now = unix_now();
        if let Some(previous) = self.state.focused.take() {
            debug!("closing focused interval for {}", previous.app_class);
            self.storage.close_interval(previous.interval_id, now)?;
        }
        for (_, interval_id) in self.state.open_interval_ids.drain() {
            self.storage.close_interval(interval_id, now)?;
        }
        Ok(())
    }
}

fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(unix)]
async fn terminate_signal() {
    let Ok(mut signal) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    else {
        std::future::pending::<()>().await;
        return;
    };
    signal.recv().await;
}

#[cfg(not(unix))]
async fn terminate_signal() {
    std::future::pending::<()>().await
}

#[cfg(test)]
mod tests {
    use super::Tracker;
    use crate::{
        config::Config,
        hyprland::{Snapshot, Window},
        storage::Storage,
    };

    #[test]
    fn snapshot_starts_one_open_interval_per_app() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .apply_snapshot(Snapshot {
                active_address: None,
                windows: vec![
                    window("0x1", "firefox"),
                    window("0x2", "firefox"),
                    window("0x3", "code"),
                ],
            })
            .unwrap();

        assert_eq!(tracker.state.app_open_counts.get("firefox"), Some(&2));
        assert_eq!(tracker.state.app_open_counts.get("code"), Some(&1));
        assert_eq!(tracker.state.open_interval_ids.len(), 2);
    }

    #[test]
    fn focus_switch_replaces_focused_interval() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker.open_window(window("0x1", "firefox")).unwrap();
        tracker.open_window(window("0x2", "code")).unwrap();
        tracker.focus_window(Some("0x1")).unwrap();
        tracker.focus_window(Some("0x2")).unwrap();

        assert_eq!(
            tracker
                .state
                .focused
                .as_ref()
                .map(|focused| focused.address.as_str()),
            Some("0x2")
        );
    }

    #[test]
    fn duplicate_openwindow_does_not_inflate_open_count() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker.open_window(window("0x1", "firefox")).unwrap();
        tracker.open_window(window("0x1", "firefox")).unwrap();

        assert_eq!(tracker.state.app_open_counts.get("firefox"), Some(&1));
        assert_eq!(tracker.state.open_interval_ids.len(), 1);
    }

    fn window(address: &str, class: &str) -> Window {
        Window {
            address: address.to_string(),
            class: class.to_string(),
            initial_class: Some(class.to_string()),
            title: Some("title".to_string()),
            pid: Some(1),
        }
    }
}
