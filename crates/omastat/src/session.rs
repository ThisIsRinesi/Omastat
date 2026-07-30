use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::{env, path::PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStatus {
    pub idle: bool,
    pub locked: bool,
    pub stay_awake: bool,
    pub source: &'static str,
}

impl Default for SessionStatus {
    fn default() -> Self {
        Self {
            idle: false,
            locked: false,
            stay_awake: false,
            source: "default",
        }
    }
}

impl SessionStatus {
    pub fn should_pause(&self, pause_on_session_idle: bool, pause_on_session_locked: bool) -> bool {
        (pause_on_session_idle && self.idle) || (pause_on_session_locked && self.locked)
    }
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
    let locked = omarchy_locked().await.unwrap_or(false);

    Ok(SessionStatus {
        idle: idle.idle || idle.in_idle_cycle || idle.screensaver_started,
        locked,
        stay_awake: idle.stay_awake,
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
    Ok(status)
}

async fn command_output(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
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
    use super::{SessionStatus, parse_loginctl_status, parse_omarchy_idle_status};

    #[test]
    fn parses_loginctl_session_status() {
        assert_eq!(
            parse_loginctl_status("IdleHint=yes\nLockedHint=no\n"),
            SessionStatus {
                idle: true,
                locked: false,
                stay_awake: false,
                source: "loginctl",
            }
        );
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
            source: "test",
        };
        assert!(status.should_pause(true, true));
        assert!(!status.should_pause(false, true));
    }
}
