#!/usr/bin/env python3
"""Build the reproducible Nagori extension submission archive (unsigned)."""
from pathlib import Path
import zipfile

root = Path(__file__).resolve().parent
with zipfile.ZipFile(root / 'nagori-domain-tracker-unsigned.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
    for source in sorted((root / 'domain-tracker').iterdir()):
        entry = zipfile.ZipInfo(source.name, (2026, 1, 1, 0, 0, 0))
        entry.compress_type = zipfile.ZIP_DEFLATED
        entry.external_attr = 0o100644 << 16
        archive.writestr(entry, source.read_bytes())
