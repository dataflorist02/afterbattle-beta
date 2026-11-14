# Changelog

## 1.0.0-beta.1

Pierwsze publiczne wydanie — beta, instalator bez podpisu. Wewnętrznie ta wersja to „v2” (etapy E0–E8 niżej). / First public release — a beta with an unsigned installer. Internally this version is "v2" (stages E0–E8 below).

### Po polsku — co się zmienia dla gracza

- **Nowy wygląd.** Ekrany Sesja, Czołgi, Cele, Historia, Bitwa i Ustawienia na grafikach map z Twojej gry, z własnym paskiem tytułu okna. Na dużych monitorach mapy są ostre: aplikacja bierze ekrany ładowania z gry i dopasowuje je do miejsca, w którym je pokazuje. Nazwy czołgów są takie jak w grze.
- **Sesja opowiada, jak poszło.** Jedno zdanie na górze („Mocna sesja: obrażenia o 18% wyżej niż zwykle”), bitwa po bitwie z najlepszą wyróżnioną i najciekawsze momenty sesji.
- **Cele na tydzień.** Dwa–trzy cele wyliczone z Twoich czterech poprzednich tygodni (przetrwanie, obrażenia z asystą na ulubionym czołgu, XP), postęp na bieżąco i ocena poprzedniego tygodnia. Procent wygranych nigdy nie jest celem.
- **Liczą się tylko bitwy losowe** z pełnym wynikiem; trening, turnieje i wydarzenia są zapisane, ale nie psują statystyk. Asysta to wykrycie + gąsienice, XP bez premium.
- **Nic nie ginie.** Cała historia powtórek wczytuje się w tle, bez limitu 500 plików i 90 dni. Przed każdą zmianą bazy powstaje kopia; uszkodzona baza jest odsuwana na bok i odtwarzana z powtórek.
- **Prywatność.** Aplikacja czyta tylko powtórki i pliki gry, nic nie wysyła; jedynym połączeniem są dane WN8 z XVM, i to dopiero po włączeniu. Usunięto Tomato.gg, klucz Wargaming i czytanie `python.log`.
- **Spokojniejsza praca.** Długie operacje nie blokują okna, odczyt powtórek można zatrzymać, a ruch na ekranie wyłącza ustawienie „ogranicz ruch” w Windows.

### In English — what changes for the player

- **New look.** Session, Tanks, Goals, History, Battle and Settings screens on map pictures from your own game, with the window's own title bar. On large monitors maps are sharp: the app uses the game's loading screens and fits them to where they are shown. Tank names are the game's own.
- **The session tells you how it went.** One sentence at the top ("Strong session: damage 18% above usual"), battle by battle with the best one framed, and the session's notable moments.
- **Goals for the week.** Two or three goals worked out from your four previous weeks (survival, damage with assist on your favourite tank, XP), live progress and a verdict on last week. Win rate is never a goal.
- **Only random battles count**, with full results; training, tournaments and events are stored but never skew your statistics. Assist is spotting + tracking, XP is without premium.
- **Nothing gets lost.** Your whole replay history is read in the background, without the 500-file and 90-day limits. Every database change makes a backup first; a damaged database is moved aside and rebuilt from replays.
- **Privacy.** The app only reads replays and game files and sends nothing; the one connection is the XVM WN8 data, and only after you turn it on. Tomato.gg, the Wargaming key and reading `python.log` are gone.
- **Smoother.** Long operations never freeze the window, reading replays can be stopped, and on-screen motion follows the Windows "reduce motion" setting.

### v2 stage public-docs — self-contained public documents

- Public documents and code comments no longer refer to files kept out of the public repository; the snapshot script stops when one does.
- The public snapshot is committed with the GitHub noreply address of the logged-in account, set only in that repository.

### v2 stage public-prep — public repository

- The public repository is the private tree minus whole excluded files (`scripts/make-public-snapshot.ps1`), scanned for secrets, personal data and game files before every commit; `docs/PUBLIC_RELEASE_REVIEW.md` lists what is left out and why.
- `NOTICE` with the font licenses next to Apache-2.0; README says the project is not affiliated with Wargaming.
- Test data: synthetic arena id and replay file names. CI keeps the unsigned installer and `SHA256SUMS.txt` as a build artifact.

### v2 stage E8-ui-fix — calmer motion

