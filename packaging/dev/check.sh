#!/usr/bin/env bash
# The same required checks run locally and in CI. No installation or deployment.
set -euo pipefail
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

for tool in cargo cc node python3 jq bwrap git rg; do
  command -v "$tool" >/dev/null 2>&1 || { echo "Missing required tool: $tool" >&2; exit 1; }
done
omarchy_bin="${OMASTAT_TEST_OMARCHY_BIN:-/usr/share/omarchy/bin}"
for tool in omarchy-plugin-remove omarchy-plugin-update omarchy-plugin-validate; do
  [[ -x "$omarchy_bin/$tool" ]] || {
    echo "Missing $omarchy_bin/$tool; set OMASTAT_TEST_OMARCHY_BIN to an Omarchy Quattro bin directory." >&2
    exit 1
  }
done

echo 'Checking formatting and shell syntax'
cargo fmt --all -- --check
while IFS= read -r script; do bash -n "$script"; done < <(rg --files -g '*.sh' packaging)
bash -n install.sh uninstall.sh
bash -n packaging/arch/PKGBUILD

echo 'Checking QML and JavaScript'
packaging/dev/check-widget-qml.sh

echo 'Checking isolated installation and removal'
python3 packaging/dev/test-installation.py

echo 'Checking Rust lints and tests'
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace

echo 'Building release binaries'
cargo build --locked --release --workspace --bins
echo 'All checks passed'
