#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
widget_dir="$repo_root/packaging/omarchy/omastat"

for tool in qmllint qmlformat; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "missing required tool: $tool" >&2
    exit 1
  fi
done

mapfile -t qml_files < <(find "$widget_dir" -maxdepth 1 -type f -name '*.qml' | sort)

qmllint "${qml_files[@]}" "$widget_dir/Model.js"

for qml_file in "${qml_files[@]}"; do
  qmlformat -n "$qml_file" >/dev/null
done

if command -v node >/dev/null 2>&1; then
  node --check "$widget_dir/Model.js"
fi
