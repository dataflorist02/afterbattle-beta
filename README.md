<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/design/brand/afterbattle-mark-dark-192.png">
    <source media="(prefers-color-scheme: light)" srcset="docs/design/brand/afterbattle-mark-light-192.png">
    <img src="docs/design/brand/afterbattle-mark-light-192.png" alt="Afterbattle" width="96" height="96">
  </picture>
</p>

# Afterbattle

Afterbattle is a local-first Windows desktop analytics application for World of Tanks. It turns locally recorded battle replays into deterministic session, vehicle, and performance reports while keeping raw replay data on the user’s machine.

| | |
|---|---|
| Status | 1.0.0-beta.2, public beta |
| Platform | Windows desktop (10/11) |
| Architecture | Tauri 2 + Rust + React/TypeScript |
| Storage | Local SQLite (schema v10) |
| Development | AI-assisted engineering workflow |
| License | Apache-2.0 |

Afterbattle is not affiliated with, endorsed by, or sponsored by Wargaming, World of Tanks, or XVM.

## Why this project exists

World of Tanks exposes a lot of post-battle data, but raw counters do not answer questions such as:

- How is my recent form changing?
- Which vehicles and sessions are consistently strong or weak?
- Where are opportunities being lost?
- Which conclusions are supported by complete data, and which should remain N/A?
- How can feedback stay useful without inventing causes from incomplete evidence?

Afterbattle treats those questions as a data-contract and product-engineering problem: parse only the supported local evidence, preserve missing values as missing, and make analytics deterministic and inspectable.

## Key features

- Local replay tracking with bounded parsing and deduplication; only strict random battles with full results count towards statistics.
- Full history import in the background (no file or age limit), resumable and cancellable; schema migrations with a backup before every step and rebuild from replays.
- Automatic session lifecycle, manual session control, tray operation, and inactivity timeout.
- Partial-to-full replay upgrades without duplicate battles or double-counted sessions.
- Session, battle, and vehicle history with filters, pagination, coverage, and read-only reports.
- Deterministic WN8 using versioned local XVM expected values with explicit coverage and N/A semantics.
- Personal and per-vehicle baselines, performance trends, deterministic session insights, and persistent vehicle challenges.
- Battle Pace Context with explicit supported scope and an Unknown state for unsupported or incomplete evidence.
- Attempts & Conversion v2: metric-specific hit-rate and penetration-rate coverage with separate raw facts.
- A weekly report with 2–3 goals from fixed rules (win rate is never a goal) and last week's verdict.
- A map-led interface: vehicle icons, map pictures and loading screens read on demand from the approved game folder, decoded and scaled in memory, never written to disk; the game's own vehicle names.
- Polish and English UI, wide-screen layouts, a custom title bar, reduced-motion support, diagnostics, and anonymous session export.
- No account or cloud service: the tracker reads only local replay files.

The team-relative results engine is currently an internal, read-only analytics module. It is not exposed as a public API or UI feature yet.

## Attempts & Conversion

Attempts & Conversion is a concrete example of the project’s contract-first approach. The v2 contract keeps coverage metric-specific instead of treating every counter as one global funnel:

- common context eligibility is separate from hit-rate eligibility;
- hit rate uses `shots → direct enemy hits`;
- penetration rate uses `direct enemy hits → piercing enemy hits`;
- direct team hits, explosion hits, and damage are retained as separate facts;
- `null` is not zero, and a zero denominator is not a rejection;
- invalid or conflicting facts fail closed with stable reason catalogs;
- Rust engine output is mapped through a stable DTO and validated again at the TypeScript boundary before rendering.

This keeps partial evidence visible without turning it into a misleading score.

## Architecture

```text
World of Tanks replay
        ↓
Bounded local tracker and replay parser
        ↓
Normalized Battle + versioned anonymous facts
        ↓
SQLite v10 persistence
        ↓
Deterministic analytics engines
        ↓
Rust/Tauri commands
        ↓
TypeScript runtime validation
        ↓
React UI
```

