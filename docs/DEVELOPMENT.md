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
