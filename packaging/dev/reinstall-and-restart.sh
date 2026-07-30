#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cargo install --path "$repo_root/crates/omastat" --locked
if command -v hours-played >/dev/null 2>&1; then
  cargo uninstall hours-played >/dev/null 2>&1 || true
fi

service_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
mkdir -p "$service_dir"
install -m 0644 "$repo_root/packaging/systemd/omastat.service" "$service_dir/omastat.service"

systemctl --user daemon-reload
if systemctl --user list-unit-files --no-legend hours-played.service >/dev/null 2>&1; then
  systemctl --user disable --now hours-played.service >/dev/null 2>&1 || true
fi
if [[ -e "$service_dir/hours-played.service" ]]; then
  rm "$service_dir/hours-played.service"
fi
systemctl --user enable omastat.service >/dev/null
systemctl --user restart omastat.service
systemctl --user --no-pager --lines=20 status omastat.service
