#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s [--output PATH] [--delay SECONDS] [--lens day|week|month|year|life] [--region GEOMETRY] [--select] [--shell-path PATH] [--keep-open] [--no-open]\n' "$0" >&2
}

shell_path="${OMARCHY_SHELL_PATH:-/usr/share/omarchy/shell}"
output="/tmp/omastat-panel-$(date +%Y%m%d-%H%M%S).png"
delay="1.2"
region=""
lens=""
keep_open=0
open_panel=1
export OMARCHY_PATH="${OMARCHY_PATH:-/usr/share/omarchy}"

while (($#)); do
  case "$1" in
    --output)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      output="$2"
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
    --lens)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      lens="$2"
      case "$lens" in
        day|week|month|year|life) ;;
        *) usage; exit 2 ;;
      esac
      shift 2
      ;;
    --select)
      if ! command -v slurp >/dev/null 2>&1; then
        echo "missing required tool: slurp" >&2
        exit 1
      fi
      region="$(slurp)"
      shift
      ;;
    --shell-path)
      [[ $# -ge 2 ]] || { usage; exit 2; }
      shell_path="$2"
      shift 2
      ;;
    --keep-open)
      keep_open=1
      shift
      ;;
    --no-open)
      open_panel=0
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

for tool in quickshell grim; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "missing required tool: $tool" >&2
    exit 1
  fi
done

if ((open_panel)); then
  if ! quickshell ipc -n -p "$shell_path" call local.omastat open; then
    cat >&2 <<'EOF'
could not open local.omastat through Quickshell IPC.
Make sure the plugin is installed, enabled, and the shell has loaded it:
  packaging/dev/reinstall-and-restart.sh
EOF
    exit 1
  fi
  if [[ -n "$lens" ]]; then
    quickshell ipc -n -p "$shell_path" call local.omastat "$lens"
  fi
  sleep "$delay"
fi

mkdir -p "$(dirname "$output")"
if [[ -n "$region" ]]; then
  grim -g "$region" "$output"
else
  grim "$output"
fi

if ((open_panel)) && ((! keep_open)); then
  quickshell ipc -n -p "$shell_path" call local.omastat close || true
fi

printf '%s\n' "$output"
