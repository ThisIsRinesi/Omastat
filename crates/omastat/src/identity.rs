pub fn canonical_app_class(app_class: &str) -> String {
    let value = clean_spaces(app_class);
    if value.is_empty() {
        return "unknown".to_string();
    }

    let lower = value.to_ascii_lowercase();
    if let Some(alias) = known_alias(&lower) {
        return alias.to_string();
    }
    if let Some(web_app) = chrome_web_app_name(&value) {
        return web_app;
    }

    value
}

pub fn display_name(app_class: &str) -> String {
    let canonical = canonical_app_class(app_class);
    let lower = canonical.to_ascii_lowercase();
    if let Some(name) = known_display_name(&lower) {
        return name.to_string();
    }

    let mut value = canonical.as_str();
    let mut package_like = false;
    for prefix in ["com.", "org.", "io.", "net.", "app."] {
        if let Some(stripped) = value.strip_prefix(prefix) {
            value = stripped;
            package_like = true;
            break;
        }
    }
    if package_like && value.contains('.') {
        value = value.rsplit('.').next().unwrap_or(value);
    }

    let mut cleaned = clean_spaces(value);
    if let Some(stripped) = cleaned.strip_suffix(".x86_64") {
        cleaned = stripped.to_string();
    }
    if cleaned.is_empty() {
        return "Unknown".to_string();
    }

    if looks_like_slug(&cleaned) {
        return title_case_slug(&cleaned);
    }

    cleaned
}

pub fn clean_window_title(title: &str, app_class: &str) -> Option<String> {
    let mut value = trim_decorative_prefix(&strip_browser_suffixes(&clean_spaces(title)));
    value = value.trim_matches([' ', '\t', '\n', '\r']).to_string();

    if value.is_empty() || value == "0x0" {
        return None;
    }

    let display = display_name(app_class);
    if value.eq_ignore_ascii_case(&display) {
        return Some(display);
    }

    const MAX_TITLE_CHARS: usize = 180;
    if value.chars().count() > MAX_TITLE_CHARS {
        let mut truncated = value.chars().take(MAX_TITLE_CHARS).collect::<String>();
        truncated.push_str("...");
        value = truncated;
    }

    Some(value)
}

fn known_alias(lower: &str) -> Option<&'static str> {
    match lower {
        "zen-bin" | "zen_browser" => Some("zen"),
        "brave-browser" => Some("brave"),
        "chrome" => Some("google-chrome"),
        "edge" => Some("microsoft-edge"),
        _ => None,
    }
}

fn known_display_name(lower: &str) -> Option<&'static str> {
    match lower {
        "zen" => Some("Zen Browser"),
        "firefox" => Some("Firefox"),
        "librewolf" => Some("LibreWolf"),
        "waterfox" => Some("Waterfox"),
        "tor-browser" => Some("Tor Browser"),
        "mullvad-browser" => Some("Mullvad Browser"),
        "google-chrome" => Some("Chrome"),
        "chromium" => Some("Chromium"),
        "brave" => Some("Brave"),
        "vivaldi" => Some("Vivaldi"),
        "microsoft-edge" => Some("Edge"),
        "discord" => Some("Discord"),
        "steam" => Some("Steam"),
        "foot" => Some("Foot"),
        "ghostty" | "com.mitchellh.ghostty" => Some("Ghostty"),
        "org.omarchy.terminal" => Some("Terminal"),
        "org.omarchy.btop" => Some("btop"),
        "org.omarchy.agent" => Some("Agent"),
        "org.gnome.nautilus" => Some("Files"),
        "app.magicpods" => Some("MagicPods"),
        "com.github.wwmm.easyeffects" => Some("EasyEffects"),
        "localsend" => Some("LocalSend"),
        "opendeck" => Some("Open Deck"),
        "qdirstat" => Some("QDirStat"),
        "r2modman" => Some("r2modman"),
        "tf_linux64" => Some("Team Fortress 2"),
        "cs2" => Some("Counter-Strike 2"),
        "chatgpt" => Some("ChatGPT"),
        "github" => Some("GitHub"),
        "youtube" => Some("YouTube"),
        "youtube music" => Some("YouTube Music"),
        _ => None,
    }
}

