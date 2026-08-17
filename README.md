# Omastat

Omastat is a local app usage utility for Arch/Omarchy systems running Hyprland.
It records how long applications are open and how long they are focused.

The current shape is a background daemon, CLI reports, an interactive TUI, and
an Omarchy shell widget. Future replay views can build on the same local SQLite
data.

## Features

- Focused and open time by application, stored locally in SQLite.
- Idle, locked, asleep, and desktop/portal focus are excluded from focused time.
- Idle and locked spans are recorded separately, and active audio playback keeps
  idle sessions counting as active focus for cases like watching video.
- Focused terminals are resolved to the foreground process when possible, so
  time can accrue to `opencode` or `btop` instead of just `ghostty`.
- The Omarchy widget shows today's focused total, a grouped donut breakdown,
  7-day usage patterns, IPC open/close/toggle/status commands, and right-click
  icon-only mode.
- The TUI can replay prior days, weeks, months, and years to show the most-used
  apps for a selected period.

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
terminal_resolve_seconds = 5
pause_on_session_idle = true
pause_on_session_locked = true
```

When enabled, focused intervals store a cleaned window title and split when
Hyprland reports a title change. Browser suffixes such as the browser name are
trimmed so title-based replay data stays readable.

Existing databases can be normalized after upgrades:

```bash
omastat repair-titles --dry-run
omastat repair-titles
```

This rewrites obvious app-class aliases, including Chrome web-app classes and
Steam app IDs when local Steam manifests are available, and fills missing
focused titles with conservative app display names.

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
cargo run -p omastat -- summary
cargo run -p omastat -- export --lens month --output /tmp/omastat-export.html
cargo run -p omastat -- week
cargo run -p omastat -- apps
cargo run -p omastat -- range --from 2026-07-01 --to 2026-07-30
cargo run -p omastat -- repair-titles --dry-run
cargo run -p omastat -- tui
```

The daemon uses Hyprland's event socket for focus/open/close events and uses
`hyprctl -j clients` plus `hyprctl -j activewindow` for startup and recovery
snapshots. It also listens for Hyprland title-change events when title capture
is enabled.

Focused time follows Omarchy's own idle service. The daemon first reads
`omarchy-shell idle status` and `omarchy-shell lock isLocked`, which means the
same stay-awake toggle used by the Omarchy bar controls whether idle accounting is
armed. If Omarchy shell IPC is unavailable, the daemon falls back to
`loginctl show-session`. Active audio playback is detected through `pactl list
sink-inputs`; when playback is running, idle status does not pause focused time.
Open time continues while apps remain open.

## TUI

The interactive terminal dashboard uses Ratatui and is organized around
tabbed views with persistent Day, Week, Month, Year, and Life lenses:

```bash
omastat tui
```

The TUI reads generated matugen/Skwd-wall colors from Noctalia or Omarchy theme
files when available, then applies a high-contrast cyberpunk fallback palette for
unconfigured terminals.

Controls:

```text
Tab / Shift+Tab     Cycle Overview, Apps, Timeline, System views
Left/Right or h/l   Cycle Day, Week, Month, Year, Life lenses
[/]                 Move to previous/next period for replay
1/2/3/4/5           Jump to a lens
Up/Down or j/k      Move the app selection
PageUp/PageDown     Jump the selection
p                   Toggle expanded pattern details
r                   Refresh from SQLite
q or Esc            Quit
```

## HTML Export

Create a static, self-contained one-page visual replay:

```bash
omastat export --lens month --output ~/Pictures/omastat-month.html
omastat export --lens week --offset -1 --output ~/Pictures/omastat-last-week.html
omastat export --lens life --title "Lifetime App Replay"
```

The export is a single HTML file with inline CSS and SVG charts. It includes
focused/open summary stats, stacked daily app columns, ranked top apps, an app
constellation bubble chart, a week-by-hour heatmap, a behavior radar, top
captured titles, and Day/Week/Month/Year/Life totals.

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

It reads `omastat summary`, shows today's total focused time, and opens a
native Omarchy popup on click. The popup includes a grouped app donut and an
expandable 7-day patterns section. Middle-click opens the TUI in a held
terminal; right-click toggles icon-only mode.

Install it into your user plugin directory when you want to try it:

```bash
mkdir -p ~/.config/omarchy/plugins/local.omastat
cp packaging/omarchy/omastat/* ~/.config/omarchy/plugins/local.omastat/
omarchy plugin rescan
omarchy plugin enable local.omastat
omarchy bar plugin add local.omastat right
```
