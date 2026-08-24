use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use serde::Deserialize;
use std::{env, path::PathBuf};
use tokio::{
    process::Command,
    time::{Duration, timeout},
};
use zbus::{Connection, Proxy, proxy::SignalStream, zvariant::OwnedFd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStatus {
    pub idle: bool,
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

pub struct SleepEventMonitor {
    proxy: Proxy<'static>,
    stream: SignalStream<'static>,
    delay_inhibitor: Option<OwnedFd>,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self {
            idle: false,
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
        let connection = Connection::system()
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
        let value = matches!(value.trim(), "yes" | "true" | "1");
        match key.trim() {
            "IdleHint" => status.idle = value,
            "LockedHint" => status.locked = value,
            _ => {}
        }
    }

    status
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
}
