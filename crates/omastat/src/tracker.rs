use crate::{
    config::Config,
    hyprland::{self, Event, EventStream, Snapshot, Window},
    session,
    steam::SteamResolver,
    storage::{ActiveInterval, IntervalKind, Storage},
};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tokio::time::{self, Duration, MissedTickBehavior};
use tracing::{debug, info, warn};

pub struct Tracker {
    storage: Storage,
    config: Config,
    steam: SteamResolver,
    state: TrackerState,
}

#[derive(Default)]
struct TrackerState {
    windows: HashMap<String, Window>,
    app_open_counts: HashMap<String, usize>,
    open_interval_ids: HashMap<String, i64>,
    active_address: Option<String>,
    focused: Option<FocusedInterval>,
    focus_paused: bool,
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
            steam: SteamResolver::default(),
            state: TrackerState::default(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.recover_startup_state().await?;

        let mut reconnect_backoff = Duration::from_secs(1);
        self.refresh_session_status().await?;
        let ctrl_c = tokio::signal::ctrl_c();
        tokio::pin!(ctrl_c);
        let mut terminate = terminate_signal();

        let mut reconcile_timer = time::interval(Duration::from_secs(
            self.config.tracking.reconcile_seconds.max(30),
        ));
        reconcile_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        reconcile_timer.tick().await;
        let mut session_timer = time::interval(Duration::from_secs(
            self.config.tracking.session_poll_seconds.max(15),
        ));
        session_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        session_timer.tick().await;

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
                    _ = session_timer.tick() => {
                        self.refresh_session_status().await?;
                    }
                    _ = &mut ctrl_c => {
                        self.shutdown()?;
                        return Ok(());
                    }
                    _ = recv_terminate_signal(&mut terminate) => {
                        self.shutdown()?;
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn recover_startup_state(&mut self) -> Result<()> {
        let now = unix_now();
        match hyprland::snapshot().await {
            Ok(snapshot) => self.apply_startup_snapshot(snapshot, now),
            Err(error) => {
                warn!("startup snapshot failed; closing unverified intervals: {error:#}");
                self.storage.close_open_intervals(now)?;
                Ok(())
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
                    self.set_active_window(Some(&address))?;
                } else {
                    self.set_active_window(None)?;
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
            .collect::<HashSet<_>>();

        for address in self.state.windows.keys().cloned().collect::<Vec<_>>() {
            if !live_addresses.contains(&address) {
                self.close_window(&address)?;
            }
        }

        for mut window in snapshot.windows {
            window.class = self.steam.resolve_class(&window.class);
            if !self.state.windows.contains_key(&window.address) {
                self.open_window(window)?;
            } else {
                self.state.windows.insert(window.address.clone(), window);
            }
        }

        self.set_active_window(snapshot.active_address.as_deref())?;
        Ok(())
    }

    fn apply_startup_snapshot(&mut self, snapshot: Snapshot, now: i64) -> Result<()> {
        let stored = self.storage.unclosed_intervals()?;
        let mut windows = HashMap::new();
        let mut app_open_counts = HashMap::<String, usize>::new();

        for mut window in snapshot.windows {
            window.class = self.steam.resolve_class(&window.class);
            *app_open_counts.entry(window.class.clone()).or_default() += 1;
            windows.insert(window.address.clone(), window);
        }

        let active_address = snapshot
            .active_address
            .filter(|address| windows.contains_key(address));
        let mut stored_open = HashMap::<String, Vec<i64>>::new();
        let mut stored_focused = Vec::new();

        for interval in stored {
            match interval.kind {
                IntervalKind::Open => {
                    stored_open
                        .entry(interval.app_class)
                        .or_default()
                        .push(interval.id);
                }
                IntervalKind::Focused => stored_focused.push(interval),
            }
        }

        self.state = TrackerState::default();
        self.state.windows = windows;
        self.state.app_open_counts = app_open_counts;
        self.state.active_address = active_address;

        let live_app_classes = self
            .state
            .app_open_counts
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for app_class in live_app_classes {
            let Some(ids) = stored_open.remove(&app_class) else {
                let id =
                    self.storage
                        .start_interval(IntervalKind::Open, &app_class, None, None, now)?;
                self.state.open_interval_ids.insert(app_class.clone(), id);
                continue;
            };

            if let Some((keep, stale)) = ids.split_first() {
                self.state
                    .open_interval_ids
                    .insert(app_class.clone(), *keep);
                for id in stale {
                    self.storage.close_interval(*id, now)?;
                }
            }
        }

        for ids in stored_open.into_values() {
            for id in ids {
                self.storage.close_interval(id, now)?;
            }
        }

        self.restore_focused_interval(stored_focused, now)?;
        self.sync_focused_interval()
    }

    fn restore_focused_interval(&mut self, stored: Vec<ActiveInterval>, now: i64) -> Result<()> {
        let active = self
            .state
            .active_address
            .as_ref()
            .and_then(|address| self.state.windows.get(address))
            .map(|window| (window.address.clone(), window.class.clone()));
        let mut restored = false;

        for interval in stored {
            let matches_active = !restored
                && active.as_ref().is_some_and(|(address, app_class)| {
                    interval.window_address.as_deref() == Some(address.as_str())
                        && interval.app_class == *app_class
                });

            if matches_active {
                let (address, app_class) = active.as_ref().expect("active window checked above");
                self.state.focused = Some(FocusedInterval {
                    address: address.clone(),
                    app_class: app_class.clone(),
                    interval_id: interval.id,
                });
                restored = true;
            } else {
                self.storage.close_interval(interval.id, now)?;
            }
        }

        Ok(())
    }

    fn open_window(&mut self, mut window: Window) -> Result<()> {
        window.class = self.steam.resolve_class(&window.class);

        if self
            .state
            .windows
            .get(&window.address)
            .is_some_and(|existing| existing.class == window.class)
        {
            self.state.windows.insert(window.address.clone(), window);
            return Ok(());
        }

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
            .active_address
            .as_ref()
            .is_some_and(|active| active == address)
        {
            self.set_active_window(None)?;
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

    fn set_active_window(&mut self, address: Option<&str>) -> Result<()> {
        self.state.active_address = address.map(ToOwned::to_owned);
        self.sync_focused_interval()
    }

    fn set_focus_paused(&mut self, paused: bool) -> Result<()> {
        if self.state.focus_paused == paused {
            return Ok(());
        }
        self.state.focus_paused = paused;
        self.sync_focused_interval()
    }

    async fn refresh_session_status(&mut self) -> Result<()> {
        match session::status().await {
            Ok(status) => {
                let paused = status.should_pause(
                    self.config.tracking.pause_on_session_idle,
                    self.config.tracking.pause_on_session_locked,
                );
                self.set_focus_paused(paused)?;
            }
            Err(error) => {
                debug!("session status unavailable: {error:#}");
            }
        }
        Ok(())
    }

    fn sync_focused_interval(&mut self) -> Result<()> {
        let now = unix_now();
        let address = self.state.active_address.as_deref();

        if self
            .state
            .focused
            .as_ref()
            .is_some_and(|focused| Some(focused.address.as_str()) == address)
            && !self.state.focus_paused
        {
            return Ok(());
        }

        if let Some(previous) = self.state.focused.take() {
            self.storage.close_interval(previous.interval_id, now)?;
        }

        if self.state.focus_paused {
            return Ok(());
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
type TerminateSignal = tokio::signal::unix::Signal;

#[cfg(not(unix))]
struct TerminateSignal;

#[cfg(unix)]
fn terminate_signal() -> Option<TerminateSignal> {
    match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
        Ok(signal) => Some(signal),
        Err(error) => {
            warn!("failed to install SIGTERM handler: {error}");
            None
        }
    }
}

#[cfg(not(unix))]
fn terminate_signal() -> Option<TerminateSignal> {
    None
}

async fn recv_terminate_signal(signal: &mut Option<TerminateSignal>) {
    #[cfg(unix)]
    if let Some(signal) = signal {
        signal.recv().await;
        return;
    }

    std::future::pending::<()>().await;
}

#[cfg(test)]
mod tests {
    use super::Tracker;
    use crate::{
        config::Config,
        hyprland::{Snapshot, Window},
        storage::{IntervalKind, Storage},
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
    fn startup_snapshot_reuses_live_intervals_and_closes_stale() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let live_open = storage
            .start_interval(IntervalKind::Open, "firefox", None, None, 100)
            .unwrap();
        let stale_open = storage
            .start_interval(IntervalKind::Open, "code", None, None, 100)
            .unwrap();
        let live_focus = storage
            .start_interval(
                IntervalKind::Focused,
                "firefox",
                Some("0x1"),
                Some("title"),
                100,
            )
            .unwrap();
        let stale_focus = storage
            .start_interval(IntervalKind::Focused, "code", Some("0x2"), None, 100)
            .unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .apply_startup_snapshot(
                Snapshot {
                    active_address: Some("0x1".to_string()),
                    windows: vec![window("0x1", "firefox")],
                },
                200,
            )
            .unwrap();

        assert_eq!(
            tracker.state.open_interval_ids.get("firefox"),
            Some(&live_open)
        );
        assert_eq!(
            tracker
                .state
                .focused
                .as_ref()
                .map(|focused| focused.interval_id),
            Some(live_focus)
        );

        let unclosed_ids = tracker
            .storage
            .unclosed_intervals()
            .unwrap()
            .into_iter()
            .map(|interval| interval.id)
            .collect::<std::collections::HashSet<_>>();
        assert!(unclosed_ids.contains(&live_open));
        assert!(unclosed_ids.contains(&live_focus));
        assert!(!unclosed_ids.contains(&stale_open));
        assert!(!unclosed_ids.contains(&stale_focus));
        assert_eq!(unclosed_ids.len(), 2);
    }

    #[test]
    fn focus_switch_replaces_focused_interval() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker.open_window(window("0x1", "firefox")).unwrap();
        tracker.open_window(window("0x2", "code")).unwrap();
        tracker.set_active_window(Some("0x1")).unwrap();
        tracker.set_active_window(Some("0x2")).unwrap();

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

    #[test]
    fn duplicate_openwindow_preserves_focus_for_same_app() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker.open_window(window("0x1", "firefox")).unwrap();
        tracker.set_active_window(Some("0x1")).unwrap();
        tracker.open_window(window("0x1", "firefox")).unwrap();

        assert_eq!(
            tracker
                .state
                .focused
                .as_ref()
                .map(|focused| focused.address.as_str()),
            Some("0x1")
        );
        assert_eq!(tracker.state.app_open_counts.get("firefox"), Some(&1));
    }

    #[test]
    fn steam_app_classes_use_readable_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .open_window(window("0x1", "steam_app_999999999"))
            .unwrap();

        assert_eq!(
            tracker.state.app_open_counts.get("Steam App 999999999"),
            Some(&1)
        );
        assert!(
            !tracker
                .state
                .app_open_counts
                .contains_key("steam_app_999999999")
        );
    }

    #[test]
    fn session_pause_closes_and_resumes_focus_interval() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker.open_window(window("0x1", "firefox")).unwrap();
        tracker.set_active_window(Some("0x1")).unwrap();
        assert!(tracker.state.focused.is_some());

        tracker.set_focus_paused(true).unwrap();
        assert!(tracker.state.focused.is_none());

        tracker.set_focus_paused(false).unwrap();
        assert!(tracker.state.focused.is_some());
    }

    #[tokio::test]
    async fn fake_event_stream_drives_tracking_state() {
        struct FakeEventStream {
            events: std::vec::IntoIter<crate::hyprland::Event>,
        }

        impl FakeEventStream {
            fn new(events: Vec<crate::hyprland::Event>) -> Self {
                Self {
                    events: events.into_iter(),
                }
            }

            async fn next_event(&mut self) -> Option<crate::hyprland::Event> {
                self.events.next()
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);
        let mut stream = FakeEventStream::new(vec![
            crate::hyprland::Event::WindowOpened(window("0x1", "firefox")),
            crate::hyprland::Event::FocusChanged {
                address: Some("0x1".to_string()),
            },
            crate::hyprland::Event::WindowClosed {
                address: "0x1".to_string(),
            },
        ]);

        while let Some(event) = stream.next_event().await {
            tracker.apply_event(event).await.unwrap();
        }

        assert!(tracker.state.windows.is_empty());
        assert!(tracker.state.focused.is_none());
        assert!(tracker.state.open_interval_ids.is_empty());
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
