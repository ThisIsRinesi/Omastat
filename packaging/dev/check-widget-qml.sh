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

qmllint \
  "$widget_dir/BarWidget.qml" \
  "$widget_dir/Panel.qml" \
  "$widget_dir/Model.js"

qmlformat -n "$widget_dir/BarWidget.qml" >/dev/null
qmlformat -n "$widget_dir/Panel.qml" >/dev/null

if command -v node >/dev/null 2>&1; then
  node --check "$widget_dir/Model.js"
fi
