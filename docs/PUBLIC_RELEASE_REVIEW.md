# Public release review — Afterbattle 1.0.0-beta.3

The public repository is the tree of the private `v2/integration` **minus whole excluded files** — no file is edited only for the public copy (author decision 2026-11-14). It is produced by [`scripts/make-public-snapshot.ps1`](../scripts/make-public-snapshot.ps1), which holds the exclusion list as data, scans the result and commits only when no new finding appears. This file is part of the private tree and therefore of the snapshot. Nothing is pushed or published by the script; the author pushes.

## 1. Excluded (whole files and folders)

The exact list is data in `scripts/make-public-snapshot.ps1` (`$Exclusions`); the script and its test are the only public files that name the excluded paths. By category:

| Category | Files | Reason |
|---|---|---|
| Research notes folder | 9 | Research, including path B (decoding the encrypted part of replays), which stays private. |
| Research trial code folder | 0 | Trial code of research runs; it lives only on research branches and is never merged. Listed for safety (added 2026-11-15). |
| Internal audit folder | 1 | Names commits with personal data in the private history. |
| Internal task and status notes | 4 | Working notes of the development run. The synthetic screenshots next to them stay (README, the repository test and `tools/screens` use them). |
| Internal run log | 1 | Local paths of the author's game folder, golden numbers from the author's replays. |
| Instructions for the coding agent | 1 | Internal workflow. |
| Outdated alpha plan | 1 | Describes removed integrations and contradicts the code. |
| Archived alpha tool folder | 1 | A tool that could modify a real database in `%APPDATA%` (audit N9). |
| Local agent settings | 0 | Not tracked; listed for safety. |
| Dependabot configuration | 1 | Dependency updates happen only in the private repository; the public repository gets security alerts only (no update pull requests). |

19 files excluded. No public file refers to an excluded file by name or path: the script stops when one does (section 3).

## 2. Difference from the previous snapshot

The public repository is updated with a new commit by the script's update mode (its history is kept). Differences from the 1.0.0-beta.2 snapshot (2026-11-15):

