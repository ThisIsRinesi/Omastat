#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./install.sh [--with-browser]

Build and install the backend, start its user service, and install/enable the
Omarchy plugin. Run again from an updated checkout to upgrade.

  --with-browser  Also install the optional Zen/Firefox domain integration.
  -h, --help      Show this help.

Run as your normal user inside Omarchy. Requires Cargo/Rust, a C compiler,
systemd, Omarchy Quattro, and jq.
Activity data and configuration are preserved. Local plugin copies are backed up;
existing Git-managed plugins are updated through Omarchy and keep their checkout.
USAGE
}

with_browser=0
while (($#)); do
  case "$1" in
    --with-browser) with_browser=1 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$repo_root/packaging/install-common.sh"
require_session
require_commands cc cp mv mktemp readlink
if ((with_browser)); then require_commands find; fi

# Moving the installed plugin must never move the checkout executing this script.
plugin_resolved=$(readlink -m "$plugin_dir")
[[ "$repo_root/" != "$plugin_resolved/"* ]] ||
  fail "Run install.sh from a separate checkout outside the installed plugin directory."
widget_dir="$repo_root/packaging/omarchy/omastat"
omarchy plugin validate "$widget_dir"
backend_source="$repo_root"
git_managed=0
if [[ -e "$plugin_dir/.git" ]]; then
  require_commands git
  # Preserve origin, history, and local changes; the native updater only fast-forwards.
  omarchy plugin update "$plugin_id" --yes
  omarchy plugin validate "$plugin_dir"
  [[ -f "$plugin_dir/crates/omastat/Cargo.toml" ]] ||
    fail "The installed Git plugin has no backend sources; its checkout was preserved."
  backend_source="$plugin_dir"
  git_managed=1
fi

# Use the same source revision for the backend and widget.
cargo install --path "$backend_source/crates/omastat" --root "$backend_root" --locked
"$repo_root/packaging/systemd/install-user-service.sh"

if ((!git_managed)); then
  mkdir -p "$(dirname "$plugin_dir")"
  stage=$(mktemp -d "$(dirname "$plugin_dir")/.omastat-install.XXXXXX")
  trap 'if [[ -n ${stage:-} && -d $stage ]]; then rm -rf -- "$stage"; fi' EXIT
  cp -a "$widget_dir/." "$stage/"
  omarchy plugin validate "$stage"
  if [[ -e "$plugin_dir" || -L "$plugin_dir" ]]; then
    mkdir -p "$backup_root"
    backup=$(mktemp -d "$backup_root/plugin.XXXXXX")
    mv -- "$plugin_dir" "$backup/plugin"
    printf 'Previous plugin saved to %s\n' "$backup/plugin"
  fi
  mv -- "$stage" "$plugin_dir"
  stage=""
fi
omarchy-shell shell rescanPlugins >/dev/null
wait_for_plugin
# No placement override: preserve an existing user's bar placement/settings.
omarchy plugin enable "$plugin_id"
plugins=$(omarchy-shell shell listPlugins)
jq -e --arg id "$plugin_id" 'any(.[]; .id == $id and .enabled == true)' <<<"$plugins" >/dev/null ||
  fail "Plugin installed but not enabled. Try: omarchy plugin enable $plugin_id"
systemctl --user is-active --quiet omastat.service || fail "The tracking service is not running."

if ((with_browser)); then
  PATH="$backend_root/bin:$PATH" "$backend_source/packaging/browser-extension/install.sh"
fi
printf '\nOmastat installed. Open its widget in the bar.\n'
printf 'CLI: %s/bin/omastat\nUninstall: %s/uninstall.sh\n' "$backend_root" "$repo_root"
