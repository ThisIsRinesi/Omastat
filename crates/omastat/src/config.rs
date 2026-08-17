use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(default)]
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub tracking: TrackingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrivacyConfig {
    #[serde(default)]
    pub title_capture: TitleCapture,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackingConfig {
    #[serde(default = "default_reconcile_seconds")]
    pub reconcile_seconds: u64,
    #[serde(default = "default_session_poll_seconds")]
    pub session_poll_seconds: u64,
    #[serde(default = "default_terminal_resolve_seconds")]
    pub terminal_resolve_seconds: u64,
    #[serde(default = "default_pause_on_session_idle")]
    pub pause_on_session_idle: bool,
    #[serde(default = "default_pause_on_session_locked")]
    pub pause_on_session_locked: bool,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TitleCapture {
    #[default]
    Off,
    All,
}

impl Config {
    pub fn load(explicit_path: Option<&std::path::Path>) -> Result<Self> {
        let path = explicit_path
            .map(PathBuf::from)
            .unwrap_or_else(default_config_path);
        if explicit_path.is_none() {
            copy_legacy_config_if_needed(&path)?;
        }

        if !path.exists() {
            return Ok(Self {
                path,
                ..Self::default()
            });
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        let mut config: Self = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config {}", path.display()))?;
        config.path = path;
        Ok(config)
    }

    pub fn capture_titles(&self) -> bool {
        self.privacy.title_capture == TitleCapture::All
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: default_config_path(),
            privacy: PrivacyConfig::default(),
            tracking: TrackingConfig::default(),
        }
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            title_capture: TitleCapture::Off,
        }
    }
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            reconcile_seconds: default_reconcile_seconds(),
            session_poll_seconds: default_session_poll_seconds(),
            terminal_resolve_seconds: default_terminal_resolve_seconds(),
            pause_on_session_idle: default_pause_on_session_idle(),
            pause_on_session_locked: default_pause_on_session_locked(),
        }
    }
}

fn default_reconcile_seconds() -> u64 {
    300
}

fn default_session_poll_seconds() -> u64 {
    60
}

fn default_terminal_resolve_seconds() -> u64 {
    5
}

fn default_pause_on_session_idle() -> bool {
    true
}

fn default_pause_on_session_locked() -> bool {
    true
}

fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("omastat")
        .join("config.toml")
}

fn legacy_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hours-played")
        .join("config.toml")
}

fn copy_legacy_config_if_needed(path: &std::path::Path) -> Result<()> {
    let legacy = legacy_config_path();
    if path.exists() || !legacy.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(&legacy, path).with_context(|| {
        format!(
            "failed to copy legacy config {} to {}",
            legacy.display(),
            path.display()
        )
    })?;
    Ok(())
}