The Rust backend owns path validation, file handles, parsing, credential access, SQLite, and analytics rules. The WebView receives typed IPC responses and has no direct filesystem, credential, or arbitrary network capability. Optional expected-value updates and integrations are controlled by the backend.

## Engineering decisions

- Fail-closed versioned contracts for parser, feature, facts, DTO, and UI boundaries.
- Nullable raw facts instead of false zero values.
- Separate domain concerns: parsing, persistence, aggregation, presentation, and coaching.
- Read-only deterministic analytics engines with checked arithmetic and stable ordering.
- Explicit parser provenance and idempotent reprocessing.
- Transactional, additive migrations with integrity and foreign-key checks.
- Local-first privacy model with bounded file access and no telemetry.
- Test-first boundary validation followed by controlled manual acceptance on Windows.

## Engineering case study

### Problem

The first Attempts & Conversion model treated counter availability too globally. A missing counter could incorrectly exclude a different metric that was still calculable.

### Discovery

An end-to-end audit showed that hit rate and penetration rate have different required inputs and must not share one denominator or rejection decision.

### Correction

The v2 implementation separated common context, hit-rate eligibility, penetration-rate eligibility, and independent fact coverage. It aggregates checked sums before division and preserves `null` values.

### Validation

Regression tests cover missing piercing without blocking hit rate, missing shots without blocking penetration rate, valid zero values, zero denominators, conflict reasons, DTO invariants, and TypeScript validation.

### Outcome

The contract is explicit, deterministic, fail-closed, and testable at the Rust engine, JSON DTO, and UI boundaries.

## AI-assisted development

Afterbattle is developed through an AI-assisted engineering workflow. Coding agents are used for scoped implementation, refactoring, test generation, code review, and validation. Requirements and contracts are written before implementation, and agent output is checked through automated tests, security audits, controlled real-data copies, and manual acceptance.

The working process is intentionally explicit:

```text
specification → implementation → tests → runtime validation
→ audit → narrow correction → re-audit → checkpoint
```

AI output is not treated as a source of truth; the repository’s contracts, tests, and observed behavior are the authority.

## Quality and validation

The 1.0.0-beta.2 candidate has been checked locally on Windows:

- Frontend tests: 308/308 passed; ESLint and Prettier clean; production build passes.
- Rust tests: 744 passed, 0 failed; 5 ignored tests read local game data and are run by the author only.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: pass.
- Mutation testing on new and changed logic, at least 85% per run: `cargo mutants` for the Rust parsers, rules and analytics, Stryker (`tools/mutants`) for the frontend logic (motion, live refresh, loading screen).
- A read-only golden test over the author's local replays keeps the statistics population and a metrics digest fixed between stages.

GitHub Actions CI (actions pinned to commit SHAs) runs lint, frontend tests and build, Rust formatting, Clippy and tests, `cargo audit` and `pnpm audit --prod`, and an installer build. This repository does not claim a production-readiness guarantee.

## Project status and limitations

1.0.0-beta.2 is the second public beta, marked as a pre-release. The scope is Windows desktop usage with locally recorded random-battle replays.

Known limitations include:

- Replay formats are version-gated; unsupported or ambiguous shapes remain partial or Unknown.
- Attempts & Conversion deliberately excludes SPG mechanics and does not infer missing counters or causes.
- Optional XVM expected-value updates require a validated dataset; offline operation uses the local snapshot when available.
- The installer is unsigned and has no automatic updater (see "Installing" below).
- See [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md) for the full list.
- Native Tauri behavior at every Windows scaling and display configuration remains part of the release checklist.

## Running locally

Prerequisites:

- Windows 10 or 11.
- Node.js 20 or newer.
- pnpm 11.
- Rust 1.89 or newer.
- Microsoft C++ Build Tools and WebView2 for Tauri development.

```powershell
pnpm install --frozen-lockfile
pnpm lint:setup
pnpm lint
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm tauri dev
```

The application asks the user to approve a local World of Tanks installation before tracking replays. Core replay parsing, SQLite persistence, and analytics run locally. The installer is built with `scripts/release.ps1` (see [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md)).

## Installing

### Po polsku

