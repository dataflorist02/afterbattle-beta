# Architecture

## Boundaries

The React WebView renders bundled assets and invokes a narrow Tauri command surface. It cannot read files, retrieve credentials, or make internet requests. Rust is the security boundary and owns path validation, file handles, parsing, correlation, network clients, credentials, SQLite, and coaching rules.

```text
approved WoT root
  ├─ replays/*.wotreplay ─┬─> LocalTracker ─> normalized Battle ─> SQLite v9
                          │         │                   │
Windows native watcher ───┘         └─ SourceStatus     └─ deterministic analysis

Credential Manager: canonical WoT path only (leftover Tomato.gg/WG entries removed at startup)
Optional HTTPS (opt-in): static.modxvm.com fixed dataset only
WebView: typed IPC only; no external network/file capability
```

## LocalTracker

- Canonicalizes the approved root, replay directory, and replay entries; targets must remain inside the allowlisted directory.
- Coalesces native watcher events and runs a 30-second recovery scan through a single-flight operation lock.
- Requires two matching size/mtime observations before parsing a newly written live replay. A later change requeues the same file with bounded retries.
- Rejects replay files over 64 MiB, metadata blocks over 2 MiB, result blocks over 8 MiB, and invalid block boundaries before allocation. It parses only the declared metadata/result JSON blocks, not the encrypted packet timeline.
- Uses `arenaUniqueID`, then a hashed source-path key, then stable metadata as replay identity. The full-file SHA-256 is stored only as change/integrity state.
- Maintains a bounded log cursor `(prefix, offset, carry)`. Appended records are consumed once. A changed prefix or shorter file is treated as rotation and primes a new baseline without generating a delta.
- Queues a bounded stats delta briefly so a replay that is still stabilizing can arrive. Correlation uses vehicle, ordering, an event-time window, and battle count. Multiple candidates become `ambiguous`; no aggregate is assigned to a single battle.
- Imports at most 500 replay metadata/result records whose battle time is within 90 days, without creating an active session.

### Final frags provenance

Replay parser version 5 implements `final-frags-v1`. For a Full result the
canonical player frag count is `R[0].personal.<own-player>.kills`. The parser
joins the player's `accountDBID` to the unique arena-vehicle key in
`R[0].vehicles` and, when available, uses only `R[2].frags[arena-vehicle]` as an
independent confirmation. `R[1].frags` is an initial/non-final value and is
never a source or fallback.

Matching `P.kills` and `R[2]` confirms the scalar. A mismatch or malformed
present confirmation fails closed with a safe parser error and cannot overwrite
an existing Full battle. Missing `R[2]` still permits the bounded `P.kills`
scalar; missing or invalid `P.kills` remains `NULL`, including when another
section contains a number. Zero is valid only when supplied by bounded
`P.kills`.

`legacy-replay-reconciliation-v1` explicitly permits parser versions 2, 3, 4,
and 5 to be reparsed by parser 6. `NULL`, versions 0 and 1, negative values,
version 6, and unknown future versions are not members of the reconciliation
cohort. A hashed source-path association, a readable source replay, a successful
parser result, and non-conflicting battle identity are all required; parser
version alone is insufficient.

A successful Full reparse transactionally updates the existing battle and
parser provenance without changing identity or session membership. Canonical
facts may be corrected only from the source replay; the update never guesses,
uses broad-counter fallbacks, or converts missing values to zero. Schema-only
compatibility does not change analytics, while a verified canonical fact
correction can legitimately change WN8 and dependent read-time analytics
without changing the WN8 formula, expected values, point-in-time cutoffs, or
challenge ledger. Repeated parser-6 processing is a duplicate. Missing, corrupt,
or conflicting sources leave the legacy version and prior Full data intact.

### Attempts and conversion raw provenance

Replay parser version 6 and feature schema 2 add four nullable raw counters
from the canonical personal results object: `directEnemyHits`,
`directTeamHits`, `piercingEnemyHits`, and `explosionHits`. Missing or malformed
values remain `NULL`, while a valid zero remains zero. The parser does not infer
these fields from the broader `directHits` or `piercings` counters and does not
calculate rates, eligibility, aggregation, coaching, or UI output at this stage.

SQLite schema version 8 adds the corresponding nullable integer columns without
defaults or SQL backfill. A successful parser-6 reprocessing pass updates the
existing replay row to feature schema 2 exactly once; missing, corrupt, or
conflicting sources retain their previous provenance and facts.

Feature schema 2 is an additive superset of feature schema 1. Parser version 7
adds feature schema 3 as an additive superset of schema 2. Existing core
consumers (WN8, Battle Pace, challenges, baseline, and trends) explicitly allow
only schema 1, 2, or 3. The active `attempts-conversion-v2` engine allows only
schema 2 or 3. Unknown future schema versions fail closed; these explicit
allow-lists must not be replaced with a broad condition such as
`feature_schema_version >= 1`.

### Anonymous team-relative raw facts

