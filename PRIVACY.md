# Privacy notice

Afterbattle has no developer-operated backend, telemetry, advertising, analytics, account registration, cloud AI, or automatic crash upload. Core tracking works offline.

## Local files read

After explicit approval of a World of Tanks installation, the Rust backend may read:

- stable `.wotreplay` files inside the canonical `replays` directory, reading only bounded declared metadata/result JSON blocks with a 64 MiB per-file limit;
- `res/packages/scripts.pkg` of the approved installation, reading only the vehicle lists (`scripts/item_defs/vehicles/<nation>/list.xml`) to resolve vehicle class and the translation keys of vehicle names.
- `res/text/lc_messages/<nation>_vehicles.mo` of the approved installation (the game's own translation files for vehicle names), on demand, locally and without network access, with size and entry limits checked before reading. The names are kept only in memory and shown in the window; they are never written to the database, to disk, or to an export.
- `res/packages/gui*.pkg` of the approved installation, reading only vehicle icons (`gui/maps/icons/vehicle/`), map images (`gui/maps/icons/map/stats/` and `gui/maps/icons/map/`) and map loading screens (`gui/maps/icons/map/screen/`) on demand, locally and without network access. Loading screens are decoded and scaled down to the size shown, in memory. The images are kept only in memory (a cache of at most 64 MB, emptied when the game updates) and are never written to disk, exported, or bundled with the app.

The frontend never receives raw file contents. Afterbattle does not read `python.log`, garage data, credits, gold, bonds, chat, screenshots, or unrelated files. A native watcher is combined with a periodic recovery scan. Unsupported or ambiguous data becomes partial/N/A and is not guessed. Only strict random battles count towards statistics.

## Local storage

SQLite stores normalized battle metadata, optional combat metrics, sessions, feedback inputs, language, timeout, and non-sensitive replay processing state. The processing state contains a one-way source-path key, size, mtime, block count, and content fingerprint. It does **not** store raw replay bytes, raw log records, raw API responses, the full WoT path, filenames, or account identifiers.

For each strict random battle of the player, SQLite also keeps a local, anonymous battle roster: the vehicle and battle statistics of every participant, without nicknames, account IDs, or clans; it is excluded from the export.

When a database from an older version is upgraded, it is first copied unchanged to `afterbattle.v{n}.bak` next to it; the new database is then rebuilt from the replays. A database file that is corrupt is moved aside to `afterbattle.corrupt-<time>.sqlite3` and a new one is rebuilt from the replays. If the database cannot be opened for another reason (for example it was written by a newer version, or too many of its battles have no replay file left), Afterbattle starts in recovery mode with an empty recovery database at one fixed path next to the real one (never in the system temporary folder). In recovery mode no battles are imported and "Delete all local data" is refused, so the real history is never reported as deleted while it still exists. The recovery database is removed on the next successful start.

Windows Credential Manager stores only the approved WoT path. Earlier versions could also store a Tomato.gg profile/key and a Wargaming standalone `application_id`; those integrations are removed, and every start removes their leftover entries. Credential values are never returned to the frontend; it receives status booleans and the game folder display name only.

## Optional network requests

Both network features need the player's yes; without it no request is sent:

- XVM: after separate opt-in, a static expected-values file is requested only from `https://static.modxvm.com/wn8-data-exp/json/wg/wn8exp.json`, at most once per day. No request is sent while XVM is disabled. No player or battle identifier is sent. Redirects are disabled and response sizes and timeouts are bounded.
- Update check (since 1.0.0-rc.1): the app asks once — on the first start, or on the first start after updating from a beta — whether to check for updates at start; the answer can be changed in Settings → Privacy and data. With a yes, once per start it requests the update file `https://github.com/dataflorist02/afterbattle-beta/releases/latest/download/latest.json` (GitHub redirects the request to its download server). The request is a plain HTTP GET with the updater's generic user agent (`tauri-plugin-updater/<version>`); no player, battle, installation or app identifier and no app data is sent. As with any connection, GitHub sees the computer's IP address and the time. The file contains only the newest version number, its notes, and the installer's signature and address. The new version is downloaded and installed only after the player clicks "Install and restart", and only if its signature matches the author's public key built into the app. With a no, the About section shows the address of the releases page instead.

The WebView itself has no internet permission; both requests are made by the backend.

## Export

The session export excludes nickname, account ID, credentials, full paths, local fingerprints/arena IDs, session UUIDs, exact timestamps, vehicle IDs/names, and map names. It includes duration, performance statistics, quality labels, and feedback. Review it before publishing because a rare statistical sequence may still be recognizable to someone who already knows the session.

## Deletion

**Delete all local data** first stops access to the configured source, then removes the WoT path and any leftover Tomato.gg or Wargaming entries from Credential Manager. It transactionally deletes replay processing state, battles (including the anonymous rosters), sessions, and profile/history settings, then truncates the WAL and vacuums the database, and removes the `afterbattle.v{n}.bak` backups and `afterbattle.corrupt-*` copies. Language and inactivity preferences remain. Files owned by World of Tanks are never modified or deleted. In recovery mode deletion is refused (see Local storage).

Same-user malware or a fully compromised Windows account can still access process memory, local files, or OS credentials; that is outside the protection a same-user desktop app can provide.
