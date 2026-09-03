use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(default)]
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub tracking: TrackingConfig,
    #[serde(default)]
    pub apps: BTreeMap<String, AppConfig>,
    #[serde(default)]
    pub goals: GoalsConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWarning {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrivacyConfig {
    #[serde(default)]
    pub title_capture: TitleCapture,
    #[serde(default)]
    pub browser_history: bool,
    #[serde(default)]
    pub title_allowlist: Vec<String>,
    #[serde(default)]
    pub title_blocklist: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrackingConfig {
    #[serde(default = "default_reconcile_seconds")]
    pub reconcile_seconds: u64,
    #[serde(default = "default_session_poll_seconds")]
    pub session_poll_seconds: u64,
    #[serde(default = "default_idle_timeout_seconds")]
    pub idle_timeout_seconds: u64,
    #[serde(default = "default_terminal_resolve_seconds")]
    pub terminal_resolve_seconds: u64,
    #[serde(default = "default_heartbeat_seconds")]
    pub heartbeat_seconds: u64,
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

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GoalsConfig {
    #[serde(default)]
    pub daily_focus_seconds: Option<i64>,
    #[serde(default)]
    pub daily_focus_minutes: Option<i64>,
    #[serde(default)]
    pub app_budgets: Vec<AppBudgetConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppBudgetConfig {
    #[serde(default)]
    pub app: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub daily_seconds: Option<i64>,
    #[serde(default)]
    pub daily_minutes: Option<i64>,
    #[serde(default)]
    pub weekly_seconds: Option<i64>,
    #[serde(default)]
    pub weekly_minutes: Option<i64>,
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

    pub fn title_allowed(&self, app_class: &str, title: &str) -> bool {
        if !self.capture_titles() {
            return false;
        }
        let haystack = format!("{app_class}\n{title}").to_lowercase();
        if self
            .privacy
            .title_blocklist
            .iter()
            .any(|needle| pattern_matches(&haystack, needle))
        {
            return false;
        }
        self.privacy.title_allowlist.is_empty()
            || self
                .privacy
                .title_allowlist
                .iter()
                .any(|needle| pattern_matches(&haystack, needle))
    }

    pub fn app_alias(&self, app_class: &str) -> Option<&str> {
        self.app_rule(app_class)
            .and_then(|rule| rule.alias.as_deref())
            .filter(|value| !value.trim().is_empty())
    }

    pub fn app_label(&self, app_class: &str, fallback: impl FnOnce() -> String) -> String {
        self.app_alias(app_class)
            .map(str::to_string)
            .unwrap_or_else(fallback)
    }

    pub fn app_category(&self, app_class: &str) -> String {
        self.app_rule(app_class)
            .and_then(|rule| rule.category.as_deref())
            .map(normalize_category)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "neutral".to_string())
    }

    pub fn daily_focus_target_seconds(&self) -> Option<i64> {
        self.goals
            .daily_focus_seconds
            .or_else(|| self.goals.daily_focus_minutes.map(|minutes| minutes * 60))
            .filter(|seconds| *seconds > 0)
    }

    pub fn warnings(&self) -> Vec<ConfigWarning> {
        let mut warnings = Vec::new();
        warn_below_runtime_minimum(
            &mut warnings,
            "tracking.reconcile_seconds",
            self.tracking.reconcile_seconds,
            30,
        );
        warn_below_runtime_minimum(
            &mut warnings,
            "tracking.session_poll_seconds",
            self.tracking.session_poll_seconds,
            15,
        );
        warn_below_runtime_minimum(
            &mut warnings,
            "tracking.idle_timeout_seconds",
            self.tracking.idle_timeout_seconds,
            30,
        );
        warn_below_runtime_minimum(
            &mut warnings,
            "tracking.terminal_resolve_seconds",
            self.tracking.terminal_resolve_seconds,
            2,
        );
        warn_below_runtime_minimum(
            &mut warnings,
            "tracking.heartbeat_seconds",
            self.tracking.heartbeat_seconds,
            15,
        );

        if self.goals.daily_focus_seconds.is_some() && self.goals.daily_focus_minutes.is_some() {
            warnings.push(ConfigWarning {
                field: "goals.daily_focus_seconds".to_string(),
                message: "daily_focus_seconds and daily_focus_minutes are both set; seconds takes precedence"
                    .to_string(),
            });
        }
        warn_non_positive_duration(
            &mut warnings,
            "goals.daily_focus_seconds",
            self.goals.daily_focus_seconds,
        );
        warn_non_positive_duration(
            &mut warnings,
            "goals.daily_focus_minutes",
            self.goals.daily_focus_minutes,
        );

        if !self.capture_titles()
            && (!self.privacy.title_allowlist.is_empty()
                || !self.privacy.title_blocklist.is_empty())
        {
            warnings.push(ConfigWarning {
                field: "privacy.title_capture".to_string(),
                message: "title allow/block lists are ignored while title_capture is off"
                    .to_string(),
            });
        }
        if self.privacy.browser_history && !self.capture_titles() {
            warnings.push(ConfigWarning {
                field: "privacy.browser_history".to_string(),
                message: "browser history enrichment requires title_capture to be all".to_string(),
            });
        }

        for (app_class, rule) in &self.apps {
            if app_class.trim().is_empty() {
                warnings.push(ConfigWarning {
                    field: "apps".to_string(),
                    message: "empty app class rule is ignored".to_string(),
                });
            }
            if rule
                .alias
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                warnings.push(ConfigWarning {
                    field: format!("apps.{app_class}.alias"),
                    message: "empty alias is ignored".to_string(),
                });
            }
            if rule
                .category
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                warnings.push(ConfigWarning {
                    field: format!("apps.{app_class}.category"),
                    message: "empty category falls back to neutral".to_string(),
                });
            }
        }

        for (index, budget) in self.goals.app_budgets.iter().enumerate() {
            let field = format!("goals.app_budgets[{index}]");
            let has_app = budget
                .app
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
            let has_category = budget
                .category
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
            if has_app == has_category {
                warnings.push(ConfigWarning {
                    field: field.clone(),
                    message: "budget must set exactly one of app or category".to_string(),
                });
            }
            if budget.daily_limit_seconds().is_none() && budget.weekly_limit_seconds().is_none() {
                warnings.push(ConfigWarning {
                    field: field.clone(),
                    message: "budget has no positive daily or weekly limit".to_string(),
                });
            }
            if budget.daily_seconds.is_some() && budget.daily_minutes.is_some() {
                warnings.push(ConfigWarning {
                    field: format!("{field}.daily_seconds"),
                    message:
                        "daily_seconds and daily_minutes are both set; seconds takes precedence"
                            .to_string(),
                });
            }
            if budget.weekly_seconds.is_some() && budget.weekly_minutes.is_some() {
                warnings.push(ConfigWarning {
                    field: format!("{field}.weekly_seconds"),
                    message:
                        "weekly_seconds and weekly_minutes are both set; seconds takes precedence"
                            .to_string(),
                });
            }
            warn_non_positive_duration(
                &mut warnings,
                &format!("{field}.daily_seconds"),
                budget.daily_seconds,
            );
            warn_non_positive_duration(
                &mut warnings,
                &format!("{field}.daily_minutes"),
                budget.daily_minutes,
            );
            warn_non_positive_duration(
                &mut warnings,
                &format!("{field}.weekly_seconds"),
                budget.weekly_seconds,
            );
            warn_non_positive_duration(
                &mut warnings,
                &format!("{field}.weekly_minutes"),
                budget.weekly_minutes,
            );
        }

        warnings
    }

    pub fn log_warnings(&self) {
        for warning in self.warnings() {
            tracing::warn!(config_field = warning.field.as_str(), "{}", warning.message);
        }
    }

    fn app_rule(&self, app_class: &str) -> Option<&AppConfig> {
        self.apps
            .get(app_class)
            .or_else(|| self.apps.get(&app_class.to_lowercase()))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            path: default_config_path(),
            privacy: PrivacyConfig::default(),
            tracking: TrackingConfig::default(),
            apps: BTreeMap::new(),
            goals: GoalsConfig::default(),
        }
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            title_capture: TitleCapture::Off,
            browser_history: false,
            title_allowlist: Vec::new(),
            title_blocklist: Vec::new(),
        }
    }
}