- Tanks: the screen no longer slides in over its loading state; map tiles, their bars and the chart enter once, after the data, at the History tempo; choosing another tank shows it still.
- History: the entrance wave covers all rows visible at first display (up to 20), not just the first 10.

### v2 stage E8 — release

- The version has one source, `src-tauri/Cargo.toml`: Tauri reads it there, the window gets it at build time, and a test keeps `package.json` equal. The first public release is `1.0.0-beta.1`; the title bar and window title read "Afterbattle beta 1.0"; the channel is "Public beta".
- `scripts/release.ps1` builds the installer from a clean tree and writes `SHA256SUMS.txt`; README explains the SmartScreen warning and checking the file with `Get-FileHash`.
- CI: actions pinned to commit SHAs, separate test, audit (`cargo audit`, `pnpm audit --prod`) and installer build jobs; Dependabot also updates the actions (in the private repository only).
- The alpha's manual data cleanup left the test suite (archived outside the public repository, not built).
- Documents match 2.0: README, privacy notice (EN/PL), SECURITY, THREAT_MODEL, KNOWN_ISSUES and `docs/RELEASE_CHECKLIST.md` (replaces the alpha checklist).

### v2.0 stage E5-maps-hd — sharp map pictures, own title bar

- Map pictures are sharp on large screens: headers, moment cards and history rows use the game's loading screens (2560 × 1600), decoded and scaled down in the app to the exact size shown. A picture is never stretched more than 1.25×; when the game has only a small picture, it sits on the right over a blurred copy of itself.
- Fixed pictures being scaled down and then stretched again in tall tiles.
- The window has its own title bar in the app's style: drag anywhere on it, double-click to maximize or restore, minimize, maximize/restore and close (to the tray, as before). The Windows 11 snap layouts flyout is not available.

### v2.0 stage E7 — weekly report

- The Goals screen opens with "Goals for this week": up to three goals set from your four previous weeks (survive your median share plus 5 points, damage with assist on your most played tank plus 10%, XP plus 10%), each with where it comes from and a progress bar. Win rate is never a goal; it appears only as context.
- "This week against the previous four": damage with assist, survival and XP against your own average on the same tanks, with the range of the four weeks before; WN8 with XVM data. Fewer than 15 battles in the week: "too few battles for a trend".
- Last week's goals judged in one line, and the 5-battle challenges finished this week. Weeks are ISO weeks (Monday to Sunday) in local time; when a clock change skips Monday midnight, the week starts at its first valid hour. New command `get_weekly_report`; nothing new is stored.

### v2.0 stage E5-flare-fix2 — sticky navigation, calmer History

- The navigation rail stays in the window on desktop: Settings is always visible at its bottom on every screen, however long; the rail scrolls on its own only when the window is lower than its content. In a narrow window the rail sits above the content and does not stick.
- History motion: the whole screen no longer slides in; only the first 10 rows of the first visit enter, softly (320 ms, 6 px, staggered within about 400 ms). Rows below, rows loaded later, a filter change, switching Battles/Sessions and coming back from a battle show no motion.

### v2.0 stage E5-flare-fix — damage trend numbers match the strip

- The "damage trend" session moment shows the damage of the first and the last battle of the session when every battle had more (less) damage than the one before, the same numbers as the first and the last tile of the strip. Otherwise it keeps the averages of the first and the last third and says so next to the numbers ("Average of the first 3 battles → of the last 3").

### v2.0 stage E5-flare — map-led direction C, Big Shoulders

- Map pictures from the game files carry the screens: the Session and Battle headers sit on a map, battles are tiles with their map, History rows have a map strip. Pictures come from `map/stats` (500×235) with the square `map/` pictures as a fallback, load only when visible and are scaled to the tile before the window keeps them; without game files every tile and header has a gradient placeholder. Loading screens (DDS) are not read.
- Session: a headline sentence from fixed rules (strong, calm, tough or neutral session, same damage reference as the hero numbers), the session numbers on the best battle's map, a "battle by battle" strip (losses dimmed and named, the best battle framed) and session moments (damage trend, the tank to improve, the best battle). New command `get_session_story`.
- Tank: compact header with three numbers against the player's average and a grid of the player's maps on that tank (4+ battles, two best and the weakest named); the chart and the battle table stay.
- Battle: a big header on the map with the damage as the main number, "Your battle" next to the teams and a "You on <map>" card with all the player's battles on that map and a link to History filtered by it. New command `get_map_record`.
- History: day headers ("Today", "Yesterday", a date), survival in words, losses dimmed and named; sessions as cards with a collage of their maps. New command `get_session_maps`. Goals: a tank icon at goal titles.
- Big Shoulders (SIL OFL 1.1, local woff2) replaces Barlow Semi Condensed for numbers and headings; Inter stays for text.
- Motion explains changes only: motion tokens, hover and press feedback, a sliding segmented-control highlight, a focus ring that fades in, a light screen entry, a one-time cascade of tiles and drawing of bars, a count-up of the main number only when a new battle arrives in the running session. Only transform and opacity move, nothing loops, `prefers-reduced-motion` turns movement off.

