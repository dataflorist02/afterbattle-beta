//! Database schema v10 and incremental migrations (spec 20, E2, D5).
//!
//! v10 is built fresh: a v9 (or older) database is never altered in place. It is copied to
//! `<stem>.v{n}.bak` before anything else happens, and a new v10 database is created and filled
//! from the replays (`legacy.rs`). From v10 on, every schema change is an incremental step in
//! [`MIGRATIONS`], driven by `user_version`, with a `<stem>.v{n}.bak` copy before each step and one
//! transaction per step.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, Transaction};

use crate::error::AppError;

/// Schema version written by this build.
pub(crate) const SCHEMA_VERSION: i64 = 10;

/// One incremental migration step from `from` to `from + 1`, run inside one transaction.
pub(crate) type MigrationStep = fn(&Transaction<'_>) -> Result<(), AppError>;

/// Incremental steps after v10 as `(from, step)`. Empty in v2.0; the framework is tested with
/// synthetic steps.
pub(crate) const MIGRATIONS: &[(i64, MigrationStep)] = &[];

/// Complete v10 schema. Every column added for the post-v2.0 foundations is nullable without a
/// default (team-relative contract §18). `battle_roster` and `battle_team_stats` hold anonymous
/// facts only (D17): no nickname, account id, clan, raw prebattle id or in-battle vehicle id.
pub(crate) const V10_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL) STRICT;
CREATE TABLE IF NOT EXISTS sessions(
  id TEXT PRIMARY KEY NOT NULL,started_at INTEGER NOT NULL,last_battle_at INTEGER,ended_at INTEGER,
  status TEXT NOT NULL CHECK(status IN ('active','ended','timed_out'))
) STRICT;
CREATE TABLE IF NOT EXISTS battles(
  arena_id TEXT PRIMARY KEY NOT NULL,battle_time INTEGER NOT NULL,vehicle_key TEXT NOT NULL,wg_tank_id INTEGER,
  tank_name TEXT,map_name TEXT,battle_type TEXT,source_kind TEXT NOT NULL CHECK(source_kind IN ('replay','local_hybrid')),
  data_quality TEXT NOT NULL CHECK(data_quality IN ('full','partial','ambiguous')),won INTEGER CHECK(won IN(0,1) OR won IS NULL),
  damage INTEGER CHECK(damage>=0),assist INTEGER CHECK(assist>=0),blocked INTEGER CHECK(blocked>=0),damage_received INTEGER CHECK(damage_received>=0),
  frags INTEGER CHECK(frags>=0),spots INTEGER CHECK(spots>=0),shots INTEGER CHECK(shots>=0),hits INTEGER CHECK(hits>=0),penetrations INTEGER CHECK(penetrations>=0),
  survived INTEGER CHECK(survived IN(0,1) OR survived IS NULL),xp INTEGER CHECK(xp>=0),duration_seconds INTEGER CHECK(duration_seconds>=0),
  session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
  arena_unique_id INTEGER,parser_version INTEGER,game_version TEXT,
  data_completeness TEXT CHECK(data_completeness IN ('metadata_only','results') OR data_completeness IS NULL),
  replay_source TEXT CHECK(replay_source IN ('replay_metadata','replay_results') OR replay_source IS NULL),
  parser_error_code TEXT,defense_points INTEGER CHECK(defense_points>=0),moving_avg_damage INTEGER CHECK(moving_avg_damage>=0),damage_rating INTEGER CHECK(damage_rating>=0),
  feature_schema_version INTEGER CHECK(feature_schema_version>=1),map_key TEXT,gameplay_id TEXT,
  battle_time_source TEXT CHECK(battle_time_source IN ('metadata','file_mtime') OR battle_time_source IS NULL),
  player_team INTEGER CHECK(player_team BETWEEN 0 AND 2 OR player_team IS NULL),winner_team_raw INTEGER CHECK(winner_team_raw BETWEEN 0 AND 2 OR winner_team_raw IS NULL),
  assist_spotting INTEGER CHECK(assist_spotting>=0),assist_tracking INTEGER CHECK(assist_tracking>=0),assist_stun INTEGER CHECK(assist_stun>=0),original_xp INTEGER CHECK(original_xp>=0),
  outcome TEXT NOT NULL DEFAULT 'unknown' CHECK(outcome IN ('victory','defeat','draw','unknown')),
  battle_pace_context TEXT CHECK(battle_pace_context IN ('obvious_steamroll','borderline_steamroll','standard','unknown') OR battle_pace_context IS NULL),
  battle_pace_reason TEXT CHECK(battle_pace_reason IN ('missing_duration','missing_final_roster','missing_team_assignment','missing_finish_reason','unsupported_mode','unsupported_finish_reason','unsupported_schema','invalid_team_size','asymmetric_teams','invalid_destroyed_score','conflicting_data') OR battle_pace_reason IS NULL),
  battle_pace_resolver_version TEXT,finish_reason_raw INTEGER,battle_type_raw INTEGER,bonus_type_raw INTEGER,
  initial_team1_size INTEGER CHECK(initial_team1_size>=0),initial_team2_size INTEGER CHECK(initial_team2_size>=0),
  destroyed_team1_score INTEGER CHECK(destroyed_team1_score>=0),destroyed_team2_score INTEGER CHECK(destroyed_team2_score>=0),
  direct_enemy_hits INTEGER CHECK(direct_enemy_hits>=0),direct_team_hits INTEGER CHECK(direct_team_hits>=0),
  piercing_enemy_hits INTEGER CHECK(piercing_enemy_hits>=0),explosion_hits INTEGER CHECK(explosion_hits>=0),
  vehicle_tier INTEGER CHECK(vehicle_tier BETWEEN 1 AND 11),vehicle_catalog_fingerprint TEXT,
  life_time_seconds INTEGER CHECK(life_time_seconds>=0),death_reason_raw INTEGER,
  max_health INTEGER CHECK(max_health>=0),health_remaining INTEGER,
  direct_hits_received INTEGER CHECK(direct_hits_received>=0),piercings_received INTEGER CHECK(piercings_received>=0),
  no_damage_hits_received INTEGER CHECK(no_damage_hits_received>=0),explosion_hits_received INTEGER CHECK(explosion_hits_received>=0),
  potential_damage_received INTEGER CHECK(potential_damage_received>=0),damage_from_invisibles INTEGER CHECK(damage_from_invisibles>=0),
  assist_smoke INTEGER CHECK(assist_smoke>=0),assist_inspire INTEGER CHECK(assist_inspire>=0),
  stun_duration REAL CHECK(stun_duration>=0),stun_num INTEGER CHECK(stun_num>=0),
  capture_points INTEGER CHECK(capture_points>=0),mileage INTEGER CHECK(mileage>=0),
  arena_type_id INTEGER CHECK(arena_type_id>=0),own_platoon_size INTEGER CHECK(own_platoon_size BETWEEN 1 AND 3),
  imported_at INTEGER NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS replay_files(
  source_path_key TEXT PRIMARY KEY NOT NULL,battle_id TEXT NOT NULL REFERENCES battles(arena_id) ON DELETE CASCADE,
  file_size INTEGER NOT NULL CHECK(file_size>=0),file_mtime_ms INTEGER NOT NULL,content_fingerprint TEXT NOT NULL,
  block_count INTEGER NOT NULL CHECK(block_count>=1),last_processed_at INTEGER NOT NULL,
  packet_stream_bytes INTEGER CHECK(packet_stream_bytes>=0)
) STRICT;
CREATE TABLE IF NOT EXISTS replay_file_skips(
  source_path_key TEXT PRIMARY KEY NOT NULL,file_size INTEGER NOT NULL CHECK(file_size>=0),file_mtime_ms INTEGER NOT NULL,
  reason TEXT NOT NULL CHECK(reason IN ('not_random')),last_processed_at INTEGER NOT NULL
) STRICT;
CREATE INDEX IF NOT EXISTS idx_battles_time ON battles(battle_time DESC);
CREATE INDEX IF NOT EXISTS idx_battles_vehicle_time ON battles(vehicle_key,battle_time DESC);
CREATE INDEX IF NOT EXISTS idx_battles_session ON battles(session_id,battle_time);
CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_replay_files_battle ON replay_files(battle_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_active_session ON sessions((1)) WHERE status='active';
CREATE UNIQUE INDEX IF NOT EXISTS idx_battles_arena_unique_id ON battles(arena_unique_id) WHERE arena_unique_id IS NOT NULL;
CREATE TABLE IF NOT EXISTS battle_team_stats(
  battle_id TEXT NOT NULL REFERENCES battles(arena_id) ON DELETE CASCADE,
  side TEXT NOT NULL CHECK(side IN ('own','enemy')),
  fact_schema_version INTEGER NOT NULL CHECK(fact_schema_version=1),
  players INTEGER NOT NULL CHECK(players=15),
  damage_total INTEGER CHECK(damage_total>=0),kills_total INTEGER CHECK(kills_total>=0),
  spotted_total INTEGER CHECK(spotted_total>=0),shots_total INTEGER CHECK(shots_total>=0),
  direct_enemy_hits_total INTEGER CHECK(direct_enemy_hits_total>=0),piercing_enemy_hits_total INTEGER CHECK(piercing_enemy_hits_total>=0),
  assist_radio_total INTEGER CHECK(assist_radio_total>=0),assist_track_total INTEGER CHECK(assist_track_total>=0),
  assist_stun_total INTEGER CHECK(assist_stun_total>=0),blocked_total INTEGER CHECK(blocked_total>=0),
  damage_received_total INTEGER CHECK(damage_received_total>=0),max_health_total INTEGER CHECK(max_health_total>=0),
  survivors INTEGER CHECK(survivors BETWEEN 0 AND 15),life_time_total INTEGER CHECK(life_time_total>=0),
  tier_min INTEGER CHECK(tier_min BETWEEN 1 AND 11),tier_max INTEGER CHECK(tier_max BETWEEN 1 AND 11),
  tier_sum INTEGER CHECK(tier_sum BETWEEN 15 AND 165),
  class_light INTEGER NOT NULL CHECK(class_light BETWEEN 0 AND 15),class_medium INTEGER NOT NULL CHECK(class_medium BETWEEN 0 AND 15),
  class_heavy INTEGER NOT NULL CHECK(class_heavy BETWEEN 0 AND 15),class_td INTEGER NOT NULL CHECK(class_td BETWEEN 0 AND 15),
  class_spg INTEGER NOT NULL CHECK(class_spg BETWEEN 0 AND 15),
  unresolved_vehicles INTEGER NOT NULL CHECK(unresolved_vehicles BETWEEN 0 AND 15),
  platoons INTEGER NOT NULL CHECK(platoons BETWEEN 0 AND 7),
  platooned_players INTEGER NOT NULL CHECK(platooned_players BETWEEN 0 AND 15),
  CHECK(tier_min IS NULL OR tier_max IS NULL OR tier_min<=tier_max),
  PRIMARY KEY(battle_id,side)
) STRICT;
CREATE TABLE IF NOT EXISTS battle_roster(
  battle_id TEXT NOT NULL REFERENCES battles(arena_id) ON DELETE CASCADE,
  slot INTEGER NOT NULL CHECK(slot BETWEEN 0 AND 29),
  side TEXT NOT NULL CHECK(side IN ('own','enemy')),
  is_own_player INTEGER NOT NULL CHECK(is_own_player IN (0,1)),
  type_comp_descr INTEGER NOT NULL CHECK(type_comp_descr>0),
  vehicle_tier INTEGER CHECK(vehicle_tier BETWEEN 1 AND 11),
  vehicle_class TEXT CHECK(vehicle_class IN ('light_tank','medium_tank','heavy_tank','tank_destroyer','spg')),
  platoon_group INTEGER NOT NULL CHECK(platoon_group BETWEEN 0 AND 15),
  damage_dealt INTEGER CHECK(damage_dealt>=0),kills INTEGER CHECK(kills BETWEEN 0 AND 30),
  spotted INTEGER CHECK(spotted BETWEEN 0 AND 30),shots INTEGER CHECK(shots>=0),
  direct_enemy_hits INTEGER CHECK(direct_enemy_hits>=0),piercing_enemy_hits INTEGER CHECK(piercing_enemy_hits>=0),
  explosion_hits INTEGER CHECK(explosion_hits>=0),assist_radio INTEGER CHECK(assist_radio>=0),
  assist_track INTEGER CHECK(assist_track>=0),assist_stun INTEGER CHECK(assist_stun>=0),
  damage_blocked INTEGER CHECK(damage_blocked>=0),damage_received INTEGER CHECK(damage_received>=0),
  potential_damage_received INTEGER CHECK(potential_damage_received>=0),max_health INTEGER CHECK(max_health>=0),
  life_time INTEGER CHECK(life_time>=0),death_reason_raw INTEGER,
  killer_slot INTEGER CHECK(killer_slot BETWEEN 0 AND 29),
  capture_points INTEGER CHECK(capture_points>=0),defense_points INTEGER CHECK(defense_points>=0),
  mileage INTEGER CHECK(mileage>=0),
  PRIMARY KEY(battle_id,slot)
) STRICT;
CREATE TABLE IF NOT EXISTS legacy_battles(
  arena_id TEXT PRIMARY KEY NOT NULL,
  arena_unique_id INTEGER,
  session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
  state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','matched','conflict','not_random'))
) STRICT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_legacy_battles_unique ON legacy_battles(arena_unique_id) WHERE arena_unique_id IS NOT NULL;
";

/// Challenge tables in v10. `challenge_battles` is a frozen ledger: its rows carry their own
/// metric values, so `battle_id` no longer references `battles` (a battle without a replay is not
/// rebuilt, but the challenge history it belonged to stays intact).
pub(crate) const V10_CHALLENGE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS challenge_battles(
  challenge_id TEXT NOT NULL REFERENCES challenges(id) ON DELETE CASCADE,
  battle_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 1 AND 5),
  battle_time INTEGER NOT NULL,counted_at INTEGER NOT NULL,
  metric_value INTEGER NOT NULL CHECK(metric_value>=0),contribution_value INTEGER CHECK(contribution_value>=0),
  survived INTEGER CHECK(survived IN(0,1) OR survived IS NULL),success INTEGER NOT NULL CHECK(success IN(0,1)),
  PRIMARY KEY(challenge_id,battle_id),UNIQUE(challenge_id,ordinal)
) STRICT;
";

/// The newest schema version `steps` lead to (v10 without steps).
pub(crate) fn newest_version(steps: &[(i64, MigrationStep)]) -> i64 {
    steps
        .iter()
        .map(|(from, _)| from + 1)
        .max()
        .unwrap_or(SCHEMA_VERSION)
        .max(SCHEMA_VERSION)
}

/// `<stem>.v{version}.bak` next to the database.
pub(crate) fn backup_path(database: &Path, version: i64) -> PathBuf {
    sibling(database, &format!("v{version}.bak"))
}

/// `<stem>.<suffix>` next to the database (`afterbattle.sqlite3` → `afterbattle.<suffix>`).
pub(crate) fn sibling(database: &Path, suffix: &str) -> PathBuf {
    let stem = database
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "afterbattle".to_owned());
    database.with_file_name(format!("{stem}.{suffix}"))
}

