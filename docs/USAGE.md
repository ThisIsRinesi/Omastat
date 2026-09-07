# Usage

## Installation and removal

Run `./install.sh` from a checkout outside `~/.config/omarchy/plugins/local.omastat/`,
as your normal user inside a running Omarchy desktop session. It installs the
backend to `~/.cargo/bin`, restarts the systemd user service, and installs/enables
the bar plugin. Add `~/.cargo/bin` to your session's `PATH` for CLI use.
`./install.sh --with-browser` also installs the optional Zen/Firefox integration.

New installs use [Omarchy's supported local-plugin flow](https://github.com/omacom/omarchy/blob/quattro/shell/README.md#installing-by-hand).
Rerun the installer from an updated checkout to upgrade both components. Existing
Git-managed plugins use `omarchy plugin update`; the backend is built from that
updated checkout, preserving the plugin's origin and history. Existing bar
placement and settings are retained.

Replaced local plugin files are backed up under
`${XDG_STATE_HOME:-~/.local/state}/omastat/install-backups/`.
`./uninstall.sh` uses `omarchy plugin remove`, including its backup behavior for
local copies, then removes the user service, Cargo backend, and optional browser
integration. Recorded activity, configuration, and installer backups are kept.
Both scripts can be rerun. System-wide package installs must be removed with
their package manager.

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
omastat widget-summary --lens day
```

`summary` is the compact JSON report used by the Omarchy widget. It accepts
`--lens day|week|month|year|life` and `--offset -N`; the default remains the
current day. Its payload includes app totals, daily totals, a week-by-hour
`heatmap`, and structured `insights` records with `kind`, `category`, `tone`,
`title`, `value`, `explanation`, `confidence`, `evidence`, and `supporting`
fields so widgets and future commands can use the same analysis output without
parsing display labels.

`widget-summary` is the lightweight JSON path used by the closed bar widget. It
returns current totals, period metadata, top-app display data, and preformatted
bar text without daily history, heatmaps, timeline rollups, or full insight
analysis. Opening the widget panel still uses `summary` for the full analytics
view.

`insights --json` emits the same structured insight records with period
metadata and focus/system totals, but leaves out the heavier app rows and
daily history. Use `--lens day|week|month|year|life` and `--offset -N` to query
the same report periods used by exports and the widget.

`goals` shows configured daily focus target and app/category budget progress.
`digest` prints a compact period summary with top apps, high-signal insights,
and goal status. `widget-insight` returns one rotating fact from the shared
insight engine for scripts or bar widgets.

## Dashboard navigation

Select a calendar day or daily trend point to open that Day view. Year chart
months open Month; lifetime chart weeks open Week. Weeks start on Monday.
Click or use Tab, arrow keys on the trend, and Enter or Space to activate.
The selected app or website follows you into the new period. Historical views
provide a Today / This week / This month / This year button to return to the
current period. Blank cells and future dates cannot be opened.

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
browser_domains = true
browser_history = false
title_allowlist = []
title_blocklist = []

[tracking]
reconcile_seconds = 300
session_poll_seconds = 60
idle_timeout_seconds = 180
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

Idle tracking uses Wayland `ext-idle-notify-v1` when the compositor exposes it.
`idle_timeout_seconds` controls how long the seat must have no keyboard, mouse,
or touch input before Omastat records idle time. When the Wayland monitor is not
available, Omastat falls back to Omarchy/logind session status polling; logind's
idle-since timestamp is used when available so idle intervals can still be
backdated to the real session transition.

Browser domain tracking is the preferred browser breakdown path. With
`browser_domains = true`, the optional Zen/Firefox extension sends only the
active tab domain to `omastat native-host`; Omastat counts that domain only
where it overlaps focused browser window time. The extension does not send tab
titles, full URLs, page contents, or history.

Install the native host and profile extension with:

```bash
packaging/browser-extension/install.sh
```

Restart Zen/Firefox after installing. The development reinstall script runs the
same browser-extension install step.

Firefox release builds require signed add-ons. If Zen enforces the same policy
and refuses the local XPI, the native host is still installed but the extension
must be loaded temporarily for development or signed for regular use.

Set `title_capture = "all"` only if you want focused intervals to include
cleaned window titles. When title capture is enabled and no direct domain rows
exist for a period, browser windows can fall back to page/title inference in
the widget panel. Set `browser_history = true` to let Omastat enrich Zen
Browser titles from local `~/.zen/*/places.sqlite` history files. History
enrichment is read-only, local, best-effort, and ignored unless
`title_capture = "all"` is also enabled.

App aliases change display labels in reports and exports while raw
exports keep the original app class. Categories are local strings normalized to
lowercase kebab-case, so `productive`, `distracting`, `neutral`, and custom
categories can all be used for grouping and budgets.

Existing databases can be normalized after upgrades:

```bash
omastat repair-titles --dry-run
omastat repair-titles
```

Focused intervals also store workspace and monitor context when Hyprland exposes
it. This metadata is available in raw exports and does not require title capture.

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

## Activity details

```bash
omastat activity-detail --lens week --app zen
omastat activity-detail --lens month --offset -1 --domain github.com
```

Exactly one of `--app` or `--domain` is required. Use the stable `key` from
`summary` → `activity_analytics.activities`; labels are for display. Details
are JSON with activity statistics, daily totals, a weekday/hour heatmap,
and structured insights including occurrence dates and eligible sample counts.
An unknown activity returns empty statistics and charts for the requested
period. Privacy settings control whether website records are included.

`summary` includes additive `activity_analytics` metadata and searchable
activity statistics. The existing top-app grouping remains available for
other clients. The widget requests detail aggregates on demand and discards
responses for superseded selections.

Recorded coverage includes known inactivity and sleep, but excludes gaps in
recording. Lifetime elapsed time starts at retained recording history. A
stale daemon's unfinished focus ends at its last confirmed heartbeat rather
than growing until the next restart. Raw interval exports retain stored
start/end values; aggregate reports apply the liveness boundary.
