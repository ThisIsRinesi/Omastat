use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

const TERMINAL_CLASSES: &[&str] = &[
    "foot",
    "alacritty",
    "kitty",
    "ghostty",
    "wezterm",
    "konsole",
    "gnome-terminal",
    "tilix",
    "xfce4-terminal",
    "termite",
    "st",
    "org.omarchy.terminal",
];

const BROWSER_SUBPROCESS_COMMS: &[&str] = &[
    "Web Content",
    "forkserver",
    "socket",
    "rdd",
    "utility",
    "tab",
    "GPU Process",
    "Content Process",
    "Utility Process",
    "Isolated Web App",
    "WebExtensions",
    "spellcheck",
    "renderer",
    "Renderer",
    "zygote",
    "gpu-process",
    "GPU",
    "Crashpad Handler",
    "Chrome_ChildThread",
];

const BROWSER_ALIASES: &[(&str, &str)] = &[
    ("zen-bin", "zen"),
    ("zen_browser", "zen"),
    ("zen", "zen"),
    ("firefox", "firefox"),
    ("librewolf", "librewolf"),
    ("waterfox", "waterfox"),
    ("tor-browser", "tor-browser"),
    ("mullvad-browser", "mullvad-browser"),
    ("google-chrome", "google-chrome"),
    ("chrome", "google-chrome"),
    ("chromium", "chromium"),
    ("brave", "brave"),
    ("brave-browser", "brave"),
    ("vivaldi", "vivaldi"),
    ("microsoft-edge", "microsoft-edge"),
    ("edge", "microsoft-edge"),
];

#[derive(Debug, Clone)]
struct ProcInfo {
    comm: String,
    ppid: i64,
    tpgid: i64,
}

pub fn should_track_class(app_class: &str) -> bool {
    let id = app_class.trim().to_ascii_lowercase();
    if id.is_empty() {
        return false;
    }
    if id == "org.omarchy.screensaver" {
        return false;
    }
    !id.starts_with("xdg-desktop-portal")
}

pub fn is_terminal_class(app_class: &str) -> bool {
    let id = app_class.trim().to_ascii_lowercase();
    TERMINAL_CLASSES.iter().any(|terminal| *terminal == id)
}

pub fn canonical_app(name: &str) -> String {
    let value = name.trim();
    BROWSER_ALIASES
        .iter()
        .find_map(|(alias, canonical)| (*alias == value).then_some((*canonical).to_string()))
        .unwrap_or_else(|| value.to_string())
}

pub fn resolve_foreground_app(terminal_pid: i64) -> Option<String> {
    if terminal_pid <= 0 {
        return None;
    }

    let infos = proc_infos()?;
    let pty = infos.keys().find_map(|pid| {
        is_descendant(*pid, terminal_pid, &infos)
            .then(|| fd0_target(*pid))
            .flatten()
            .filter(|target| target.starts_with("/dev/pts/"))
    })?;

    let tpgid = infos.iter().find_map(|(pid, info)| {
        (fd0_target(*pid).as_deref() == Some(pty.as_str())).then_some(info.tpgid)
    })?;
    if tpgid <= 0 {
        return None;
    }

    let mut pid = tpgid;
    let mut name = proc_name(pid, &infos)?;
    while BROWSER_SUBPROCESS_COMMS.iter().any(|comm| *comm == name) {
        let stat = infos.get(&pid)?;
        if stat.ppid <= 1 || !infos.contains_key(&stat.ppid) {
            break;
        }
        pid = stat.ppid;
        let Some(parent_name) = proc_name(pid, &infos) else {
            break;
        };
        name = parent_name;
    }

    let canonical = canonical_app(&name);
    (!canonical.is_empty()).then_some(canonical)
}

fn proc_infos() -> Option<HashMap<i64, ProcInfo>> {
    let mut infos = HashMap::new();
    for entry in fs::read_dir("/proc").ok()?.flatten() {
        let pid = entry.file_name().to_string_lossy().parse::<i64>().ok();
        let Some(pid) = pid else {
            continue;
        };
        let path = entry.path().join("stat");
        let Ok(contents) = fs::read_to_string(path) else {
            continue;
        };
        if let Some(info) = parse_proc_stat(&contents) {
            infos.insert(pid, info);
        }
    }
    Some(infos)
}

fn parse_proc_stat(contents: &str) -> Option<ProcInfo> {
    let left = contents.find('(')?;
    let right = contents.rfind(')')?;
    let comm = contents.get(left + 1..right)?.to_string();
    let fields = contents
        .get(right + 1..)?
        .split_whitespace()
        .collect::<Vec<_>>();
    if fields.len() < 6 {
        return None;
    }

    Some(ProcInfo {
        comm,
        ppid: fields.get(1)?.parse().ok()?,
        tpgid: fields.get(5)?.parse().ok()?,
    })
}

fn is_descendant(mut pid: i64, root: i64, infos: &HashMap<i64, ProcInfo>) -> bool {
    let mut seen = HashSet::new();
    while pid > 1 && seen.insert(pid) {
        if pid == root {
            return true;
        }
        pid = infos.get(&pid).map(|info| info.ppid).unwrap_or(0);
    }
    pid == root
}

fn fd0_target(pid: i64) -> Option<String> {
    fs::read_link(format!("/proc/{pid}/fd/0"))
        .ok()?
        .to_str()
        .map(ToOwned::to_owned)
}

fn proc_name(pid: i64, infos: &HashMap<i64, ProcInfo>) -> Option<String> {
    let info = infos.get(&pid)?;
    let cmdline = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let argv0 = cmdline
        .split(|byte| *byte == 0)
        .next()
        .and_then(|value| (!value.is_empty()).then_some(value));

    if let Some(argv0) = argv0
        && let Some(name) = Path::new(std::str::from_utf8(argv0).ok()?)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
    {
        return Some(name.to_string());
    }

    Some(info.comm.clone())
}

#[cfg(test)]
mod tests {
    use super::{canonical_app, is_terminal_class, parse_proc_stat, should_track_class};

    #[test]
    fn parses_proc_stat_names_with_spaces() {
        let stat = parse_proc_stat("123 (Web Content) S 10 20 30 40 50 60 70").unwrap();
        assert_eq!(stat.comm, "Web Content");
        assert_eq!(stat.ppid, 10);
        assert_eq!(stat.tpgid, 50);
    }

    #[test]
    fn canonicalizes_browser_process_names() {
        assert_eq!(canonical_app("zen-bin"), "zen");
        assert_eq!(canonical_app("brave-browser"), "brave");
        assert_eq!(canonical_app("opencode"), "opencode");
    }

    #[test]
    fn identifies_terminal_classes() {
        assert!(is_terminal_class("ghostty"));
        assert!(is_terminal_class("org.omarchy.terminal"));
        assert!(!is_terminal_class("firefox"));
    }

    #[test]
    fn filters_non_user_facing_shell_windows() {
        assert!(!should_track_class(""));
        assert!(!should_track_class("org.omarchy.screensaver"));
        assert!(!should_track_class("xdg-desktop-portal-gtk"));
        assert!(should_track_class("firefox"));
    }
}
