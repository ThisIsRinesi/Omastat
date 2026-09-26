#!/usr/bin/env python3
"""Build the Arch package payload and check all runtime entrypoint assets."""
from pathlib import Path
import os
import subprocess
import tomllib
import tempfile

repo = Path(__file__).resolve().parents[2]
version = tomllib.loads((repo / "crates/nagori/Cargo.toml").read_text())["package"]["version"]
assert f"pkgver={version}" in (repo / "packaging/arch/PKGBUILD").read_text()
with tempfile.TemporaryDirectory(prefix="nagori-arch-package-") as directory:
    stage = Path(directory)
    (stage / f"nagori-{version}").symlink_to(repo, target_is_directory=True)
    payload = stage / "payload"
    subprocess.run(
        ["bash", "-c", 'source packaging/arch/PKGBUILD; cd "$NAGORI_STAGE"; pkgdir="$NAGORI_PAYLOAD"; package'],
        check=True,
        cwd=repo,
        env={**os.environ, "NAGORI_STAGE": str(stage), "NAGORI_PAYLOAD": str(payload)},
    )
    plugin = payload / "usr/share/nagori/omarchy/nagori"
    for name in ["BarWidget.qml", "DashboardWindow.qml", "Panel.qml", "Model.js", "InsightCopy.js", "TrackingStatus.qml", "manifest.json"]:
        assert (plugin / name).is_file(), name
    extension = payload / "usr/share/nagori/browser-extension"
    for name in ["domain-tracker/config.js", "domain-tracker/background.js", "signed/nagori-domain-tracker-firefox.xpi"]:
        assert (extension / name).is_file(), name
    print("Arch package includes the dashboard and signed browser runtime assets")
