#!/usr/bin/env bash
set -euo pipefail
python3 "$(dirname "${BASH_SOURCE[0]}")/../owned-files.py" uninstall browser
printf 'Browser integration uninstall complete. Restart Zen/Firefox to apply changes.\n'
