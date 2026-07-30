#!/usr/bin/env bash
set -euo pipefail

service_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
mkdir -p "$service_dir"
install -m 0644 "$(dirname "$0")/hours-played.service" "$service_dir/hours-played.service"
systemctl --user daemon-reload
systemctl --user enable --now hours-played.service
systemctl --user status --no-pager hours-played.service

