#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
widget_dir="$repo_root/packaging/omarchy/omastat"

if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$widget_dir"
fi

# Quickshell uses Qt 6. The unversioned Arch tools may still point to Qt 5,
# which silently rejects typed IPC methods.
qml_bin_dir="/usr/lib/qt6/bin"
if [[ -x "$qml_bin_dir/qmllint" && -x "$qml_bin_dir/qmlformat" ]]; then
  qml_lint="$qml_bin_dir/qmllint"
  qml_format="$qml_bin_dir/qmlformat"
else
  qml_lint="qmllint"
  qml_format="qmlformat"
fi
for tool in "$qml_lint" "$qml_format"; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "missing required tool: $tool" >&2
    exit 1
  fi
done

mapfile -t qml_files < <(find "$widget_dir" -maxdepth 1 -type f -name '*.qml' | sort)

qml_imports="$(mktemp -d /tmp/omastat-qml-imports.XXXXXX)"
trap 'rm -rf "$qml_imports"' EXIT
mkdir -p "$qml_imports/qs"
for module in Commons Ui; do
  if [[ -d "/usr/share/omarchy/shell/$module" ]]; then
    ln -s "/usr/share/omarchy/shell/$module" "$qml_imports/qs/$module"
  fi
done
# The shell exposes dynamic QObject properties; runtime review covers those.
"$qml_lint" -I "$qml_imports" --unqualified disable --missing-property disable \
  --signal-handler-parameters disable --unused-imports disable --import disable \
  --max-warnings 0 "${qml_files[@]}" "$widget_dir/Model.js"

for qml_file in "${qml_files[@]}"; do
  "$qml_format" -n "$qml_file" >/dev/null
done

if command -v node >/dev/null 2>&1; then
  node --check "$widget_dir/Model.js"
  node "$repo_root/packaging/dev/test-model-js.mjs"
  node "$repo_root/packaging/dev/test-widget-controller.mjs"
  node "$repo_root/packaging/dev/test-browser-extension.mjs"
fi
