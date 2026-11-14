//! One-time v9 (or older) → v10 rebuild (spec 20, D5, P8; author's correction of 2026-11-12).
//!
//! The legacy database is never altered:
//! 1. it is copied to `<stem>.v{n}.bak` with `VACUUM INTO` before anything else;
//! 2. the P8 gate counts **full** legacy battles without a replay file. Above 5% of the full
//!    battles nothing else happens and the start is refused with
//!    [`AppError::LegacyRebuildBlocked`]. Partial rows never counted towards statistics; they
//!    are outside the gate and stay only in the backup (author's decision of 2026-11-13);
//! 3. a new v10 database is built next to it and receives only what replays cannot recreate:
//!    settings, the sessions that have a full battle or are the source of a challenge (the
//!    active one closed as timed out), challenges with their frozen data, and a map from the
//!    full legacy battle ids to those sessions (`legacy_battles`);
//! 4. the new database replaces the old file; the import engine then fills it from the replays
//!    in the background and writes the "recalculated / not random / without replay" report.
//!
//! Battles without a replay file stay only in the backup (P8).

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use rusqlite::{Connection, OpenFlags, Transaction, params};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    schema::{
        backup_into, backup_path, create_v10, remove_database_files, remove_file_if_exists, sibling,
    },
    storage::Database,
};

/// P8: the rebuild does not start when more than this share of legacy battles has no replay.
pub(crate) const MAX_MISSING_PERCENT: i64 = 5;
/// Settings key of the rebuild report.
pub(crate) const REBUILD_REPORT_KEY: &str = "v10_rebuild";

/// Where the legacy rebuild looks for replay files.
#[derive(Debug, Clone, Default)]
pub struct LegacySource {
    /// The game's `replays` directory, when the source is configured.
    pub replay_dir: Option<PathBuf>,
}

/// P8 gate input: full legacy battles and how many of them have no replay file today.
/// `partial` rows (any other data quality) are outside the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GateReport {
    pub total: i64,
    pub missing: i64,
    pub partial: i64,
}

