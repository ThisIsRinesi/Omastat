#!/usr/bin/env bash
set -euo pipefail

python3 "$(dirname "$0")/../owned-files.py" install service
systemctl --user daemon-reload
systemctl --user enable omastat.service
systemctl --user restart omastat.service
systemctl --user is-active --quiet omastat.service
