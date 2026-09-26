#!/usr/bin/env bash
set -euo pipefail

python3 "$(dirname "$0")/../owned-files.py" install service
systemctl --user daemon-reload
systemctl --user enable nagori.service
systemctl --user restart nagori.service
systemctl --user is-active --quiet nagori.service
