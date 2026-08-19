# Usage

## Reports

```bash
omastat today
omastat week
omastat apps
omastat range --from 2026-07-01 --to 2026-07-30
omastat --json today
omastat summary
```

`summary` is the compact JSON report used by the Omarchy widget.

## TUI

```bash
omastat tui
```

Controls:

```text
Tab / Shift+Tab     Cycle Overview, Apps, Timeline, System views
Left/Right or h/l   Cycle Day, Week, Month, Year, Life lenses
[/]                 Move to previous/next period
1/2/3/4/5           Jump to a lens
Up/Down or j/k      Move the app selection
PageUp/PageDown     Jump the selection
p                   Toggle overview focus stats / period signals
r                   Refresh from SQLite
q or Esc            Quit
```

## HTML Export

Create a static, self-contained replay:

```bash
omastat export --lens week --output ~/Pictures/omastat-week.html
omastat export --lens month --output ~/Pictures/omastat-month.html
omastat export --lens life --title "Lifetime App Replay"
```

The export includes focused/open totals, stacked daily app columns, ranked apps,
an app bubble chart, a week-by-hour heatmap, title rows when title capture is
enabled, and Day/Week/Month/Year/Life totals.

## Configuration

Config path:

```text
${XDG_CONFIG_HOME:-~/.config}/omastat/config.toml
```

Database path:

```text
${XDG_DATA_HOME:-~/.local/share}/omastat/omastat.db
```

Default config values:

```toml
[privacy]
title_capture = "off"

[tracking]
reconcile_seconds = 300
session_poll_seconds = 60
terminal_resolve_seconds = 5
pause_on_session_idle = true
pause_on_session_locked = true
```

Set `title_capture = "all"` only if you want focused intervals to include
cleaned window titles. Existing databases can be normalized after upgrades:

```bash
omastat repair-titles --dry-run
omastat repair-titles
```

## Tracking Model

The daemon uses Hyprland's event socket for focus/open/close events and uses
`hyprctl -j clients` plus `hyprctl -j activewindow` for startup and recovery
snapshots.

Focused time follows Omarchy's own idle and lock state when available. If
Omarchy shell IPC is unavailable, the daemon falls back to `loginctl
show-session`. Active audio playback is detected through `pactl list
sink-inputs`; when playback is running, idle status does not pause focused time.
Open time continues while apps remain open.