1. Pobierz `Afterbattle_1.0.0-beta.2_x64-setup.exe` i `SHA256SUMS.txt` z wydania `v1.0.0-beta.2` na GitHubie (oznaczonego jako pre-release).
2. Sprawdź, czy plik jest nienaruszony. W PowerShellu, w folderze z pobranymi plikami:
   ```powershell
   (Get-FileHash -Algorithm SHA256 .\Afterbattle_1.0.0-beta.2_x64-setup.exe).Hash.ToLower()
   Get-Content .\SHA256SUMS.txt
   ```
   Oba ciągi znaków muszą być identyczne. Jeśli się różnią, nie instaluj i zgłoś to w zgłoszeniach (Issues) repozytorium.
3. Uruchom instalator. Instalator nie jest podpisany, więc Windows SmartScreen pokaże ostrzeżenie „System Windows ochronił ten komputer”. Kliknij **Więcej informacji**, sprawdź nazwę pliku i wybierz **Uruchom mimo to**. Instalacja jest tylko dla bieżącego użytkownika i nie wymaga uprawnień administratora.
4. Przy pierwszym uruchomieniu wskaż folder World of Tanks. Aplikacja czyta tylko powtórki i pliki gry; nic nie wysyła, dopóki nie włączysz danych WN8 z XVM.

### In English

1. Download `Afterbattle_1.0.0-beta.2_x64-setup.exe` and `SHA256SUMS.txt` from the `v1.0.0-beta.2` release on GitHub (marked as a pre-release).
2. Check the file is intact. In PowerShell, in the folder with the downloads:
   ```powershell
   (Get-FileHash -Algorithm SHA256 .\Afterbattle_1.0.0-beta.2_x64-setup.exe).Hash.ToLower()
   Get-Content .\SHA256SUMS.txt
   ```
   Both strings must be identical. If they differ, do not install and report it in the repository's Issues.
3. Run the installer. It is not signed, so Windows SmartScreen shows "Windows protected your PC". Click **More info**, check the file name and choose **Run anyway**. It installs for the current user only and needs no administrator rights.
4. On first start, choose your World of Tanks folder. The app only reads replays and game files; it sends nothing until you turn on XVM WN8 data.

## Privacy and security

Afterbattle is local-first:

- replay processing happens locally;
- raw replay contents are not sent to the frontend or stored in battle tables;
- SQLite history is local and uses nullable, versioned facts;
- the approved game path is kept in Windows Credential Manager, never in SQLite or exports;
- there is no telemetry, advertising, cloud AI, or developer backend;
- the only network request is the opt-in, validated XVM expected-value dataset;
- game pictures are read from the approved game folder into a size-limited RAM cache only.

The application validates approved paths, bounds untrusted files, uses a narrow Tauri capability set, and keeps unsupported or conflicting data out of deterministic analytics.

## Repository structure

```text
src/                    React/TypeScript UI, contracts, tests, and local assets
src-tauri/src/           Rust backend, parser, SQLite storage, and analytics engines
src-tauri/capabilities/  Tauri permission declarations
docs/                    Architecture, privacy, threat model, checklists, and research
.github/workflows/       CI (test, audit, installer build)
scripts/                 Release script (installer + SHA256SUMS.txt)
```

## Screenshots

Screenshots of every screen in every state are in [docs/status/screens/](docs/status/screens/). They are made outside the app from synthetic data, with placeholders instead of game graphics, so they contain no account names, replay metadata or Wargaming artwork.

## Not affiliated with Wargaming

Afterbattle is an independent, unofficial project. It is not affiliated with, endorsed by, or sponsored by Wargaming.net, World of Tanks, or XVM. World of Tanks and related names, marks and game assets belong to Wargaming.net. This repository contains no game files or game graphics: the app reads vehicle icons, map pictures and names from the player's own game installation at run time, keeps them only in memory and never ships or stores them. It works only after a battle, from finished replay files; it adds no in-game overlay and reads neither game memory nor network traffic.

## License

The project is licensed under Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE). The bundled Big Shoulders and Inter fonts are under the SIL Open Font License 1.1; their notices are listed in [NOTICE](NOTICE) and kept in [src/assets/fonts/](src/assets/fonts/).
