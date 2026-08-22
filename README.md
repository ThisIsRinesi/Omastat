# Omastat

Omastat is a local app usage tracker for Arch/Omarchy desktops running
Hyprland. It records focused time and open time to a local SQLite database, then
shows the data through CLI reports, a terminal dashboard, HTML exports, and an
Omarchy Quattro bar widget.

## Features

- Focused and open time by application.
- Idle, locked, asleep, and desktop/portal focus are excluded from focused time.
- Daemon outages and restart recovery gaps are marked as unobserved excluded
  time instead of being counted as active focus.
- Active audio playback keeps idle video or music sessions counted as active
  focus.
- Terminal windows can be attributed to the foreground process, such as `btop`
  or `opencode`, instead of only the terminal emulator.
- Steam app IDs and common desktop classes are normalized to readable names.
- Omarchy Quattro widget with today's total, app breakdown, 7-day patterns, and
  a shortcut into the TUI.
- Rich terminal dashboard with app composition pies, focus-flow charts, hourly
  peaks, heatmaps, workspace focus, focus block stats, and idle, locked, sleep,
  and unobserved signal gauges.
- Structured insights, goals, budgets, weekly digests, and one-line widget
  facts reuse the same local analysis engine.
- App aliases and categories can be configured locally for cleaner labels and
  productive/distracting/custom budget groups.
- Raw and aggregate JSON/CSV exports include local timestamps, app totals,
  daily totals, insights, and excluded system gaps.
- Retention purges support dry-run review, cutoff trimming, and optional
  SQLite vacuuming.
- TUI colors follow Noctalia, skwd-wall/Matugen, or the current Omarchy theme
  when those files are present.
- Static HTML export for shareable day, week, month, year, and lifetime replays.

## Install

From a checkout or release tarball:

```bash
cargo install --path crates/omastat --locked
packaging/systemd/install-user-service.sh
```

Check the daemon:

```bash
omastat doctor
omastat today
```

The Arch packaging recipe is in [packaging/arch/PKGBUILD](packaging/arch/PKGBUILD).

## Omarchy Widget

The Quattro widget requires the `omastat` binary on `PATH` and the user service
running. Install the plugin from GitHub:

```bash
omarchy plugin add https://github.com/ThisIsRinesi/Omastat.git
omarchy plugin enable local.omastat
```

It appears in the right bar section by default. Move it later with:

```bash
omarchy bar move local.omastat --section right
```

Click the widget for the popup, middle-click to open the TUI, and right-click to
toggle icon-only mode.

## Usage

```bash
omastat today
omastat week
omastat summary
omastat insights --json
omastat goals --lens week
omastat digest --lens week
omastat tui
omastat export --lens month --output ~/Pictures/omastat-month.html
omastat export-data --lens month --format csv --output ~/omastat-month-csv
```

More command examples and configuration notes are in [docs/USAGE.md](docs/USAGE.md).

## skwd-wall Theme

Omastat can read skwd-wall/Matugen colors from the default skwd cache paths, or
from a dedicated Omastat output. The template and integration notes are in
[packaging/skwd-wall](packaging/skwd-wall).

## Privacy

By default, Omastat stores application class names, timing intervals, and
workspace/monitor context when Hyprland provides it. It does not store window
titles, page names, file names, screenshots, or browser history.

Optional title capture can be enabled explicitly in the config file when richer
replay labels are worth the extra local data. Optional title allowlists and
blocklists can restrict captured titles further, and `omastat purge` can remove
older local telemetry after a dry-run review. See [docs/USAGE.md](docs/USAGE.md).

## Development

```bash
cargo test
cargo run -p omastat -- doctor
cargo run -p omastat --bin omastatd
```

Development workflow notes are in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## License

MIT
