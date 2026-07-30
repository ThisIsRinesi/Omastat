#!/usr/bin/env bash
set -euo pipefail

service_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
mkdir -p "$service_dir"
install -m 0644 "$(dirname "$0")/omastat.service" "$service_dir/omastat.service"
systemctl --user daemon-reload
if systemctl --user list-unit-files --no-legend hours-played.service >/dev/null 2>&1; then
  systemctl --user disable --now hours-played.service >/dev/null 2>&1 || true
fi
if [[ -e "$service_dir/hours-played.service" ]]; then
  rm "$service_dir/hours-played.service"
fi
systemctl --user enable --now omastat.service
systemctl --user status --no-pager omastat.service
