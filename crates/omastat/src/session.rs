use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use serde::Deserialize;
use std::{env, path::PathBuf, thread};
use tokio::{
    process::Command,
    sync::mpsc,
    time::{Duration, timeout},
};
use wayland_client::{
    Connection as WaylandConnection, Dispatch, EventQueue, Proxy as WaylandProxy, QueueHandle,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry, wl_seat::WlSeat},
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{Event as WaylandIdleNotificationEvent, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};
use zbus::{Connection as ZbusConnection, Proxy, proxy::SignalStream, zvariant::OwnedFd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStatus {
    pub idle: bool,
    pub idle_since_unix: Option<i64>,
    pub locked: bool,
    pub stay_awake: bool,
    pub audio_playing: bool,
    pub source: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepEvent {
    Preparing,
    Resumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleEvent {
    Idled { since_unix: i64 },
    Resumed { at_unix: i64 },
}

pub struct SleepEventMonitor {
    proxy: Proxy<'static>,
    stream: SignalStream<'static>,
    delay_inhibitor: Option<OwnedFd>,
}

pub struct IdleEventMonitor {
    receiver: mpsc::UnboundedReceiver<IdleEvent>,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self {
            idle: false,
            idle_since_unix: None,
            locked: false,
            stay_awake: false,
            audio_playing: false,
            source: "default",
        }
    }
}

impl SessionStatus {
    pub fn should_pause(&self, pause_on_session_idle: bool, pause_on_session_locked: bool) -> bool {
        (pause_on_session_locked && self.locked)
            || (pause_on_session_idle && self.idle && !self.audio_playing)
    }
}

impl SleepEvent {
    fn from_logind_active(active: bool) -> Self {
        if active {
            Self::Preparing
        } else {
            Self::Resumed
        }
    }
}

impl SleepEventMonitor {
    pub async fn connect() -> Result<Self> {
        let connection = ZbusConnection::system()
            .await
            .context("failed to connect to system D-Bus")?;
        let proxy = Proxy::new_owned(
            connection,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await
        .context("failed to create logind manager proxy")?;
        let stream = proxy
            .receive_signal("PrepareForSleep")
            .await
            .context("failed to subscribe to logind PrepareForSleep")?;
        let delay_inhibitor = match take_sleep_delay_inhibitor(&proxy).await {
            Ok(inhibitor) => Some(inhibitor),
            Err(error) => {
                tracing::debug!(
                    "logind sleep delay inhibitor unavailable; monitoring sleep passively: {error:#}"
                );
                None
            }
        };

        Ok(Self {
            proxy,
            stream,
            delay_inhibitor,
        })
    }

    pub async fn next_event(&mut self) -> Result<Option<SleepEvent>> {
        let Some(message) = self.stream.next().await else {
            return Ok(None);
        };
        let active: bool = message
            .body()
            .deserialize()
            .context("failed to parse logind PrepareForSleep signal")?;
        Ok(Some(SleepEvent::from_logind_active(active)))
    }

    pub async fn mark_handled(&mut self, event: SleepEvent) -> Result<()> {
        match event {
            SleepEvent::Preparing => {
                self.delay_inhibitor.take();
            }
            SleepEvent::Resumed => {
                if self.delay_inhibitor.is_none() {
                    match take_sleep_delay_inhibitor(&self.proxy).await {
                        Ok(inhibitor) => self.delay_inhibitor = Some(inhibitor),
                        Err(error) => {
                            tracing::debug!(
                                "failed to reacquire logind sleep delay inhibitor: {error:#}"
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl IdleEventMonitor {
    pub async fn connect(timeout_seconds: u64) -> Result<Self> {
        let timeout_seconds = timeout_seconds.max(30);
        let (sender, receiver) = mpsc::unbounded_channel();
        let (ready_sender, ready_receiver) = std::sync::mpsc::sync_channel(1);

        thread::Builder::new()
            .name("omastat-wayland-idle".to_string())
            .spawn(move || {
                let result = init_wayland_idle_monitor(timeout_seconds, sender);
                match result {
                    Ok((mut event_queue, mut state)) => {
                        let _ = ready_sender.send(Ok(()));
                        if let Err(error) = run_wayland_idle_monitor(&mut event_queue, &mut state) {
                            tracing::debug!("Wayland idle monitor stopped: {error:#}");
                        }
                    }
                    Err(error) => {
                        let _ = ready_sender.send(Err(error));
                    }
                }
            })
            .context("failed to spawn Wayland idle monitor")?;

        tokio::task::spawn_blocking(move || ready_receiver.recv_timeout(Duration::from_secs(2)))
            .await
            .context("Wayland idle monitor startup wait failed")?
            .context("Wayland idle monitor did not initialize")?
            .context("failed to initialize Wayland idle monitor")?;

        Ok(Self { receiver })
    }

    pub async fn next_event(&mut self) -> Result<Option<IdleEvent>> {
        Ok(self.receiver.recv().await)
    }
}

struct WaylandIdleState {
    sender: mpsc::UnboundedSender<IdleEvent>,
    timeout_seconds: u64,
    _connection: WaylandConnection,
    _notifier: Option<ExtIdleNotifierV1>,
    _seat: Option<WlSeat>,
    _notification: Option<ExtIdleNotificationV1>,
}

fn init_wayland_idle_monitor(
    timeout_seconds: u64,
    sender: mpsc::UnboundedSender<IdleEvent>,
) -> Result<(EventQueue<WaylandIdleState>, WaylandIdleState)> {
    let connection =
        WaylandConnection::connect_to_env().context("failed to connect to Wayland display")?;
    let (globals, mut event_queue) = registry_queue_init::<WaylandIdleState>(&connection)
        .context("failed to read Wayland globals")?;
    let queue_handle = event_queue.handle();
    let notifier: ExtIdleNotifierV1 = globals
        .bind(&queue_handle, 1..=2, ())
        .context("Wayland compositor does not expose ext-idle-notify-v1")?;
    let seat: WlSeat = globals
        .bind(&queue_handle, 1..=9, ())
        .context("Wayland compositor does not expose wl_seat")?;
    let timeout_ms = timeout_seconds.saturating_mul(1_000).min(u32::MAX as u64) as u32;
    let notification = if notifier.version() >= 2 {
        notifier.get_input_idle_notification(timeout_ms, &seat, &queue_handle, ())
    } else {
        notifier.get_idle_notification(timeout_ms, &seat, &queue_handle, ())
    };
    let mut state = WaylandIdleState {
        sender,
        timeout_seconds,
        _connection: connection,
        _notifier: Some(notifier),
        _seat: Some(seat),
        _notification: Some(notification),
    };
    event_queue
        .roundtrip(&mut state)
        .context("failed to initialize Wayland idle notification")?;
    Ok((event_queue, state))
}

fn run_wayland_idle_monitor(
    event_queue: &mut EventQueue<WaylandIdleState>,
    state: &mut WaylandIdleState,
) -> Result<()> {
    loop {
        event_queue
            .blocking_dispatch(state)
            .context("Wayland idle monitor dispatch failed")?;
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WaylandIdleState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &WaylandConnection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for WaylandIdleState {
    fn event(
        _state: &mut Self,
        _proxy: &WlSeat,
        _event: wayland_client::protocol::wl_seat::Event,
        _data: &(),
        _conn: &WaylandConnection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtIdleNotifierV1, ()> for WaylandIdleState {
    fn event(
        _state: &mut Self,
        _proxy: &ExtIdleNotifierV1,
        _event: wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::Event,
        _data: &(),
        _conn: &WaylandConnection,
        _queue_handle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtIdleNotificationV1, ()> for WaylandIdleState {
    fn event(
        state: &mut Self,
        _proxy: &ExtIdleNotificationV1,
        event: WaylandIdleNotificationEvent,
        _data: &(),
        _conn: &WaylandConnection,
        _queue_handle: &QueueHandle<Self>,
    ) {
        match event {
            WaylandIdleNotificationEvent::Idled => {
                let since_unix = crate::clock::unix_now() - state.timeout_seconds as i64;
                let _ = state.sender.send(IdleEvent::Idled { since_unix });
            }
            WaylandIdleNotificationEvent::Resumed => {
                let at_unix = crate::clock::unix_now();
                let _ = state.sender.send(IdleEvent::Resumed { at_unix });
            }
            _ => {}
        }
    }
}

async fn take_sleep_delay_inhibitor(proxy: &Proxy<'_>) -> Result<OwnedFd> {
    proxy
        .call(
            "Inhibit",
            &(
                "sleep",
                "Omastat",
                "Record sleep interval boundaries before suspend",
                "delay",
            ),
        )
        .await
        .context("failed to acquire logind sleep delay inhibitor")
}

pub async fn status() -> Result<SessionStatus> {
    match omarchy_status().await {
        Ok(status) => Ok(status),
        Err(omarchy_error) => match loginctl_status().await {
            Ok(status) => Ok(status),
            Err(loginctl_error) => Err(anyhow!(
                "omarchy idle status failed: {omarchy_error:#}; loginctl fallback failed: {loginctl_error:#}"
            )),
        },
    }
}

async fn omarchy_status() -> Result<SessionStatus> {
    let idle_output = command_output("omarchy-shell", &["idle", "status"]).await?;
    let idle = parse_omarchy_idle_status(&idle_output)?;
    let locked = match omarchy_locked().await {
        Ok(locked) => locked,
        Err(error) => {
            tracing::debug!("omarchy lock status unavailable; assuming unlocked: {error:#}");
            false
        }
    };
    let audio_playing = audio_playing_with_fallback().await;

    Ok(SessionStatus {
        idle: idle.idle || idle.in_idle_cycle || idle.screensaver_started,
        idle_since_unix: None,
        locked,
        stay_awake: idle.stay_awake,
        audio_playing,
        source: "omarchy-shell",
    })
}

async fn omarchy_locked() -> Result<bool> {
    let output = command_output("omarchy-shell", &["lock", "isLocked"]).await?;
    Ok(matches!(output.trim(), "true" | "yes" | "1"))
}

async fn loginctl_status() -> Result<SessionStatus> {
    let session_id = env::var("XDG_SESSION_ID").context("XDG_SESSION_ID is not set")?;
    let output = command_output(
        "loginctl",
        &[
            "show-session",
            &session_id,
            "-p",
            "IdleHint",
            "-p",
            "LockedHint",
            "-p",
            "IdleSinceHint",
        ],
    )
    .await?;
    let mut status = parse_loginctl_status(&output);
    status.stay_awake = stay_awake_state_path().is_file();
    status.audio_playing = audio_playing_with_fallback().await;
    Ok(status)
}

async fn audio_playing_with_fallback() -> bool {
    match audio_playing().await {
        Ok(audio_playing) => audio_playing,
        Err(error) => {
            tracing::debug!("audio playback status unavailable; assuming silent: {error:#}");
            false
        }
    }
}

async fn audio_playing() -> Result<bool> {
    let output = command_output("pactl", &["list", "sink-inputs"]).await?;
    Ok(parse_pactl_sink_inputs_playing(&output))
}

async fn command_output(program: &str, args: &[&str]) -> Result<String> {
    let output = timeout(
        Duration::from_secs(2),
        Command::new(program).args(args).output(),
    )
    .await
    .with_context(|| format!("timed out running {program} {}", args.join(" ")))?
    .with_context(|| format!("failed to run {program} {}", args.join(" ")))?;

    if !output.status.success() {
        return Err(anyhow!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_loginctl_status(output: &str) -> SessionStatus {
    let mut status = SessionStatus {
        source: "loginctl",
        ..SessionStatus::default()
    };

    for line in output.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "IdleHint" => status.idle = parse_bool_hint(value),
            "LockedHint" => status.locked = parse_bool_hint(value),
            "IdleSinceHint" => status.idle_since_unix = parse_logind_usec_epoch(value),
            _ => {}
        }
    }

    status
}

fn parse_bool_hint(value: &str) -> bool {
    matches!(value.trim(), "yes" | "true" | "1")
}

fn parse_logind_usec_epoch(value: &str) -> Option<i64> {
    let usec = value.trim().parse::<u64>().ok()?;
    if usec == 0 {
        return None;
    }
    i64::try_from(usec / 1_000_000).ok()
}

fn parse_omarchy_idle_status(output: &str) -> Result<OmarchyIdleStatus> {
    serde_json::from_str(output).context("failed to parse omarchy-shell idle status JSON")
}

fn parse_pactl_sink_inputs_playing(output: &str) -> bool {
    output.lines().any(|line| {
        let Some((key, value)) = line.trim().split_once(':') else {
            return false;
        };
        key.trim().eq_ignore_ascii_case("State") && value.trim().eq_ignore_ascii_case("RUNNING")
    })
}

fn stay_awake_state_path() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("omarchy")
        .join("indicators")
        .join("stay-awake")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OmarchyIdleStatus {
    #[serde(default)]
    stay_awake: bool,
    #[serde(default)]
    idle: bool,
    #[serde(default)]
    in_idle_cycle: bool,
    #[serde(default)]
    screensaver_started: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        SessionStatus, SleepEvent, parse_loginctl_status, parse_omarchy_idle_status,
        parse_pactl_sink_inputs_playing,
    };

    #[test]
    fn parses_loginctl_session_status() {
        assert_eq!(
            parse_loginctl_status("IdleHint=yes\nLockedHint=no\n"),
            SessionStatus {
                idle: true,
                idle_since_unix: None,
                locked: false,
                stay_awake: false,
                audio_playing: false,
                source: "loginctl",
            }
        );
    }

    #[test]
    fn maps_logind_sleep_signal_boolean() {
        assert_eq!(SleepEvent::from_logind_active(true), SleepEvent::Preparing);
        assert_eq!(SleepEvent::from_logind_active(false), SleepEvent::Resumed);
    }

    #[test]
    fn parses_omarchy_idle_cycle_status() {
        let status = parse_omarchy_idle_status(
            r#"{"stayAwake":false,"idle":false,"inIdleCycle":true,"screensaverStarted":false}"#,
        )
        .unwrap();

        assert!(status.in_idle_cycle);
        assert!(!status.stay_awake);
    }

    #[test]
    fn pause_policy_is_configurable() {
        let status = SessionStatus {
            idle: true,
            idle_since_unix: None,
            locked: false,
            stay_awake: false,
            audio_playing: false,
            source: "test",
        };
        assert!(status.should_pause(true, true));
        assert!(!status.should_pause(false, true));
    }

    #[test]
    fn active_audio_suppresses_idle_pause_but_not_lock_pause() {
        let mut status = SessionStatus {
            idle: true,
            idle_since_unix: None,
            locked: false,
            stay_awake: false,
            audio_playing: true,
            source: "test",
        };
        assert!(!status.should_pause(true, true));

        status.locked = true;
        assert!(status.should_pause(true, true));
    }

    #[test]
    fn parses_running_pactl_sink_inputs_as_audio_playback() {
        assert!(parse_pactl_sink_inputs_playing(
            "Sink Input #42\n\tState: RUNNING\n\tMute: no\n"
        ));
        assert!(!parse_pactl_sink_inputs_playing(
            "Sink Input #42\n\tState: CORKED\n"
        ));
    }

    #[test]
    fn parses_loginctl_idle_since_hint() {
        let status =
            parse_loginctl_status("IdleHint=yes\nLockedHint=no\nIdleSinceHint=1783123456000000\n");

        assert!(status.idle);
        assert_eq!(status.idle_since_unix, Some(1_783_123_456));
    }
}
