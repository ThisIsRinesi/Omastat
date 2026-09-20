This directory contains the Mozilla-signed Omastat Domain Tracker 0.3.0
Firefox package, imported from `36a86570ad7447deb292-0.3.0.zip` on 2026-09-19.

SHA-256: `8714b49454c5ee05fad281393640b3be65e86416694857b9f5b34ee9e28b8585`

The installer copies this archive unchanged to preserve its signatures. Its
manifest and background script match `../domain-tracker/`. Version 0.3.0 adds
audible, unmuted tab domains for background-audio attribution. The embedded
configuration uses `appClass: "firefox"` and `source: "omastat-firefox"`.

After changing extension source, increment the extension version, submit the
updated Firefox package to Mozilla for signing, and replace this archive with
the signed result. Update this record and verify the installation test. Editing
or repacking the signed archive invalidates its signatures.

The same archive is installed for Zen. The native host detects the launching
browser and overrides the embedded identity to attribute Zen activity correctly.
