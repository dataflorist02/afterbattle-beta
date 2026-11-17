# Threat model

Last reviewed: 2026-11-16 (1.0.0-beta.4: template summaries add no permission, network access, download or stored data; 1.0.0-beta.3: the window may open the folder dialog, game folder detection reads the registry and probes fixed drives; 1.0.0-beta.2: the window may listen to the backend's own events; 1.0.0-beta.1, internally v2: window capability for the custom title bar, game pictures including DDS loading screens, CI with pinned actions and audits, release checksums; earlier in v2.0: `python.log`, Tomato.gg and Wargaming integrations removed, XVM opt-in enforced, recovery mode hardened).

## Security objectives and assets

Protect the approved filesystem boundary, normalized battle history, coaching integrity, Credential Manager entries, database deletion guarantees, binaries, and release signing identities. Collect no economy, garage, chat, or unrelated filesystem data.

## Main threats and controls

### Malicious, partial, or changing game files

Replay and game package input is untrusted. Risks include oversized or never-ending writes, malformed JSON, traversal/reparse targets, timestamp manipulation, event storms, format drift, and misleading numeric values.

Controls: canonical containment, regular-file checks, strict extension, numeric `battleType`/`bonusType` mode checks (text such as `gameplayID` never classifies a battle), two-observation replay stabilization, hard file/block limits, boundary checks before allocation, bounded retries/caches/queues, numeric/time caps, `arenaUniqueID` deduplication with transactional provisional upsert, SHA-256 change detection, and nullable fields. The backend never exposes raw contents to the WebView.

Residual risk: TOCTOU between path validation and open cannot be eliminated completely with portable Rust filesystem APIs. A hostile same-user process that can continuously replace files inside the approved WoT directory may cause missed/partial records. It must not gain access outside the canonical allowlist.

### Incorrect correlation and harmful feedback

Controls: one statistics predicate (strict random, full results, allowed feature schema) used by every metric; require complete baseline samples; never convert missing data to zero; estimated WN8 is excluded from coaching. The `python.log` correlation path was removed in v2.0 (it did not work on the real game format and could break reports).

Residual risk: upstream format changes or missing snapshots can reduce coverage. A read-only golden test over local replays is rerun after every supported WoT update.

### WebView compromise

Controls: bundled assets, restrictive CSP, frozen prototypes, no external frontend networking, no remote HTML/scripts, minimal Tauri capability, typed commands, no raw-file or secret-return command.

Window capability (v2.0, custom title bar; events since 1.0.0-beta.2; folder dialog since 1.0.0-beta.3): the only permissions are `core:window:allow-start-dragging`, `core:window:allow-internal-toggle-maximize` (double click on the drag region), `core:window:allow-minimize`, `core:window:allow-toggle-maximize`, `core:window:allow-close`, `core:event:allow-listen`, `core:event:allow-unlisten` and `dialog:allow-open`, for the main window only. A compromised WebView could move, minimize, maximize or hide the window to the tray (closing hides it, as Alt+F4 does) and listen to the events the backend sends anyway (import progress counts, "data changed", the id of an ended session); it cannot emit events, resize the window to arbitrary values, open new windows, read window state, or reach files, shell or network through them. A test keeps the list exact.

Folder dialog and detection (1.0.0-beta.3, hotfix for testers who could not set up the app; official `tauri-plugin-dialog`): `dialog:allow-open` lets the window open the system folder picker, nothing else of the plugin (no save, message or ask dialogs). The dialog returns only the path the player chose; the file system and asset plugins are not registered, so no scope is widened. A compromised WebView could open the picker and could already pass any string to `configure_local_source`, which normalizes it (a `replays` folder → its parent, `WorldOfTanks.exe` → its folder) and accepts only a canonical folder with `WorldOfTanks.exe` and a `replays` folder inside it, as before. Detection reads the registry (uninstall entries' `InstallLocation`, the Steam path) and a fixed list of folders on fixed drives plus Steam's `libraryfolders.vdf`; it does not recurse, skips network and removable drives (no hang on a disconnected share) and stops after 4 seconds. Paths found are shown only to the player; nothing leaves the computer. Residual risk: a crafted `libraryfolders.vdf` or registry entry can make detection probe other local folders, which only yields a candidate when it looks like a game folder and still needs the player's click.

Game pictures (v2.0): map loading screens (`map/screen/*.dds`) are decoded in Rust: our own code reads the DDS header and checks it before any allocation, the BC1 blocks are decoded by `bcdec_rs` (MIT, pure Rust, no dependencies, no `unsafe`), and the scaled result is written as PNG by `png` (already in the build through Tauri). The header check covers (magic, header sizes, BC1 FourCC only, sides 1–4096, at most 13 mipmaps, first level fully present, file at most 4 MB from the ZIP's declared size); output boxes are limited to 4096 × 4096 and the result is never larger than the source. Truncation at every length and deterministic mutations are tested. Pictures stay in a RAM cache of at most 64 MB.

### Network and response attacks

Controls: network off by default and opt-in enforced before every request; one compiled-in HTTPS origin (XVM); no redirects; connect/total timeouts; streamed hard byte limits; schema validation; no player identifiers sent to XVM; daily XVM attempt limit and single-flight.

### Secret or identifier disclosure

Controls: the full game path lives in Windows Credential Manager; leftover Tomato.gg/Wargaming entries are removed at startup. SQLite contains normalized pseudonymous battles only. No request logging, telemetry, environment secrets, crash upload, or secret-return IPC. Exports omit credentials, identifiers, fingerprints, UUIDs, paths, and exact times.

Residual risk: ordered tank/map data in an export may be recognizable. The UI and privacy notice require review before publication.

### Database, migration, or deletion failure

Controls: recovery mode with one fixed recovery database next to the real one (never `%TEMP%`), no imports and refused deletion while the real database cannot be opened; exact required-column validation, staged v1→v3 migration, constraints, transactions, evaluated `foreign_key_check`/`integrity_check` results, explicit idempotent legacy cleanup, rollback injection tests, secure deletion, WAL truncation, VACUUM, operation lock before deletion, and regression tests proving raw payload removal.

### Supply chain and updates

Controls: lockfiles, minimal dependencies (each new one justified: license, size, maintenance; `cargo tree` checked before and after), CI with actions pinned to commit SHAs running tests, Clippy, format, `cargo audit` and `pnpm audit --prod`, Dependabot for npm, cargo and actions in the private repository only (the public repository gets security alerts, no update pull requests), and SHA-256 checksums of the installer from `scripts/release.ps1`. Required for the stable 1.0.0: SBOM, Authenticode and Tauri updater signatures. The public beta 1.0.0-beta.1 (author decision) has no updater and remains unsigned; users verify `SHA256SUMS.txt`.

## Out of scope

- a compromised Windows account, administrator, or kernel;
- confidentiality of files already readable by same-user malware;
- XVM privacy outside requests made by Afterbattle;
- tactical positioning inference in MVP;
- a user intentionally publishing their own export.

Re-review before adding a custom WoT mod, updater, telemetry, crash reporting, arbitrary URL, new filesystem scope, authentication flow, or public release.
