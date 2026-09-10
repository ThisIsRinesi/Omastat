#!/usr/bin/env bash
# Shared paths and preflight checks for the user-level installer/uninstaller.

plugin_id="local.omastat"
backend_root="$HOME/.cargo"
# Omarchy's plugin registry uses this path independently of XDG_CONFIG_HOME.
plugin_dir="$HOME/.config/omarchy/plugins/$plugin_id"
service_dir="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
backup_root="${XDG_STATE_HOME:-$HOME/.local/state}/omastat/install-backups"
export OMARCHY_PATH="${OMARCHY_PATH:-/usr/share/omarchy}"

fail() { printf 'omastat: %s\n' "$*" >&2; exit 1; }

require_commands() {
  local command
  for command in "$@"; do
    command -v "$command" >/dev/null 2>&1 || fail "Missing required command: $command"
  done
}

require_session() {
  [[ $EUID -ne 0 ]] || fail "Run this as your desktop user, without sudo."
  require_commands python3 cargo systemctl omarchy omarchy-shell jq
  systemctl --user show-environment >/dev/null || fail "A running systemd user session is required."
  omarchy-shell shell ping >/dev/null || fail "Run this inside your running Omarchy desktop session."
}

plugin_known() {
  local plugins
  plugins=$(omarchy-shell shell listPlugins) || return 1
  jq -e --arg id "$plugin_id" 'any(.[]; .id == $id)' <<<"$plugins" >/dev/null
}

wait_for_plugin() {
  local attempt
  for ((attempt = 0; attempt < 40; attempt++)); do
    plugin_known && return 0
    sleep 0.1
  done
  fail "Omarchy did not discover $plugin_id. Try: omarchy-shell shell rescanPlugins"
}
