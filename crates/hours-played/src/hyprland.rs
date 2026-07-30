use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::{
    env,
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    net::UnixStream,
    process::Command,
};

#[derive(Debug, Clone)]
pub struct SocketPaths {
    pub request: PathBuf,
    pub event: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub address: String,
    pub class: String,
    pub initial_class: Option<String>,
    pub title: Option<String>,
    pub pid: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub windows: Vec<Window>,
    pub active_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    WindowOpened(Window),
    WindowClosed { address: String },
    FocusChanged { address: Option<String> },
    Unknown,
}

pub struct EventStream {
    lines: Lines<BufReader<UnixStream>>,
}

impl EventStream {
    pub async fn connect() -> Result<Self> {
        let paths = socket_paths()?;
        let stream = UnixStream::connect(&paths.event)
            .await
            .with_context(|| format!("failed to connect {}", paths.event.display()))?;
        Ok(Self {
            lines: BufReader::new(stream).lines(),
        })
    }

    pub async fn next_event(&mut self) -> Result<Option<Event>> {
        let Some(line) = self.lines.next_line().await? else {
            return Ok(None);
        };
        Ok(Some(parse_event(&line)))
    }
}

pub fn socket_paths() -> Result<SocketPaths> {
    let runtime_dir = env::var("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR is not set")?;
    let signature = env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .context("HYPRLAND_INSTANCE_SIGNATURE is not set")?;
    let base = Path::new(&runtime_dir).join("hypr").join(signature);

    Ok(SocketPaths {
        request: base.join(".socket.sock"),
        event: base.join(".socket2.sock"),
    })
}

pub async fn snapshot() -> Result<Snapshot> {
    let windows = clients().await?;
    let active_address = active_window().await?;
    Ok(Snapshot {
        windows,
        active_address,
    })
}

async fn clients() -> Result<Vec<Window>> {
    let output = Command::new("hyprctl")
        .args(["-j", "clients"])
        .output()
        .await
        .context("failed to run hyprctl -j clients")?;

    if !output.status.success() {
        return Err(anyhow!(
            "hyprctl -j clients failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let clients: Vec<ClientJson> =
        serde_json::from_slice(&output.stdout).context("failed to parse hyprctl clients JSON")?;
    Ok(clients.into_iter().map(Window::from).collect())
}

async fn active_window() -> Result<Option<String>> {
    let output = Command::new("hyprctl")
        .args(["-j", "activewindow"])
        .output()
        .await
        .context("failed to run hyprctl -j activewindow")?;

    if !output.status.success() {
        return Err(anyhow!(
            "hyprctl -j activewindow failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("failed to parse hyprctl activewindow JSON")?;
    Ok(value
        .get("address")
        .and_then(|value| value.as_str())
        .filter(|address| !address.is_empty() && *address != "0x0")
        .map(normalize_address))
}

fn parse_event(line: &str) -> Event {
    let Some((name, payload)) = line.split_once(">>") else {
        return Event::Unknown;
    };

    match name {
        "activewindowv2" => Event::FocusChanged {
            address: non_empty_address(payload),
        },
        "openwindow" => {
            let parts = split_payload(payload, 4);
            let Some(address) = parts.first().and_then(|value| non_empty_address(value)) else {
                return Event::Unknown;
            };
            let class = parts.get(2).copied().unwrap_or("unknown").to_string();
            let title = parts.get(3).and_then(|value| non_empty(value));
            Event::WindowOpened(Window {
                address,
                class,
                initial_class: None,
                title,
                pid: None,
            })
        }
        "closewindow" => match non_empty(payload) {
            Some(address) => Event::WindowClosed {
                address: normalize_address(&address),
            },
            None => Event::Unknown,
        },
        _ => Event::Unknown,
    }
}

fn split_payload(payload: &str, max_parts: usize) -> Vec<&str> {
    payload.splitn(max_parts, ',').collect()
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value != "0x0").then(|| value.to_string())
}

fn non_empty_address(value: &str) -> Option<String> {
    non_empty(value).map(|value| normalize_address(&value))
}

fn normalize_address(address: &str) -> String {
    if address.starts_with("0x") {
        address.to_string()
    } else {
        format!("0x{address}")
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientJson {
    address: String,
    class: String,
    initial_class: Option<String>,
    title: Option<String>,
    pid: Option<i64>,
}

impl From<ClientJson> for Window {
    fn from(value: ClientJson) -> Self {
        Self {
            address: normalize_address(&value.address),
            class: if value.class.is_empty() {
                "unknown".to_string()
            } else {
                value.class
            },
            initial_class: value.initial_class,
            title: value.title,
            pid: value.pid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, Window, parse_event};

    #[test]
    fn parses_focus_event() {
        assert_eq!(
            parse_event("activewindowv2>>abc"),
            Event::FocusChanged {
                address: Some("0xabc".to_string())
            }
        );
    }

    #[test]
    fn parses_openwindow_title_with_commas() {
        assert_eq!(
            parse_event("openwindow>>0xabc,1,firefox,Title, With Commas"),
            Event::WindowOpened(Window {
                address: "0xabc".to_string(),
                class: "firefox".to_string(),
                initial_class: None,
                title: Some("Title, With Commas".to_string()),
                pid: None,
            })
        );
    }
}
