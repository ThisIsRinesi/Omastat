#!/usr/bin/env bash
set -euo pipefail

host_name="io.github.thisisrinesi.omastat"
extension_id="omastat-domain-tracker@thisisrinesi.github.io"
removed=0
for host_dir in \
  "${XDG_CONFIG_HOME:-$HOME/.config}/zen/native-messaging-hosts" \
  "${XDG_CONFIG_HOME:-$HOME/.config}/mozilla/native-messaging-hosts" \
  "$HOME/.mozilla/native-messaging-hosts" "$HOME/.zen/native-messaging-hosts"; do
  if [[ -e "$host_dir/$host_name.json" ]]; then removed=1; fi
  rm -f -- "$host_dir/$host_name.json"
done
for profile_root in "$HOME/.zen" "$HOME/.mozilla/firefox"; do
  [[ -d "$profile_root" ]] || continue
  while IFS= read -r -d '' profile; do
    extension="$profile/extensions/$extension_id.xpi"
    if [[ -e "$extension" ]]; then removed=1; fi
    rm -f -- "$extension"
  done < <(find "$profile_root" -mindepth 1 -maxdepth 1 -type d -print0)
done
rm -f -- "${XDG_BIN_HOME:-$HOME/.local/bin}/omastat-native-host"
rm -rf -- "${XDG_DATA_HOME:-$HOME/.local/share}/omastat/browser-extension"
if ((removed)); then
  printf 'Removed Omastat browser integration. Restart Zen/Firefox to unload it.\n'
fi
