#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cargo install --path "$repo_root/crates/hours-played" --locked

service_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
mkdir -p "$service_dir"
install -m 0644 "$repo_root/packaging/systemd/hours-played.service" "$service_dir/hours-played.service"

systemctl --user daemon-reload
systemctl --user enable hours-played.service >/dev/null
systemctl --user restart hours-played.service
systemctl --user --no-pager --lines=20 status hours-played.service

