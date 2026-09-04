#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
host_name="io.github.thisisrinesi.omastat"
extension_id="omastat-domain-tracker@thisisrinesi.github.io"
if [[ -d "$script_dir/domain-tracker" ]]; then
  extension_src="$script_dir/domain-tracker"
else
  extension_src="$repo_root/packaging/browser-extension/domain-tracker"
fi
install_root="${XDG_DATA_HOME:-$HOME/.local/share}/omastat/browser-extension"
host_wrapper="${XDG_BIN_HOME:-$HOME/.local/bin}/omastat-native-host"

install_native_wrapper() {
  mkdir -p "$(dirname "$host_wrapper")"
  local omastat_bin
  omastat_bin="$(command -v omastat || true)"
  if [[ -z "$omastat_bin" && -x "$HOME/.cargo/bin/omastat" ]]; then
    omastat_bin="$HOME/.cargo/bin/omastat"
  fi
  if [[ -z "$omastat_bin" ]]; then
    echo "omastat binary not found; install it before installing the browser extension." >&2
    exit 1
  fi
  printf '#!/usr/bin/env bash\nexec %q native-host\n' "$omastat_bin" >"$host_wrapper"
  chmod 0755 "$host_wrapper"
}

install_native_manifest() {
  local dir="$1"
  mkdir -p "$dir"
  printf '{\n  "name": "%s",\n  "description": "Omastat browser domain native host",\n  "path": "%s",\n  "type": "stdio",\n  "allowed_extensions": ["%s"]\n}\n' \
    "$host_name" "$host_wrapper" "$extension_id" >"$dir/$host_name.json"
}

build_xpi() {
  local app_class="$1"
  local source="$2"
  local out="$install_root/omastat-domain-tracker-$app_class.xpi"
  local build_dir="$install_root/build-$app_class"

  rm -rf "$build_dir"
  mkdir -p "$build_dir"
  cp "$extension_src/manifest.json" "$extension_src/background.js" "$build_dir/"
  printf 'var OMastatDomainTrackerConfig = {\n  hostName: "%s",\n  appClass: "%s",\n  source: "%s"\n}\n' \
    "$host_name" "$app_class" "$source" >"$build_dir/config.js"

  rm -f "$out"
  (cd "$build_dir" && zip -qr "$out" manifest.json background.js config.js)
  printf '%s\n' "$out"
}

install_xpi_to_profiles() {
  local root="$1"
  local xpi="$2"
  [[ -d "$root" ]] || return 0

  local installed=0
  local profile
  while IFS= read -r -d '' profile; do
    [[ -f "$profile/prefs.js" || -f "$profile/extensions.json" ]] || continue
    mkdir -p "$profile/extensions"
    cp "$xpi" "$profile/extensions/$extension_id.xpi"
    installed=$((installed + 1))
  done < <(find "$root" -maxdepth 1 -type d -print0)

  if (( installed > 0 )); then
    echo "Installed Omastat browser extension into $installed profile(s) under $root"
  fi
}

install_native_wrapper
install_native_manifest "${XDG_CONFIG_HOME:-$HOME/.config}/zen/native-messaging-hosts"
install_native_manifest "${XDG_CONFIG_HOME:-$HOME/.config}/mozilla/native-messaging-hosts"
install_native_manifest "$HOME/.mozilla/native-messaging-hosts"
install_native_manifest "$HOME/.zen/native-messaging-hosts"

mkdir -p "$install_root"
zen_xpi="$(build_xpi zen omastat-zen)"
firefox_xpi="$(build_xpi firefox omastat-firefox)"

install_xpi_to_profiles "$HOME/.zen" "$zen_xpi"
install_xpi_to_profiles "$HOME/.mozilla/firefox" "$firefox_xpi"

echo "Browser native host installed as $host_name"
echo "Restart Zen/Firefox to load the profile extension if the browser is already running."
