# Development

## Common Commands

```bash
cargo test
cargo run -p omastat -- doctor
cargo run -p omastat --bin omastatd
cargo run -p omastat -- today
cargo run -p omastat -- --json today
cargo run -p omastat -- summary
cargo run -p omastat -- insights --lens week --json
cargo run -p omastat -- export --lens month --output /tmp/omastat-export.html
cargo run -p omastat -- export-data --lens week --format csv --output /tmp/omastat-export-data
cargo run -p omastat -- goals --lens week
cargo run -p omastat -- digest --lens week
cargo run -p omastat -- widget-insight --json
cargo run -p omastat -- purge --older-than-days 90 --dry-run
cargo run -p omastat -- tui
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
