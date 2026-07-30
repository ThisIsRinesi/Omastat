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
        }
    }
}

fn default_reconcile_seconds() -> u64 {
    300
}

fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hours-played")
        .join("config.toml")
}
