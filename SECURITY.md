# Security policy

Security and user control are release-blocking requirements for Afterbattle.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. Use the repository's private GitHub Security Advisory reporting flow. Include the affected version, reproduction steps, impact, and any suggested mitigation. Do not include credentials or private player data.

Until a public repository and private advisory channel are configured, do not distribute the application outside the local alpha.

## Supported versions

Only the newest published beta will receive security fixes. The project is currently an unreleased local alpha.

## Security invariants

- The frontend has no external network access.
- Rust network requests use a compiled-in HTTPS base URL and disable redirects.
- The approved WoT path is stored only in Windows Credential Manager; leftover entries of the removed Tomato.gg and Wargaming integrations are deleted at every start.
- Paths, credentials, profile/account identifiers, and raw inputs are never stored in SQLite, exports, logs, environment files, or crash reports.
- Replay, game package, and XVM inputs are untrusted: canonicalized where applicable, hard-size-limited, parsed, normalized, and stored with parameterized SQL.
- Missing or ambiguous metrics remain NULL/N/A and cannot produce categorical coaching.
- Optional network bodies are streamed through hard limits; redirects are disabled and destinations are compiled in.
- The application does not request administrator privileges.
- The window capability grants only the custom title bar's calls: start dragging, double-click maximize, minimize, maximize/restore and close (which hides to the tray). No other Tauri permission is granted; a test keeps the list exact.
- Game package pictures are parsed in Rust with limits checked before allocation (PNG at most 1 MB; BC1 DDS at most 4 MB, sides up to 4096, at most 13 mipmaps) and held only in a RAM cache of at most 64 MB.
- No remote scripts, fonts, images, or frames are loaded.
- Telemetry and automated crash uploads are absent.
- Pseudonymized exports omit nickname, account ID, fingerprints/arena IDs, exact timestamps, paths, UUIDs, and credentials.
- Deletion stops the tracker, removes every Credential Manager entry, transactionally deletes history while preserving language/timeout preferences, truncates WAL, and vacuums the database. In recovery mode (real database cannot be opened) deletion is refused and no battles are imported.
- XVM downloads happen only after opt-in; no request is sent while XVM is disabled.

## Release requirements

The public beta 1.0.0-beta.2 (author decisions 2026-11-14 and 2026-11-15; a GitHub pre-release) requires:

1. Passing Rust and TypeScript tests, lint, formatting and Clippy, and the read-only golden test on the author's replays.
2. `cargo audit` and `pnpm audit --prod` with no high-severity findings.
3. The installer built by the author with `scripts/release.ps1` from a clean tree of the release commit, with `SHA256SUMS.txt`; users check it with `Get-FileHash` (README).
4. The author's manual test of the release candidate (the author's internal manual test list).
5. A public snapshot reviewed for secrets, real player data and game files (`docs/PUBLIC_RELEASE_REVIEW.md` in the public repository).

The beta is unsigned and has no updater; users see a SmartScreen warning and verify the checksum. A stable public release (1.0.0) requires:

1. Passing Rust, TypeScript, UI, and migration tests.
2. `cargo audit` and `pnpm audit` with no unaccepted high-severity findings.
3. A generated software bill of materials and SHA-256 checksums.
4. A protected release workflow with human approval.
5. A Tauri updater signature whose private key is kept outside the repository.
6. A Windows Authenticode signature from a protected code-signing identity.
7. Review of all changes to IPC commands, network destinations, credentials, persistence, exports, and the updater.
8. Verification that source maps and secrets are absent from the release artifact.

Updater signing and Authenticode signing are independent controls; both are required for the stable 1.0.0 release (the author decided to ship the 1.0 beta without them).

## Dependency policy

Dependencies are pinned through `pnpm-lock.yaml` and `Cargo.lock`. New dependencies need a concrete justification, license review, and source/reputation review. Runtime plugins are avoided unless their capability is required. Automated dependency updates must pass the full test and audit suite.
