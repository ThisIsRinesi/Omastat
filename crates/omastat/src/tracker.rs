use crate::{
    clock,
    config::Config,
    hyprland::{self, Event, EventStream, Snapshot, Window},
    identity, session,
    steam::SteamResolver,
    storage::{
        IntervalKind, IntervalMetadata, SessionIntervalKind, SessionTransition, Storage,
        SystemIntervalKind,
    },
    terminal,
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
    daemon_run_id: Option<i64>,
    observing: bool,
    last_observed_at: Option<i64>,
    observation_started_at: Option<i64>,
    last_audio_at: Option<i64>,
    last_resume_at: Option<i64>,
    outage_interval_id: Option<i64>,
}

#[derive(Default)]
struct TrackerState {
    windows: HashMap<String, Window>,
    app_open_counts: HashMap<String, usize>,
    open_interval_ids: HashMap<String, i64>,
    active_address: Option<String>,
    focused: Option<FocusedInterval>,
    focus_paused: bool,
    session_pause: Option<SessionPauseInterval>,
    sleep_pause: Option<SleepPauseInterval>,
}

struct FocusedInterval {
    address: String,
    app_class: String,
    title: Option<String>,
    workspace: Option<String>,
    monitor: Option<String>,
    interval_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FocusTarget {
    address: String,
    app_class: String,
    title: Option<String>,
    workspace: Option<String>,
    monitor: Option<String>,
}

impl FocusedInterval {
    fn matches_target(&self, target: &FocusTarget) -> bool {
        self.address == target.address
            && self.app_class == target.app_class
            && self.title == target.title
            && self.workspace == target.workspace
            && self.monitor == target.monitor
    }
}

struct SessionPauseInterval {
    kind: SessionIntervalKind,
    source: Option<String>,
    interval_id: i64,
}

struct SleepPauseInterval {
    interval_id: i64,
}

impl Tracker {
    pub fn new(storage: Storage, config: Config) -> Self {
        Self {
            storage,
            config,
            steam: SteamResolver::default(),
            state: TrackerState::default(),
            daemon_run_id: None,
            observing: true,
            last_observed_at: None,
            observation_started_at: None,
            last_audio_at: None,
            last_resume_at: None,
            outage_interval_id: None,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let daemon_start = self.storage.start_daemon_run(clock::unix_now())?;
        self.daemon_run_id = Some(daemon_start.run_id);
        if let Some(recovery) = daemon_start.recovery {
            info!(
                "recovered stale daemon state at {}; excluded {}s of unobserved time",
                recovery.closed_at, recovery.unobserved_seconds
            );
        }
        // Cancellation covers startup, snapshot queries and monitor setup too.
        let mut terminate = terminate_signal();
        let result = tokio::select! {
            result = self.track_events() => Some(result),
            _ = tokio::signal::ctrl_c() => None,
            _ = recv_terminate_signal(&mut terminate) => None,
        };
        match result {
            Some(result) => result,
            None => self.shutdown(),
        }
    }

    async fn track_events(&mut self) -> Result<()> {
        self.begin_outage(clock::unix_now())?;
        let mut stream: Option<EventStream> = None;
        let mut reconnect_backoff = Duration::from_secs(1);
        let mut reconnect_at = time::Instant::now();
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
        let mut terminal_timer = time::interval(Duration::from_secs(
            self.config.tracking.terminal_resolve_seconds.max(2),
        ));
        terminal_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        terminal_timer.tick().await;
        let mut heartbeat_timer = time::interval(Duration::from_secs(
            self.config.tracking.heartbeat_seconds.max(15),
        ));
        heartbeat_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        heartbeat_timer.tick().await;
        let mut sleep_monitor = self.connect_sleep_monitor().await;
        let mut sleep_monitor_retry = time::interval(Duration::from_secs(60));
        sleep_monitor_retry.set_missed_tick_behavior(MissedTickBehavior::Skip);
        sleep_monitor_retry.tick().await;
        let mut idle_monitor = self.connect_idle_monitor().await;
        let mut idle_monitor_retry = time::interval(Duration::from_secs(60));
        idle_monitor_retry.set_missed_tick_behavior(MissedTickBehavior::Skip);
        idle_monitor_retry.tick().await;

        loop {
            tokio::select! {
                connection = async {
                    time::sleep_until(reconnect_at).await;
                    let stream = EventStream::connect().await?;
                    let status = session::status().await;
                    let snapshot = hyprland::snapshot().await?;
                    Ok::<_, anyhow::Error>((stream, snapshot, status))
                }, if stream.is_none() && self.state.sleep_pause.is_none() => {
                    match connection {
                        Ok((connected, snapshot, status)) => {
                            let now = clock::unix_now();
                            self.end_outage(now)?;
                            self.observation_started_at = Some(now);
                            self.last_audio_at = None;
                            self.last_resume_at = None;
                            self.observing = true;
                            if let Ok(status) = status {
                                self.apply_session_status(status, now)?;
                            }
                            self.apply_startup_snapshot(snapshot, now)?;
                            self.record_heartbeat()?;
                            stream = Some(connected);
                            reconnect_backoff = Duration::from_secs(1);
                            info!("tracking Hyprland usage");
                        }
                        Err(error) => {
                            warn!("Hyprland connection/snapshot unavailable: {error:#}");
                            reconnect_at = time::Instant::now() + reconnect_backoff;
                            reconnect_backoff = (reconnect_backoff * 2).min(Duration::from_secs(30));
                        }
                    }
                }
                event = next_hyprland_event(&mut stream), if stream.is_some() => {
                    match event {
                        Ok(Some(event)) => {
                            self.apply_event(event).await?;
                            if self.observing {
                                self.last_observed_at = Some(clock::unix_now());
                            } else {
                                stream = None;
                                reconnect_at = time::Instant::now();
                            }
                        }
                        Ok(None) | Err(_) => {
                            warn!("Hyprland event stream disconnected");
                            self.begin_outage(clock::unix_now())?;
                            stream = None;
                            reconnect_at = time::Instant::now();
                        }
                    }
                }
                _ = reconcile_timer.tick(), if stream.is_some() => {
                    self.reconcile().await?;
                    if !self.observing {
                        stream = None;
                        reconnect_at = time::Instant::now();
                    }
                }
                _ = session_timer.tick(), if self.observing => {
                    self.refresh_session_status().await?;
                    self.last_observed_at = Some(clock::unix_now());
                }
                _ = terminal_timer.tick(), if self.observing => {
                    self.refresh_terminal_focus().await?;
                    self.last_observed_at = Some(clock::unix_now());
                }
                _ = heartbeat_timer.tick(), if self.observing || self.state.sleep_pause.is_some() => {
                    self.record_heartbeat()?;
                }
                sleep_event = next_sleep_event(&mut sleep_monitor) => {
                    match sleep_event {
                        Ok(Some(event)) => {
                            self.apply_sleep_event(event).await?;
                            stream = None;
                            reconnect_at = time::Instant::now();
                            if let Some(monitor) = sleep_monitor.as_mut()
                                && let Err(error) = monitor.mark_handled(event).await
                            {
                                warn!("failed to update logind sleep inhibitor: {error:#}");
                                sleep_monitor = None;
                            }
                        }
                        Ok(None) => {
                            warn!("logind sleep monitor disconnected");
                            sleep_monitor = None;
                        }
                        Err(error) => {
                            warn!("logind sleep monitor failed: {error:#}");
                            sleep_monitor = None;
                        }
                    }
                }
                idle_event = next_idle_event(&mut idle_monitor), if self.config.tracking.pause_on_session_idle => {
                    match idle_event {
                        Ok(Some(event)) => {
                            if self.observing { self.apply_idle_event(event).await?; }
                        },
                        Ok(None) => {
                            warn!("Wayland idle monitor disconnected");
                            idle_monitor = None;
                        }
                        Err(error) => {
                            warn!("Wayland idle monitor failed: {error:#}");
                            idle_monitor = None;
                        }
                    }
                }
                _ = sleep_monitor_retry.tick(), if sleep_monitor.is_none() => {
                    sleep_monitor = self.connect_sleep_monitor().await;
                }
                _ = idle_monitor_retry.tick(), if idle_monitor.is_none() && self.config.tracking.pause_on_session_idle => {
                    idle_monitor = self.connect_idle_monitor().await;
                }
            }
        }
    }

    fn begin_outage(&mut self, now: i64) -> Result<()> {
        self.observing = false;
        if self.outage_interval_id.is_some() || self.state.sleep_pause.is_some() {
            return Ok(());
        }
        let boundary = self.last_observed_at.unwrap_or(now).min(now);
        self.storage.close_media_system(boundary)?;
        let id = self.storage.begin_observation_gap(boundary)?;
        self.state = TrackerState::default();
        self.outage_interval_id = Some(id);
        self.observation_started_at = None;
        Ok(())
    }

    fn end_outage(&mut self, now: i64) -> Result<()> {
        if let Some(id) = self.outage_interval_id {
            self.storage.close_system_interval(id, now)?;
            self.outage_interval_id = None;
        }
        Ok(())
    }

    async fn apply_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::WindowOpened(window) => self.open_window(window)?,
            Event::WindowClosed { address } => self.close_window(&address)?,
            Event::FocusChanged { address } => {
                if let Some(address) = address {
                    self.refresh_window_details(&address).await?;
                    if !self.state.windows.contains_key(&address) {
                        self.reconcile().await?;
                    }
                    self.set_active_window(Some(&address))?;
                } else {
                    self.set_active_window(None)?;
                }
            }
            Event::WindowTitleChanged { address } => {
                if let Some(address) = address {
                    self.refresh_window_details(&address).await?;
                    if self.state.active_address.as_deref() == Some(address.as_str()) {
                        self.sync_focused_interval()?;
                    }
                }
            }
            Event::Unknown => {}
        }
        Ok(())
    }