Replay parser version 7 and feature schema 3 implement the ingestion and
storage portion of `team-relative-results-v1`. For Full Random 15v15 CTF
results, the parser resolves the own player by one exact positive
`accountDBID` join held only inside the parsing call. It then discards identity
and emits only aggregate facts: team player count, team damage total and
competition counts, team final-frag total, XP competition counts, and
metric-specific failure reasons. It persists no account identifier, arena
vehicle identifier, nickname, roster row/list, raw JSON, or replay path. Missing
facts remain `NULL`; valid zero totals remain zero.

SQLite schema version 9 adds the STRICT `team_relative_facts` table with at
most one fact-schema-1 row per battle. Its foreign key follows the existing
text battle key (`battles.arena_id`) and cascades on battle deletion. CHECK
constraints enforce the closed common and metric reason catalogs, the exact
eligible/ineligible state shapes, and aggregate ranges. Migration 8 to 9 is
additive and does not backfill historical rows.

Normal parser-7 import writes the battle and anonymous facts in one transaction.
Legacy reconciliation considers only Full parser versions 2 through 6 with an
existing `replay_files` association, processes at most 100 rows ordered by
`battle_time, arena_id`, and upgrades only parser/schema provenance plus one
facts row. Source lookup first uses the one-way path key and, after an exact
file move, may use the stored whole-file SHA-256; battle identity and every
canonical battle fact must still match. Conflicts fail closed without
overwriting data.

### Team-relative results read-only engine

`team-relative-results-v1` has an internal deterministic report engine with
session and last-10/30/100 scopes. Its narrow SQLite projection uses one
parameterized `LEFT JOIN` from `battles` to `team_relative_facts`, opens the
database read-only with `query_only=ON` and `trusted_schema=OFF`, and reads no
replay, roster, participant identity, or path. Selection happens before
eligibility; recent scopes never backfill rejected rows with older battles.

The version-state gate accepts current Full facts only as parser/feature/fact
versions `7/3/1`. It also accepts exactly one current Partial sentinel:
`DataQuality::Partial`, parser 7, null feature schema, fact schema 1,
`partial_replay`, and null team and metric facts/reasons. A legacy battle
without a fact row and without current parser state becomes
`parser_too_old`. Missing current rows, unknown versions, and every other
hybrid state fail closed as a report invariant violation.

Damage and final-frag shares are ratios of checked aggregate sums, never means
of per-battle ratios. Zero denominators remain eligible and N/A. Damage and WG
XP use exact 15-bucket competition-rank distributions with explicit tie
counts. Common, damage, frags, and XP coverage remain independent and use
complete, stable reason catalogs; missing values never become zero.

R0.3d adds no public serde DTO, Tauri report command, TypeScript contract, UI,
persisted report, migration, reconciliation, network request, coaching,
baseline, trend, challenge, WN8, Battle Pace, or outcome change.

### Attempts and conversion read-only engine

`attempts-conversion-v2` is the active read-only, non-SPG report engine. The
historical v1 NO-GO audit remains a research record; it is not an active public
contract. V2 accepts only
Full `replay_results` rows parsed by version 6 or newer with feature schema 2
or 3,
the exact Random 15v15 identity (`battleType=1`, `bonusType=1`, `gameplayID=ctf`),
and a resolved non-SPG vehicle class from one immutable catalog snapshot. That
common context intentionally does not inspect counters. Its stable first
rejection catalog has ten reasons: partial replay, missing results provenance,
old parser, unsupported schema, unsupported mode, invalid team structure,
catalog unavailable, class unknown, class conflict, and SPG.

The engine calculates enemy hit rate as `sum(direct_enemy_hits) / sum(shots)`
and penetration rate as `sum(piercing_enemy_hits) / sum(direct_enemy_hits)`.
It aggregates checked integer sums before dividing, keeps independent coverage
for each rate, and returns N/A for a zero denominator. Hit eligibility only
requires `shots` and `direct_enemy_hits`; penetration eligibility only requires
`direct_enemy_hits` and `piercing_enemy_hits`, while valid optional `shots`
provide a cross-check. Missing team, explosion, damage, or piercing data cannot
lower hit coverage; missing shots cannot lower penetration coverage. Each rate
has the ten common reasons plus `missing_counters`, `counter_out_of_range`, and
`counter_conflict` in a frozen thirteen-code catalog.

`direct_team_hits`, `explosion_hits`, and `damage_dealt` are independent facts,
not a complete funnel: each has its own coverage and nullable sum. A valid zero
is a fact; a missing fact is N/A. Explosion hits deliberately have no causal
comparison with shots. The UI presents two separate relations, `shots → enemy
hits` and `enemy hits → penetrations`, plus fact cards; it never implies a
shared cohort where metric coverage differs. AC-001 is fixed by these separate
evaluators and AC-002 by localized singular/plural contributing labels.

Session and last-10/30/100 reports select all rows first and then apply
eligibility. Recent windows are deterministically ordered by
`battle_time DESC, arena_id DESC` and never backfill rejected rows with older
eligible rows. The engine writes no ratings, eligibility, percentages, baseline
data, coaching signals, or SQLite state.

