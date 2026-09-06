# Development

## Common Commands

```bash
cargo test
cargo run -p omastat -- doctor
cargo run -p omastat --bin omastatd
cargo run -p omastat -- today
cargo run -p omastat -- --json today
cargo run -p omastat -- summary
cargo run -p omastat -- summary --lens week --offset -1
cargo run -p omastat -- insights --lens week --json
cargo run -p omastat -- export --lens month --output /tmp/omastat-export.html
cargo run -p omastat -- export-data --lens week --format csv --output /tmp/omastat-export-data
cargo run -p omastat -- goals --lens week
cargo run -p omastat -- digest --lens week
cargo run -p omastat -- widget-insight --json
cargo run -p omastat -- purge --older-than-days 90 --dry-run
cargo run -p omastat -- tui
packaging/browser-extension/install.sh
packaging/dev/check-widget-qml.sh
packaging/dev/capture-widget-panel.sh
```

## Local Auto-Update

Install the current build and a Git `post-commit` hook that rebuilds,
reinstalls, and restarts the user service after every commit:

```bash
packaging/dev/install-autoupdate.sh
```

Run the reinstall step directly when needed:

```bash
packaging/dev/reinstall-and-restart.sh
```

## Systemd Service

The sample user service is available at:

```text
packaging/systemd/omastat.service
```

After installing the binary somewhere in `PATH`, install the service with:

```bash
packaging/systemd/install-user-service.sh
```

## Omarchy Plugin Files

The packaged plugin copy lives in:

```text
packaging/omarchy/omastat/
```

The repository root also has a `manifest.json` so Omarchy Quattro can install
the widget directly from GitHub with `omarchy plugin add`.

Keep the root manifest and packaged manifest in sync, except for their
`entryPoints.barWidget` paths.

Check that the manifests remain synchronized with:

```bash
cargo test -p omastat --test manifest_sync
```

`packaging/dev/check-widget-qml.sh` also runs `omarchy plugin validate` when
Omarchy is installed, so invalid plugin data is caught with the QML checks.

`packaging/dev/reinstall-and-restart.sh` validates the source plugin, copies it
to `~/.config/omarchy/plugins/local.omastat/`, validates the installed copy,
installs the Zen/Firefox browser domain extension and native host, then asks
the running shell to rescan plugin data and confirm discovery. If the IPC
reload is unavailable or Omarchy still cannot see the plugin, it falls back to
`omarchy restart shell`.

## Browser Extension

The domain-only browser extension lives in:

```text
packaging/browser-extension/domain-tracker/
```

It sends only active-tab domains to the native messaging host
`io.github.thisisrinesi.omastat`; the host runs `omastat native-host` through a
wrapper installed at `~/.local/bin/omastat-native-host`.

Install or refresh the local browser integration with:

```bash
packaging/browser-extension/install.sh
```

The installer builds separate Zen and Firefox XPI files under
`${XDG_DATA_HOME:-~/.local/share}/omastat/browser-extension/`, copies native
messaging manifests into Zen/Firefox locations, and places the XPI into each
detected profile. Restart the browser after changing extension files.

Capture the live Omarchy panel for visual review with:

```bash
packaging/dev/reinstall-and-restart.sh
packaging/dev/capture-widget-panel.sh
```

The capture script opens `local.omastat` through Quickshell IPC, waits briefly,
writes a PNG under `/tmp`, prints the path, and closes the panel. Use
`--lens day|week|month|year|life` to capture a specific analytics lens,
`--region GEOMETRY` for a panel-only crop, `--select` to pick a region
interactively with `slurp`, or `--keep-open`, `--delay SECONDS`, and
`--output PATH` when a review needs a specific state.

## Dashboard validation

Run `cargo test --locked` and `packaging/dev/check-widget-qml.sh` before
installing. The latter uses Qt 6 tools and covers QML parsing, chart helpers, report/detail
request races, and browser domain lifecycle events. The Rust integration
suite runs activity reports across spring and autumn DST transitions without
changing the process-global timezone.

The dashboard uses `activity.rs` for observation-backed recurrence and visit
analytics. JavaScript formats charts; it does not infer extra insights.
Pattern baselines are separate from visible chart history. Website foreground
slices are shared with the browser breakdown so totals reconcile.

