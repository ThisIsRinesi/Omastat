This directory contains the Mozilla-signed Omastat Domain Tracker 0.2.0
Firefox package, imported from `omastat-signed.xpi` on 2026-09-07.

SHA-256: `1e932357e511573e52cf6c00151d04289928c7f37464a86e84a37fdbfe8328d7`

The installer copies this archive unchanged to preserve its signatures. Its
manifest matches `../domain-tracker/manifest.json` and its background script
matches `../domain-tracker/background.js`. The embedded configuration uses
`appClass: "firefox"` and `source: "omastat-firefox"`.

After changing extension source, increment the extension version, submit the
updated Firefox package to Mozilla for signing, and replace this archive with
the signed result. Update this record and verify the installation test. Editing
or repacking the signed archive invalidates its signatures.

Zen still uses a separate local build; this archive attributes activity to Firefox.
