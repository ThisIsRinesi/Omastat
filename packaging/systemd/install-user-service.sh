#!/usr/bin/env bash
set -euo pipefail

service_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
mkdir -p "$service_dir"
install -m 0644 "$(dirname "$0")/omastat.service" "$service_dir/omastat.service"
systemctl --user daemon-reload
systemctl --user enable omastat.service
systemctl --user restart omastat.service
systemctl --user is-active --quiet omastat.service
