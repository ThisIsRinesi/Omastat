use anyhow::{Context, Result, anyhow};
use std::env;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionStatus {
    pub idle: bool,
    pub locked: bool,
}

impl SessionStatus {
    pub fn should_pause(self, pause_on_session_idle: bool, pause_on_session_locked: bool) -> bool {
        (pause_on_session_idle && self.idle) || (pause_on_session_locked && self.locked)
    }
}

pub async fn status() -> Result<SessionStatus> {
    let session_id = env::var("XDG_SESSION_ID").context("XDG_SESSION_ID is not set")?;
    status_for_session(&session_id).await
}

async fn status_for_session(session_id: &str) -> Result<SessionStatus> {
    let output = Command::new("loginctl")
        .args([
            "show-session",
            session_id,
            "-p",
            "IdleHint",
            "-p",
            "LockedHint",
        ])
        .output()
        .await
        .context("failed to run loginctl show-session")?;

    if !output.status.success() {
        return Err(anyhow!(
            "loginctl show-session failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(parse_loginctl_status(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn parse_loginctl_status(output: &str) -> SessionStatus {
    let mut status = SessionStatus::default();

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

#[cfg(test)]
mod tests {
    use super::{SessionStatus, parse_loginctl_status};

    #[test]
    fn parses_loginctl_session_status() {
        assert_eq!(
            parse_loginctl_status("IdleHint=yes\nLockedHint=no\n"),
            SessionStatus {
                idle: true,
                locked: false,
            }
        );
    }

    #[test]
    fn pause_policy_is_configurable() {
        let status = SessionStatus {
            idle: true,
            locked: false,
        };
        assert!(status.should_pause(true, true));
        assert!(!status.should_pause(false, true));
    }
}