### v2.0 stage E5-ui — UI corrections after the author's manual test

- Wide screens: up to 1440 px the layout is one column as before; above it the content grows to about 1840 px, centred next to the navigation, with two columns on Session (hero card and insights | battles), Battle (result and statistics | team comparison) and Tank (cards and chart | maps and battles, side by side from 1920 px), and extra History columns (destroyed, survival). Spacing and size tokens are unchanged.
- Tank names: the game's own names are read on demand from its translation files (`res/text/lc_messages/<nation>_vehicles.mo`, keys from `list.xml`) and kept only in memory, in the window language or English (a file without a declared language is the game client's own language and serves both); the UK reads `gb_vehicles.mo`; without them the stored code is shown readable ("Vz 60S Dravec" instead of "czech-Cz21_Vz_60S_Dravec"). Lists show the short name, the Tank header and tooltips the full one; names never wrap. New command `get_vehicle_names`; names are never stored in the database or in an export.
- Attempts and conversion: a card with a three-step funnel (shots → hits → penetrations) with large numbers, captions and transition rates; team and explosion hits on one secondary line; the range is a segmented control (radio group with arrow keys); one "How we count this" dialog lists the skipped battles per indicator instead of five expandable sections.
- Battle pace: one summary line ("2 of 9 battles had a one-sided course") and a small split bar with the name and count of every part in words.
- Session insights fix: a session far below the average on the same tanks was described as "close to your average" when no tank had 3 battles in the session. The hero card and the insights now use one damage reference; when confidence is too low the insights say so.
- Battle screen: the header sits on the map image (placeholder without game files), full page width.
- Session hero card no longer stretches to the cards next to it and shows this session's battles against the average.
- Tank screen: no repeated tank column in its battle table; the chart has a value axis and the player's average as a dashed line; the header names the last battle ("last yesterday").
- Clickable rows: no text selection, pointer cursor, visible keyboard focus. History: the tank filter says when its list failed to load instead of staying empty.

### v2.0 — rebuild gate counts full battles only

- The check before the one-time v10 rebuild ("more than 5% of battles without a replay file") now looks at full battles only. Incomplete battle records from early alpha versions never counted towards statistics; they no longer block the rebuild and stay only in the `.bak` copy. The 5% limit is unchanged.
- Sessions that have no battle after the rebuild are not carried over to the new database (they stay in the `.bak` copy). Sessions with battles, challenges and their history are kept as before.
- The rebuild report in Settings and in the diagnostic report shows separately: full battles without a replay file, skipped incomplete records and skipped empty sessions.

### v2.0 stage E5-fix — corrections after E5

- The Tank screen header shows the vehicle tier: "Medium tank, tier X, 214 battles". The tier comes from the newest battle of that vehicle that has one stored (`tier` in the vehicle summary of `get_vehicle_history_overview` and `get_vehicle_history_report`); when it is unknown the header reads "Medium tank, 214 battles".
- Tests only: the reader of images from the game folder went through a complete mutation run, with new tests for the key pattern, cache replacement and limits, the package filter and the index limit; every screen now has a test for each of its loading, empty, error, no-XVM and too-few-battles states.

### v2.0 stage E5 — UI redesign (with the former E6)

