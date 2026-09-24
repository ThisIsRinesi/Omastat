# Development checks

From the checkout, run:

```sh
packaging/dev/check.sh
```

This is also the required command in `.github/workflows/check.yml` for pushes,
pull requests, and manual workflow runs. It stops on failure and does not install
Omastat or restart desktop services. Missing required tools fail the check;
JavaScript checks are never silently skipped.

The command runs Rust formatting, shell syntax checks, QML lint/parse checks,
JavaScript model/controller/motion/browser checks, isolated installer tests,
Clippy with warnings denied, all workspace Rust tests, and locked release builds
of the backend binaries. Runtime UI review and performance benchmarks remain
separate: lint checks cannot verify rendering or desktop integration.

## Dependencies

- Rust/Cargo with rustfmt and Clippy, plus a C compiler.
- Bash, Node.js (CI uses 22), Python 3, Git, jq, ripgrep, and bubblewrap.
- Qt 6 lint and format tools (CI pins 6.8.3; local validation also uses 6.11.2).
  `OMASTAT_QT_BIN_DIR` can select their directory; otherwise `QT_ROOT_DIR/bin`,
  `/usr/lib/qt6/bin`, or the tools on PATH are used.
- Omarchy Quattro's `omarchy-plugin-remove`, `omarchy-plugin-update`, and
  `omarchy-plugin-validate` scripts. The default directory is
  `/usr/share/omarchy/bin`; set `OMASTAT_TEST_OMARCHY_BIN` to another checkout's
  `bin` directory. CI fetches the revision pinned in the workflow.

The installer suite mounts a temporary home and uses fake Cargo, systemd, and
desktop IPC commands. It exercises Omarchy's real plugin update/removal scripts
inside that namespace. The Omarchy script directory is mounted separately so
checkouts under the user's home remain accessible after the home is replaced.
It requires unprivileged user/mount/network namespaces. The disposable Ubuntu
CI runner enables these through its AppArmor user-namespace setting. Daemon
lifecycle tests additionally require local Unix sockets. A restrictive execution
sandbox may need permission to run these tests outside that sandbox.

## Tracking status

`omastat tracking-status` returns JSON from a read-only database connection.
It reads the latest daemon run, current pause/gap records, and latest browser
update, without calculating analytics or exposing domains and window titles.
The tracker freshness threshold matches reporting: three heartbeat intervals,
with a 15-second minimum interval. Browser reports expire after 90 seconds;
clear-domain updates count as reports too. Browser status is aggregate: a recent
report establishes that at least one browser integration is sending updates.

The Settings component requests status only while open. Failed commands or
invalid JSON clear previously successful status and display an unavailable
message. A missing or incompatible database also appears as unavailable; use
`omastat doctor` for detailed diagnostics. Current status is independent of the
historical period selected in the dashboard.

## Dashboard layout audit

On a machine with Quickshell, run:

```sh
python3 packaging/dev/test-widget-layout.py --output /tmp/omastat-layout-audit
```

This separate offscreen audit renders the production panel across five lenses,
four widths (380, 760, 1160, and 2000), three appearances, and seven states:
populated, empty, selected website, loading, error, Settings, and insight evidence.
The 420 cases check text and control bounds, a usable scroll viewport, Settings
inside the scroller, and empty support columns. Screenshots include every
populated layout and compact Settings. Full-content images have transparency.

The fixtures supply synthetic activity, style tokens, a shell lifecycle, and a
window host; the tracking-status process receives synthetic JSON. The audit
never reads the activity database or loads user plugins. Live bar positioning,
compositor focus/dismissal, theme scaling, and Notchbar embedding still require
separate desktop review. The standard CI suite does not require Quickshell.
