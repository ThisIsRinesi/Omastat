# Usage

## Reports

```bash
omastat today
omastat week
omastat apps
omastat range --from 2026-07-01 --to 2026-07-30
omastat --json today
omastat summary
omastat summary --lens week --offset -1
omastat summary --lens month --days 31
omastat insights --json
omastat insights --lens week --offset -1 --json
omastat goals --lens week
omastat digest --lens week
omastat widget-insight --json
```

`summary` is the compact JSON report used by the Omarchy widget. It accepts
`--lens day|week|month|year|life` and `--offset -N`; the default remains the
current day. Its payload includes app totals, daily totals, a week-by-hour
`heatmap`, and structured `insights` records with `kind`, `category`, `tone`,
`title`, `value`, `explanation`, `confidence`, `evidence`, and `supporting`
fields so widgets and future commands can use the same analysis output without
parsing display labels.

`insights --json` emits the same structured insight records with period
metadata and focused/open/system totals, but leaves out the heavier app rows and
daily history. Use `--lens day|week|month|year|life` and `--offset -N` to query
the same report periods used by exports and the TUI.

`goals` shows configured daily focus target and app/category budget progress.
`digest` prints a compact period summary with top apps, high-signal insights,
and goal status. `widget-insight` returns one rotating fact from the shared
insight engine for scripts or bar widgets.

## TUI

```bash
omastat tui
```

Controls:

```text
Tab / Shift+Tab     Cycle Overview, Insights, Apps, Timeline, System views
Left/Right or h/l   Cycle Day, Week, Month, Year, Life lenses
[/]                 Move to previous/next period
1/2/3/4/5           Jump to a lens
Up/Down or j/k      Move the app selection
PageUp/PageDown     Jump the selection
p                   Toggle overview focus stats / period signals
r                   Refresh from SQLite
q or Esc            Quit
```

The TUI loads theme colors from Noctalia, skwd-wall/Matugen, Omarchy, then the
built-in fallback palette. Press `r` to reload colors after skwd-wall generates a
new palette.

## HTML Export

Create a static, self-contained CLI overview:

```bash
omastat export --lens week --output ~/Pictures/omastat-week.html
omastat export --lens month --output ~/Pictures/omastat-month.html
omastat export --lens life --title "Lifetime App Overview"
```

The export includes focused/open totals, structured period insights, daily
pattern bars, ranked apps, app composition, workspace ranking, session length
distribution, a week-by-hour heatmap, title rows when title capture is enabled,
and Day/Week/Month/Year/Life totals.

## Data Export

Export raw intervals, aggregate rows, or both:

```bash
omastat export-data --lens month --format json --output ~/omastat-month.json
omastat export-data --lens week --format csv --output ~/omastat-week-csv
omastat export-data --scope raw --format json --output ~/omastat-raw.json
omastat export-data --scope aggregate --format csv --output ~/omastat-aggregate
```

JSON writes one file. CSV writes a directory containing `metadata.json` plus
tables such as `raw_intervals.csv`, `raw_session_intervals.csv`,
`raw_system_intervals.csv`, `app_totals.csv`, `app_breakdown.csv`,
`daily_totals.csv`, and `insights.csv`. Raw rows include Unix seconds and local
timestamp strings so exports are explicit about the local time window. System
rows include idle, locked, sleep, and unobserved gaps.

## Data Lifecycle

Delete older local telemetry after reviewing a dry run:

```bash
omastat purge --older-than-days 90 --dry-run
omastat purge --older-than-days 90 --confirm --vacuum
omastat purge --before 2026-01-01 --confirm
```

`purge` requires exactly one selector: `--before YYYY-MM-DD`,
`--older-than-days N`, or `--all`. Destructive purges require `--confirm`;
`--dry-run` reports the affected rows without deleting them. Intervals that
cross the cutoff are trimmed instead of deleted wholesale.

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
title_allowlist = []
title_blocklist = []

[tracking]
reconcile_seconds = 300
session_poll_seconds = 60
terminal_resolve_seconds = 5
heartbeat_seconds = 30
pause_on_session_idle = true
pause_on_session_locked = true

[apps."com.mitchellh.ghostty"]
alias = "Terminal"
category = "productive"

[apps.discord]
category = "distracting"

[goals]
daily_focus_minutes = 180

[[goals.app_budgets]]
category = "distracting"
daily_minutes = 45

[[goals.app_budgets]]
app = "firefox"
weekly_minutes = 600
```

Set `title_capture = "all"` only if you want focused intervals to include
cleaned window titles. `title_allowlist` and `title_blocklist` are optional
case-insensitive substring filters applied to the app class and cleaned title;
blocklist matches win over allowlist matches.

App aliases change display labels in reports, exports, and the TUI while raw
exports keep the original app class. Categories are local strings normalized to
lowercase kebab-case, so `productive`, `distracting`, `neutral`, and custom
categories can all be used for grouping and budgets.

Existing databases can be normalized after upgrades:

```bash
omastat repair-titles --dry-run
omastat repair-titles
```

Focused intervals also store workspace and monitor context when Hyprland exposes
it. This powers the TUI Workspace Focus chart and does not require title capture.

## skwd-wall / Matugen

Omastat reads skwd-wall colors from:

```text
${XDG_CONFIG_HOME:-~/.config}/omastat/theme/colors.json
${XDG_CONFIG_HOME:-~/.config}/omastat/theme/matugen.json
${XDG_CONFIG_HOME:-~/.config}/skwd-wall/colors.json
${XDG_CACHE_HOME:-~/.cache}/skwd/colors.json
${XDG_CACHE_HOME:-~/.cache}/skwd-wall/colors.json
```

For a dedicated integration, copy
`packaging/skwd-wall/omastat-colors.json` into your skwd-wall templates
directory and add this to the `integrations` array in
`~/.config/skwd-wall/config.json`:

```json
{
  "name": "omastat",
  "template": "omastat-colors.json",
  "output": "~/.config/omastat/theme/colors.json"
}
```

## Tracking Model

The daemon uses Hyprland's event socket for focus/open/close events and uses
`hyprctl -j clients` plus `hyprctl -j activewindow` for startup and recovery
snapshots.

Focused time follows Omarchy's own idle and lock state when available. If
Omarchy shell IPC is unavailable, the daemon falls back to `loginctl
show-session`. Active audio playback is detected through `pactl list
sink-inputs`; when playback is running, idle status does not pause focused time.
The daemon also records local heartbeat events. If it restarts after an
unclean stop, open focus/session intervals are closed at the last observed
boundary and the remaining gap is reported as unobserved excluded time rather
than active focus. Open time continues while apps remain open and the daemon is
observing the session.

On systemd desktops, Omastat listens for logind's `PrepareForSleep` signal on
the system D-Bus and holds a short sleep delay inhibitor when available so it
can close active focus/open/session intervals before suspend. The matching
resume signal closes the sleep interval and rebuilds live Hyprland state from a
fresh snapshot.