/// Copies the open database to `target` with `VACUUM INTO`, through a temporary name so a
/// partial copy is never mistaken for a backup. An existing backup is kept as it is.
pub(crate) fn backup_into(connection: &Connection, target: &Path) -> Result<(), AppError> {
    if target.exists() {
        return Ok(());
    }
    let mut temporary = target.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    remove_file_if_exists(&temporary)?;
    connection.execute("VACUUM INTO ?1", [temporary.to_string_lossy()])?;
    std::fs::rename(&temporary, target)?;
    Ok(())
}

pub(crate) fn remove_file_if_exists(path: &Path) -> Result<(), AppError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Removes the database file and its `-wal`, `-shm` and `-journal` companions.
pub(crate) fn remove_database_files(path: &Path) -> Result<(), AppError> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        remove_file_if_exists(Path::new(&name))?;
    }
    Ok(())
}

pub(crate) fn user_version(connection: &Connection) -> Result<i64, AppError> {
    Ok(connection.pragma_query_value(None, "user_version", |row| row.get(0))?)
}

/// Creates the complete v10 schema in an empty database.
pub(crate) fn create_v10(transaction: &Transaction<'_>) -> Result<(), AppError> {
    transaction.execute_batch(V10_SCHEMA)?;
    crate::storage::Database::create_challenge_tables_v10(transaction)?;
    crate::storage::Database::create_team_relative_tables(transaction)?;
    transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Runs the incremental steps from the stored version up to the newest one. Before each step the
/// database is copied to `<stem>.v{n}.bak`; each step runs in one transaction and sets
/// `user_version` itself only through this function. A database newer than `steps` know is
/// refused.
pub(crate) fn run_migrations(
    path: &Path,
    connection: &mut Connection,
    base_version: i64,
    steps: &[(i64, MigrationStep)],
) -> Result<i64, AppError> {
    let newest = newest_version(steps).max(base_version);
    let mut version = user_version(connection)?;
    if version > newest {
        return Err(AppError::DatabaseTooNew);
    }
    while version < newest {
        let (_, step) = steps
            .iter()
            .find(|(from, _)| *from == version)
            .ok_or(AppError::UnsupportedDatabaseSchema)?;
        backup_into(connection, &backup_path(path, version))?;
        let transaction = connection.transaction()?;
        step(&transaction)?;
        transaction.pragma_update(None, "user_version", version + 1)?;
        transaction.commit()?;
        version += 1;
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_note_table(transaction: &Transaction<'_>) -> Result<(), AppError> {
        transaction.execute_batch("CREATE TABLE note(value TEXT) STRICT;")?;
        Ok(())
    }

    fn failing_step(transaction: &Transaction<'_>) -> Result<(), AppError> {
        transaction.execute_batch("CREATE TABLE half_done(value TEXT) STRICT;")?;
        Err(AppError::UnsupportedDatabaseSchema)
    }

    fn fresh(path: &Path) -> Connection {
        let mut connection = Connection::open(path).unwrap();
        let transaction = connection.transaction().unwrap();
        create_v10(&transaction).unwrap();
        transaction.commit().unwrap();
        connection
    }

    #[test]
    fn newest_version_follows_the_last_step() {
        assert_eq!(newest_version(&[]), SCHEMA_VERSION);
        assert_eq!(newest_version(&[(10, add_note_table)]), 11);
        assert_eq!(
            newest_version(&[(10, add_note_table), (11, add_note_table)]),
            12
        );
        assert_eq!(newest_version(&[(3, add_note_table)]), SCHEMA_VERSION);
    }

    #[test]
    fn sibling_names_follow_the_database_stem() {
        let path = Path::new("data").join("afterbattle.sqlite3");
        assert_eq!(
            backup_path(&path, 9),
            Path::new("data").join("afterbattle.v9.bak")
        );
        assert_eq!(
            sibling(&path, "corrupt-1.sqlite3"),
            Path::new("data").join("afterbattle.corrupt-1.sqlite3")
        );
    }

    #[test]
    fn incremental_step_backs_up_first_and_runs_in_one_transaction() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let mut connection = fresh(&path);
        connection
            .execute("INSERT INTO settings(key,value) VALUES('k','v')", [])
            .unwrap();
        let steps: &[(i64, MigrationStep)] = &[(10, add_note_table)];
        assert_eq!(
            run_migrations(&path, &mut connection, SCHEMA_VERSION, steps).unwrap(),
            11
        );
        assert_eq!(user_version(&connection).unwrap(), 11);
        let backup = Connection::open(backup_path(&path, 10)).unwrap();
        assert_eq!(user_version(&backup).unwrap(), 10);
        let value: String = backup
            .query_row("SELECT value FROM settings WHERE key='k'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(value, "v");
        // Idempotent: a second run does nothing.
        assert_eq!(
            run_migrations(&path, &mut connection, SCHEMA_VERSION, steps).unwrap(),
            11
        );
    }

    #[test]
    fn failed_step_rolls_back_and_keeps_the_version() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let mut connection = fresh(&path);
        let steps: &[(i64, MigrationStep)] = &[(10, failing_step)];
        assert!(run_migrations(&path, &mut connection, SCHEMA_VERSION, steps).is_err());
        assert_eq!(user_version(&connection).unwrap(), 10);
        let half_done: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='half_done')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!half_done);
        assert!(backup_path(&path, 10).exists());
    }

    #[test]
    fn newer_schema_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let mut connection = fresh(&path);
        connection.pragma_update(None, "user_version", 11).unwrap();
        assert!(matches!(
            run_migrations(&path, &mut connection, SCHEMA_VERSION, MIGRATIONS),
            Err(AppError::DatabaseTooNew)
        ));
    }

    #[test]
    fn missing_step_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let mut connection = fresh(&path);
        let steps: &[(i64, MigrationStep)] = &[(11, add_note_table)];
        assert!(matches!(
            run_migrations(&path, &mut connection, SCHEMA_VERSION, steps),
            Err(AppError::UnsupportedDatabaseSchema)
        ));
    }

    #[test]
    fn existing_backup_is_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let connection = fresh(&path);
        let target = backup_path(&path, 10);
        std::fs::write(&target, b"original backup").unwrap();
        backup_into(&connection, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"original backup");
    }
}
