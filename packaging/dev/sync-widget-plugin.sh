#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s [--capture PATH] [--lens day|week|month|year|life] [--delay SECONDS] [--region GEOMETRY] [--select] [--keep-open] [--restart-shell]\n' "$0" >&2
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
plugin_id="local.omastat"
widget_dir="$repo_root/packaging/omarchy/omastat"
plugin_dir="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/$plugin_id"
capture=""
lens=""
delay="0.7"
keep_open=0
restart_shell=0
region=""
select_region=0
export OMARCHY_PATH="${OMARCHY_PATH:-/usr/share/omarchy}"

while (($#)); do
  case "$1" in
    --capture)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      capture="$2"
      shift 2
      ;;
    --lens)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      lens="$2"
      case "$lens" in
        day|week|month|year|life) ;;
        *) usage; exit 2 ;;
      esac
      shift 2
      ;;
    --delay)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      delay="$2"
      shift 2
      ;;
    --region)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      region="$2"
      shift 2
      ;;
    --select)
      select_region=1
      shift
      ;;
    --keep-open)
      keep_open=1
      shift
      ;;
    --restart-shell)
      restart_shell=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$widget_dir"
fi

mkdir -p "$plugin_dir"
rsync -a --delete "$widget_dir/" "$plugin_dir/"

if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$plugin_dir"
fi

if command -v quickshell >/dev/null 2>&1; then
  quickshell ipc -n -p "$OMARCHY_PATH/shell" call shell rescanPlugins >/dev/null
else
  echo "missing required tool: quickshell" >&2
  exit 1
fi

if ((restart_shell)); then
  if command -v omarchy >/dev/null 2>&1; then
    omarchy restart shell || true
  else
    echo "missing required tool: omarchy" >&2
    exit 1
  fi
fi

for ((attempt = 0; attempt < 100; attempt++)); do
  if quickshell ipc -n -p "$OMARCHY_PATH/shell" show 2>/dev/null | grep -q "target $plugin_id"; then
    break
  fi
  sleep 0.05
done

if ! quickshell ipc -n -p "$OMARCHY_PATH/shell" show 2>/dev/null | grep -q "target $plugin_id"; then
  echo "Omarchy shell did not register $plugin_id after plugin rescan." >&2
  exit 1
fi

if [[ -n "$capture" ]]; then
  capture_args=(--output "$capture" --delay "$delay")
  [[ -n "$lens" ]] && capture_args+=(--lens "$lens")
  [[ -n "$region" ]] && capture_args+=(--region "$region")
  ((select_region)) && capture_args+=(--select)
  ((keep_open)) && capture_args+=(--keep-open)
  "$repo_root/packaging/dev/capture-widget-panel.sh" "${capture_args[@]}"
else
  echo "Synced $widget_dir -> $plugin_dir"
fi
