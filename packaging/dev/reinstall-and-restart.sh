#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
plugin_id="local.omastat"
widget_dir="$repo_root/packaging/omarchy/omastat"
service_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
plugin_dir="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/$plugin_id"

validate_omarchy_plugin() {
  local dir="$1"

  if command -v omarchy >/dev/null 2>&1; then
    omarchy plugin validate "$dir"
  fi
}

wait_for_omarchy_plugin() {
  local id="$1"
  local plugins

  command -v jq >/dev/null 2>&1 || return 1
  command -v omarchy-shell >/dev/null 2>&1 || return 1

  for (( attempt = 0; attempt < 40; attempt++ )); do
    if plugins=$(omarchy-shell shell listPlugins 2>/dev/null) &&
      jq -e --arg id "$id" 'any(.[]; .id == $id)' >/dev/null <<<"$plugins"; then
      return 0
    fi
    sleep 0.05
  done

  return 1
}

refresh_omarchy_plugin_data() {
  command -v omarchy >/dev/null 2>&1 || return 0

  validate_omarchy_plugin "$plugin_dir"

  if command -v omarchy-shell >/dev/null 2>&1 &&
    omarchy-shell shell ping >/dev/null 2>&1; then
    if omarchy-shell shell rescanPlugins >/dev/null 2>&1 &&
      wait_for_omarchy_plugin "$plugin_id"; then
      return 0
    fi

    echo "Omarchy did not discover $plugin_id after plugin data rescan; restarting shell." >&2
  fi

  omarchy restart shell
  wait_for_omarchy_plugin "$plugin_id" || {
    echo "Omarchy shell restarted, but $plugin_id is still missing from plugin data." >&2
    return 1
  }
}

validate_omarchy_plugin "$widget_dir"
cargo install --path "$repo_root/crates/omastat" --locked
if command -v hours-played >/dev/null 2>&1; then
  cargo uninstall hours-played >/dev/null 2>&1 || true
fi

mkdir -p "$service_dir"
install -m 0644 "$repo_root/packaging/systemd/omastat.service" "$service_dir/omastat.service"
mkdir -p "$plugin_dir"
rsync -a --delete "$widget_dir/" "$plugin_dir/"
validate_omarchy_plugin "$plugin_dir"

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

refresh_omarchy_plugin_data
