use crate::storage::TitleTotals;
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
pub struct BrowserActivity {
    pub app_class: String,
    pub browser_label: String,
    pub label: String,
    pub title: String,
    pub site: Option<String>,
    pub focused_seconds: i64,
    pub share: f64,
    pub source: BrowserActivitySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrowserActivitySource {
    History,
    Title,
}

#[derive(Debug, Default)]
pub struct BrowserHistoryResolver {
    profiles: Vec<PathBuf>,
    cache: BTreeMap<String, Option<String>>,
}

#[derive(Debug)]
struct BrowserActivityAggregate {
    focused_seconds: i64,
    title: String,
    source: BrowserActivitySource,
}

impl BrowserHistoryResolver {
    pub fn disabled() -> Self {
        Self::default()
    }

    pub fn discover_zen() -> Self {
        Self {
            profiles: discover_zen_places_dbs(),
            cache: BTreeMap::new(),
        }
    }

    pub fn site_for_title(&mut self, title: &str) -> Option<String> {
        let title = title.trim();
        if title.is_empty() || self.profiles.is_empty() {
            return None;
        }
        if let Some(cached) = self.cache.get(title) {
            return cached.clone();
        }

        let site = self
            .profiles
            .iter()
            .find_map(|path| title_host_from_places(path, title).ok().flatten())
            .map(|host| display_site(&host));
        self.cache.insert(title.to_string(), site.clone());
        site
    }
}

pub fn browser_activity(
    titles: &[TitleTotals],
    history: &mut BrowserHistoryResolver,
    limit: usize,
) -> Vec<BrowserActivity> {
    let mut grouped = BTreeMap::<(String, String, Option<String>), BrowserActivityAggregate>::new();

    for row in titles {
        if row.focused_seconds <= 0 || !is_browser_class(&row.app_class) {
            continue;
        }
        let title = clean_title(&row.title);
        if title.is_empty() {
            continue;
        }

        let history_site = history.site_for_title(&title);
        let inferred_site = history_site.clone().or_else(|| site_from_title(&title));
        let source = if history_site.is_some() {
            BrowserActivitySource::History
        } else {
            BrowserActivitySource::Title
        };
        let label = inferred_site.clone().unwrap_or_else(|| title.clone());
        let key = (row.app_class.clone(), label, inferred_site);
        grouped
            .entry(key)
            .and_modify(|current| {
                current.focused_seconds += row.focused_seconds.max(0);
                if source == BrowserActivitySource::History {
                    current.source = BrowserActivitySource::History;
                }
            })
            .or_insert(BrowserActivityAggregate {
                focused_seconds: row.focused_seconds.max(0),
                title,
                source,
            });
    }

    let total = grouped
        .values()
        .map(|row| row.focused_seconds)
        .sum::<i64>()
        .max(0);
    if total <= 0 {
        return Vec::new();
    }

    let mut rows = grouped
        .into_iter()
        .map(|((app_class, label, site), aggregate)| {
            let focused_seconds = aggregate.focused_seconds;
            BrowserActivity {
                browser_label: crate::identity::display_name(&app_class),
                title: aggregate.title,
                app_class,
                label,
                site,
                focused_seconds,
                share: focused_seconds.max(0) as f64 / total as f64,
                source: aggregate.source,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .focused_seconds
            .cmp(&left.focused_seconds)
            .then_with(|| left.label.cmp(&right.label))
    });
    rows.truncate(limit.max(1));
    rows
}

fn is_browser_class(app_class: &str) -> bool {
    matches!(
        app_class.to_ascii_lowercase().as_str(),
        "zen"
            | "firefox"
            | "librewolf"
            | "waterfox"
            | "google-chrome"
            | "chromium"
            | "brave"
            | "vivaldi"
            | "microsoft-edge"
    )
}

fn clean_title(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn site_from_title(title: &str) -> Option<String> {
    let lower = title.to_ascii_lowercase();
    let candidates = [
        ("youtube", "YouTube"),
        ("github", "GitHub"),
        ("chatgpt", "ChatGPT"),
        ("protondb", "ProtonDB"),
        ("discord", "Discord"),
        ("reddit", "Reddit"),
        ("twitch", "Twitch"),
        ("netflix", "Netflix"),
        ("hacker news", "Hacker News"),
        ("lobsters", "Lobsters"),
        ("gmail", "Gmail"),
        ("google docs", "Google Docs"),
        ("google drive", "Google Drive"),
    ];
    candidates
        .iter()
        .find(|(needle, _)| lower.contains(needle))
        .map(|(_, label)| (*label).to_string())
}

fn discover_zen_places_dbs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    discover_places_dbs(&home.join(".zen"))
}

fn discover_places_dbs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("places.sqlite"))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn title_host_from_places(path: &Path, title: &str) -> Result<Option<String>> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("failed to open browser history {}", path.display()))?;
    conn.query_row(
        "
        SELECT url
        FROM moz_places
        WHERE title = ?1
          AND url IS NOT NULL
        ORDER BY last_visit_date DESC
        LIMIT 1
        ",
        params![title],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|url| url.and_then(|value| host_from_url(&value)))
    .context("failed to query browser history title")
}

fn host_from_url(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

fn display_site(host: &str) -> String {
    let host = host.strip_prefix("www.").unwrap_or(host);
    let known = [
        ("youtube.com", "YouTube"),
        ("youtu.be", "YouTube"),
        ("github.com", "GitHub"),
        ("chatgpt.com", "ChatGPT"),
        ("chat.openai.com", "ChatGPT"),
        ("protondb.com", "ProtonDB"),
        ("discord.com", "Discord"),
        ("reddit.com", "Reddit"),
        ("twitch.tv", "Twitch"),
    ];
    if let Some((_, label)) = known.iter().find(|(suffix, _)| host.ends_with(suffix)) {
        return (*label).to_string();
    }
    host.split('.')
        .next()
        .filter(|part| !part.is_empty())
        .map(title_case)
        .unwrap_or_else(|| host.to_string())
}

fn title_case(value: &str) -> String {
    value
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            first.to_uppercase().collect::<String>() + chars.as_str()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{BrowserActivitySource, BrowserHistoryResolver, browser_activity, host_from_url};
    use crate::storage::TitleTotals;

    #[test]
    fn groups_browser_titles_by_inferred_site() {
        let titles = vec![
            title("zen", "Video title - YouTube", 1200),
            title("zen", "Another video | YouTube", 600),
            title("zen", "PR 42 by user - GitHub", 900),
            title("code", "main.rs", 3600),
        ];

        let rows = browser_activity(&titles, &mut BrowserHistoryResolver::disabled(), 8);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "YouTube");
        assert_eq!(rows[0].focused_seconds, 1800);
        assert_eq!(rows[0].source, BrowserActivitySource::Title);
        assert_eq!(rows[1].label, "GitHub");
    }

    #[test]
    fn parses_hosts_from_urls() {
        assert_eq!(
            host_from_url("https://www.youtube.com/watch?v=abc").as_deref(),
            Some("www.youtube.com")
        );
        assert_eq!(
            host_from_url("https://user@example.com:443/path").as_deref(),
            Some("example.com")
        );
    }

    fn title(app_class: &str, title: &str, focused_seconds: i64) -> TitleTotals {
        TitleTotals {
            app_class: app_class.to_string(),
            title: title.to_string(),
            focused_seconds,
        }
    }
}
