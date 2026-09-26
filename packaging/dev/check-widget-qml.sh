#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
widget_dir="$repo_root/packaging/omarchy/nagori"

# CI has Qt tooling but no running Omarchy/Quickshell installation.
qml_check_mode="${NAGORI_QML_CHECK_MODE:-desktop}"
case "$qml_check_mode" in
  desktop|portable) ;;
  *) echo "Invalid NAGORI_QML_CHECK_MODE: $qml_check_mode (use desktop or portable)" >&2; exit 2 ;;
esac

if [[ "$qml_check_mode" == desktop ]] && command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$widget_dir"
fi

# Quickshell uses Qt 6. The unversioned Arch tools may still point to Qt 5,
# which silently rejects typed IPC methods.
qml_bin_dir="${NAGORI_QT_BIN_DIR:-${QT_ROOT_DIR:-/usr/lib/qt6}/bin}"
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

qml_imports="$(mktemp -d /tmp/nagori-qml-imports.XXXXXX)"
trap 'rm -rf "$qml_imports"' EXIT
mkdir -p "$qml_imports/qs"
for module in Commons Ui; do
  if [[ -d "/usr/share/omarchy/shell/$module" ]]; then
    ln -s "/usr/share/omarchy/shell/$module" "$qml_imports/qs/$module"
  fi
done
runtime_flags=()
if [[ "$qml_check_mode" == portable ]]; then
  # Without the external base types, Qt also emits false required-property
  # warnings for assignments it cannot resolve (e.g. DashboardWindow.bar).
  # Keep syntax and the remaining Qt/JavaScript diagnostics enabled.
  runtime_flags=(--unresolved-type disable --required disable --incompatible-type disable)
  echo 'QML portable checks: external type resolution, assignment compatibility, and required-property checks need an installed desktop'
else
  echo 'QML desktop checks: external type and required-property diagnostics enabled'
fi
# The shell exposes dynamic QObject properties; runtime review covers those.
"$qml_lint" -I "$qml_imports" --unqualified disable --missing-property disable \
  --signal-handler-parameters disable --unused-imports disable --import disable \
  "${runtime_flags[@]}" --max-warnings 0 "${qml_files[@]}" "$widget_dir/Model.js" "$widget_dir/InsightCopy.js"

for qml_file in "${qml_files[@]}"; do
  "$qml_format" -n "$qml_file" >/dev/null
done

command -v node >/dev/null 2>&1 || { echo 'missing required tool: node' >&2; exit 1; }
node --check "$widget_dir/Model.js"
node --check "$widget_dir/InsightCopy.js"
node "$repo_root/packaging/dev/test-model-js.mjs"
node "$repo_root/packaging/dev/test-widget-controller.mjs"
node "$repo_root/packaging/dev/test-widget-motion.mjs"
node "$repo_root/packaging/dev/test-tracking-status.mjs"
node "$repo_root/packaging/dev/test-browser-extension.mjs"
