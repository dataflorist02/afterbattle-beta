# Release checklist — 1.0.0-beta.4

Replaces the alpha's Windows checklist. The current public release is `1.0.0-beta.4` ("Afterbattle beta 4"; template summaries in Polish, spec 30); before it `1.0.0-beta.3` (a hotfix for finding the game folder), `1.0.0-beta.2` (internally v2 with E9, spec 29) and `1.0.0-beta.1`. The detailed manual test of the release candidate, in the order it is run, is the author's internal manual test list; this checklist is the frame around it.

## Before building

- The release commit holds every stage; the author's internal release notes list each stage's checks, golden result and mutation results.
- Local checks pass: `pnpm lint`, `pnpm test`, `pnpm build`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (in `src-tauri/`).
- The read-only golden test (`golden_local_replays`, run with `--ignored`) over the author's replays up to 2026-11-11 21:00 gives 2013 battles and digest `1703f8dbeb68775e`.
- `cargo audit` and `pnpm audit --prod` report no high-severity finding.
- The version is set only in `src-tauri/Cargo.toml` (`release.test.ts` keeps `package.json` equal; `tauri.conf.json` has none).
- `CHANGELOG.md` has the 1.0.0-beta.4 section; README, privacy notice (EN/PL), SECURITY, THREAT_MODEL and KNOWN_ISSUES match the code.

## Building (author)

- Clean checkout of the release commit, then `powershell -ExecutionPolicy Bypass -File scripts\release.ps1`.
- Note the commit, installer path, size and SHA-256 the script prints; `SHA256SUMS.txt` lies next to the installer in `src-tauri\target\release\bundle\nsis\`.
- Check the file the way users will: `(Get-FileHash -Algorithm SHA256 <installer>).Hash.ToLower()` equals the line in `SHA256SUMS.txt`.

## Manual test (author)

- Run the author's internal manual test list on the author's machine, then a controlled installation on a second physical Windows 10 or 11 x64 machine: current-user install, no administrator prompt, no autostart entry, SmartScreen warning as described in README.
- Record WoT build, Afterbattle commit, OS build, battle count, partial count and every deviation in the author's internal release notes.

## Publishing (author)

- Private repository: `main` fast-forwarded to the release commit, tag `v1.0.0-beta.4`, push.
- Public repository: updated by the author with the reviewed snapshot commit (`docs/PUBLIC_RELEASE_REVIEW.md` there); its CI builds the installer artifact.
- GitHub release `v1.0.0-beta.4` **marked as a pre-release**, with the installer and `SHA256SUMS.txt` (`gh release create v1.0.0-beta.4 --prerelease`).