impl AppBudgetConfig {
    pub fn daily_limit_seconds(&self) -> Option<i64> {
        self.daily_seconds
            .or_else(|| self.daily_minutes.map(|minutes| minutes * 60))
            .filter(|seconds| *seconds > 0)
    }

    pub fn weekly_limit_seconds(&self) -> Option<i64> {
        self.weekly_seconds
            .or_else(|| self.weekly_minutes.map(|minutes| minutes * 60))
            .filter(|seconds| *seconds > 0)
    }
}

impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            reconcile_seconds: default_reconcile_seconds(),
            session_poll_seconds: default_session_poll_seconds(),
            idle_timeout_seconds: default_idle_timeout_seconds(),
            terminal_resolve_seconds: default_terminal_resolve_seconds(),
            heartbeat_seconds: default_heartbeat_seconds(),
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

fn default_idle_timeout_seconds() -> u64 {
    180
}

fn default_terminal_resolve_seconds() -> u64 {
    5
}

fn default_heartbeat_seconds() -> u64 {
    30
}

fn default_pause_on_session_idle() -> bool {
    true
}

fn default_pause_on_session_locked() -> bool {
    true
}

fn warn_below_runtime_minimum(
    warnings: &mut Vec<ConfigWarning>,
    field: &'static str,
    value: u64,
    minimum: u64,
) {
    if value < minimum {
        warnings.push(ConfigWarning {
            field: field.to_string(),
            message: format!(
                "value {value} is below runtime minimum {minimum}; daemon will clamp it"
            ),
        });
    }
}

