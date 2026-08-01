# Omastat

Omastat is a local app usage utility for Arch/Omarchy systems running Hyprland.
It records how long applications are open and how long they are focused.

The current shape is a background daemon, CLI reports, an interactive TUI, and
an Omarchy shell widget. Future replay views can build on the same local SQLite
data.

## Privacy Defaults

By default, Omastat stores application class names and timing intervals. It
does not store window titles, page names, file names, or screenshots.

Title capture can be enabled explicitly in:

```toml
[privacy]
title_capture = "all"

[tracking]
reconcile_seconds = 300
session_poll_seconds = 30
pause_on_session_idle = true
pause_on_session_locked = true
```

Config path:

```text
${XDG_CONFIG_HOME:-~/.config}/omastat/config.toml
```

Database path:

```text
${XDG_DATA_HOME:-~/.local/share}/omastat/omastat.db
```

## Development

```bash
cargo test
cargo run -p omastat -- doctor
cargo run -p omastat --bin omastatd
cargo run -p omastat -- today
cargo run -p omastat -- --json today
cargo run -p omastat -- week
cargo run -p omastat -- apps
cargo run -p omastat -- range --from 2026-07-01 --to 2026-07-30
cargo run -p omastat -- tui
```

The daemon uses Hyprland's event socket for focus/open/close events and uses
`hyprctl -j clients` plus `hyprctl -j activewindow` for startup and recovery
snapshots.

Focused time follows Omarchy's own idle service. The daemon first reads
`omarchy-shell idle status` and `omarchy-shell lock isLocked`, which means the
same stay-awake toggle used by the Omarchy bar controls whether idle accounting is
armed. If Omarchy shell IPC is unavailable, the daemon falls back to
`loginctl show-session`. Open time continues while apps remain open.

## TUI

The interactive terminal dashboard uses Ratatui:

```bash
omastat tui
```

The TUI reads generated matugen/Skwd-wall colors from Noctalia or Omarchy theme
files when available, then applies a high-contrast cyberpunk fallback palette for
unconfigured terminals.

Controls:

```text
Left/Right or h/l  Cycle Day, Week, Month, Year, Life lenses
1/2/3/4/5          Jump to a lens
Up/Down or j/k     Move the app inspector selection
PageUp/PageDown    Jump the selection
r                  Refresh from SQLite
q or Esc           Quit
```

## Systemd User Service

A sample service is available at:

```text
packaging/systemd/omastat.service
```

After installing the binary somewhere in PATH, install the user service with:

```bash
packaging/systemd/install-user-service.sh
```

For local development, install the current build and a Git `post-commit` hook
that rebuilds, reinstalls, and restarts the user service after every commit:

```bash
packaging/dev/install-autoupdate.sh
```

## Arch Packaging

A starter `PKGBUILD` lives at:

```text
packaging/arch/PKGBUILD
```

It expects a release tarball named `omastat-0.1.0.tar.gz`.

## Omarchy Quattro Plugin

A repository-contained Omarchy Quattro/Quickshell plugin lives at:

```text
packaging/omarchy/omastat/
```

It reads `omastat --json today`, shows the top focused app and today's total
focused time, and opens a native Omarchy popup on click. Middle-click opens the
TUI in a held terminal; right-click refreshes.

Install it into your user plugin directory when you want to try it:

```bash
mkdir -p ~/.config/omarchy/plugins/local.omastat
cp packaging/omarchy/omastat/* ~/.config/omarchy/plugins/local.omastat/
omarchy plugin rescan
omarchy plugin enable local.omastat
omarchy bar plugin add local.omastat right
```
