# Hours Played

Hours Played is a local usage tracker for Arch/Omarchy systems running Hyprland.
It records how long applications are open and how long they are focused, similar
to a mix of Screen Time and game "hours played" stats.

The initial scaffold is a foreground daemon plus CLI reports. A future Omarchy
shell widget can read the same SQLite data for glanceable stats and replay-style
summaries.

## Privacy Defaults

By default, Hours Played stores application class names and timing intervals. It
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
${XDG_CONFIG_HOME:-~/.config}/hours-played/config.toml
```

Database path:

```text
${XDG_DATA_HOME:-~/.local/share}/hours-played/hours-played.db
```

## Development

```bash
cargo test
cargo run -p hours-played -- doctor
cargo run -p hours-played -- daemon
cargo run -p hours-played -- today
cargo run -p hours-played -- --json today
cargo run -p hours-played -- week
cargo run -p hours-played -- apps
cargo run -p hours-played -- range --from 2026-07-01 --to 2026-07-30
cargo run -p hours-played -- tui
```

The tracker uses Hyprland's event socket for focus/open/close events and uses
`hyprctl -j clients` plus `hyprctl -j activewindow` for startup and recovery
snapshots.

Focused time follows Omarchy's own idle service. The daemon first reads
`omarchy-shell idle status` and `omarchy-shell lock isLocked`, which means the
same stay-awake toggle used by the Omarchy bar controls whether idle tracking is
armed. If Omarchy shell IPC is unavailable, the daemon falls back to
`loginctl show-session`. Open time continues while apps remain open.

## TUI

The interactive terminal dashboard uses Ratatui:

```bash
hours-played tui
```

Controls:

```text
Left/Right or h/l  Switch Today, Week, All Time
1/2/3              Jump to a view
r                  Refresh from SQLite
q or Esc           Quit
```

## Systemd User Service

A sample service is available at:

```text
packaging/systemd/hours-played.service
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

It expects a release tarball named `hours-played-0.1.0.tar.gz`.

## Omarchy Widget

A repository-contained Omarchy shell plugin lives at:

```text
packaging/omarchy/hours-played/
```

It reads `hours-played --json today`, shows the top focused app and today's
total focused time, and opens `hours-played today` in a terminal on click.

Install it into your user plugin directory when you want to try it:

```bash
mkdir -p ~/.config/omarchy/plugins/local.hours-played
cp packaging/omarchy/hours-played/* ~/.config/omarchy/plugins/local.hours-played/
omarchy plugin rescan
```