Migration 10 adds browser confirmation timestamps and latest-event state.
Install the CLI, daemon, widget, and browser extension together using the
existing reinstall script. Restart the browser to activate its updated
extension. The migration is additive; older binaries cannot read the new
schema. Back up the SQLite database before installation when validating an
upgrade. Do not rewrite old historical domain attribution from guesses.

Visual checks should include every lens, activity selection, insight evidence,
long labels, an empty period, and a narrow panel. Reports were also checked
against an isolated copy of the local database for overview/detail agreement.

## Pattern and insight engine

`activity.rs` loads a request-scoped analysis context; `routines.rs` evaluates
local-clock recurrence using date bitsets and 15-minute buckets. Coverage uses
merged intervals and cumulative lengths, so overlap queries use binary search.
The context supplies foreground metadata to overview rollups, domain slices to
browser summaries, and history to same-time comparisons. Routine analysis never
extends beyond 56 completed days, even when the selected chart is lifetime.

`InsightSupport.routine` is additive JSON evidence: cadence, status, local
start/end minutes, timing basis (`usage`, `visit-start`, or
`usage-and-visit-start`), eligible dates, and an optional visit-start window.
Ends are exclusive and may wrap past midnight. Matching dates identify the date
on which the window begins. Combined findings require identical matching dates,
eligible dates, cadence, and baseline. The longer qualifying baseline wins;
recent-only observation cannot establish a long-term everyday routine.

Candidates require 60% recurrence and 90% coverage, plus the minimum counts in
the README. A window must contain at least 1.5 times the activity expected from
its fraction of the clock day; this suppresses arbitrary windows for all-day
foreground activity. Confidence, occurrence count, recurrence share, window
width, and stable textual ties determine rank. Related overlapping or adjacent
windows are deduplicated before taking three routines per activity. Overview
selection takes one finding per activity before additional findings and the
12-result limit. No JavaScript detector or persistent analytics cache exists.

Comparisons require 90% observation on both sides and include observed zero-use
periods. Daily anomalies use completed observed dates, seven other baseline
dates, and a median / median-absolute-deviation threshold with a 30-minute floor.
Contiguous same-app telemetry fragments form one foreground block; any time gap
breaks continuity for blocks and app switches. Concentration facts are descriptive
and make no claim about productivity or historical unusualness.

Run the standard Rust and widget checks, plus
`cargo clippy --locked --all-targets -- -D warnings`. `routine_reports` tests overview/detail and cross-lens evidence;
`activity_timezones` tests recurrence and totals through both DST transitions.
`packaging/dev/benchmark-insights.py OLD NEW --database SNAPSHOT` compares release
binaries and asserts unchanged recorded usage totals. Add
`--synthetic-intervals 200000` for 400,000 generated focus/open rows in a temporary
database using the supplied schema, without copying private telemetry.

See [INSIGHTS_AUDIT.md](INSIGHTS_AUDIT.md) for findings and measured performance.

Insight copy is written for the person using the app: use "app time," "a typical
day," and "days with enough tracking" in prose. Keep technical terms and exact
methods in structured evidence and these docs. Routine wording is applied after
ranking so presentation changes cannot change detection. Display local times as
readable clock ranges, while retaining numeric minutes in JSON evidence.

### Dashboard density review (September 2026)

The refined panel was rendered at 1160×920, 600×920, and 380×920 using
Quickshell with the installed Commons/Ui modules, desktop font, and theme.
An isolated offscreen harness substituted a fixed viewport for KeyboardPanel;
this checks dashboard content, not compositor placement or popup anchoring.
Reports came from a consistent SQLite snapshot. Captures covered all five
periods with overview, app, and website selections; narrow chart sections;
long names; empty, loading, and error states; and expanded routine evidence.

At 1160×920, the week overview's rhythm section ends at y=899. The default insight list fits complete
explanations to the available viewport instead of stopping at three; in the
review snapshot, its section ends at y=806. The whole seven-day heatmap is visible.
QtTest keyboard checks exercised trend arrows, heatmap Enter activation,
the search shortcut, and Tab at all three widths. Slay the Spire 2 retains
its explanation above the expanded supporting dates and counting rules.
The prior README week capture and refreshed capture use the same week and
“All activity”; the newer snapshot includes additional recorded usage.

`cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`,
and `packaging/dev/check-widget-qml.sh` passed. The final fresh preview run
reported no dashboard QML errors. QtTest hot reload needed a preview restart;
the offscreen platform's startup warning is expected. README images are
captures of the panel content from this harness.