- New interface following the approved mockup: a vertical navigation with Session, Tanks, Goals, History, Maps (greyed out in the private build, hidden in the public build) and Settings; one design-token system with a single orange accent; local Barlow Semi Condensed and Inter fonts (SIL OFL 1.1), no network fonts.
- Screens: Session (average damage against your own average on the same tanks, highlights, battles), Tanks (your last 10 battles against your average of 100 on that tank, damage chart of up to 200 points, best and worst maps, battles in pages of 50), Battle (your result, remaining health, comparison of both teams from team totals only, never a player list), Goals (the existing 5-battle tank challenges), History (battles filtered by tank and map, sessions), Settings with privacy, the rebuild report, WN8 data and diagnostics.
- Tank icons and map images are read on demand from the selected game folder and kept only in a size-limited RAM cache, emptied when the game updates; nothing is written to disk. Placeholders appear when an image is missing. New commands `get_game_image` and `get_battle_view`; the battle history accepts a map filter.
- Every screen has loading, empty, error, no-XVM and too-few-battles states; errors are shown in Polish or English from one dictionary covering every backend code; dialogs close with Escape and keep the focus inside.
- Code structure `app/`, `pages/`, `components/`, `hooks/`, `i18n/`, `lib/`, `styles/`; the old interface, `styles.css` and `alpha.css` are gone. TypeScript types of the IPC DTOs are generated by `ts-rs`; a contract test keeps `api.ts` in line with the registered commands; ESLint (`react-hooks`, `no-floating-promises`) runs with `pnpm lint`.
- Synthetic screenshots of every screen in every state in `docs/status/screens/`.

### v2.0 stage E4 — analysis and WN8

- XVM download: the attempt is recorded when it ends, with its result; an automatic retry waits 1 hour after a failure and 24 hours after a success (D3).
- A bad row in the XVM file (missing field, wrong type, value out of range) is skipped and counted instead of rejecting the whole file, up to 1% of rows; the count is shown in the WN8 status and diagnostics (M8). The parser accepts the live format with `header` after `data` and an extra `url` field (P4 check, 2026-11-12).
- Reference averages, session insights and challenge suggestions qualify each metric on its own: damage, survival and assist work without XVM, WN8 alone needs the expected values (M5, D7). A baseline that is too small carries no signals (M10).
- The attempts-conversion contract accepts feature schema 2 or 3 (M12).
- Per-battle feedback uses two SQL aggregates instead of loading up to 200 full rows (P4).
- The XVM data version is shown once, in the footer and in Settings, not next to values (D3).
- WN8 is checked against an independent calculator in three cases (±0.5).

### v2.0 stage E3 — threads, events, import performance

- Commands that touch files or many rows run off the window thread (`spawn_blocking`); a command that panics reports an error instead of crashing.
- `start_import` and `cancel_import` replace `import_local_history`: the full import runs in the background, reports progress with the `import-progress` event and stops after the current batch on request; the next start continues from the cursor without duplicates.
- Events `data-changed` and `session-ended` replace the 10-second polling; the 30-second timer is gone. Automatic session end has one schedule, in the tracker thread, with an injected clock (tests no longer sleep).
- The replay content hash covers only the JSON blocks, not the whole file.
- Lighter snapshot: WN8 of the last 30 sessions moved to the new command `get_recent_sessions_wn8` (spec §7, 2026-11-12).
- The window shows "Reading replays: x / y" with a "Stop reading" button.

### v2.0 stage E2 — database v10, migrations, recovery, no missing battles, data foundations

- Database schema v10. A database from an older version is first copied unchanged to `afterbattle.v{n}.bak`; if more than 5% of its battles have no replay file left, nothing else happens and the app starts in recovery mode with the code `legacy_rebuild_blocked`. Otherwise a new v10 database replaces it, keeping settings, all sessions (one still active is closed as timed out) and challenges with their frozen data; every battle is then recalculated from its replay in the background, keeps its id and its session, and a report "recalculated / not random / without replay / conflicts" is written (shown in diagnostics). Battles without a replay stay only in the backup.
- From v10 on, migrations are incremental steps driven by `user_version`, with a `.bak` copy before each step and one transaction per step; a database from a newer version is refused (`database_too_new`). The integrity check runs once at start.
- Full import without the 90-day and 500-file limits, in the background, in batches of 100 with a resumable cursor; at every start files missing from the database or changed since (size, modification time) are read. Battles older than the inactivity limit do not create sessions. Non-random replays are remembered and not parsed again. Progress is part of the tracker status.
- A corrupt database file is moved to `afterbattle.corrupt-<time>.sqlite3` and a new database is rebuilt from the replays; "Delete all local data" also removes `.bak` and `.corrupt-*` files.
- One long-lived write connection behind a mutex; read-only reports keep their own connections.
- Replay parser version 9 stores the data foundations for later analysis: extra own-player columns, vehicle tier with the catalog fingerprint, an anonymous 30-row battle roster (`battle_roster`, no nicknames, account ids, clans, platoon ids or in-battle vehicle ids) with per-team aggregates (`battle_team_stats`), and the size of the replay's packet stream. Both new tables are excluded from the export. `PRIVACY.md` and `docs/PRIVACY_PL.md` describe the roster.
- Removed: the bounded team-relative reconciliation, the in-place parser refresh of old rows, the dead WN8 columns and the Tomato source kind. Team-relative contract amendment A1 accepts parsers 7, 8 and 9.