fn warn_non_positive_duration(warnings: &mut Vec<ConfigWarning>, field: &str, value: Option<i64>) {
    if value.is_some_and(|seconds| seconds <= 0) {
        warnings.push(ConfigWarning {
            field: field.to_string(),
            message: "non-positive duration is ignored".to_string(),
        });
    }
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

fn normalize_category(value: &str) -> String {
    value.trim().to_lowercase().replace([' ', '_'], "-")
}

fn pattern_matches(haystack: &str, needle: &str) -> bool {
    let needle = needle.trim().to_lowercase();
    !needle.is_empty() && haystack.contains(&needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_alias_category_and_goals_are_local_config() {
        let config: Config = toml::from_str(
            r#"
            [apps."com.mitchellh.ghostty"]
            alias = "Terminal"
            category = "productive"

            [goals]
            daily_focus_minutes = 180

            [[goals.app_budgets]]
            category = "distracting"
            daily_minutes = 30
            "#,
        )
        .unwrap();

        assert_eq!(
            config.app_label("com.mitchellh.ghostty", || "Ghostty".to_string()),
            "Terminal"
        );
        assert_eq!(config.app_category("com.mitchellh.ghostty"), "productive");
        assert_eq!(config.app_category("discord"), "neutral");
        assert_eq!(config.daily_focus_target_seconds(), Some(10_800));
        assert_eq!(
            config.goals.app_budgets[0].daily_limit_seconds(),
            Some(1800)
        );
    }

    #[test]
    fn title_filters_block_or_allow_cleaned_titles() {
        let mut config = Config::default();
        config.privacy.title_capture = TitleCapture::All;
        config.privacy.browser_history = true;
        config.privacy.title_allowlist = vec!["issue".to_string()];
        config.privacy.title_blocklist = vec!["secret".to_string()];

        assert!(config.title_allowed("code", "Issue 123"));
        assert!(!config.title_allowed("code", "Calendar"));
        assert!(!config.title_allowed("code", "Issue secret"));
        assert!(config.privacy.browser_history);
    }

    #[test]
    fn warnings_explain_clamped_and_ambiguous_config() {
        let mut config = Config::default();
        config.tracking.reconcile_seconds = 1;
        config.privacy.browser_history = true;
        config.privacy.title_allowlist = vec!["issue".to_string()];
        config.goals.daily_focus_seconds = Some(3600);
        config.goals.daily_focus_minutes = Some(60);
        config.goals.app_budgets = vec![AppBudgetConfig {
            app: Some("code".to_string()),
            category: Some("productive".to_string()),
            daily_seconds: Some(-1),
            ..AppBudgetConfig::default()
        }];

        let warnings = config
            .warnings()
            .into_iter()
            .map(|warning| (warning.field, warning.message))
            .collect::<Vec<_>>();

        assert!(warnings.iter().any(|(field, message)| {
            field == "tracking.reconcile_seconds" && message.contains("runtime minimum")
        }));
        assert!(warnings.iter().any(|(field, message)| {
            field == "privacy.title_capture" && message.contains("ignored")
        }));
        assert!(warnings.iter().any(|(field, message)| {
            field == "privacy.browser_history" && message.contains("title_capture")
        }));
        assert!(warnings.iter().any(|(field, message)| {
            field == "goals.daily_focus_seconds" && message.contains("takes precedence")
        }));
        assert!(warnings.iter().any(|(field, message)| {
            field == "goals.app_budgets[0]" && message.contains("exactly one")
        }));
        assert!(warnings.iter().any(|(field, message)| {
            field == "goals.app_budgets[0]" && message.contains("no positive")
        }));
    }
}