    async fn reconcile(&mut self) -> Result<()> {
        match hyprland::snapshot().await {
            Ok(snapshot) => {
                self.apply_snapshot(snapshot)?;
                self.last_observed_at = Some(clock::unix_now());
                Ok(())
            }
            Err(error) => {
                warn!("snapshot reconciliation failed: {error:#}");
                self.begin_outage(clock::unix_now())
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
            window.class = self.canonical_class(&window.class);
            self.open_window(window)?;
        }

        self.set_active_window(snapshot.active_address.as_deref())?;
        Ok(())
    }

    fn apply_startup_snapshot(&mut self, snapshot: Snapshot, now: i64) -> Result<()> {
        let stored = self.storage.unclosed_intervals()?;
        let mut windows = HashMap::new();
        let mut app_open_counts = HashMap::<String, usize>::new();

        for mut window in snapshot.windows {
            window.class = self.canonical_class(&window.class);
            if terminal::should_track_class(&window.class) {
                *app_open_counts.entry(window.class.clone()).or_default() += 1;
            }
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

        let focus_paused = self.state.focus_paused;
        let session_pause = self.state.session_pause.take();
        let sleep_pause = self.state.sleep_pause.take();
        self.state = TrackerState {
            windows,
            app_open_counts,
            active_address,
            focus_paused,
            session_pause,
            sleep_pause,
            ..TrackerState::default()
        };

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

        for interval in stored_focused {
            self.storage.close_interval(interval.id, now)?;
        }
        self.sync_focused_interval_at(now)
    }

    fn open_window(&mut self, mut window: Window) -> Result<()> {
        window.class = self.canonical_class(&window.class);

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

        if !terminal::should_track_class(&app_class) {
            return Ok(());
        }

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
                clock::unix_now(),
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
                    self.storage
                        .close_interval(interval_id, clock::unix_now())?;
                }
            }
        }
        Ok(())
    }

    fn set_active_window(&mut self, address: Option<&str>) -> Result<()> {
        self.state.active_address = address.map(ToOwned::to_owned);
        self.sync_focused_interval()
    }

    #[cfg(test)]
    fn set_focus_paused(&mut self, paused: bool) -> Result<()> {
        self.set_focus_paused_at(paused, clock::unix_now())
    }

    #[cfg(test)]
    fn set_focus_paused_at(&mut self, paused: bool, now: i64) -> Result<()> {
        if self.state.focus_paused == paused {
            return Ok(());
        }
        self.state.focus_paused = paused;
        self.sync_focused_interval_at(now)
    }

    fn set_session_pause(
        &mut self,
        kind: Option<SessionIntervalKind>,
        source: Option<&str>,
        transition_at: i64,
        now: i64,
    ) -> Result<()> {
        let current = self.state.session_pause.as_ref().map(|pause| pause.kind);
        if current == kind {
            return Ok(());
        }

        let segment_start = self.observation_started_at.unwrap_or(i64::MIN);
        let at = if kind == Some(SessionIntervalKind::Idle) && current.is_none() {
            transition_at
                .min(now)
                .max(segment_start)
                .max(self.last_audio_at.unwrap_or(i64::MIN))
                .max(self.last_resume_at.unwrap_or(i64::MIN))
        } else {
            now.max(segment_start)
        };
        let target = if kind.is_none() {
            self.focus_target_unpaused()
        } else {
            None
        };
        let (pause_id, focus_id) = self.storage.transition_session(SessionTransition {
            previous_id: self.state.session_pause.as_ref().map(|p| p.interval_id),
            kind,
            source,
            at,
            segment_start,
            resume_focus: target.as_ref().map(|t| {
                (
                    t.app_class.as_str(),
                    IntervalMetadata {
                        window_address: Some(&t.address),
                        title: t.title.as_deref(),
                        workspace: t.workspace.as_deref(),
                        monitor: t.monitor.as_deref(),
                    },
                )
            }),
        })?;
        // Do not discard live IDs until the complete database change commits.
        self.state.session_pause =
            pause_id
                .zip(kind)
                .map(|(interval_id, kind)| SessionPauseInterval {
                    kind,
                    source: source.map(str::to_string),
                    interval_id,
                });
        self.state.focused = focus_id
            .zip(target)
            .map(|(interval_id, target)| FocusedInterval {
                address: target.address,
                app_class: target.app_class,
                title: target.title,
                workspace: target.workspace,
                monitor: target.monitor,
                interval_id,
            });
        self.state.focus_paused = kind.is_some();
        if kind.is_none() {
            self.last_resume_at = Some(at);
        }
        Ok(())
    }

    async fn refresh_session_status(&mut self) -> Result<()> {
        if self.state.sleep_pause.is_some() {
            return Ok(());
        }
        match session::status().await {
            Ok(status) => self.apply_session_status(status, clock::unix_now())?,
            Err(error) => {
                debug!("session status unavailable: {error:#}");
            }
        }
        Ok(())
    }

    fn apply_session_status(&mut self, status: session::SessionStatus, now: i64) -> Result<()> {
        let sources = if status.locked {
            Vec::new()
        } else {
            status
                .audio_sources
                .iter()
                .map(|source| {
                    let window = self
                        .state
                        .windows
                        .values()
                        .find(|w| source.pid.is_some() && w.pid == source.pid);
                    let app = window
                        .map(|w| identity::canonical_app_class(&w.class))
                        .unwrap_or_else(|| source.app_class.clone());
                    // Stream titles describe the playing media; an unrelated selected tab does not.
                    let title = if self.config.capture_titles() {
                        source
                            .title
                            .clone()
                            .filter(|title| self.config.title_allowed(&app, title))
                    } else {
                        None
                    };
                    (app, title.unwrap_or_default())
                })
                .collect()
        };
        self.storage.record_media_snapshot(
            "system",
            &sources,
            now,
            self.config
                .tracking
                .session_poll_seconds
                .max(15)
                .saturating_mul(3) as i64,
        )?;
        if status.audio_playing {
            self.last_audio_at = Some(now);
        }
        if self.wayland_idle_pause_is_authoritative(&status) {
            return Ok(());
        }
        let pause_kind = self.session_pause_kind(&status);
        let transition_at = self.session_pause_transition_at(pause_kind, &status, now);
        self.set_session_pause(pause_kind, Some(status.source), transition_at, now)
    }

    fn session_pause_kind(&self, status: &session::SessionStatus) -> Option<SessionIntervalKind> {
        if self.config.tracking.pause_on_session_locked && status.locked {
            return Some(SessionIntervalKind::Locked);
        }
        if self.config.tracking.pause_on_session_idle && status.idle && !status.audio_playing {
            return Some(SessionIntervalKind::Idle);
        }
        None
    }

    fn wayland_idle_pause_is_authoritative(&self, status: &session::SessionStatus) -> bool {
        self.state.session_pause.as_ref().is_some_and(|pause| {
            pause.kind == SessionIntervalKind::Idle
                && pause.source.as_deref() == Some("wayland-idle")
                && !status.locked
                && !status.audio_playing
        })
    }

    fn session_pause_transition_at(
        &self,
        pause_kind: Option<SessionIntervalKind>,
        status: &session::SessionStatus,
        now: i64,
    ) -> i64 {
        if pause_kind == Some(SessionIntervalKind::Idle)
            && let Some(idle_since) = status.idle_since_unix
            && idle_since > 0
            && idle_since <= now
        {
            return idle_since;
        }
        now
    }

    async fn apply_sleep_event(&mut self, event: session::SleepEvent) -> Result<()> {
        let now = clock::unix_now();
        match event {
            session::SleepEvent::Preparing => self.start_sleep_at(now)?,
            session::SleepEvent::Resumed => {
                self.finish_sleep_at(now)?;
                self.last_observed_at = Some(now);
                self.begin_outage(now)?;
            }
        }
        self.record_heartbeat()?;
        Ok(())
    }

    async fn connect_sleep_monitor(&self) -> Option<session::SleepEventMonitor> {
        match session::SleepEventMonitor::connect().await {
            Ok(monitor) => Some(monitor),
            Err(error) => {
                debug!("logind sleep monitor unavailable: {error:#}");
                None
            }
        }
    }

    async fn connect_idle_monitor(&self) -> Option<session::IdleEventMonitor> {
        match session::IdleEventMonitor::connect(self.config.tracking.idle_timeout_seconds).await {
            Ok(monitor) => {
                debug!("Wayland idle monitor connected");
                Some(monitor)
            }
            Err(error) => {
                debug!("Wayland idle monitor unavailable: {error:#}");
                None
            }
        }
    }

    async fn apply_idle_event(&mut self, event: session::IdleEvent) -> Result<()> {
        if self.state.sleep_pause.is_some() {
            return Ok(());
        }
        match event {
            session::IdleEvent::Idled { since_unix } => {
                let now = clock::unix_now();
                match session::status().await {
                    Ok(status) if status.locked && self.config.tracking.pause_on_session_locked => {
                        self.set_session_pause(
                            Some(SessionIntervalKind::Locked),
                            Some(status.source),
                            now,
                            now,
                        )?;
                    }
                    Ok(status) if status.audio_playing => {
                        self.last_audio_at = Some(now);
                        debug!("Wayland idle event ignored while audio is playing");
                    }
                    Ok(_) | Err(_) => {
                        self.set_session_pause(
                            Some(SessionIntervalKind::Idle),
                            Some("wayland-idle"),
                            since_unix.min(now),
                            now,
                        )?;
                    }
                }
            }
            session::IdleEvent::Resumed { at_unix } => {
                self.last_resume_at = Some(at_unix);
                if self
                    .state
                    .session_pause
                    .as_ref()
                    .is_some_and(|pause| pause.kind == SessionIntervalKind::Idle)
                {
                    self.set_session_pause(None, None, at_unix, at_unix)?;
                }
                self.refresh_session_status().await?;
            }
        }
        Ok(())
    }

    fn start_sleep_at(&mut self, now: i64) -> Result<()> {
        if self.state.sleep_pause.is_some() {
            return Ok(());
        }

        self.end_outage(now)?;
        self.storage.close_media_system(now)?;
        self.storage.close_observed_intervals(now)?;
        self.observing = false;
        self.last_observed_at = Some(now);
        let interval_id =
            self.storage
                .start_system_interval(SystemIntervalKind::Sleep, Some("logind"), now)?;
        self.state = TrackerState {
            focus_paused: true,
            sleep_pause: Some(SleepPauseInterval { interval_id }),
            ..TrackerState::default()
        };
        Ok(())
    }

    fn finish_sleep_at(&mut self, now: i64) -> Result<()> {
        if let Some(previous) = self.state.sleep_pause.take() {
            self.storage
                .close_system_interval(previous.interval_id, now)?;
        }
        self.state.focus_paused = false;
        Ok(())
    }

    async fn refresh_window_details(&mut self, address: &str) -> Result<()> {
        match hyprland::active_window_details().await {
            Ok(Some(mut window)) if window.address == address => {
                window.class = self.canonical_class(&window.class);
                if self
                    .state
                    .windows
                    .get(address)
                    .is_some_and(|existing| existing.class != window.class)
                {
                    self.close_window(address)?;
                }
                self.open_window(window)?;
            }
            Ok(_) => {}
            Err(error) => {
                debug!("active window detail refresh failed: {error:#}");
            }
        }
        Ok(())
    }

    async fn refresh_terminal_focus(&mut self) -> Result<()> {
        let Some(address) = self.state.active_address.clone() else {
            return Ok(());
        };
        if !self
            .state
            .windows
            .get(&address)
            .is_some_and(|window| terminal::is_terminal_class(&window.class))
        {
            return Ok(());
        }

        self.refresh_window_details(&address).await?;
        self.sync_focused_interval()
    }

    fn focused_app_class(&mut self, window: &Window) -> String {
        if terminal::is_terminal_class(&window.class)
            && let Some(pid) = window.pid
            && let Some(app) = terminal::resolve_foreground_app(pid)
        {
            return self.canonical_class(&app);
        }

        window.class.clone()
    }

    fn canonical_class(&mut self, app_class: &str) -> String {
        identity::canonical_app_class(&self.steam.resolve_class(app_class))
    }

    fn focused_title(&self, window: &Window, app_class: &str) -> Option<String> {
        self.config
            .capture_titles()
            .then_some(window.title.as_deref())
            .flatten()
            .and_then(|title| identity::clean_window_title(title, app_class))
            .filter(|title| self.config.title_allowed(app_class, title))
    }

    fn sync_focused_interval(&mut self) -> Result<()> {
        self.sync_focused_interval_at(clock::unix_now())
    }

    fn sync_focused_interval_at(&mut self, now: i64) -> Result<()> {
        let target = self.focus_target();

        if let (Some(focused), Some(target)) = (self.state.focused.as_ref(), target.as_ref())
            && focused.matches_target(target)
        {
            return Ok(());
        }

        if let Some(previous) = self.state.focused.take() {
            self.storage.close_interval(previous.interval_id, now)?;
        }

        let Some(target) = target else {
            return Ok(());
        };

        let interval_id = self.storage.start_interval_with_metadata(
            IntervalKind::Focused,
            &target.app_class,
            IntervalMetadata {
                window_address: Some(&target.address),
                title: target.title.as_deref(),
                workspace: target.workspace.as_deref(),
                monitor: target.monitor.as_deref(),
            },
            now,
        )?;

        self.state.focused = Some(FocusedInterval {
            address: target.address,
            app_class: target.app_class,
            title: target.title,
            workspace: target.workspace,
            monitor: target.monitor,
            interval_id,
        });
        Ok(())
    }

    fn focus_target(&mut self) -> Option<FocusTarget> {
        if self.state.focus_paused {
            return None;
        }

        self.focus_target_unpaused()
    }

    fn focus_target_unpaused(&mut self) -> Option<FocusTarget> {
        let address = self.state.active_address.clone()?;
        let window = self.state.windows.get(&address).cloned()?;
        if !terminal::should_track_class(&window.class) {
            return None;
        }

        let app_class = self.focused_app_class(&window);
        let title = self.focused_title(&window, &app_class);
        Some(FocusTarget {
            address: window.address,
            app_class,
            title,
            workspace: window.workspace,
            monitor: window.monitor,
        })
    }

    fn shutdown(&mut self) -> Result<()> {
        let now = clock::unix_now();
        self.storage.close_media_system(now)?;
        if let Some(previous) = self.state.focused.take() {
            debug!("closing focused interval for {}", previous.app_class);
            self.storage.close_interval(previous.interval_id, now)?;
        }
        for (_, interval_id) in self.state.open_interval_ids.drain() {
            self.storage.close_interval(interval_id, now)?;
        }
        if let Some(previous) = self.state.session_pause.take() {
            self.storage
                .close_session_interval(previous.interval_id, now)?;
        }
        if let Some(sleep) = self.state.sleep_pause.take() {
            self.storage.close_system_interval(sleep.interval_id, now)?;
        }
        self.end_outage(now)?;
        if let Some(run_id) = self.daemon_run_id.take() {
            self.storage.finish_daemon_run(run_id, now)?;
        }
        Ok(())
    }

    fn record_heartbeat(&mut self) -> Result<()> {
        if self.observing {
            self.last_observed_at = Some(clock::unix_now());
        }
        if let Some(run_id) = self.daemon_run_id {
            self.storage
                .record_daemon_heartbeat(run_id, clock::unix_now())?;
        }
        Ok(())
    }
}