impl GateReport {
    pub(crate) fn blocks(self) -> bool {
        self.missing.saturating_mul(100) > self.total.saturating_mul(MAX_MISSING_PERCENT)
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebuildState {
    Pending,
    Done,
}

/// The player-facing report of the v10 rebuild ("recalculated N battles, skipped M non-random,
/// K without a replay file").
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildReport {
    pub legacy_version: i64,
    pub legacy_battles: i64,
    pub missing_at_start: i64,
    pub state: RebuildState,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub recalculated: i64,
    pub not_random: i64,
    /// Full legacy battles whose replay file was not found by the import.
    pub without_replay: i64,
    pub conflicts: i64,
    /// Legacy rows that are not full battles; they stay only in the backup.
    #[serde(default)]
    pub skipped_partial: i64,
    /// Legacy sessions left without any battle; they stay only in the backup.
    #[serde(default)]
    pub skipped_empty_sessions: i64,
}

/// Rebuilds a legacy database as v10. On success the file at `path` is a new v10 database and
/// `<stem>.v{n}.bak` holds the untouched legacy data.
pub(crate) fn rebuild(
    path: &Path,
    legacy_version: i64,
    source: &LegacySource,
) -> Result<GateReport, AppError> {
    let backup = backup_path(path, legacy_version);
    {
        let original = Connection::open(path)?;
        backup_into(&original, &backup)?;
    }
    let work = sibling(path, "legacy-work.sqlite3");
    let build = sibling(path, "v10-build.sqlite3");
    remove_database_files(&work)?;
    remove_database_files(&build)?;
    // Always rebuild from the backup: it is complete even if an earlier attempt was interrupted
    // after the original's write-ahead log was removed.
    fs::copy(&backup, &work)?;
    let outcome = (|| {
        let gate = {
            let mut legacy = Connection::open(&work)?;
            legacy.execute_batch("PRAGMA journal_mode=DELETE;")?;
            Database::migrate_legacy_to_v9(&mut legacy)?;
            let keys = replay_keys(&legacy, source);
            gate_report(&legacy, keys.as_ref())?
        };
        if gate.blocks() {
            return Err(AppError::LegacyRebuildBlocked {
                missing: gate.missing,
                total: gate.total,
            });
        }
        build_v10(&build, &work, legacy_version, gate)?;
        Ok(gate)
    })();
    remove_database_files(&work)?;
    let gate = match outcome {
        Ok(gate) => gate,
        Err(error) => {
            remove_database_files(&build)?;
            return Err(error);
        }
    };
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        remove_file_if_exists(Path::new(&name))?;
    }
    fs::rename(&build, path)?;
    Ok(gate)
}

fn replay_keys(legacy: &Connection, source: &LegacySource) -> Option<HashSet<String>> {
    let replay_dir = source.replay_dir.clone().or_else(|| {
        let root: String = legacy
            .query_row(
                "SELECT value FROM settings WHERE key='wot_root'",
                [],
                |row| row.get(0),
            )
            .ok()?;
        Some(PathBuf::from(root).join("replays"))
    })?;
    crate::local_tracker::replay_source_keys(&replay_dir)
}

/// SQL condition selecting the legacy rows the P8 gate and the rebuild look at.
const FULL_BATTLE: &str = "b.data_quality='full'";

/// Counts full legacy battles and those without a replay file among `keys` (P8). Without a
/// replay directory every full battle counts as missing. Rows of another data quality are
/// only counted as `partial`.
pub(crate) fn gate_report(
    legacy: &Connection,
    keys: Option<&HashSet<String>>,
) -> Result<GateReport, AppError> {
    let (total, missing) = gate_counts(legacy, keys, FULL_BATTLE)?;
    let partial = legacy.query_row(
        &format!("SELECT COUNT(*) FROM battles b WHERE NOT {FULL_BATTLE}"),
        [],
        |row| row.get(0),
    )?;
    Ok(GateReport {
        total,
        missing,
        partial,
    })
}

/// (rows, rows without a replay file among `keys`) for the legacy rows matching `filter`.
fn gate_counts(
    legacy: &Connection,
    keys: Option<&HashSet<String>>,
    filter: &str,
) -> Result<(i64, i64), AppError> {
    legacy.execute_batch(
        "DROP TABLE IF EXISTS temp.current_replay_keys;
         CREATE TEMP TABLE current_replay_keys(key TEXT PRIMARY KEY NOT NULL);",
    )?;
    if let Some(keys) = keys {
        let mut insert =
            legacy.prepare("INSERT OR IGNORE INTO temp.current_replay_keys(key) VALUES(?1)")?;
        for key in keys {
            insert.execute([key])?;
        }
    }
    let counts = legacy.query_row(
        &format!(
            "SELECT COUNT(*),COALESCE(SUM(CASE WHEN EXISTS(
                SELECT 1 FROM replay_files f JOIN temp.current_replay_keys k ON k.key=f.source_path_key
                WHERE f.battle_id=b.arena_id) THEN 0 ELSE 1 END),0)
             FROM battles b WHERE {filter}"
        ),
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    legacy.execute_batch("DROP TABLE temp.current_replay_keys;")?;
    Ok(counts)
}

fn copy_table(transaction: &Transaction<'_>, table: &str, filter: &str) -> Result<(), AppError> {
    let columns = {
        let mut statement = transaction.prepare(&format!("PRAGMA main.table_info({table})"))?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?
    };
    let legacy_columns = {
        let mut statement = transaction.prepare(&format!("PRAGMA legacy.table_info({table})"))?;
        statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<HashSet<_>, _>>()?
    };
    let shared = columns
        .into_iter()
        .filter(|column| legacy_columns.contains(column))
        .collect::<Vec<_>>()
        .join(",");
    transaction.execute(
        &format!("INSERT INTO main.{table}({shared}) SELECT {shared} FROM legacy.{table} {filter}"),
        [],
    )?;
    Ok(())
}