### Attempts and conversion public backend contract

The read-only `attempts-conversion-v2` engine is exposed through exactly two
local Tauri commands:

- `get_attempts_conversion_session_report(sessionId)` for one existing session;
- `get_attempts_conversion_recent_report(window)` for the latest 10, 30, or 100
  battle records.

The recent command rejects every other window with the safe `invalid_window`
command error before issuing a storage query. A missing session returns the
existing safe `session_not_found` error. Storage failures are exposed only as
the existing generic command error and never include SQL, paths, replay names,
or account data. A DTO invariant failure returns
`report_invariant_violation`, also without internal details.

Both commands take one immutable vehicle-class catalog snapshot, read the
existing narrow query projection, invoke the existing engine once, and map its
report without writing SQLite, reprocessing replay files, changing session
membership, or starting any network request. Catalog unavailability is a normal
per-battle `vehicle_class_catalog_unavailable` rejection, not a command error.

The response is camelCase JSON with this stable shape:

```text
{
  contractVersion: "attempts-conversion-v2",
  scope: { type: "session", sessionId } | { type: "recent", window },
  selectedBattles,
  commonContext: { eligibleBattles, ineligibleBattles, rejectionReasons },
  hitRate: {
    selectedBattles, commonEligibleBattles, metricEligibleBattles,
    metricIneligibleBattles, contributingBattles, zeroDenominatorBattles,
    missingBattles, invalidBattles, numeratorSum, denominatorSum, value,
    rejectionReasons
  },
  penetrationRate: { ...same rate coverage shape... },
  facts: { directTeamHits, explosionHits, damageDealt }
}
```

`value` is an unformatted ratio in `0.0..=1.0`; unavailable ratios are JSON
`null`, never `0`, `"N/A"`, a percentage string, or localized text. Fact sums
are unformatted integer sums and remain `null` when their own fact has no
eligible values; no approximation is made.

Common-context `rejectionReasons` always contain its 10 frozen codes; every
rate and fact catalog contains all 13 frozen snake_case codes in engine
priority order, including zero counts: `partial_replay`,
`results_provenance_missing`, `parser_too_old`, `unsupported_schema`,
`unsupported_mode`, `invalid_team_structure`,
`vehicle_class_catalog_unavailable`, `vehicle_class_unknown`,
`vehicle_class_conflict`, `unsupported_vehicle_mechanics`,
`missing_counters`, `counter_out_of_range`, and `counter_conflict`. Their count
sum equals the ineligible count of that specific coverage. The DTO mapper and
TypeScript boundary fail closed on count, reason-catalog, ratio, null, NaN, and
infinite-value violations. There is no persisted attempts/conversion report,
parser/schema/storage migration, reprocessing, network request, coaching,
baseline comparison, WN8, Battle Pace, challenge, or trend change.

## Data model and sessions

Every metric is nullable. `sourceKind` is `replay` (`local_hybrid` and `tomato` exist only in data written by older versions); `dataQuality` is `full`, `partial`, or `ambiguous`. Missing values remain `NULL`/N/A. Raw payloads and paths have no battle-table column. The private replay-file state table stores only a one-way path key, size, mtime, block count, and content fingerprint.

The first newly inserted live battle atomically creates a session. A duplicate cannot create an empty session. A manually started session may remain empty while active, but ending or timing it out removes it instead of persisting empty history. A battle after the inactivity threshold times out the old session before starting a new one. A partial unique index permits only one active session. A background timer continues while the window is hidden in the tray.

Feedback begins with at least five complete baseline battles and requires five samples for the evaluated metric. Confidence is low below 10, medium at 10–29, and high at 30+. Estimated WN8 is display-only and excluded from coaching.

## Persistence and deletion

SQLite uses STRICT tables, foreign keys, parameterized statements, WAL, `trusted_schema=OFF`, schema version 9, staged legacy migrations that drop `raw_json`, and additive migrations for replay identity, provenance, outcomes, Battle Pace, challenges, attempts/conversion raw counters, and anonymous team-relative facts. Real `integrity_check` and `foreign_key_check` results gate initialization and are available through an internal report. Credentials and identifying configuration live in Windows Credential Manager rather than SQLite.

Legacy cleanup is explicit and never runs at startup. It transactionally merges only certain duplicate groups, preserves complementary nullable metrics, reparents replay processing state, removes empty non-active sessions, and leaves ambiguous candidates unchanged. Repeated cleanup is idempotent.

Deletion is ordered: stop/clear the source under the tracker lock, remove every credential, enable SQLite secure deletion, transactionally delete replay state/history/profile settings, truncate WAL, then VACUUM. Language and inactivity preferences are preserved.

## Optional networking

XVM is a separate opt-in GET to one compiled-in URL. A single-flight lock and persisted attempt timestamp enforce at most one download per day; streamed content is limited to 4 MiB and schema-validated. Defense is treated as zero, so output is labelled Estimated WN8.