- **Exclusions:** the same 19 files. The rule list gains the research trial code folder (0 files; it never reaches the integration branch), and a one-segment excluded folder is now searched in public files as a path prefix (section 3).
- **New (hotfix 1.0.0-beta.3, game folder):** `src-tauri/src/game_folder.rs` (detection from the registry, fixed drives and Steam libraries; folder validation with specific errors; normalization of a chosen `replays` folder or `WorldOfTanks.exe`) and `src/components/GameFolderActions.tsx` (detection results and the manual choice, in Settings and on the first-start screen).
- **Window capability:** adds `dialog:allow-open` (the system folder picker only) with the official `tauri-plugin-dialog` 2.7.3 and `@tauri-apps/plugin-dialog` ~2.7.3 (THREAT_MODEL, SECURITY). Windows-only runtime crates `winreg` 0.55 and `windows-sys` 0.61 for detection were already in `Cargo.lock`.
- **Changed documents:** CHANGELOG, README, SECURITY, THREAT_MODEL, KNOWN_ISSUES and RELEASE_CHECKLIST for 1.0.0-beta.3; spec 20 (reference tables per tier and class as a feature candidate) and spec 28 (the author's decisions on local AI summaries, not in the app); the snapshot script and its test (the exclusion rules above).
- **Screenshots:** refreshed with `tools/screens` ("Afterbattle beta 3" in the title bar, "Choose the game folder" in Settings); 2 new synthetic screenshots of the first-start screen (`session-firstrun.png`, `session-firstrun-found.png`), so `docs/status/screens/` holds 47 files. The mocked IPC of the screenshot page gets a `firstrun` state.
- **Unchanged:** fonts, brand files and app icons.

## 3. Scans (run by the script on the snapshot before committing)

**gitleaks 8.30.1** (`gitleaks dir`): 8 findings, all known false positives of rule `generic-api-key` (game identifiers next to the word "key"), listed in the script with file and value:

| File | Value | Kind |
|---|---|---|
| `src/test/fixtures.ts` | `90_synthetic_hills`, `92_synthetic_hills` | synthetic map keys |
| `src/app/screens.test.tsx` | `90_synthetic_hills`, `synth-sx9_vz_60s_test` | synthetic map / vehicle keys |
| `src/lib/presentation.test.ts` | `99_synthetic_map`, `usa-A179_Black_Rock` (×2) | synthetic map key; public game catalog vehicle code |
| `src-tauri/src/local_tracker.rs` | `france-amx_13` | public game catalog vehicle code |

**Excluded names:** the path and the file name of every excluded file (and every excluded folder path) are searched in all snapshot text files except the script and its test — 0 hits. Since 2026-11-15 a one-segment excluded folder is searched as a path prefix (its name followed by `/`), because such a name can be an ordinary word in prose.

**Pattern searches** (text files, lockfiles skipped) — 0 hits each:

| Pattern | Notes |
|---|---|
| account id (6–10 digits next to `account_id`, `accountDBID`, `playerID`, `player_id`, `dbid`) | `account_id` occurs only as a field name; the diagnostics test uses the setting value `123456789` (not next to the field name pattern). |
| Windows user folder path (drive C, folder Users, any user name) | Only the synthetic user folder `private` of the diagnostics privacy test is allowed; any other name stops the script. |
| AWS, GitHub, Slack, `sk-` API keys, private keys, Tomato.gg keys (16+ characters) | `tmgg_synthetic` in a test is shorter and synthetic. |
| replay file name with a date other than `20200101_0000_` | all test names are synthetic. |

**Game files:** no `.dds`, `.pkg`, `.mo`, `.wotreplay`. **Images:** only `src-tauri/icons/` (the app's own icons generated by Tauri from the brand icon: 16 PNG, ICO, ICNS and the brand SVG), `docs/design/brand/` (the Afterbattle mark, original) and `docs/status/screens/` (synthetic, placeholders instead of game graphics). The other design mockups (`docs/design/`) embed no images and reference only Google Fonts and claude.ai (the design tool).

**Personal data from the private history:** the nickname and account ID that the audit found in old commits occur 0 times (checked by value; the values are not written here). The public repository starts without that history.

## 4. Checks

On the private branch before the snapshot: `pnpm lint`, `pnpm test`, `pnpm build`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (5 ignored tests read local game data and run only with `--ignored`), golden test on the author's replays unchanged. CI in the public repository needs no secrets; its `build` job keeps the unsigned installer and `SHA256SUMS.txt` as a workflow artifact.

## 5. Commit identity

The script reads the logged-in GitHub account with `gh` (read only) and sets `user.name` and a `…@users.noreply.github.com` address in the public repository only (never globally). It refuses to commit when `gh` is not logged in, the read fails, or the address is not a GitHub noreply address; `-AuthorEmail` / `-AuthorName` override the read under the same check.

## 6. File tree (tracked files)

```text
.gitattributes
.gitignore
.prettierignore
.prettierrc
CHANGELOG.md
CONTRIBUTING.md
LICENSE
NOTICE
PRIVACY.md
README.md
SECURITY.md
index.html
package.json
pnpm-lock.yaml
screens.html
tsconfig.json
tsconfig.node.json
vite.config.ts
.cargo/  (1 files)
.github/workflows/  (1 files)
docs/  (6 files)
docs/design/  (6 files)
docs/design/brand/  (9 files)
docs/design/flare/  (6 files)
docs/specs/  (6 files)
docs/status/screens/  (47 files)
scripts/  (2 files)
src-tauri/  (4 files)
src-tauri/.cargo/  (1 files)
src-tauri/capabilities/  (1 files)
src-tauri/gen/schemas/  (4 files)
src-tauri/icons/  (18 files)
src-tauri/src/  (37 files)
src/  (10 files)
src/api/generated/  (159 files)
src/api/generated/serde_json/  (1 files)
src/app/  (9 files)
src/assets/fonts/  (15 files)
src/components/  (15 files)
src/components/session/  (4 files)
src/hooks/  (3 files)
src/i18n/  (6 files)
src/lib/  (12 files)
src/pages/  (6 files)
src/screens/  (2 files)
src/styles/  (3 files)
src/test/  (4 files)
tools/lint/  (3 files)
tools/mutants/  (3 files)
tools/screens/  (3 files)
```

425 tracked files.