async fn next_hyprland_event(stream: &mut Option<EventStream>) -> Result<Option<Event>> {
    match stream {
        Some(stream) => stream.next_event().await,
        None => std::future::pending().await,
    }
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

async fn next_sleep_event(
    monitor: &mut Option<session::SleepEventMonitor>,
) -> Result<Option<session::SleepEvent>> {
    match monitor {
        Some(monitor) => monitor.next_event().await,
        None => std::future::pending().await,
    }
}

async fn next_idle_event(
    monitor: &mut Option<session::IdleEventMonitor>,
) -> Result<Option<session::IdleEvent>> {
    match monitor {
        Some(monitor) => monitor.next_event().await,
        None => std::future::pending().await,
    }
}

#[cfg(test)]
mod tests {
    use super::Tracker;
    use crate::{
        config::{Config, TitleCapture},
        hyprland::{Snapshot, Window},
        session::SessionStatus,
        storage::{IntervalKind, SessionIntervalKind, Storage},
    };

    fn timed_tracker(dir: &tempfile::TempDir) -> Tracker {
        let mut config = Config::default();
        config.privacy.title_capture = TitleCapture::All;
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);
        tracker.observation_started_at = Some(100);
        tracker
            .apply_startup_snapshot(
                Snapshot {
                    active_address: Some("0x1".into()),
                    windows: vec![window("0x1", "firefox")],
                },
                100,
            )
            .unwrap();
        tracker
    }

    #[test]
    fn late_idle_trims_all_focus_fragments_and_resumes_without_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let mut tracker = timed_tracker(&dir);
        for (at, title) in [(250, "second"), (275, "third")] {
            tracker.state.windows.get_mut("0x1").unwrap().title = Some(title.into());
            tracker.sync_focused_interval_at(at).unwrap();
        }
        tracker
            .set_session_pause(
                Some(SessionIntervalKind::Idle),
                Some("wayland-idle"),
                200,
                300,
            )
            .unwrap();
        tracker
            .set_session_pause(
                Some(SessionIntervalKind::Idle),
                Some("wayland-idle"),
                200,
                310,
            )
            .unwrap();
        tracker.set_session_pause(None, None, 350, 350).unwrap();
        let focus = tracker.storage.focused_timeline_between(100, 400).unwrap();
        assert_eq!(
            focus
                .iter()
                .map(|i| (i.started_at, i.ended_at))
                .filter(|(a, b)| b > a)
                .collect::<Vec<_>>(),
            vec![(100, 200), (350, 400)]
        );
        assert_eq!(
            tracker.storage.totals_between(100, 400).unwrap()[0].focused_seconds,
            150
        );
        assert_eq!(
            tracker
                .storage
                .session_totals_between(100, 400)
                .unwrap()
                .idle_seconds,
            150
        );
    }

    #[test]
    fn idle_backdating_preserves_observation_and_audio_boundaries() {
        for (audio, expected) in [(None, 100), (Some(230), 230)] {
            let dir = tempfile::tempdir().unwrap();
            let mut tracker = timed_tracker(&dir);
            tracker.last_audio_at = audio;
            tracker
                .set_session_pause(Some(SessionIntervalKind::Idle), Some("test"), 50, 300)
                .unwrap();
            let rows = tracker.storage.raw_export_between(0, 400).unwrap();
            assert_eq!(rows.session_intervals[0].started_at, expected);
            assert_eq!(
                rows.intervals
                    .iter()
                    .find(|i| i.kind == IntervalKind::Focused)
                    .unwrap()
                    .ended_at,
                Some(expected)
            );
        }
    }

    #[test]
    fn idle_backdating_never_overlaps_an_earlier_pause_cycle() {
        let dir = tempfile::tempdir().unwrap();
        let mut tracker = timed_tracker(&dir);
        tracker
            .set_session_pause(Some(SessionIntervalKind::Locked), Some("test"), 200, 200)
            .unwrap();
        tracker.set_session_pause(None, None, 300, 300).unwrap();
        tracker
            .set_session_pause(Some(SessionIntervalKind::Idle), Some("test"), 150, 400)
            .unwrap();
        let rows = tracker.storage.raw_export_between(0, 500).unwrap();
        let idle = rows
            .session_intervals
            .iter()
            .find(|r| r.kind == SessionIntervalKind::Idle)
            .unwrap();
        assert_eq!(idle.started_at, 300);
        assert_eq!(
            tracker
                .storage
                .session_totals_between(0, 500)
                .unwrap()
                .locked_seconds,
            100
        );
    }

    #[test]
    fn startup_locked_session_never_creates_a_focused_interval() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);
        tracker.observation_started_at = Some(100);
        tracker
            .apply_session_status(
                SessionStatus {
                    locked: true,
                    ..SessionStatus::default()
                },
                100,
            )
            .unwrap();
        tracker
            .apply_startup_snapshot(
                Snapshot {
                    active_address: Some("0x1".into()),
                    windows: vec![window("0x1", "firefox")],
                },
                100,
            )
            .unwrap();
        assert!(tracker.state.focused.is_none());
        assert_eq!(
            tracker
                .storage
                .session_totals_between(100, 200)
                .unwrap()
                .locked_seconds,
            100
        );
    }

    #[test]
    fn failed_idle_transaction_preserves_focus_and_tracker_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut tracker = timed_tracker(&dir);
        let focused_id = tracker.state.focused.as_ref().unwrap().interval_id;
        // Force the pause INSERT to fail after its tentative focus UPDATE.
        tracker
            .storage
            .start_session_interval(SessionIntervalKind::Idle, None, 150)
            .unwrap();
        assert!(
            tracker
                .set_session_pause(Some(SessionIntervalKind::Idle), None, 200, 300)
                .is_err()
        );
        assert_eq!(
            tracker.state.focused.as_ref().unwrap().interval_id,
            focused_id
        );
        assert!(!tracker.state.focus_paused);
        assert!(
            tracker
                .storage
                .unclosed_intervals()
                .unwrap()
                .iter()
                .any(|i| i.id == focused_id)
        );
    }

    #[test]
    fn disconnect_excludes_gap_and_sleep_remains_separate() {
        let dir = tempfile::tempdir().unwrap();
        let mut tracker = timed_tracker(&dir);
        tracker.last_observed_at = Some(150);
        tracker.begin_outage(180).unwrap();
        tracker.begin_outage(190).unwrap();
        assert!(tracker.state.focused.is_none());
        tracker.start_sleep_at(200).unwrap();
        tracker.finish_sleep_at(300).unwrap();
        tracker.last_observed_at = Some(300);
        tracker.begin_outage(300).unwrap();
        tracker.end_outage(350).unwrap();
        tracker
            .apply_startup_snapshot(
                Snapshot {
                    active_address: Some("0x1".into()),
                    windows: vec![window("0x1", "firefox")],
                },
                350,
            )
            .unwrap();
        assert_eq!(
            tracker.storage.totals_between(100, 400).unwrap()[0].focused_seconds,
            100
        );
        let totals = tracker.storage.session_totals_between(100, 400).unwrap();
        assert_eq!(totals.unobserved_seconds, 100);
        assert_eq!(totals.sleep_seconds, 100);
    }

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
    fn startup_snapshot_reuses_live_open_intervals_and_restarts_focus() {
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
        let restored_focus = tracker
            .state
            .focused
            .as_ref()
            .map(|focused| focused.interval_id);
        assert!(restored_focus.is_some());
        assert_ne!(restored_focus, Some(live_focus));

        let unclosed_ids = tracker
            .storage
            .unclosed_intervals()
            .unwrap()
            .into_iter()
            .map(|interval| interval.id)
            .collect::<std::collections::HashSet<_>>();
        assert!(unclosed_ids.contains(&live_open));
        assert!(restored_focus.is_some_and(|id| unclosed_ids.contains(&id)));
        assert!(!unclosed_ids.contains(&live_focus));
        assert!(!unclosed_ids.contains(&stale_open));
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
    fn focused_title_change_starts_new_interval() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.privacy.title_capture = TitleCapture::All;
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .open_window(window_with_title(
                "0x1",
                "firefox",
                "Issue #1 - Mozilla Firefox",
            ))
            .unwrap();
        tracker.set_active_window(Some("0x1")).unwrap();
        let first_id = tracker
            .state
            .focused
            .as_ref()
            .map(|focused| focused.interval_id)
            .unwrap();

        tracker
            .open_window(window_with_title(
                "0x1",
                "firefox",
                "Issue #2 - Mozilla Firefox",
            ))
            .unwrap();
        tracker.sync_focused_interval().unwrap();

        let second_id = tracker
            .state
            .focused
            .as_ref()
            .map(|focused| focused.interval_id)
            .unwrap();
        assert_ne!(first_id, second_id);

        let titles = tracker.storage.focused_titles_for_tests().unwrap();
        assert_eq!(titles, vec!["Issue #1", "Issue #2"]);
    }

    #[test]
    fn focused_title_capture_respects_allow_and_block_lists() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.privacy.title_capture = TitleCapture::All;
        config.privacy.title_allowlist = vec!["issue".to_string()];
        config.privacy.title_blocklist = vec!["secret".to_string()];
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .open_window(window_with_title(
                "0x1",
                "firefox",
                "Secret Issue - Mozilla Firefox",
            ))
            .unwrap();
        tracker.set_active_window(Some("0x1")).unwrap();
        tracker
            .open_window(window_with_title(
                "0x1",
                "firefox",
                "Issue #2 - Mozilla Firefox",
            ))
            .unwrap();
        tracker.sync_focused_interval().unwrap();

        let titles = tracker.storage.focused_titles_for_tests().unwrap();
        assert_eq!(titles, vec!["Issue #2"]);
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

    #[test]
    fn session_pause_records_idle_interval() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .set_session_pause(Some(SessionIntervalKind::Idle), Some("test"), 90, 100)
            .unwrap();
        tracker
            .set_session_pause(Some(SessionIntervalKind::Idle), Some("test"), 150, 150)
            .unwrap();
        assert_eq!(
            tracker
                .storage
                .unclosed_session_intervals()
                .unwrap()
                .into_iter()
                .map(|interval| interval.kind)
                .collect::<Vec<_>>(),
            vec![SessionIntervalKind::Idle]
        );

        tracker.set_session_pause(None, None, 220, 220).unwrap();
        let totals = tracker.storage.session_totals_between(0, 300).unwrap();
        assert_eq!(totals.idle_seconds, 130);
        assert_eq!(totals.locked_seconds, 0);
    }

    #[test]
    fn wayland_idle_pause_survives_non_idle_poll_status() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .set_session_pause(
                Some(SessionIntervalKind::Idle),
                Some("wayland-idle"),
                100,
                180,
            )
            .unwrap();

        let status = SessionStatus {
            idle: false,
            idle_since_unix: None,
            locked: false,
            stay_awake: false,
            audio_playing: false,
            audio_sources: Vec::new(),
            source: "loginctl",
        };
        assert!(tracker.wayland_idle_pause_is_authoritative(&status));
    }

    #[test]
    fn sleep_pause_splits_live_usage_and_records_sleep_gap() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .apply_startup_snapshot(
                Snapshot {
                    active_address: Some("0x1".to_string()),
                    windows: vec![window("0x1", "firefox")],
                },
                100,
            )
            .unwrap();
        assert!(tracker.state.focused.is_some());
        assert_eq!(tracker.state.open_interval_ids.len(), 1);

        tracker.start_sleep_at(160).unwrap();
        assert!(tracker.state.focused.is_none());
        assert!(tracker.state.open_interval_ids.is_empty());
        assert!(tracker.state.sleep_pause.is_some());

        tracker.finish_sleep_at(280).unwrap();
        tracker
            .apply_startup_snapshot(
                Snapshot {
                    active_address: Some("0x1".to_string()),
                    windows: vec![window("0x1", "firefox")],
                },
                280,
            )
            .unwrap();

        let rows = tracker.storage.totals_between(100, 340).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].focused_seconds, 120);
        assert_eq!(rows[0].open_seconds, 120);
        let totals = tracker.storage.session_totals_between(100, 340).unwrap();
        assert_eq!(totals.sleep_seconds, 120);
        assert_eq!(totals.unobserved_seconds, 0);
    }

    #[test]
    fn audio_playback_prevents_idle_pause_kind() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let tracker = Tracker::new(storage, config);
        let mut status = SessionStatus {
            idle: true,
            idle_since_unix: None,
            locked: false,
            stay_awake: false,
            audio_playing: true,
            audio_sources: Vec::new(),
            source: "test",
        };

        assert_eq!(tracker.session_pause_kind(&status), None);

        status.audio_playing = false;
        assert_eq!(
            tracker.session_pause_kind(&status),
            Some(SessionIntervalKind::Idle)
        );

        status.locked = true;
        status.audio_playing = true;
        assert_eq!(
            tracker.session_pause_kind(&status),
            Some(SessionIntervalKind::Locked)
        );
    }

    #[test]
    fn non_user_facing_windows_are_not_tracked() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::default();
        let storage = Storage::open(Some(&dir.path().join("test.db")), &config).unwrap();
        let mut tracker = Tracker::new(storage, config);

        tracker
            .open_window(window("0x1", "xdg-desktop-portal-gtk"))
            .unwrap();
        tracker.set_active_window(Some("0x1")).unwrap();

        assert!(tracker.state.focused.is_none());
        assert!(tracker.state.open_interval_ids.is_empty());
        assert!(tracker.state.app_open_counts.is_empty());
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
        window_with_title(address, class, "title")
    }

    fn window_with_title(address: &str, class: &str, title: &str) -> Window {
        Window {
            address: address.to_string(),
            class: class.to_string(),
            initial_class: Some(class.to_string()),
            title: Some(title.to_string()),
            workspace: None,
            monitor: None,
            pid: Some(1),
        }
    }
}
