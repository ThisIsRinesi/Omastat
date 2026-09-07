use crate::{
    browser, clock,
    config::Config,
    storage::{Storage, StorageOpenMode},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

const MAX_MESSAGE_BYTES: u32 = 1024 * 1024;

#[derive(Debug, Deserialize)]
struct BrowserDomainMessage {
    #[serde(rename = "type")]
    kind: String,
    source: Option<String>,
    app_class: Option<String>,
    domain: Option<String>,
    timestamp: Option<i64>,
}

#[derive(Debug, Serialize)]
struct NativeHostResponse<'a> {
    ok: bool,
    status: &'a str,
}

pub fn run(config: &Config, database: Option<&std::path::Path>) -> Result<()> {
    let browser_class = launching_browser();
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    while let Some(message) = read_message(&mut stdin)? {
        let status = handle_message(config, database, &message, browser_class)?;
        write_message(&mut stdout, &NativeHostResponse { ok: true, status })?;
    }
    Ok(())
}

fn handle_message(
    config: &Config,
    database: Option<&std::path::Path>,
    message: &BrowserDomainMessage,
    browser_class: Option<&str>,
) -> Result<&'static str> {
    if message.kind != "active-domain" && message.kind != "clear-domain" {
        return Ok("ignored");
    }
    if !config.privacy.browser_domains {
        return Ok("disabled");
    }

    let domain = message
        .domain
        .as_deref()
        .and_then(browser::normalize_domain);
    if message.kind == "active-domain" && domain.is_none() {
        return Ok("ignored");
    }
    let app_class = message
        .app_class
        .as_deref()
        .map(normalize_app_class)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "zen".to_string());
    let (app_class, source) = browser_identity(browser_class, app_class, message.source.as_deref());
    let now = clock::unix_now();
    let timestamp = message.timestamp.unwrap_or(now).min(now);
    if timestamp < now - 90 {
        return Ok("stale");
    }

    let mut storage = Storage::open_with_mode(database, config, StorageOpenMode::ReadWriteMigrate)?;
    storage.record_browser_state(
        &source,
        &app_class,
        if message.kind == "clear-domain" {
            None
        } else {
            domain.as_deref()
        },
        timestamp,
    )?;
    Ok("recorded")
}

// Native messaging is launched by the browser. Use its executable identity so
// the same signed Firefox package also works in Zen, including focus-loss events.
fn launching_browser() -> Option<&'static str> {
    let mut pid = std::process::id();
    for _ in 0..8 {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        pid = status
            .lines()
            .find_map(|line| line.strip_prefix("PPid:"))?
            .trim()
            .parse()
            .ok()?;
        if pid == 0 {
            return None;
        }
        if let Ok(executable) = std::fs::read_link(format!("/proc/{pid}/exe"))
            && let Some(class) = executable
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(browser_executable)
        {
            return Some(class);
        }
    }
    None
}

fn browser_executable(name: &str) -> Option<&'static str> {
    match name {
        "zen" | "zen-bin" => Some("zen"),
        "firefox" | "firefox-bin" => Some("firefox"),
        _ => None,
    }
}

fn browser_identity(
    browser_class: Option<&str>,
    reported_class: String,
    reported_source: Option<&str>,
) -> (String, String) {
    if let Some(class) = browser_class {
        return (class.to_owned(), format!("omastat-{class}"));
    }
    let source = reported_source
        .map(normalize_source)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| reported_class.clone());
    (reported_class, source)
}

fn read_message(stdin: &mut impl Read) -> Result<Option<BrowserDomainMessage>> {
    let mut len_bytes = [0_u8; 4];
    match stdin.read_exact(&mut len_bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error).context("failed to read native message length"),
    }

    let len = u32::from_le_bytes(len_bytes);
    if len > MAX_MESSAGE_BYTES {
        anyhow::bail!("native message exceeded {MAX_MESSAGE_BYTES} bytes");
    }

    let mut buffer = vec![0_u8; len as usize];
    stdin
        .read_exact(&mut buffer)
        .context("failed to read native message body")?;
    let message = serde_json::from_slice(&buffer).context("failed to parse native message JSON")?;
    Ok(Some(message))
}

fn write_message(stdout: &mut impl Write, response: &NativeHostResponse<'_>) -> Result<()> {
    let body = serde_json::to_vec(response)?;
    let len = u32::try_from(body.len()).context("native response is too large")?;
    stdout.write_all(&len.to_le_bytes())?;
    stdout.write_all(&body)?;
    stdout.flush()?;
    Ok(())
}

fn normalize_app_class(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn normalize_source(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{NativeHostResponse, read_message, write_message};
    use std::io::Cursor;

    #[test]
    fn signed_firefox_messages_use_launching_browser_identity() {
        use super::{browser_executable, browser_identity};
        for (executable, class) in [
            ("zen", "zen"),
            ("zen-bin", "zen"),
            ("firefox", "firefox"),
            ("firefox-bin", "firefox"),
        ] {
            assert_eq!(
                browser_identity(
                    browser_executable(executable),
                    "firefox".into(),
                    Some("omastat-firefox")
                ),
                (class.into(), format!("omastat-{class}"))
            );
        }
        assert_eq!(
            browser_identity(
                browser_executable("sh"),
                "zen".into(),
                Some("custom-source")
            ),
            ("zen".into(), "custom-source".into())
        );
    }

    #[test]
    fn reads_native_message_frame() {
        let json = br#"{"type":"active-domain","app_class":"zen","domain":"github.com"}"#;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(json);

        let message = read_message(&mut Cursor::new(bytes)).unwrap().unwrap();
        assert_eq!(message.kind, "active-domain");
        assert_eq!(message.domain.as_deref(), Some("github.com"));
    }

    #[test]
    fn writes_native_message_frame() {
        let mut bytes = Vec::new();
        write_message(
            &mut bytes,
            &NativeHostResponse {
                ok: true,
                status: "recorded",
            },
        )
        .unwrap();
        let len = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        assert_eq!(len, bytes.len() - 4);
    }
}
