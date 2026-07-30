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
cargo run -p hours-played -- week
cargo run -p hours-played -- apps
```

The tracker uses Hyprland's event socket for focus/open/close events and uses
`hyprctl -j clients` plus `hyprctl -j activewindow` for startup and recovery
snapshots.

## Systemd User Service

A sample service is available at:

```text
packaging/systemd/hours-played.service
```

Install it manually after building or packaging the binary.

