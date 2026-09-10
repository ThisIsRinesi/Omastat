#!/usr/bin/env bash
set -euo pipefail
python3 "$(dirname "${BASH_SOURCE[0]}")/../owned-files.py" install browser
printf 'Browser integration install complete. Restart Zen/Firefox to apply changes.\n'