fn build_v10(
    build: &Path,
    work: &Path,
    legacy_version: i64,
    gate: GateReport,
) -> Result<(), AppError> {
    let mut connection = Connection::open(build)?;
    connection.execute_batch("PRAGMA journal_mode=DELETE; PRAGMA foreign_keys=ON;")?;
    connection.execute(
        "ATTACH DATABASE ?1 AS legacy",
        [work.to_string_lossy().into_owned()],
    )?;
    let transaction = connection.transaction()?;
    create_v10(&transaction)?;
    copy_table(&transaction, "settings", "")?;
    // Sessions cannot be recreated from replays (D14), so they move (author's correction of
    // 2026-11-12), except those that can never get a battle back: without any full battle
    // (author's decision of 2026-11-13). A session that is the source of a challenge always
    // moves. The session active at the rebuild is closed as timed out at its last activity
    // first; the copy in the backup is untouched.
    copy_table(
        &transaction,
        "sessions",
        &format!(
            "WHERE EXISTS(SELECT 1 FROM legacy.battles b
               WHERE b.session_id=legacy.sessions.id AND {FULL_BATTLE})
             OR id IN (SELECT source_session_id FROM legacy.challenges)"
        ),
    )?;
    let skipped_empty_sessions: i64 = transaction.query_row(
        "SELECT (SELECT COUNT(*) FROM legacy.sessions)-(SELECT COUNT(*) FROM main.sessions)",
        [],
        |row| row.get(0),
    )?;
    transaction.execute(
        "UPDATE main.sessions SET status='timed_out',
         ended_at=COALESCE(ended_at,last_battle_at,started_at) WHERE status='active'",
        [],
    )?;
    copy_table(&transaction, "challenges", "")?;
    copy_table(&transaction, "challenge_battles", "")?;
    // Only full battles are expected back from the replays; partial rows stay in the backup.
    transaction.execute(
        &format!(
            "INSERT INTO legacy_battles(arena_id,arena_unique_id,session_id)
             SELECT b.arena_id,b.arena_unique_id,s.id
             FROM legacy.battles b LEFT JOIN main.sessions s ON s.id=b.session_id
             WHERE {FULL_BATTLE}"
        ),
        [],
    )?;
    let report = RebuildReport {
        legacy_version,
        legacy_battles: gate.total,
        missing_at_start: gate.missing,
        state: RebuildState::Pending,
        started_at: Utc::now().timestamp_millis(),
        finished_at: None,
        recalculated: 0,
        not_random: 0,
        without_replay: 0,
        conflicts: 0,
        skipped_partial: gate.partial,
        skipped_empty_sessions,
    };
    transaction.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![
            REBUILD_REPORT_KEY,
            serde_json::to_string(&report).map_err(|_| AppError::InvalidApiResponse)?
        ],
    )?;
    transaction.commit()?;
    connection.execute_batch("DETACH DATABASE legacy;")?;
    if !Database::integrity_is_healthy(&connection)? {
        return Err(AppError::DatabaseIntegrity);
    }
    Ok(())
}