fn chrome_web_app_name(app_class: &str) -> Option<String> {
    let lower = app_class.to_ascii_lowercase();
    let body = [
        "chrome-",
        "chromium-",
        "brave-",
        "vivaldi-",
        "microsoft-edge-",
    ]
    .iter()
    .find_map(|prefix| lower.strip_prefix(prefix))?;

    let body = body
        .strip_suffix("-default")
        .or_else(|| body.split_once("-profile").map(|(left, _)| left))
        .unwrap_or(body);
    let host = body.split("__").next().unwrap_or(body);
    let host = host.trim_matches(['-', '_', '.']);
    if host.is_empty() {
        return None;
    }

    match host {
        "discord.com" | "canary.discord.com" | "ptb.discord.com" => Some("discord".to_string()),
        "chatgpt.com" | "chat.openai.com" => Some("ChatGPT".to_string()),
        "github.com" => Some("GitHub".to_string()),
        "youtube.com" | "www.youtube.com" => Some("YouTube".to_string()),
        "music.youtube.com" => Some("YouTube Music".to_string()),
        _ => Some(domain_display_name(host)),
    }
}

fn domain_display_name(host: &str) -> String {
    let host = host.strip_prefix("www.").unwrap_or(host);
    let label = host.split('.').next().unwrap_or(host);
    title_case_slug(label)
}

fn strip_browser_suffixes(title: &str) -> String {
    let mut value = title.to_string();
    for suffix in [
        "Zen Browser",
        "Mozilla Firefox",
        "Firefox",
        "Google Chrome",
        "Chromium",
        "Brave",
        "Microsoft Edge",
        "Discord",
    ] {
        for separator in [" - ", " | ", " \u{2013} ", " \u{2014} "] {
            let ending = format!("{separator}{suffix}");
            if value.ends_with(&ending) {
                value.truncate(value.len() - ending.len());
                return value.trim().to_string();
            }
        }
    }
    value
}

fn trim_decorative_prefix(title: &str) -> String {
    let trimmed = title.trim_start();
    let Some((index, _)) = trimmed.char_indices().find(|(_, ch)| {
        ch.is_alphanumeric() || matches!(ch, '#' | '@' | '[' | '(' | '{' | '/' | '~' | '.')
    }) else {
        return trimmed.to_string();
    };
    trimmed[index..].to_string()
}

fn clean_spaces(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_slug(value: &str) -> bool {
    value.contains('-')
        || value.contains('_')
        || value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
}

fn title_case_slug(value: &str) -> String {
    value
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!(
                "{}{}",
                first.to_uppercase(),
                chars.as_str().to_ascii_lowercase()
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{canonical_app_class, clean_window_title, display_name};

    #[test]
    fn labels_common_package_classes() {
        assert_eq!(display_name("com.mitchellh.ghostty"), "Ghostty");
        assert_eq!(display_name("org.gnome.Nautilus"), "Files");
        assert_eq!(display_name("zen"), "Zen Browser");
        assert_eq!(display_name("BeamNG.drive"), "BeamNG.drive");
        assert_eq!(display_name("Minecraft* 1.21.1"), "Minecraft* 1.21.1");
    }

    #[test]
    fn canonicalizes_chrome_web_apps() {
        assert_eq!(
            canonical_app_class("chrome-discord.com__channels_@me-Default"),
            "discord"
        );
        assert_eq!(
            canonical_app_class("chrome-chatgpt.com__-Default"),
            "ChatGPT"
        );
    }

    #[test]
    fn cleans_browser_window_titles() {
        assert_eq!(
            clean_window_title("Issue #1 - Mozilla Firefox", "firefox").as_deref(),
            Some("Issue #1")
        );
        assert_eq!(
            clean_window_title("\u{2827} timetracker", "com.mitchellh.ghostty").as_deref(),
            Some("timetracker")
        );
        assert_eq!(
            clean_window_title("Steam", "steam").as_deref(),
            Some("Steam")
        );
    }
}
