#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./uninstall.sh

Disable and remove the Omarchy plugin, stop/remove the user service, uninstall
Cargo-installed Omastat binaries, and remove the optional browser integration.
Recorded activity, user configuration, and installer backups are preserved.
Run as your normal user inside Omarchy. System package installs are not removed.

  -h, --help  Show this help.
USAGE
}
if (($#)); then
  case "$1" in
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
fi
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$repo_root/packaging/install-common.sh"
require_session
require_commands find readlink
plugin_resolved=$(readlink -m "$plugin_dir")
[[ "$repo_root/" != "$plugin_resolved/"* ]] ||
  fail "Run uninstall.sh from a separate checkout outside the installed plugin directory."

# Let Omarchy unload the widget and apply its standard backup/removal behavior.
omarchy-shell shell rescanPlugins >/dev/null
if [[ -e "$plugin_dir" || -L "$plugin_dir" ]]; then
  omarchy plugin remove "$plugin_id" --yes
elif plugin_known; then
  omarchy plugin disable "$plugin_id"
fi

python3 "$repo_root/packaging/owned-files.py" uninstall service
systemctl --user daemon-reload

installed=$(cargo install --list --root "$backend_root")
if [[ "$installed" =~ (^|$'\n')omastat\ v ]]; then
  cargo uninstall --root "$backend_root" omastat
fi
"$repo_root/packaging/browser-extension/uninstall.sh"
printf '\nOmastat uninstalled. Recorded activity, configuration, and installer backups were kept.\n'
if [[ -x /usr/bin/omastat || -x /usr/local/bin/omastat ]]; then
  printf 'A system-wide Omastat binary is still installed; remove it with its package manager.\n'
fi
