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
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    while let Some(message) = read_message(&mut stdin)? {
        let status = handle_message(config, database, &message)?;
        write_message(&mut stdout, &NativeHostResponse { ok: true, status })?;
    }
    Ok(())
}

fn handle_message(
    config: &Config,
    database: Option<&std::path::Path>,
    message: &BrowserDomainMessage,
) -> Result<&'static str> {
    if message.kind != "active-domain" {
        return Ok("ignored");
    }
    if !config.privacy.browser_domains {
        return Ok("disabled");
    }

    let Some(domain) = message
        .domain
        .as_deref()
        .and_then(browser::normalize_domain)
    else {
        return Ok("ignored");
    };
    let app_class = message
        .app_class
        .as_deref()
        .map(normalize_app_class)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "zen".to_string());
    let source = message
        .source
        .as_deref()
        .map(normalize_source)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| app_class.clone());
    let timestamp = message.timestamp.unwrap_or_else(clock::unix_now);

    let mut storage = Storage::open_with_mode(database, config, StorageOpenMode::ReadWriteMigrate)?;
    storage.record_browser_domain(&source, &app_class, &domain, timestamp)?;
    Ok("recorded")
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