### v2.0 stage E1b — vehicle tier and roles in the catalog

- The vehicle catalog built from the game's `scripts.pkg` now holds each vehicle's tier (1–11) and `role_*` tags (catalog schema 2). Records with a decimal-string `id` are accepted.
- A missing known nation or a new nation folder no longer rejects the whole catalog; both are reported. Settings show tier coverage and warn when the catalog is out of date for the installed game version. Schema 1 catalogs stay usable (without tiers) until the next refresh rebuilds them.

### v2.0 stage E1 — privacy, data loss, less code, correct inputs

- Privacy: XVM expected values are downloaded only after the player enables XVM; startup, the switch and manual refresh share one gate and send nothing while XVM is off. The no-op `refresh_xvm` command is removed.
- Data safety: if the database cannot be opened, the app uses one fixed recovery database next to the real one (no new files in `%TEMP%`), does not import battles in that mode, removes the recovery database on the next successful start, and refuses "Delete all local data" instead of reporting a false success.
- Statistics count only strict random battles (`battleType == 1` and `bonusType == 1`, read as numbers) with full results; one predicate is used by session metrics, baselines, trends, WN8, challenges and battle feedback. Training, tournament, clan, ranked and event battles are stored but never counted.
- Assist is spotting + tracking (stun is separate); XP averages use XP without premium; a replay time in the repeated autumn DST hour is kept.
- Removed: the `python.log` reader, the Tomato.gg integration and profile, the Wargaming `application_id`, and the commands `local_source_status`, `sync_battles`, `import_history`, `configure_profile`, `store_api_key`, `remove_api_key`, `test_api_connection`, `store_wg_application_id`, `remove_wg_application_id`. Their leftover Credential Manager entries are deleted at every start and by "Delete all local data". The `json5` dependency is removed.
- Replay parser version 8. `PRIVACY.md`, `SECURITY.md`, `THREAT_MODEL.md` and README match the code; `docs/PRIVACY_PL.md` is a faithful translation of `PRIVACY.md`.

### v2.0 stage E0 — safety net, dependencies, formatting

- Security: dependency updates clear every `cargo audit` vulnerability (bundled SQLite via `rusqlite` 0.41 / `libsqlite3-sys` 0.39.1, `rustls`, `quinn-proto`, `rustls-webpki`, `webpki-roots`, `zip` 9.5) and `pnpm audit` for all dependencies. The unused `zod` dependency is removed.
- Dependencies: `sha2` 0.11, `serde_json`, `uuid`, `vite`, `@vitejs/plugin-react`, `react` / `react-dom` 19.3, `jsdom` 30, `@testing-library/jest-dom` 7, `vitest` 4.1.12. Minimum Rust version is now 1.89 (required by `rusqlite` 0.41).
- Frontend sources are formatted with Prettier (`pnpm format`, `pnpm format:check`); no logic changes.
- Tests: characterization tests pin current WN8, baseline, trend, challenge and import-gate behavior, including known defects, as the safety net for the following stages. An ignored, read-only `golden_local_replays` test prints aggregate statistics for a local replay folder.
- No change to application behavior.

## 0.1.0-alpha.1 — private alpha

- Local one-block and two-block `.wotreplay` import with Partial → Full upgrade and deduplication.
- Automatic sessions, inactivity timeout, tray operation, history and per-session reports.
- Validated local XVM expected-values cache, safe background updater and deterministic WN8 with coverage.
- Polish and English alpha interface with readable replay identities and WN8 categories.
- First-run guide, About panel, safe copied diagnostics and friendly UI failure state.
- Official Tauri single-instance protection.
- Current-user Windows x64 NSIS packaging under the stable `app.afterbattle.desktop` identifier.

This build is unsigned, has no automatic updater and is intended only for controlled private testing.
