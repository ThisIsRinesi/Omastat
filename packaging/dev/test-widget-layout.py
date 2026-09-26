#!/usr/bin/env python3
"""Render the real panel with synthetic data and an isolated shell/window host.

Requires Quickshell and Qt's offscreen platform. Does not load user plugins or
query activity data. The host is a test double: compositor placement, bar button
integration, and the optional Notchbar shared surface need desktop review.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, help='Keep screenshots and logs here')
args = parser.parse_args()
output = (args.output or Path(tempfile.mkdtemp(prefix='nagori-layout-'))).resolve()
output.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix='nagori-layout-shell-') as directory:
    stage = Path(directory)
    shutil.copytree(ROOT / 'packaging/dev/layout-fixtures', stage, dirs_exist_ok=True)
    for name in ['Panel.qml', 'Model.js', 'InsightCopy.js', 'TrackingStatus.qml']:
        shutil.copy2(ROOT / 'packaging/omarchy/nagori' / name, stage / name)
    # Keep the real status component but substitute its external process input.
    tracking = stage / 'TrackingStatus.qml'
    status = json.dumps({'tracker': 'reporting', 'browser': 'stale', 'checked_at': 1790000000})
    tracking.write_text('\n'.join(
        '    command: ["printf", "%s", ' + json.dumps(status) + ']'
        if line.strip().startswith('command:') else line
        for line in tracking.read_text().splitlines()) + '\n')
    panel = stage / 'Panel.qml'
    source = panel.read_text().replace('  id: root\n', '''  id: root
  property alias auditBody: body
  property alias auditScroll: scroll
  property alias auditKeys: keyCatcher
  property alias auditSupport: supportRail
''', 1)
    panel.write_text(source)
    shell = stage / 'shell.qml'
    shell.write_text(shell.read_text().replace('OUTPUT_PATH', json.dumps(str(output))))
    env = dict(os.environ, QT_QPA_PLATFORM='offscreen', QT_QPA_PLATFORMTHEME='generic')
    result = subprocess.run(['quickshell', '-p', str(stage), '--no-color'], env=env,
                            capture_output=True, text=True, timeout=300)
    log = result.stdout + result.stderr
    (output / 'render.log').write_text(log)
    reports = [json.loads(line.split('AUDIT ', 1)[1]) for line in log.splitlines() if 'AUDIT ' in line]
    failures = [report for report in reports if report['failures']]
    warnings = [line for line in log.splitlines() if 'WARN scene:' in line or 'ERROR' in line]
    if result.returncode or failures or warnings or len(reports) != 540:
        print(json.dumps(failures, indent=2))
        print('\n'.join(warnings))
        raise SystemExit(f'Layout audit failed: {len(reports)}/540 cases. Log: {output / "render.log"}')
    print(f'All {len(reports)} layout cases passed. Screenshots and log: {output}')
