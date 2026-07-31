use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

use crate::storage::AppTotals;

#[derive(Debug, Default)]
pub struct SteamResolver {
    loaded: bool,
    names: HashMap<u64, String>,
}

impl SteamResolver {
    pub fn resolve_class(&mut self, class: &str) -> String {
        let Some(app_id) = steam_app_id(class) else {
            return class.to_string();
        };

        self.load();
        self.names
            .get(&app_id)
            .cloned()
            .unwrap_or_else(|| format!("Steam App {app_id}"))
    }

    pub fn resolve_totals(&mut self, rows: Vec<AppTotals>) -> Vec<AppTotals> {
        let mut totals = HashMap::<String, AppTotals>::new();
        for row in rows {
            let app_class = self.resolve_class(&row.app_class);
            let entry = totals.entry(app_class.clone()).or_insert(AppTotals {
                app_class,
                focused_seconds: 0,
                open_seconds: 0,
            });
            entry.focused_seconds += row.focused_seconds;
            entry.open_seconds += row.open_seconds;
        }

        let mut rows = totals.into_values().collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            right
                .focused_seconds
                .cmp(&left.focused_seconds)
                .then_with(|| right.open_seconds.cmp(&left.open_seconds))
                .then_with(|| left.app_class.cmp(&right.app_class))
        });
        rows
    }

    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;

        for steamapps in steamapps_dirs() {
            let Ok(entries) = fs::read_dir(&steamapps) else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if is_appmanifest(&path)
                    && let Some((app_id, name)) = parse_appmanifest(&path)
                {
                    self.names.entry(app_id).or_insert(name);
                }
            }
        }
    }
}

pub fn steam_app_id(class: &str) -> Option<u64> {
    class
        .strip_prefix("steam_app_")
        .and_then(|value| value.parse().ok())
}

fn steamapps_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for root in steam_roots() {
        let steamapps = root.join("steamapps");
        push_unique(&mut dirs, steamapps.clone());

        let libraryfolders = steamapps.join("libraryfolders.vdf");
        let Ok(contents) = fs::read_to_string(libraryfolders) else {
            continue;
        };
        for library in parse_library_paths(&contents) {
            push_unique(&mut dirs, library.join("steamapps"));
        }
    }
    dirs
}

fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };

    vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ]
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn is_appmanifest(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("appmanifest_") && name.ends_with(".acf"))
}

fn parse_appmanifest(path: &Path) -> Option<(u64, String)> {
    let contents = fs::read_to_string(path).ok()?;
    let app_id = key_value(&contents, "appid")?.parse().ok()?;
    let name = key_value(&contents, "name")?;
    (!name.is_empty()).then_some((app_id, name))
}

fn parse_library_paths(contents: &str) -> Vec<PathBuf> {
    contents
        .lines()
        .filter_map(|line| key_value_line(line, "path").map(PathBuf::from))
        .collect()
}

fn key_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| key_value_line(line, key))
}

fn key_value_line(line: &str, key: &str) -> Option<String> {
    let mut parts = line.split('"').filter(|part| !part.trim().is_empty());
    let parsed_key = parts.next()?.trim();
    (parsed_key == key).then(|| parts.next().unwrap_or_default().to_string())
}

#[cfg(test)]
mod tests {
    use super::{SteamResolver, key_value, parse_library_paths, steam_app_id};
    use crate::storage::AppTotals;
    use std::path::PathBuf;

    #[test]
    fn extracts_steam_app_id_from_hyprland_class() {
        assert_eq!(steam_app_id("steam_app_646570"), Some(646570));
        assert_eq!(steam_app_id("steam"), None);
        assert_eq!(steam_app_id("steam_app_notanid"), None);
    }

    #[test]
    fn parses_appmanifest_key_values() {
        let manifest = r#"
        "AppState"
        {
            "appid" "646570"
            "name"  "Slay the Spire"
        }
        "#;

        assert_eq!(key_value(manifest, "appid").as_deref(), Some("646570"));
        assert_eq!(
            key_value(manifest, "name").as_deref(),
            Some("Slay the Spire")
        );
    }

    #[test]
    fn parses_library_paths() {
        let folders = r#"
        "libraryfolders"
        {
            "0"
            {
                "path" "/home/example/.local/share/Steam"
            }
            "1"
            {
                "path" "/mnt/games/SteamLibrary"
            }
        }
        "#;

        assert_eq!(
            parse_library_paths(folders),
            vec![
                PathBuf::from("/home/example/.local/share/Steam"),
                PathBuf::from("/mnt/games/SteamLibrary")
            ]
        );
    }

    #[test]
    fn resolves_and_merges_total_rows() {
        let mut resolver = SteamResolver::default();
        let rows = resolver.resolve_totals(vec![
            AppTotals {
                app_class: "steam_app_999999999".to_string(),
                focused_seconds: 10,
                open_seconds: 20,
            },
            AppTotals {
                app_class: "Steam App 999999999".to_string(),
                focused_seconds: 5,
                open_seconds: 7,
            },
        ]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].app_class, "Steam App 999999999");
        assert_eq!(rows[0].focused_seconds, 15);
        assert_eq!(rows[0].open_seconds, 27);
    }
}
