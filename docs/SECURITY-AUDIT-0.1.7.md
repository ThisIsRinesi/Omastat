# Security review for 0.1.7

This source review covered install/uninstall targets, browser native messaging,
SQL inputs, widget command construction and text rendering, activity database
creation, CSV export, and package integration. It is a focused review, not a
penetration test or a guarantee that no other vulnerabilities exist.

## Findings addressed

- Service and browser integration files were overwritten/deleted without ownership
  evidence. Install now saves adjacent Omarchy-style `.bak.<timestamp>` backups
  for foreign or changed files, publishes replacements atomically, and records
  hashes and modes. Uninstall removes only matching receipt-owned files. Symlink
  targets are backed up as links; symlinked parents are refused. Backups are
  independent copies and names are exclusively created to avoid collisions.
- User-service daemon discovery depended on a writable PATH. It now runs
  `%h/.cargo/bin/omastatd`; the Arch package uses `/usr/bin/omastatd`.
- SQLite activity files inherited the process umask. Writable opens now restrict
  the database and existing SQLite sidecars to mode 0600. New and migrated
  databases start with mode 0600. Read-only reporting does not change permissions;
  an existing installation receives the fix when its updated daemon/native host
  next opens the database for writing. Original legacy databases are preserved.
- CSV fields derived from app identities and optional window titles could become
  spreadsheet formulas. CSV export now prefixes formula-like cells with an
  apostrophe and preserves numeric values. JSON remains an exact data export.
- Arch packaging now includes Python and the shared browser installer helper.

## Other checks and boundaries

Native-message bodies have a 1 MiB limit; browser-state SQL uses bound parameters.
Widget activity identifiers are passed as process arguments, and report lenses
are allowlisted before command construction. The dashboard's shared Label
component renders plain text. The signed browser extension archive is unchanged.

Receipt checks protect against accidental replacement and ordinary file changes;
they are not a security boundary against another process running as the same
user, which can edit receipts or race filesystem operations. The installer lock
serializes this installer's operations, not arbitrary processes. Backups and
explicit exports contain user data and should be protected like the originals.
The local-source Arch PKGBUILD expects a trusted source archive; it does not
verify that archive with a pinned checksum. No package build/signing or live
desktop reinstall was performed for this release.

## Validation

- 146 Rust tests, including database permissions and CSV payload regressions.
- 15 isolated installer tests, including backup collisions, symlinks, modified
  files, foreign targets, repeat uninstall, and Git-managed plugin updates.
- Browser event, widget controller, and model regression checks.
- Formatting, shell syntax, Clippy with warnings denied, and locked release build.
- cargo-audit 0.22.2: 159 locked dependencies checked against 1,243 RustSec
  advisories; no vulnerabilities reported.
