# Threat model

Last reviewed: 2026-11-18 (1.0.0-rc.1: opt-in update check and signed updates through `tauri-plugin-updater`, see "Updates"; 1.0.0-beta.4: template summaries add no permission, network access, download or stored data; 1.0.0-beta.3: the window may open the folder dialog, game folder detection reads the registry and probes fixed drives; 1.0.0-beta.2: the window may listen to the backend's own events; 1.0.0-beta.1, internally v2: window capability for the custom title bar, game pictures including DDS loading screens, CI with pinned actions and audits, release checksums; earlier in v2.0: `python.log`, Tomato.gg and Wargaming integrations removed, XVM opt-in enforced, recovery mode hardened).

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

Controls: network off by default and opt-in enforced before every request. XVM: one compiled-in HTTPS origin, no redirects, connect/total timeouts, streamed hard byte limits, schema validation, no player identifiers sent, daily attempt limit and single-flight. Update check: see "Updates".

### Updates (1.0.0-rc.1)

The update check is a second network connection, made by the backend with the official `tauri-plugin-updater` 2 (Tauri v2, NSIS). The WebView gets no updater or process permission: it calls four typed commands (`get_update_consent`, `set_update_consent`, `check_for_update`, `install_update`), and the last two refuse with `update_check_off` unless the stored consent is an explicit "on" (unasked or off: no request). The endpoint is compiled in (`tauri.conf.json`): `https://github.com/dataflorist02/afterbattle-beta/releases/latest/download/latest.json`, HTTPS, following GitHub's redirect to its download host; the request carries only the plugin's generic user agent, no identifier. At most one check per start (20 s timeout); download only after the player's click (300 s timeout).

Threat: a replaced `latest.json` or installer (a compromised GitHub account or release, a hostile network) offering malicious code. Defense: every update package is verified against the author's minisign (Ed25519) public key embedded in `tauri.conf.json` before it runs; an unsigned or wrongly signed package is refused. The private key and its password stay outside the repository and outside the public CI; `scripts/release.ps1` reads them from `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` and refuses to build a release without the key, while the configuration keeps `createUpdaterArtifacts` off so the public CI never needs it. A test checks the public key's format (minisign header, key id matching the comment, Ed25519 length) and the endpoint.

Residual risk: a stolen private key (with its password) lets an attacker who also controls the release sign a malicious update; the key is the root of trust, and after a leak a new key ships in a manual release. `latest.json` itself is not signed: a hostile network or release can hide an update (freeze) or show misleading notes in the notice, but cannot make the app install an unsigned package, and nothing installs without the player's click. The plugin does not limit the size of `latest.json`. The installer is not Authenticode-signed (author decision 2026-11-17); a manual install is still verified with `SHA256SUMS.txt`.

### Secret or identifier disclosure

Controls: the full game path lives in Windows Credential Manager; leftover Tomato.gg/Wargaming entries are removed at startup. SQLite contains normalized pseudonymous battles only. No request logging, telemetry, environment secrets, crash upload, or secret-return IPC. Exports omit credentials, identifiers, fingerprints, UUIDs, paths, and exact times.

Residual risk: ordered tank/map data in an export may be recognizable. The UI and privacy notice require review before publication.

### Database, migration, or deletion failure

Controls: recovery mode with one fixed recovery database next to the real one (never `%TEMP%`), no imports and refused deletion while the real database cannot be opened; exact required-column validation, staged v1→v3 migration, constraints, transactions, evaluated `foreign_key_check`/`integrity_check` results, explicit idempotent legacy cleanup, rollback injection tests, secure deletion, WAL truncation, VACUUM, operation lock before deletion, and regression tests proving raw payload removal.

### Supply chain and updates

Controls: lockfiles, minimal dependencies (each new one justified: license, size, maintenance; `cargo tree` checked before and after), CI with actions pinned to commit SHAs running tests, Clippy, format, `cargo audit` and `pnpm audit --prod`, Dependabot for npm, cargo and actions in the private repository only (the public repository gets security alerts, no update pull requests), and SHA-256 checksums of the installer from `scripts/release.ps1`. Since 1.0.0-rc.1 releases carry Tauri updater signatures (see "Updates"); Authenticode is outside 1.0.0 (author decision 2026-11-17) and returns only if testers drop off at the SmartScreen warning. The betas had no updater; users verify `SHA256SUMS.txt`.

## Out of scope

- a compromised Windows account, administrator, or kernel;
- confidentiality of files already readable by same-user malware;
- XVM privacy outside requests made by Afterbattle;
- tactical positioning inference in MVP;
- a user intentionally publishing their own export.

Re-review before adding a custom WoT mod, updater, telemetry, crash reporting, arbitrary URL, new filesystem scope, authentication flow, or public release.