/// Opens a file read-only to read its `user_version` and whether it holds any table.
pub(crate) fn inspect(path: &Path) -> Result<(i64, bool), AppError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let version = crate::schema::user_version(&connection)?;
    let has_tables: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table')",
        [],
        |row| row.get(0),
    )?;
    Ok((version, has_tables))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_blocks_strictly_above_five_percent() {
        assert!(
            !GateReport {
                total: 0,
                missing: 0,
                partial: 0
            }
            .blocks()
        );
        assert!(
            !GateReport {
                total: 100,
                missing: 5,
                partial: 0
            }
            .blocks()
        );
        assert!(
            GateReport {
                total: 100,
                missing: 6,
                partial: 0
            }
            .blocks()
        );
        assert!(
            !GateReport {
                total: 40,
                missing: 2,
                partial: 0
            }
            .blocks()
        );
        assert!(
            GateReport {
                total: 39,
                missing: 2,
                partial: 0
            }
            .blocks()
        );
        assert!(
            GateReport {
                total: 1,
                missing: 1,
                partial: 0
            }
            .blocks()
        );
    }

    #[test]
    fn gate_report_works_on_a_read_only_connection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("v9.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE battles(arena_id TEXT PRIMARY KEY, data_quality TEXT);
                 CREATE TABLE replay_files(source_path_key TEXT PRIMARY KEY, battle_id TEXT);
                 INSERT INTO battles VALUES('a','full'),('b','full'),('c','full'),
                   ('p1','partial'),('p2','partial'),('p3','ambiguous');
                 INSERT INTO replay_files VALUES('ka','a'),('kb','b'),('gone','c'),('kp','p1');",
            )
            .unwrap();
        drop(connection);
        let read_only = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        // Partial and ambiguous rows are outside the gate, with or without a replay file.
        let keys = ["ka", "kb", "kp"].map(str::to_owned).into_iter().collect();
        assert_eq!(
            gate_report(&read_only, Some(&keys)).unwrap(),
            GateReport {
                total: 3,
                missing: 1,
                partial: 3
            }
        );
        assert_eq!(
            gate_report(&read_only, None).unwrap(),
            GateReport {
                total: 3,
                missing: 3,
                partial: 3
            }
        );
        // The definition before 2026-11-13 counted every row.
        assert_eq!(gate_counts(&read_only, Some(&keys), "1").unwrap(), (6, 3));
    }

    /// P8 for the author (spec 20, E2 checkpoint): number and share of battles in a **copy** of
    /// the real v9 database that have no replay file today. Read-only on both; prints numbers
    /// only. Run manually:
    ///
    /// ```text
    /// AFTERBATTLE_V9_DB_COPY=<copy of afterbattle.sqlite3> AFTERBATTLE_GOLDEN_DIR=<replays>
    /// cargo test --lib legacy_gate_on_a_copy -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a copy of the author's v9 database"]
    fn legacy_gate_on_a_copy() {
        let copy = std::env::var_os("AFTERBATTLE_V9_DB_COPY").expect("AFTERBATTLE_V9_DB_COPY");
        let replays = std::env::var_os("AFTERBATTLE_GOLDEN_DIR").expect("AFTERBATTLE_GOLDEN_DIR");
        let connection = Connection::open_with_flags(
            PathBuf::from(copy),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        let version = crate::schema::user_version(&connection).unwrap();
        let keys = crate::local_tracker::replay_source_keys(Path::new(&replays));
        let gate = gate_report(&connection, keys.as_ref()).unwrap();
        let (all_rows, all_missing) = gate_counts(&connection, keys.as_ref(), "1").unwrap();
        let share = |missing: i64, total: i64| {
            if total > 0 {
                format!("{:.2}%", missing as f64 * 100.0 / total as f64)
            } else {
                "-".to_owned()
            }
        };
        println!("=== P8 gate on a copy of the v9 database (numbers only) ===");
        println!("user_version            = {version}");
        println!("-- gate since 2026-11-13: full battles only --");
        println!("full battles            = {}", gate.total);
        println!("without a replay file   = {}", gate.missing);
        println!(
            "share without replay    = {}",
            share(gate.missing, gate.total)
        );
        println!("partial rows (skipped)  = {}", gate.partial);
        println!(
            "rebuild would start     = {}",
            if gate.blocks() { "no (> 5%)" } else { "yes" }
        );
        println!("-- earlier definition: every row --");
        println!("battles in database     = {all_rows}");
        println!("without a replay file   = {all_missing}");
        println!("share without replay    = {}", share(all_missing, all_rows));
        println!("=== end ===");
    }

    /// "Has a replay file" exactly as [`gate_report`] decides it: a `replay_files` row of the
    /// battle whose path key belongs to a file present in the folder today.
    const HAS_REPLAY: &str = "EXISTS(SELECT 1 FROM replay_files f \
         JOIN temp.breakdown_keys k ON k.key=f.source_path_key WHERE f.battle_id=b.arena_id)";

    /// Diagnostics of a blocked P8 gate: aggregate numbers only (no ids, names or paths).
    /// `keys` are the path keys of the files in the folder, `arena_ids` the `arenaUniqueID`
    /// values read from their result blocks. Read-only; uses temporary tables only.
    fn gate_breakdown(
        legacy: &Connection,
        keys: &HashSet<String>,
        arena_ids: &HashSet<i64>,
    ) -> Result<Vec<String>, AppError> {
        legacy.execute_batch(
            "DROP TABLE IF EXISTS temp.breakdown_keys;
             DROP TABLE IF EXISTS temp.breakdown_arenas;
             DROP TABLE IF EXISTS temp.breakdown_missing;
             CREATE TEMP TABLE breakdown_keys(key TEXT PRIMARY KEY NOT NULL);
             CREATE TEMP TABLE breakdown_arenas(id INTEGER PRIMARY KEY NOT NULL);",
        )?;
        {
            let mut insert =
                legacy.prepare("INSERT OR IGNORE INTO temp.breakdown_keys(key) VALUES(?1)")?;
            for key in keys {
                insert.execute([key])?;
            }
            let mut insert =
                legacy.prepare("INSERT OR IGNORE INTO temp.breakdown_arenas(id) VALUES(?1)")?;
            for id in arena_ids {
                insert.execute([id])?;
            }
        }
        legacy.execute_batch(&format!(
            "CREATE TEMP TABLE breakdown_missing AS
             SELECT b.arena_id AS arena_id FROM battles b WHERE NOT {HAS_REPLAY};"
        ))?;
        let number = |sql: &str| legacy.query_row(sql, [], |row| row.get::<_, i64>(0));
        let missing = "battles b JOIN temp.breakdown_missing m ON m.arena_id=b.arena_id";
        let mut lines = Vec::new();

        lines.push("battles by source_kind x data_quality (with replay / without):".to_owned());
        {
            let mut statement = legacy.prepare(&format!(
                "SELECT b.source_kind,b.data_quality,
                        COALESCE(SUM(CASE WHEN {HAS_REPLAY} THEN 1 ELSE 0 END),0),
                        COALESCE(SUM(CASE WHEN {HAS_REPLAY} THEN 0 ELSE 1 END),0)
                 FROM battles b GROUP BY 1,2 ORDER BY 1,2"
            ))?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            for row in rows {
                let (kind, quality, with, without) = row?;
                lines.push(format!("  {kind} x {quality} : {with} / {without}"));
            }
        }

        lines.push(format!(
            "replay_files rows = {}, of them with a file in the folder = {}",
            number("SELECT COUNT(*) FROM replay_files")?,
            number(
                "SELECT COUNT(*) FROM replay_files f
                 JOIN temp.breakdown_keys k ON k.key=f.source_path_key"
            )?
        ));
        lines.push(format!(
            "without replay = {}",
            number("SELECT COUNT(*) FROM temp.breakdown_missing")?
        ));
        lines.push(format!(
            "  no replay_files row at all            = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing}
                 WHERE NOT EXISTS(SELECT 1 FROM replay_files f WHERE f.battle_id=b.arena_id)"
            ))?
        ));
        lines.push(format!(
            "  replay_files row, file not in folder  = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing}
                 WHERE EXISTS(SELECT 1 FROM replay_files f WHERE f.battle_id=b.arena_id)"
            ))?
        ));
        lines.push(format!(
            "  arena_unique_id found among replays   = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing}
                 WHERE b.arena_unique_id IN (SELECT id FROM temp.breakdown_arenas)"
            ))?
        ));
        lines.push(format!(
            "  arena_unique_id not among replays     = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing} WHERE b.arena_unique_id IS NOT NULL
                 AND b.arena_unique_id NOT IN (SELECT id FROM temp.breakdown_arenas)"
            ))?
        ));
        lines.push(format!(
            "  arena_unique_id empty                 = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing} WHERE b.arena_unique_id IS NULL"
            ))?
        ));
        lines.push(format!(
            "  battle_type_raw empty                 = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing} WHERE b.battle_type_raw IS NULL"
            ))?
        ));
        lines.push(format!(
            "  bonus_type_raw empty                  = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing} WHERE b.bonus_type_raw IS NULL"
            ))?
        ));
        lines.push(format!(
            "  both raw types empty                  = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing}
                 WHERE b.battle_type_raw IS NULL AND b.bonus_type_raw IS NULL"
            ))?
        ));

        lines.push("without replay by parser_version:".to_owned());
        {
            let mut statement = legacy.prepare(&format!(
                "SELECT COALESCE(CAST(b.parser_version AS TEXT),'none'),COUNT(*)
                 FROM {missing} GROUP BY 1 ORDER BY b.parser_version"
            ))?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (version, count) = row?;
                lines.push(format!("  parser {version} : {count}"));
            }
        }

        const MONTH: &str = "strftime('%Y-%m',b.battle_time/1000,'unixepoch')";
        for (label, source) in [("all battles", "battles b"), ("without replay", missing)] {
            let (first, last): (Option<String>, Option<String>) = legacy.query_row(
                &format!("SELECT MIN({MONTH}),MAX({MONTH}) FROM {source}"),
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            lines.push(format!(
                "{label} by month of battle_time (UTC), first {} last {}:",
                first.as_deref().unwrap_or("-"),
                last.as_deref().unwrap_or("-")
            ));
            let mut statement = legacy.prepare(&format!(
                "SELECT {MONTH},COUNT(*) FROM {source} GROUP BY 1 ORDER BY 1"
            ))?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (month, count) = row?;
                lines.push(format!("  {month} : {count}"));
            }
        }

        lines.push(format!(
            "sessions = {}, with a battle without replay = {}, with only such battles = {}",
            number("SELECT COUNT(*) FROM sessions")?,
            number(&format!(
                "SELECT COUNT(DISTINCT b.session_id) FROM {missing} WHERE b.session_id IS NOT NULL"
            ))?,
            number(
                "SELECT COUNT(*) FROM sessions s
                 WHERE EXISTS(SELECT 1 FROM battles b WHERE b.session_id=s.id)
                 AND NOT EXISTS(SELECT 1 FROM battles b WHERE b.session_id=s.id
                   AND b.arena_id NOT IN (SELECT arena_id FROM temp.breakdown_missing))"
            )?
        ));
        lines.push(format!(
            "battles without replay attached to a session = {}",
            number(&format!(
                "SELECT COUNT(*) FROM {missing} WHERE b.session_id IS NOT NULL"
            ))?
        ));
        let has_challenges: bool = legacy.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='challenge_battles')",
            [],
            |row| row.get(0),
        )?;
        if has_challenges {
            lines.push(format!(
                "challenges = {}, with a battle without replay = {}, such challenge battles = {} of {}",
                number("SELECT COUNT(*) FROM challenges")?,
                number(
                    "SELECT COUNT(DISTINCT c.challenge_id) FROM challenge_battles c
                     JOIN temp.breakdown_missing m ON m.arena_id=c.battle_id"
                )?,
                number(
                    "SELECT COUNT(*) FROM challenge_battles c
                     JOIN temp.breakdown_missing m ON m.arena_id=c.battle_id"
                )?,
                number("SELECT COUNT(*) FROM challenge_battles")?
            ));
        } else {
            lines.push("challenges = no challenge tables".to_owned());
        }
        legacy.execute_batch(
            "DROP TABLE temp.breakdown_missing;
             DROP TABLE temp.breakdown_arenas;
             DROP TABLE temp.breakdown_keys;",
        )?;
        Ok(lines)
    }

    #[test]
    fn gate_breakdown_counts_groups_on_a_read_only_connection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("v9.sqlite3");
        let connection = Connection::open(&path).unwrap();
        // 2024-01-15 and 2024-03-10 (UTC) in milliseconds.
        connection
            .execute_batch(
                "CREATE TABLE sessions(id TEXT PRIMARY KEY);
                 CREATE TABLE battles(arena_id TEXT PRIMARY KEY, battle_time INTEGER,
                   source_kind TEXT, data_quality TEXT, arena_unique_id INTEGER,
                   battle_type_raw INTEGER, bonus_type_raw INTEGER, parser_version INTEGER,
                   session_id TEXT);
                 CREATE TABLE replay_files(source_path_key TEXT PRIMARY KEY, battle_id TEXT);
                 CREATE TABLE challenges(id TEXT PRIMARY KEY);
                 CREATE TABLE challenge_battles(challenge_id TEXT, battle_id TEXT);
                 INSERT INTO sessions VALUES('s1'),('s2'),('s3');
                 INSERT INTO battles VALUES
                   ('a',1705312800000,'replay','full',101,1,1,7,'s1'),
                   ('b',1705312800000,'replay','full',102,1,1,7,'s1'),
                   ('c',1705312800000,'replay','full',103,NULL,NULL,5,'s2'),
                   ('d',1710064800000,'replay','partial',NULL,NULL,1,NULL,'s2'),
                   ('e',1710064800000,'local_hybrid','full',105,1,NULL,5,NULL);
                 INSERT INTO replay_files VALUES('ka','a'),('gone-b','b'),('gone-d','d');
                 INSERT INTO challenges VALUES('c1'),('c2');
                 INSERT INTO challenge_battles VALUES('c1','a'),('c1','b'),('c2','a');",
            )
            .unwrap();
        drop(connection);
        let read_only = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        let keys = ["ka", "other"].map(str::to_owned).into_iter().collect();
        let arena_ids = [101, 102, 999].into_iter().collect();
        let lines = gate_breakdown(&read_only, &keys, &arena_ids).unwrap();
        // The breakdown counts every row, like the gate did before 2026-11-13; the gate
        // itself now looks at full battles only.
        assert_eq!(gate_counts(&read_only, Some(&keys), "1").unwrap(), (5, 4));
        assert_eq!(
            gate_report(&read_only, Some(&keys)).unwrap(),
            GateReport {
                total: 4,
                missing: 3,
                partial: 1
            }
        );
        assert_eq!(
            lines,
            [
                "battles by source_kind x data_quality (with replay / without):",
                "  local_hybrid x full : 0 / 1",
                "  replay x full : 1 / 2",
                "  replay x partial : 0 / 1",
                "replay_files rows = 3, of them with a file in the folder = 1",
                "without replay = 4",
                "  no replay_files row at all            = 2",
                "  replay_files row, file not in folder  = 2",
                "  arena_unique_id found among replays   = 1",
                "  arena_unique_id not among replays     = 2",
                "  arena_unique_id empty                 = 1",
                "  battle_type_raw empty                 = 2",
                "  bonus_type_raw empty                  = 2",
                "  both raw types empty                  = 1",
                "without replay by parser_version:",
                "  parser none : 1",
                "  parser 5 : 2",
                "  parser 7 : 1",
                "all battles by month of battle_time (UTC), first 2024-01 last 2024-03:",
                "  2024-01 : 3",
                "  2024-03 : 2",
                "without replay by month of battle_time (UTC), first 2024-01 last 2024-03:",
                "  2024-01 : 2",
                "  2024-03 : 2",
                "sessions = 3, with a battle without replay = 2, with only such battles = 1",
                "battles without replay attached to a session = 3",
                "challenges = 2, with a battle without replay = 1, such challenge battles = 1 of 3",
            ]
        );
    }

    /// Diagnostics for the author after a blocked P8 gate: what the battles "without a replay
    /// file" are. Read-only on a **copy** of the v9 database and on the replay folder; prints
    /// aggregate numbers only (no nicknames, ids, paths or file names). Run manually:
    ///
    /// ```text
    /// AFTERBATTLE_V9_DB_COPY=<copy of afterbattle.sqlite3> AFTERBATTLE_GOLDEN_DIR=<replays>
    /// cargo test --lib legacy_gate_breakdown_on_a_copy -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs a copy of the author's v9 database"]
    fn legacy_gate_breakdown_on_a_copy() {
        let copy = std::env::var_os("AFTERBATTLE_V9_DB_COPY").expect("AFTERBATTLE_V9_DB_COPY");
        let replays = std::env::var_os("AFTERBATTLE_GOLDEN_DIR").expect("AFTERBATTLE_GOLDEN_DIR");
        let connection = Connection::open_with_flags(
            PathBuf::from(copy),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        let version = crate::schema::user_version(&connection).unwrap();
        let keys = crate::local_tracker::replay_source_keys(Path::new(&replays))
            .expect("replay folder is readable");
        let (files, with_results, arena_ids) =
            crate::local_tracker::replay_arena_unique_ids(Path::new(&replays))
                .expect("replay folder is readable");
        let gate = gate_report(&connection, Some(&keys)).unwrap();
        println!("=== P8 gate breakdown on a copy of the v9 database (numbers only) ===");
        println!("user_version = {version}");
        println!(
            "gate (full battles only): battles = {}, without a replay file = {}, partial rows = {}",
            gate.total, gate.missing, gate.partial
        );
        println!(
            "replay folder: files = {files}, path keys = {}, with a result block = {with_results}, distinct arenaUniqueID = {}",
            keys.len(),
            arena_ids.len()
        );
        for line in gate_breakdown(&connection, &keys, &arena_ids).unwrap() {
            println!("{line}");
        }
        println!("=== end ===");
    }
}
