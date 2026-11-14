use crate::{
    analysis::FeedbackBaseline,
    attempts_conversion::AttemptsConversionBattle,
    challenges::{
        CHALLENGE_ENGINE_VERSION, CHALLENGE_SCHEMA_VERSION, ChallengeBattleEvaluation,
        evaluate_battle_for_challenge, valid_suggestion_contract,
    },
    error::AppError,
    foundations::ImportExtras,
    models::{
        BaselineConfidence, BaselineWindowKind, Battle, BattleHistoryCursor, BattleHistoryFilter,
        BattleOutcome, BattlePaceContext, BattlePaceReason, BattlePaceRecord,
        ChallengeBattlePresentation, ChallengeContributionMetricKind, ChallengeDiagnosticState,
        ChallengeHistoryPage, ChallengePresentation, ChallengeStatus, ChallengeSuggestion,
        ChallengeType, CleanupReport, CompactDescriptorProtocolSource, DataQuality,
        DatabaseDiagnostics, HistoryDataQualityFilter, IntegrityReport, STATS_ELIGIBLE_SQL,
        Session, SessionMetrics, SourceKind, TeamComparison, TeamTotals, VehicleClass,
        VehicleClassResolutionState, VehicleClassSource, stats_eligibility,
    },
    team_relative::{
        TEAM_RELATIVE_FACT_SCHEMA_VERSION, TeamRelativeCommonReason, TeamRelativeDamageReason,
        TeamRelativeFactsV1, TeamRelativeFragsReason, TeamRelativeXpReason,
    },
};
use chrono::Utc;
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Row, TransactionBehavior, params, params_from_iter,
    types::Value,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
    },
};
use uuid::Uuid;

const BATTLE_COLUMNS: &str = "arena_id,battle_time,vehicle_key,wg_tank_id,tank_name,map_name,battle_type,source_kind,data_quality,won,damage,assist,blocked,damage_received,frags,spots,shots,hits,penetrations,survived,xp,duration_seconds,session_id,arena_unique_id,parser_version,game_version,data_completeness,replay_source,parser_error_code,defense_points,moving_avg_damage,damage_rating,feature_schema_version,map_key,gameplay_id,battle_time_source,player_team,winner_team_raw,assist_spotting,assist_tracking,assist_stun,original_xp,outcome,battle_pace_context,battle_pace_reason,battle_pace_resolver_version,finish_reason_raw,battle_type_raw,bonus_type_raw,initial_team1_size,initial_team2_size,destroyed_team1_score,destroyed_team2_score,direct_enemy_hits,direct_team_hits,piercing_enemy_hits,explosion_hits";
const TEAM_RELATIVE_RESULTS_COLUMNS: &str =
    "b.arena_id,b.battle_time,b.data_quality,b.parser_version,b.feature_schema_version,
       CASE WHEN f.battle_id IS NULL THEN 0 ELSE 1 END,f.fact_schema_version,
       b.damage,b.frags,b.original_xp,f.common_reason,f.team_player_count,
       f.team_damage_total,f.damage_greater_count,f.damage_equal_count,f.damage_reason,
       f.team_frags_total,f.frags_reason,f.xp_greater_count,f.xp_equal_count,f.xp_reason";
type ExistingReplayIdentity = (
    String,
    Option<String>,
    String,
    Option<i64>,
    Option<String>,
    Option<i64>,
);
const CHALLENGE_COLUMNS: &str = "id,schema_version,engine_version,suggestion_version,status,challenge_type,vehicle_id,vehicle_name_snapshot,vehicle_class_snapshot,vehicle_class_source_snapshot,vehicle_class_resolution_state_snapshot,vehicle_class_resolver_version,vehicle_class_catalog_schema_version,compact_descriptor_protocol_source,created_at,started_at,eligible_after,finished_at,target_battles,required_successes,frozen_baseline_value,threshold_value,contribution_metric_kind,contribution_floor,baseline_version,baseline_cutoff,baseline_window,baseline_confidence,baseline_eligible_battles,completed_battles,successful_battles";
const CHALLENGES_TABLE: &str = "CREATE TABLE IF NOT EXISTS challenges(
              id TEXT PRIMARY KEY NOT NULL,
              schema_version INTEGER NOT NULL CHECK(schema_version=1),
              engine_version TEXT NOT NULL,
              suggestion_version TEXT NOT NULL,
              suggestion_id TEXT NOT NULL,
              source_session_id TEXT NOT NULL,
              status TEXT NOT NULL CHECK(status IN ('active','completed','failed','abandoned')),
              active_slot INTEGER,
              challenge_type TEXT NOT NULL CHECK(challenge_type IN ('damage_consistency','effective_survival','spotting_assist_consistency','stun_assist_consistency')),
              vehicle_id INTEGER NOT NULL CHECK(vehicle_id>0),
              vehicle_name_snapshot TEXT NOT NULL,
              vehicle_class_snapshot TEXT NOT NULL CHECK(vehicle_class_snapshot IN ('light_tank','medium_tank','heavy_tank','tank_destroyer','spg')),
              vehicle_class_source_snapshot TEXT NOT NULL CHECK(vehicle_class_source_snapshot='wg_client_packed_catalog'),
              vehicle_class_resolution_state_snapshot TEXT NOT NULL CHECK(vehicle_class_resolution_state_snapshot='resolved'),
              vehicle_class_resolver_version TEXT NOT NULL,
              vehicle_class_catalog_schema_version INTEGER NOT NULL CHECK(vehicle_class_catalog_schema_version>=1),
              compact_descriptor_protocol_source TEXT NOT NULL CHECK(compact_descriptor_protocol_source='externally_documented_and_runtime_validated'),
              created_at INTEGER NOT NULL,started_at INTEGER NOT NULL,eligible_after INTEGER NOT NULL,finished_at INTEGER,
              target_battles INTEGER NOT NULL CHECK(target_battles=5),
              required_successes INTEGER NOT NULL CHECK(required_successes BETWEEN 2 AND 4 AND required_successes<=target_battles),
              frozen_baseline_value REAL NOT NULL CHECK(frozen_baseline_value>0),
              threshold_value INTEGER CHECK(threshold_value>=1),
              contribution_metric_kind TEXT NOT NULL CHECK(contribution_metric_kind IN ('damage','combined_contribution','spotting_tracking_assist','stun_assist')),
              contribution_floor INTEGER CHECK(contribution_floor>=1),
              frozen_damage_baseline REAL,frozen_survival_baseline REAL,frozen_combined_contribution_baseline REAL,
              frozen_spotting_assist_baseline REAL,frozen_tracking_assist_baseline REAL,frozen_stun_assist_baseline REAL,
              baseline_version TEXT NOT NULL,baseline_cutoff INTEGER NOT NULL,baseline_window TEXT NOT NULL CHECK(baseline_window IN ('last10','last30','last100')),
              baseline_confidence TEXT NOT NULL CHECK(baseline_confidence IN ('useful','reliable','stable')),
              baseline_eligible_battles INTEGER NOT NULL CHECK(baseline_eligible_battles>=30),
              baseline_metric_eligible_battles INTEGER NOT NULL CHECK(baseline_metric_eligible_battles>=30),
              insight_version TEXT NOT NULL,formula_version TEXT NOT NULL,expected_snapshot_version TEXT NOT NULL,expected_snapshot_sha256 TEXT NOT NULL,
              evidence_metric TEXT NOT NULL CHECK(evidence_metric IN ('damage','survival','spotting_tracking_assist','stun_assist')),
              evidence_delta REAL NOT NULL,evidence_normalized_strength REAL NOT NULL CHECK(evidence_normalized_strength>=0),
              completed_battles INTEGER NOT NULL DEFAULT 0 CHECK(completed_battles BETWEEN 0 AND target_battles),
              successful_battles INTEGER NOT NULL DEFAULT 0 CHECK(successful_battles BETWEEN 0 AND completed_battles),
              CHECK((status='active' AND active_slot=1 AND finished_at IS NULL AND completed_battles<target_battles) OR
                    (status IN ('completed','failed') AND active_slot IS NULL AND finished_at IS NOT NULL AND completed_battles=target_battles) OR
                    (status='abandoned' AND active_slot IS NULL AND finished_at IS NOT NULL)),
              CHECK(status!='completed' OR successful_battles>=required_successes),
              CHECK(status!='failed' OR successful_battles<required_successes)
            ) STRICT;";
const CHALLENGE_INDEXES: &str = "CREATE UNIQUE INDEX IF NOT EXISTS idx_one_active_challenge ON challenges(active_slot) WHERE active_slot IS NOT NULL;
            CREATE INDEX IF NOT EXISTS idx_challenges_history ON challenges(started_at DESC,id DESC);
            CREATE INDEX IF NOT EXISTS idx_challenge_battles_challenge ON challenge_battles(challenge_id,ordinal);
            CREATE INDEX IF NOT EXISTS idx_battles_challenge_candidates ON battles(wg_tank_id,battle_time,arena_id) WHERE data_quality='full' AND feature_schema_version IN (1,2,3);";
/// Remaining health, maximum health, most damage in the team (Battle screen).
pub type OwnBattleExtras = (Option<i64>, Option<i64>, Option<bool>);
const CHALLENGE_CATCH_UP_PAGE_SIZE: usize = 500;

/// rusqlite 0.41 no longer implements `FromSql` for `usize`; read an `i64` and keep the old
/// behavior of failing on values that do not fit.
fn row_usize(row: &Row<'_>, index: usize) -> rusqlite::Result<usize> {
    let value: i64 = row.get(index)?;
    usize::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

fn parse_team_relative_reason<T>(
    value: Option<String>,
    parser: fn(&str) -> Option<T>,
) -> Result<Option<T>, AppError> {
    value
        .map(|value| parser(&value).ok_or(AppError::UnsupportedDatabaseSchema))
        .transpose()
}

/// The local database. Writes go through one long-lived connection behind a mutex (D6);
/// read-only reports open their own `query_only` connections (team-relative contract §36).
#[derive(Debug, Clone)]
pub struct Database {
    path: PathBuf,
    writer: Arc<WriterSlot>,
}

#[derive(Debug, Default)]
struct WriterSlot {
    connection: Mutex<Option<Connection>>,
    /// Id of the thread holding the connection, to turn a nested lock (a deadlock) into an error.
    owner: AtomicU64,
}

static NEXT_THREAD_TOKEN: AtomicU64 = AtomicU64::new(1);
thread_local! {
    static THREAD_TOKEN: u64 = NEXT_THREAD_TOKEN.fetch_add(1, AtomicOrdering::Relaxed);
}

/// The shared write connection, held for the guard's lifetime.
pub(crate) struct WriterGuard<'a> {
    guard: MutexGuard<'a, Option<Connection>>,
    owner: &'a AtomicU64,
}

impl std::ops::Deref for WriterGuard<'_> {
    type Target = Connection;
    fn deref(&self) -> &Connection {
        self.guard
            .as_ref()
            .expect("connection is opened before the guard is returned")
    }
}

impl std::ops::DerefMut for WriterGuard<'_> {
    fn deref_mut(&mut self) -> &mut Connection {
        self.guard
            .as_mut()
            .expect("connection is opened before the guard is returned")
    }
}

impl Drop for WriterGuard<'_> {
    fn drop(&mut self) {
        self.owner.store(0, AtomicOrdering::SeqCst);
    }
}

#[derive(Debug, Clone)]
pub struct HistoryBattleRow {
    pub battle: Battle,
    pub session_started_at: Option<i64>,
    pub session_status: Option<String>,
}

/// Narrow, internal projection for the read-only team-relative-results-v1
/// engine. The battle key exists only for deterministic storage ordering and
/// never enters the report model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRelativeResultsReadRow {
    pub(crate) battle_key: String,
    pub(crate) battle_time: i64,
    pub(crate) data_quality: DataQuality,
    pub(crate) parser_version: Option<i64>,
    pub(crate) feature_schema_version: Option<i64>,
    pub(crate) facts_row_present: bool,
    pub(crate) fact_schema_version: Option<i64>,
    pub(crate) damage: Option<i64>,
    pub(crate) frags: Option<i64>,
    pub(crate) original_xp: Option<i64>,
    pub(crate) common_reason: Option<String>,
    pub(crate) team_player_count: Option<i64>,
    pub(crate) team_damage_total: Option<i64>,
    pub(crate) damage_greater_count: Option<i64>,
    pub(crate) damage_equal_count: Option<i64>,
    pub(crate) damage_reason: Option<String>,
    pub(crate) team_frags_total: Option<i64>,
    pub(crate) frags_reason: Option<String>,
    pub(crate) xp_greater_count: Option<i64>,
    pub(crate) xp_equal_count: Option<i64>,
    pub(crate) xp_reason: Option<String>,
}

/// Narrow, read-only projection for attempts-conversion-v1. It intentionally
/// excludes player/account information, replay content, and unrelated metrics.
fn attempts_conversion_battle(row: &Row<'_>) -> rusqlite::Result<AttemptsConversionBattle> {
    Ok(AttemptsConversionBattle {
        arena_id: row.get(0)?,
        battle_time: row.get(1)?,
        vehicle_key: row.get(2)?,
        wg_tank_id: row.get(3)?,
        data_quality: DataQuality::parse(&row.get::<_, String>(4)?),
        data_completeness: row.get(5)?,
        replay_source: row.get(6)?,
        parser_version: row.get(7)?,
        feature_schema_version: row.get(8)?,
        gameplay_id: row.get(9)?,
        battle_type_raw: row.get(10)?,
        bonus_type_raw: row.get(11)?,
        initial_team1_size: row.get(12)?,
        initial_team2_size: row.get(13)?,
        shots: row.get(14)?,
        direct_hits: row.get(15)?,
        direct_enemy_hits: row.get(16)?,
        direct_team_hits: row.get(17)?,
        piercings: row.get(18)?,
        piercing_enemy_hits: row.get(19)?,
        explosion_hits: row.get(20)?,
        damage_dealt: row.get(21)?,
    })
}

fn team_relative_results_row(row: &Row<'_>) -> rusqlite::Result<TeamRelativeResultsReadRow> {
    Ok(TeamRelativeResultsReadRow {
        battle_key: row.get(0)?,
        battle_time: row.get(1)?,
        data_quality: DataQuality::parse(&row.get::<_, String>(2)?),
        parser_version: row.get(3)?,
        feature_schema_version: row.get(4)?,
        facts_row_present: row.get::<_, i64>(5)? != 0,
        fact_schema_version: row.get(6)?,
        damage: row.get(7)?,
        frags: row.get(8)?,
        original_xp: row.get(9)?,
        common_reason: row.get(10)?,
        team_player_count: row.get(11)?,
        team_damage_total: row.get(12)?,
        damage_greater_count: row.get(13)?,
        damage_equal_count: row.get(14)?,
        damage_reason: row.get(15)?,
        team_frags_total: row.get(16)?,
        frags_reason: row.get(17)?,
        xp_greater_count: row.get(18)?,
        xp_equal_count: row.get(19)?,
        xp_reason: row.get(20)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayFileState {
    pub source_path_key: String,
    pub file_size: i64,
    pub file_mtime_ms: i64,
    pub content_fingerprint: String,
    pub block_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayUpsertOutcome {
    Inserted,
    Updated,
    Duplicate,
}

#[derive(Debug, Clone)]
struct CleanupBattle {
    battle: Battle,
    imported_at: i64,
}

#[derive(Debug, Default)]
struct DuplicatePlan {
    groups: Vec<Vec<usize>>,
    ambiguous_candidate_groups: usize,
}

struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(length: usize) -> Self {
        Self {
            parent: (0..length).collect(),
        }
    }

    fn root(&mut self, index: usize) -> usize {
        if self.parent[index] != index {
            self.parent[index] = self.root(self.parent[index]);
        }
        self.parent[index]
    }

    fn join(&mut self, first: usize, second: usize) {
        let first = self.root(first);
        let second = self.root(second);
        if first != second {
            self.parent[second] = first;
        }
    }
}

fn duplicate_plan(rows: &[CleanupBattle]) -> DuplicatePlan {
    let mut sets = DisjointSet::new(rows.len());
    let mut by_arena = HashMap::<i64, Vec<usize>>::new();
    let mut by_stable = HashMap::<(i64, String, String, String), Vec<usize>>::new();
    let mut by_weak = HashMap::<(i64, String), Vec<usize>>::new();
    for (index, row) in rows.iter().enumerate() {
        if let Some(arena_unique_id) = row.battle.arena_unique_id {
            by_arena.entry(arena_unique_id).or_default().push(index);
        }
        {
            by_weak
                .entry((row.battle.battle_time, row.battle.vehicle_key.clone()))
                .or_default()
                .push(index);
            if let (Some(map), Some(battle_type)) = (&row.battle.map_name, &row.battle.battle_type)
                && !map.is_empty()
                && !battle_type.is_empty()
            {
                by_stable
                    .entry((
                        row.battle.battle_time,
                        row.battle.vehicle_key.clone(),
                        map.clone(),
                        battle_type.clone(),
                    ))
                    .or_default()
                    .push(index);
            }
        }
    }
    for indices in by_arena.values().filter(|indices| indices.len() > 1) {
        for index in indices.iter().skip(1) {
            sets.join(indices[0], *index);
        }
    }
    for indices in by_stable.values().filter(|indices| indices.len() > 1) {
        let distinct_arena_ids = indices
            .iter()
            .filter_map(|index| rows[*index].battle.arena_unique_id)
            .collect::<HashSet<_>>();
        if distinct_arena_ids.len() <= 1 {
            for index in indices.iter().skip(1) {
                sets.join(indices[0], *index);
            }
        }
    }
    let mut components = HashMap::<usize, Vec<usize>>::new();
    for index in 0..rows.len() {
        components.entry(sets.root(index)).or_default().push(index);
    }
    let groups = components
        .into_values()
        .filter(|indices| indices.len() > 1)
        .collect::<Vec<_>>();
    let ambiguous_candidate_groups = by_weak
        .values()
        .filter(|indices| {
            if indices.len() < 2 {
                return false;
            }
            indices
                .iter()
                .map(|index| sets.root(*index))
                .collect::<HashSet<_>>()
                .len()
                > 1
        })
        .count();
    DuplicatePlan {
        groups,
        ambiguous_candidate_groups,
    }
}

fn quality_rank(quality: &DataQuality) -> usize {
    match quality {
        DataQuality::Full => 2,
        DataQuality::Ambiguous => 1,
        DataQuality::Partial => 0,
    }
}

fn metric_richness(battle: &Battle) -> usize {
    [
        battle.outcome.is_known(),
        battle.damage.is_some(),
        battle.assist.is_some(),
        battle.blocked.is_some(),
        battle.damage_received.is_some(),
        battle.frags.is_some(),
        battle.spots.is_some(),
        battle.shots.is_some(),
        battle.hits.is_some(),
        battle.direct_enemy_hits.is_some(),
        battle.direct_team_hits.is_some(),
        battle.penetrations.is_some(),
        battle.piercing_enemy_hits.is_some(),
        battle.explosion_hits.is_some(),
        battle.defense_points.is_some(),
        battle.moving_avg_damage.is_some(),
        battle.damage_rating.is_some(),
        battle.assist_spotting.is_some(),
        battle.assist_tracking.is_some(),
        battle.assist_stun.is_some(),
        battle.original_xp.is_some(),
        battle.survived.is_some(),
        battle.xp.is_some(),
        battle.duration_seconds.is_some(),
        battle.battle_pace.context.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count()
}

fn fill_missing<T: Clone>(target: &mut Option<T>, source: &Option<T>) {
    if target.is_none() {
        *target = source.clone();
    }
}

fn merge_battle_data(target: &mut Battle, source: &Battle) {
    let vehicle_id_conflict = target.parser_error_code.as_deref() == Some("vehicle_id_conflict")
        || source.parser_error_code.as_deref() == Some("vehicle_id_conflict")
        || matches!(
            (target.wg_tank_id, source.wg_tank_id),
            (Some(first), Some(second)) if first != second
        );
    if quality_rank(&source.data_quality) > quality_rank(&target.data_quality) {
        target.data_quality = source.data_quality.clone();
        target.source_kind = source.source_kind.clone();
    }
    fill_missing(&mut target.arena_unique_id, &source.arena_unique_id);
    if vehicle_id_conflict {
        target.wg_tank_id = None;
    } else {
        fill_missing(&mut target.wg_tank_id, &source.wg_tank_id);
    }
    fill_missing(&mut target.tank_name, &source.tank_name);
    fill_missing(&mut target.map_name, &source.map_name);
    fill_missing(&mut target.map_key, &source.map_key);
    fill_missing(&mut target.battle_type, &source.battle_type);
    fill_missing(&mut target.gameplay_id, &source.gameplay_id);
    fill_missing(&mut target.battle_time_source, &source.battle_time_source);
    target.parser_version = match (target.parser_version, source.parser_version) {
        (Some(first), Some(second)) => Some(first.max(second)),
        (first, second) => first.or(second),
    };
    target.feature_schema_version =
        match (target.feature_schema_version, source.feature_schema_version) {
            (Some(first), Some(second)) => Some(first.max(second)),
            (first, second) => first.or(second),
        };
    fill_missing(&mut target.game_version, &source.game_version);
    if source.data_completeness.as_deref() == Some("results") {
        target.data_completeness = Some("results".into());
    } else {
        fill_missing(&mut target.data_completeness, &source.data_completeness);
    }
    if source.replay_source.as_deref() == Some("replay_results") {
        target.replay_source = Some("replay_results".into());
    } else {
        fill_missing(&mut target.replay_source, &source.replay_source);
    }
    if vehicle_id_conflict {
        target.parser_error_code = Some("vehicle_id_conflict".into());
    } else if target.data_quality != DataQuality::Full {
        fill_missing(&mut target.parser_error_code, &source.parser_error_code);
    } else {
        target.parser_error_code = None;
    }
    target.outcome = match (target.outcome, source.outcome) {
        (BattleOutcome::Unknown, source) => source,
        (target, BattleOutcome::Unknown) => target,
        (target, source) if target == source => target,
        _ => BattleOutcome::Unknown,
    };
    fill_missing(&mut target.player_team, &source.player_team);
    fill_missing(&mut target.winner_team_raw, &source.winner_team_raw);
    fill_missing(&mut target.damage, &source.damage);
    fill_missing(&mut target.assist, &source.assist);
    fill_missing(&mut target.assist_spotting, &source.assist_spotting);
    fill_missing(&mut target.assist_tracking, &source.assist_tracking);
    fill_missing(&mut target.assist_stun, &source.assist_stun);
    fill_missing(&mut target.blocked, &source.blocked);
    fill_missing(&mut target.damage_received, &source.damage_received);
    fill_missing(&mut target.frags, &source.frags);
    fill_missing(&mut target.spots, &source.spots);
    fill_missing(&mut target.shots, &source.shots);
    fill_missing(&mut target.hits, &source.hits);
    fill_missing(&mut target.direct_enemy_hits, &source.direct_enemy_hits);
    fill_missing(&mut target.direct_team_hits, &source.direct_team_hits);
    fill_missing(&mut target.penetrations, &source.penetrations);
    fill_missing(&mut target.piercing_enemy_hits, &source.piercing_enemy_hits);
    fill_missing(&mut target.explosion_hits, &source.explosion_hits);
    fill_missing(&mut target.defense_points, &source.defense_points);
    fill_missing(&mut target.moving_avg_damage, &source.moving_avg_damage);
    fill_missing(&mut target.damage_rating, &source.damage_rating);
    fill_missing(&mut target.survived, &source.survived);
    fill_missing(&mut target.xp, &source.xp);
    fill_missing(&mut target.original_xp, &source.original_xp);
    fill_missing(&mut target.duration_seconds, &source.duration_seconds);
    if target.battle_pace.context.is_none()
        || matches!(target.battle_pace.context, Some(BattlePaceContext::Unknown))
            && source
                .battle_pace
                .context
                .is_some_and(|context| context != BattlePaceContext::Unknown)
    {
        target.battle_pace.context = source.battle_pace.context;
        target.battle_pace.reason = source.battle_pace.reason;
        target.battle_pace.resolver_version = source.battle_pace.resolver_version.clone();
    } else {
        fill_missing(&mut target.battle_pace.reason, &source.battle_pace.reason);
        fill_missing(
            &mut target.battle_pace.resolver_version,
            &source.battle_pace.resolver_version,
        );
    }
    fill_missing(
        &mut target.battle_pace.finish_reason_raw,
        &source.battle_pace.finish_reason_raw,
    );
    fill_missing(
        &mut target.battle_pace.battle_type_raw,
        &source.battle_pace.battle_type_raw,
    );
    fill_missing(
        &mut target.battle_pace.bonus_type_raw,
        &source.battle_pace.bonus_type_raw,
    );
    fill_missing(
        &mut target.battle_pace.initial_team1_size,
        &source.battle_pace.initial_team1_size,
    );
    fill_missing(
        &mut target.battle_pace.initial_team2_size,
        &source.battle_pace.initial_team2_size,
    );
    fill_missing(
        &mut target.battle_pace.destroyed_team1_score,
        &source.battle_pace.destroyed_team1_score,
    );
    fill_missing(
        &mut target.battle_pace.destroyed_team2_score,
        &source.battle_pace.destroyed_team2_score,
    );
    fill_missing(&mut target.session_id, &source.session_id);
}

impl Database {
    /// Opens (or creates) the database without a replay source for a legacy rebuild.
    pub fn initialize(path: PathBuf) -> Result<Self, AppError> {
        Self::initialize_with(path, &crate::legacy::LegacySource::default())
    }

    /// Opens the database at `path`:
    /// - no file or an empty file: a fresh v10 schema;
    /// - v10 or newer known version: `integrity_check` once (N5), then incremental steps;
    /// - a newer unknown version: refused with [`AppError::DatabaseTooNew`];
    /// - v9 or older: the one-time rebuild of `legacy.rs` (backup first, P8 gate, new v10 file).
    pub fn initialize_with(
        path: PathBuf,
        legacy: &crate::legacy::LegacySource,
    ) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let existing = fs::metadata(&path).is_ok_and(|metadata| metadata.len() > 0);
        if existing {
            let (version, has_tables) = crate::legacy::inspect(&path)?;
            if version > crate::schema::newest_version(crate::schema::MIGRATIONS) {
                return Err(AppError::DatabaseTooNew);
            }
            if version < crate::schema::SCHEMA_VERSION && has_tables {
                crate::legacy::rebuild(&path, version, legacy)?;
            }
        }
        let db = Self::with_path(path);
        // Schema work uses its own connection; the shared write connection opens on first use.
        let mut c = Connection::open(&db.path)?;
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000; PRAGMA trusted_schema=OFF;")?;
        let version = crate::schema::user_version(&c)?;
        if version == 0 {
            c.execute_batch("PRAGMA secure_delete=ON;")?;
            let transaction = c.transaction()?;
            crate::schema::create_v10(&transaction)?;
            transaction.commit()?;
        } else if !Self::pragma_checks_are_healthy(&c)? {
            return Err(AppError::DatabaseIntegrity);
        }
        crate::schema::run_migrations(
            &db.path,
            &mut c,
            crate::schema::SCHEMA_VERSION,
            crate::schema::MIGRATIONS,
        )?;
        drop(c);
        Ok(db)
    }

    fn with_path(path: PathBuf) -> Self {
        Self {
            path,
            writer: Arc::new(WriterSlot::default()),
        }
    }

    /// The single write connection (D6), opened on first use. Locking it again on the same
    /// thread would deadlock, so that is reported as an error instead.
    pub(crate) fn open(&self) -> Result<WriterGuard<'_>, AppError> {
        let token = THREAD_TOKEN.with(|token| *token);
        if self.writer.owner.load(AtomicOrdering::SeqCst) == token {
            debug_assert!(false, "nested database lock on one thread");
            return Err(AppError::Tracker);
        }
        let mut guard = self
            .writer
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.writer.owner.store(token, AtomicOrdering::SeqCst);
        if guard.is_none() {
            let opened = Connection::open(&self.path)
                .map_err(AppError::from)
                .and_then(|c| {
                    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000; PRAGMA trusted_schema=OFF;")?;
                    Ok(c)
                });
            match opened {
                Ok(c) => *guard = Some(c),
                Err(error) => {
                    self.writer.owner.store(0, AtomicOrdering::SeqCst);
                    return Err(error);
                }
            }
        }
        Ok(WriterGuard {
            guard,
            owner: &self.writer.owner,
        })
    }

    fn open_read_only(&self) -> Result<Connection, AppError> {
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.execute_batch(
            "PRAGMA query_only=ON; PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA busy_timeout=5000;",
        )?;
        Ok(connection)
    }

    fn create_v4(c: &Connection) -> Result<(), AppError> {
        c.execute_batch("
          CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL) STRICT;
          CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY NOT NULL,started_at INTEGER NOT NULL,last_battle_at INTEGER,ended_at INTEGER,status TEXT NOT NULL CHECK(status IN ('active','ended','timed_out'))) STRICT;
          CREATE TABLE IF NOT EXISTS battles(
            arena_id TEXT PRIMARY KEY NOT NULL,battle_time INTEGER NOT NULL,vehicle_key TEXT NOT NULL,wg_tank_id INTEGER,
            tank_name TEXT,map_name TEXT,battle_type TEXT,source_kind TEXT NOT NULL CHECK(source_kind IN ('replay','local_hybrid','tomato')),
            data_quality TEXT NOT NULL CHECK(data_quality IN ('full','partial','ambiguous')),won INTEGER CHECK(won IN(0,1) OR won IS NULL),
            damage INTEGER CHECK(damage>=0),assist INTEGER CHECK(assist>=0),blocked INTEGER CHECK(blocked>=0),damage_received INTEGER CHECK(damage_received>=0),
            frags INTEGER CHECK(frags>=0),spots INTEGER CHECK(spots>=0),shots INTEGER CHECK(shots>=0),hits INTEGER CHECK(hits>=0),penetrations INTEGER CHECK(penetrations>=0),
            survived INTEGER CHECK(survived IN(0,1) OR survived IS NULL),xp INTEGER CHECK(xp>=0),duration_seconds INTEGER CHECK(duration_seconds>=0),
            wn8 REAL,wn8_estimated INTEGER NOT NULL DEFAULT 0 CHECK(wn8_estimated IN(0,1)),wnx REAL,session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
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
            imported_at INTEGER NOT NULL
          ) STRICT;
          CREATE TABLE IF NOT EXISTS replay_files(
            source_path_key TEXT PRIMARY KEY NOT NULL,battle_id TEXT NOT NULL REFERENCES battles(arena_id) ON DELETE CASCADE,
            file_size INTEGER NOT NULL CHECK(file_size>=0),file_mtime_ms INTEGER NOT NULL,content_fingerprint TEXT NOT NULL,
            block_count INTEGER NOT NULL CHECK(block_count>=1),last_processed_at INTEGER NOT NULL
          ) STRICT;
          CREATE INDEX IF NOT EXISTS idx_battles_time ON battles(battle_time DESC);
          CREATE INDEX IF NOT EXISTS idx_battles_vehicle_time ON battles(vehicle_key,battle_time DESC);
          CREATE INDEX IF NOT EXISTS idx_battles_session ON battles(session_id,battle_time);
          CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at DESC);
          CREATE INDEX IF NOT EXISTS idx_replay_files_battle ON replay_files(battle_id);")?;
        c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_one_active_session ON sessions((1)) WHERE status='active';")?;
        c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_battles_arena_unique_id ON battles(arena_unique_id) WHERE arena_unique_id IS NOT NULL;")?;
        Ok(())
    }

    pub(crate) fn create_team_relative_tables(c: &Connection) -> Result<(), AppError> {
        c.execute_batch(
            "CREATE TABLE IF NOT EXISTS team_relative_facts(
              battle_id TEXT PRIMARY KEY NOT NULL
                REFERENCES battles(arena_id) ON DELETE CASCADE,
              fact_schema_version INTEGER NOT NULL CHECK(fact_schema_version=1),
              common_reason TEXT
                CHECK(common_reason IS NULL OR common_reason IN (
                  'partial_replay','results_provenance_missing','parser_too_old',
                  'unsupported_feature_schema','unsupported_mode','missing_initial_roster',
                  'missing_final_roster','roster_size_conflict','roster_key_conflict',
                  'duplicate_rollout','invalid_team_assignment','own_player_missing',
                  'own_player_ambiguous','player_row_conflict','numeric_overflow'
                )),
              team_player_count INTEGER
                CHECK(team_player_count IS NULL OR team_player_count=15),
              team_damage_total INTEGER
                CHECK(team_damage_total IS NULL OR team_damage_total BETWEEN 0 AND 3000000),
              damage_greater_count INTEGER
                CHECK(damage_greater_count IS NULL OR damage_greater_count BETWEEN 0 AND 14),
              damage_equal_count INTEGER
                CHECK(damage_equal_count IS NULL OR damage_equal_count BETWEEN 1 AND 15),
              damage_reason TEXT
                CHECK(damage_reason IS NULL OR damage_reason IN (
                  'damage_missing','damage_out_of_range','damage_conflict'
                )),
              team_frags_total INTEGER
                CHECK(team_frags_total IS NULL OR team_frags_total BETWEEN 0 AND 15),
              frags_reason TEXT
                CHECK(frags_reason IS NULL OR frags_reason IN (
                  'frags_missing','frags_out_of_range','frags_conflict'
                )),
              xp_greater_count INTEGER
                CHECK(xp_greater_count IS NULL OR xp_greater_count BETWEEN 0 AND 14),
              xp_equal_count INTEGER
                CHECK(xp_equal_count IS NULL OR xp_equal_count BETWEEN 1 AND 15),
              xp_reason TEXT
                CHECK(xp_reason IS NULL OR xp_reason IN (
                  'xp_missing','xp_out_of_range','xp_conflict'
                )),
              CHECK(
                (common_reason IS NOT NULL
                  AND team_player_count IS NULL
                  AND team_damage_total IS NULL
                  AND damage_greater_count IS NULL
                  AND damage_equal_count IS NULL
                  AND damage_reason IS NULL
                  AND team_frags_total IS NULL
                  AND frags_reason IS NULL
                  AND xp_greater_count IS NULL
                  AND xp_equal_count IS NULL
                  AND xp_reason IS NULL)
                OR
                (common_reason IS NULL
                  AND team_player_count IS NOT NULL
                  AND team_player_count=15)
              ),
              CHECK(
                common_reason IS NOT NULL OR
                ((damage_reason IS NULL
                    AND team_damage_total IS NOT NULL
                    AND damage_greater_count IS NOT NULL
                    AND damage_equal_count IS NOT NULL
                    AND damage_greater_count + damage_equal_count <= 15)
                  OR
                 (damage_reason IS NOT NULL
                    AND team_damage_total IS NULL
                    AND damage_greater_count IS NULL
                    AND damage_equal_count IS NULL))
              ),
              CHECK(
                common_reason IS NOT NULL OR
                ((frags_reason IS NULL AND team_frags_total IS NOT NULL)
                  OR
                 (frags_reason IS NOT NULL AND team_frags_total IS NULL))
              ),
              CHECK(
                common_reason IS NOT NULL OR
                ((xp_reason IS NULL
                    AND xp_greater_count IS NOT NULL
                    AND xp_equal_count IS NOT NULL
                    AND xp_greater_count + xp_equal_count <= 15)
                  OR
                 (xp_reason IS NOT NULL
                    AND xp_greater_count IS NULL
                    AND xp_equal_count IS NULL))
              )
            ) STRICT;",
        )?;
        Ok(())
    }

    fn create_challenge_schema(c: &Connection) -> Result<(), AppError> {
        c.execute_batch(CHALLENGES_TABLE)?;
        c.execute_batch(
            "CREATE TABLE IF NOT EXISTS challenge_battles(
              challenge_id TEXT NOT NULL REFERENCES challenges(id) ON DELETE CASCADE,
              battle_id TEXT NOT NULL REFERENCES battles(arena_id) ON DELETE RESTRICT,
              ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 1 AND 5),
              battle_time INTEGER NOT NULL,counted_at INTEGER NOT NULL,
              metric_value INTEGER NOT NULL CHECK(metric_value>=0),contribution_value INTEGER CHECK(contribution_value>=0),
              survived INTEGER CHECK(survived IN(0,1) OR survived IS NULL),success INTEGER NOT NULL CHECK(success IN(0,1)),
              PRIMARY KEY(challenge_id,battle_id),UNIQUE(challenge_id,ordinal)
            ) STRICT;",
        )?;
        c.execute_batch(CHALLENGE_INDEXES)?;
        Ok(())
    }

    /// v10 challenge tables: `challenge_battles` is a frozen ledger without a foreign key to
    /// `battles` (see `schema::V10_CHALLENGE_SCHEMA`).
    pub(crate) fn create_challenge_tables_v10(c: &Connection) -> Result<(), AppError> {
        c.execute_batch(CHALLENGES_TABLE)?;
        c.execute_batch(crate::schema::V10_CHALLENGE_SCHEMA)?;
        c.execute_batch(CHALLENGE_INDEXES)?;
        Ok(())
    }

    /// Pre-v10 migration chain, used only on the working copy of a legacy database during the
    /// v10 rebuild (`legacy.rs`).
    pub(crate) fn migrate_legacy_to_v9(c: &mut Connection) -> Result<(), AppError> {
        c.execute_batch("PRAGMA secure_delete=ON; CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL) STRICT; CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY NOT NULL,started_at INTEGER NOT NULL,last_battle_at INTEGER,ended_at INTEGER,status TEXT NOT NULL CHECK(status IN ('active','ended','timed_out'))) STRICT;")?;
        let exists: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='battles')",
            [],
            |r| r.get(0),
        )?;
        if !exists {
            let transaction = c.transaction()?;
            Self::create_v4(&transaction)?;
            Self::create_challenge_schema(&transaction)?;
            Self::create_team_relative_tables(&transaction)?;
            transaction.pragma_update(None, "user_version", 9)?;
            if !Self::integrity_report_on(&transaction)?.is_healthy() {
                return Err(AppError::DatabaseIntegrity);
            }
            transaction.commit()?;
            return Ok(());
        }
        let columns = {
            let mut s = c.prepare("PRAGMA table_info(battles)")?;
            s.query_map([], |r| r.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let has_raw = columns.iter().any(|column| column == "raw_json");
        if has_raw {
            let tx = c.transaction()?;
            tx.execute_batch("ALTER TABLE battles RENAME TO battles_v1;")?;
            Self::create_v4(&tx)?;
            tx.execute("INSERT INTO battles(arena_id,battle_time,vehicle_key,wg_tank_id,tank_name,map_name,battle_type,source_kind,data_quality,won,damage,assist,blocked,damage_received,frags,spots,shots,hits,penetrations,survived,duration_seconds,wn8,wn8_estimated,wnx,session_id,outcome,imported_at)
              SELECT arena_id,battle_time,CAST(tank_id AS TEXT),tank_id,tank_name,map_name,battle_type,'tomato','full',won,damage,assist,blocked,damage_received,frags,spots,shots,hits,penetrations,survived,duration_seconds,wn8,0,wnx,session_id,CASE WHEN won=1 THEN 'victory' ELSE 'unknown' END,imported_at FROM battles_v1", [])?;
            tx.execute_batch("DROP TABLE battles_v1; PRAGMA user_version=4;")?;
            tx.commit()?;
            Self::create_v4(c)?;
            c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        } else {
            for required in [
                "vehicle_key",
                "source_kind",
                "data_quality",
                "wn8_estimated",
            ] {
                if !columns.iter().any(|column| column == required) {
                    return Err(AppError::UnsupportedDatabaseSchema);
                }
            }
            if !columns.iter().any(|column| column == "arena_unique_id") {
                let tx = c.transaction()?;
                tx.execute_batch("
                    ALTER TABLE battles ADD COLUMN arena_unique_id INTEGER;
                    ALTER TABLE battles ADD COLUMN parser_version INTEGER;
                    ALTER TABLE battles ADD COLUMN game_version TEXT;
                    ALTER TABLE battles ADD COLUMN data_completeness TEXT CHECK(data_completeness IN ('metadata_only','results') OR data_completeness IS NULL);
                    ALTER TABLE battles ADD COLUMN replay_source TEXT CHECK(replay_source IN ('replay_metadata','replay_results') OR replay_source IS NULL);
                    ALTER TABLE battles ADD COLUMN parser_error_code TEXT;
                    ALTER TABLE battles ADD COLUMN defense_points INTEGER CHECK(defense_points>=0);
                    ALTER TABLE battles ADD COLUMN moving_avg_damage INTEGER CHECK(moving_avg_damage>=0);
                    ALTER TABLE battles ADD COLUMN damage_rating INTEGER CHECK(damage_rating>=0);
                    PRAGMA user_version=3;
                ")?;
                tx.commit()?;
            }
            let columns = {
                let mut statement = c.prepare("PRAGMA table_info(battles)")?;
                statement
                    .query_map([], |row| row.get::<_, String>(1))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            if !columns
                .iter()
                .any(|column| column == "feature_schema_version")
            {
                let tx = c.transaction()?;
                tx.execute_batch(
                    "ALTER TABLE battles ADD COLUMN feature_schema_version INTEGER CHECK(feature_schema_version>=1);
                     ALTER TABLE battles ADD COLUMN map_key TEXT;
                     ALTER TABLE battles ADD COLUMN gameplay_id TEXT;
                     ALTER TABLE battles ADD COLUMN battle_time_source TEXT CHECK(battle_time_source IN ('metadata','file_mtime') OR battle_time_source IS NULL);
                     ALTER TABLE battles ADD COLUMN player_team INTEGER CHECK(player_team BETWEEN 0 AND 2 OR player_team IS NULL);
                     ALTER TABLE battles ADD COLUMN winner_team_raw INTEGER CHECK(winner_team_raw BETWEEN 0 AND 2 OR winner_team_raw IS NULL);
                     ALTER TABLE battles ADD COLUMN assist_spotting INTEGER CHECK(assist_spotting>=0);
                     ALTER TABLE battles ADD COLUMN assist_tracking INTEGER CHECK(assist_tracking>=0);
                     ALTER TABLE battles ADD COLUMN assist_stun INTEGER CHECK(assist_stun>=0);
                     ALTER TABLE battles ADD COLUMN original_xp INTEGER CHECK(original_xp>=0);
                     PRAGMA user_version=4;",
                )?;
                tx.commit()?;
            }
            Self::create_v4(c)?;
            c.pragma_update(None, "user_version", 4)?;
        }
        let transaction = c.transaction()?;
        Self::create_challenge_schema(&transaction)?;
        transaction.pragma_update(None, "user_version", 5)?;
        transaction.commit()?;
        Self::migrate_outcome_v6(c)?;
        Self::migrate_battle_pace_v7(c)?;
        Self::migrate_attempts_conversion_v8(c)?;
        Self::migrate_team_relative_v9(c)?;
        Ok(())
    }

    fn migrate_outcome_v6(c: &mut Connection) -> Result<(), AppError> {
        let has_outcome = {
            let mut statement = c.prepare("PRAGMA table_info(battles)")?;
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .any(|column| column == "outcome")
        };
        if has_outcome {
            let transaction = c.transaction()?;
            transaction.pragma_update(None, "user_version", 6)?;
            transaction.commit()?;
            return Ok(());
        }

        let transaction = c.transaction()?;
        transaction.execute_batch(
            "ALTER TABLE battles ADD COLUMN outcome TEXT NOT NULL DEFAULT 'unknown'
             CHECK(outcome IN ('victory','defeat','draw','unknown'));",
        )?;
        transaction.execute(
            "UPDATE battles SET outcome=CASE
               WHEN winner_team_raw=0 AND (won IS NULL OR won=0) THEN 'draw'
               WHEN winner_team_raw=0 THEN 'unknown'
               WHEN player_team IN (1,2) AND winner_team_raw IN (1,2)
                    AND player_team=winner_team_raw AND (won IS NULL OR won=1) THEN 'victory'
               WHEN player_team IN (1,2) AND winner_team_raw IN (1,2)
                    AND player_team!=winner_team_raw AND (won IS NULL OR won=0) THEN 'defeat'
               WHEN winner_team_raw IS NOT NULL THEN 'unknown'
               WHEN won=1 THEN 'victory'
               ELSE 'unknown'
             END",
            [],
        )?;
        transaction.execute(
            "UPDATE battles SET won=CASE outcome
               WHEN 'victory' THEN 1
               WHEN 'defeat' THEN 0
               WHEN 'draw' THEN 0
               ELSE NULL
             END",
            [],
        )?;
        transaction.pragma_update(None, "user_version", 6)?;
        transaction.commit()?;
        Ok(())
    }

    fn migrate_battle_pace_v7(c: &mut Connection) -> Result<(), AppError> {
        let has_battle_pace = {
            let mut statement = c.prepare("PRAGMA table_info(battles)")?;
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .any(|column| column == "battle_pace_context")
        };
        let transaction = c.transaction()?;
        if !has_battle_pace {
            transaction.execute_batch(
                "ALTER TABLE battles ADD COLUMN battle_pace_context TEXT
                   CHECK(battle_pace_context IN ('obvious_steamroll','borderline_steamroll','standard','unknown') OR battle_pace_context IS NULL);
                 ALTER TABLE battles ADD COLUMN battle_pace_reason TEXT
                   CHECK(battle_pace_reason IN ('missing_duration','missing_final_roster','missing_team_assignment','missing_finish_reason','unsupported_mode','unsupported_finish_reason','unsupported_schema','invalid_team_size','asymmetric_teams','invalid_destroyed_score','conflicting_data') OR battle_pace_reason IS NULL);
                 ALTER TABLE battles ADD COLUMN battle_pace_resolver_version TEXT;
                 ALTER TABLE battles ADD COLUMN finish_reason_raw INTEGER;
                 ALTER TABLE battles ADD COLUMN battle_type_raw INTEGER;
                 ALTER TABLE battles ADD COLUMN bonus_type_raw INTEGER;
                 ALTER TABLE battles ADD COLUMN initial_team1_size INTEGER CHECK(initial_team1_size>=0);
                 ALTER TABLE battles ADD COLUMN initial_team2_size INTEGER CHECK(initial_team2_size>=0);
                 ALTER TABLE battles ADD COLUMN destroyed_team1_score INTEGER CHECK(destroyed_team1_score>=0);
                 ALTER TABLE battles ADD COLUMN destroyed_team2_score INTEGER CHECK(destroyed_team2_score>=0);",
            )?;
        }
        transaction.pragma_update(None, "user_version", 7)?;
        if !Self::pragma_checks_are_healthy(&transaction)? {
            return Err(AppError::DatabaseIntegrity);
        }
        transaction.commit()?;
        Ok(())
    }

    fn migrate_attempts_conversion_v8(c: &mut Connection) -> Result<(), AppError> {
        let columns = {
            let mut statement = c.prepare("PRAGMA table_info(battles)")?;
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<Result<HashSet<_>, _>>()?
        };
        let required = [
            "direct_enemy_hits",
            "direct_team_hits",
            "piercing_enemy_hits",
            "explosion_hits",
        ];
        let present = required
            .iter()
            .filter(|column| columns.contains(**column))
            .count();
        if present != 0 && present != required.len() {
            return Err(AppError::UnsupportedDatabaseSchema);
        }
        let transaction = c.transaction()?;
        if present == 0 {
            transaction.execute_batch(
                "ALTER TABLE battles ADD COLUMN direct_enemy_hits INTEGER CHECK(direct_enemy_hits>=0);
                 ALTER TABLE battles ADD COLUMN direct_team_hits INTEGER CHECK(direct_team_hits>=0);
                 ALTER TABLE battles ADD COLUMN piercing_enemy_hits INTEGER CHECK(piercing_enemy_hits>=0);
                 ALTER TABLE battles ADD COLUMN explosion_hits INTEGER CHECK(explosion_hits>=0);",
            )?;
        }
        transaction.execute_batch(
            "DROP INDEX IF EXISTS idx_battles_challenge_candidates;
             CREATE INDEX idx_battles_challenge_candidates ON battles(wg_tank_id,battle_time,arena_id)
               WHERE data_quality='full' AND feature_schema_version IN (1,2,3);",
        )?;
        transaction.pragma_update(None, "user_version", 8)?;
        if !Self::pragma_checks_are_healthy(&transaction)? {
            return Err(AppError::DatabaseIntegrity);
        }
        transaction.commit()?;
        Ok(())
    }

    fn migrate_team_relative_v9(c: &mut Connection) -> Result<(), AppError> {
        let transaction = c.transaction()?;
        Self::create_team_relative_tables(&transaction)?;
        transaction.execute_batch(
            "DROP INDEX IF EXISTS idx_battles_challenge_candidates;
             CREATE INDEX idx_battles_challenge_candidates
               ON battles(wg_tank_id,battle_time,arena_id)
               WHERE data_quality='full' AND feature_schema_version IN (1,2,3);",
        )?;
        transaction.pragma_update(None, "user_version", 9)?;
        if !Self::pragma_checks_are_healthy(&transaction)? {
            return Err(AppError::DatabaseIntegrity);
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn integrity_is_healthy(connection: &Connection) -> Result<bool, AppError> {
        Self::pragma_checks_are_healthy(connection)
    }

    fn pragma_checks_are_healthy(connection: &Connection) -> Result<bool, AppError> {
        let integrity_ok = {
            let mut statement = connection.prepare("PRAGMA integrity_check")?;
            let values = statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            values.len() == 1 && values[0] == "ok"
        };
        let foreign_keys_ok = {
            let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
            statement.query_map([], |_| Ok(()))?.count() == 0
        };
        Ok(integrity_ok && foreign_keys_ok)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        self.open()?.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;
        Ok(())
    }
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .open()?
            .query_row(
                "SELECT value FROM settings WHERE key=?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Runs `work` in one write transaction; it commits only when `work` succeeds.
    pub(crate) fn with_transaction<T>(
        &self,
        work: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction()?;
        let value = work(&transaction)?;
        transaction.commit()?;
        Ok(value)
    }

    pub(crate) fn set_setting_tx(
        tx: &rusqlite::Transaction<'_>,
        key: &str,
        value: &str,
    ) -> Result<(), AppError> {
        tx.execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// `(size, mtime)` of every replay file already stored or skipped, by path key (D14).
    pub(crate) fn known_replay_files(&self) -> Result<HashMap<String, (i64, i64)>, AppError> {
        let connection = self.open()?;
        let mut statement = connection.prepare(
            "SELECT source_path_key,file_size,file_mtime_ms FROM replay_files
             UNION ALL SELECT source_path_key,file_size,file_mtime_ms FROM replay_file_skips",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, (row.get(1)?, row.get(2)?)))
        })?;
        Ok(rows.collect::<Result<HashMap<_, _>, _>>()?)
    }

    /// After a complete import pass: writes the final numbers of a pending v10 rebuild report
    /// (recalculated, not random, without a replay file, conflicts).
    pub(crate) fn finish_legacy_rebuild(&self) -> Result<(), AppError> {
        let Some(mut report) = self.rebuild_report()? else {
            return Ok(());
        };
        if report.state != crate::legacy::RebuildState::Pending {
            return Ok(());
        }
        let connection = self.open()?;
        let count = |state: &str| -> Result<i64, AppError> {
            Ok(connection.query_row(
                "SELECT COUNT(*) FROM legacy_battles WHERE state=?1",
                params![state],
                |row| row.get(0),
            )?)
        };
        report.recalculated = count("matched")?;
        report.not_random = count("not_random")?;
        report.without_replay = count("pending")?;
        report.conflicts = count("conflict")?;
        // Author's decision of 2026-11-13: a legacy session whose battles did not come back
        // does not stay in v10 (it is still in the backup). The active session, sessions that
        // are the source of a challenge and sessions started after the rebuild are kept.
        let removed = connection.execute(
            "DELETE FROM sessions WHERE status<>'active'
             AND id IN (SELECT session_id FROM legacy_battles WHERE session_id IS NOT NULL)
             AND NOT EXISTS(SELECT 1 FROM battles b WHERE b.session_id=sessions.id)
             AND id NOT IN (SELECT source_session_id FROM challenges)",
            [],
        )?;
        report.skipped_empty_sessions += i64::try_from(removed).unwrap_or(i64::MAX);
        report.state = crate::legacy::RebuildState::Done;
        report.finished_at = Some(Utc::now().timestamp_millis());
        drop(connection);
        self.set_setting(
            crate::legacy::REBUILD_REPORT_KEY,
            &serde_json::to_string(&report).map_err(|_| AppError::InvalidApiResponse)?,
        )
    }

    /// The v10 rebuild report, when this database was rebuilt from a legacy one.
    pub fn rebuild_report(&self) -> Result<Option<crate::legacy::RebuildReport>, AppError> {
        self.get_setting(crate::legacy::REBUILD_REPORT_KEY)?
            .map(|value| serde_json::from_str(&value).map_err(|_| AppError::InvalidApiResponse))
            .transpose()
    }

    pub fn diagnostics_read_only(&self) -> Result<DatabaseDiagnostics, AppError> {
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.execute_batch(
            "PRAGMA query_only=ON; PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA busy_timeout=5000;",
        )?;
        let schema_version =
            connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let integrity_check = {
            let mut statement = connection.prepare("PRAGMA integrity_check")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .map(|result| match result {
                    Ok(value) if value == "ok" => Ok("ok".to_owned()),
                    Ok(_) => Ok("failed".to_owned()),
                    Err(error) => Err(error),
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let foreign_key_violation_count = {
            let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
            statement.query_map([], |_| Ok(()))?.count()
        };
        let count = |table: &str| -> Result<usize, AppError> {
            let allowed = ["settings", "sessions", "battles", "replay_files"];
            if !allowed.contains(&table) {
                return Err(AppError::InvalidApiResponse);
            }
            Ok(
                connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })? as usize,
            )
        };
        let quality_count = |quality: &str| -> Result<usize, AppError> {
            Ok(connection.query_row(
                "SELECT COUNT(*) FROM battles WHERE data_quality=?1",
                params![quality],
                |row| row.get::<_, i64>(0),
            )? as usize)
        };
        let mut parser_error_counts = BTreeMap::new();
        {
            let mut statement = connection.prepare(
                "SELECT parser_error_code,COUNT(*) FROM battles WHERE parser_error_code IS NOT NULL GROUP BY parser_error_code",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })?;
            for row in rows {
                let (code, amount) = row?;
                let safe = match code.as_str() {
                    "vehicle_id_conflict" | "invalid_results_json" => code,
                    _ => "other".to_owned(),
                };
                *parser_error_counts.entry(safe).or_insert(0) += amount;
            }
        }
        Ok(DatabaseDiagnostics {
            schema_version,
            integrity_check,
            foreign_key_violation_count,
            settings: count("settings")?,
            sessions: count("sessions")?,
            battles: count("battles")?,
            full_battles: quality_count("full")?,
            partial_battles: quality_count("partial")?,
            ambiguous_battles: quality_count("ambiguous")?,
            replay_files: count("replay_files")?,
            active_session_exists: connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE status='active')",
                [],
                |row| row.get(0),
            )?,
            parser_error_counts,
            rebuild_report: connection
                .query_row(
                    "SELECT value FROM settings WHERE key=?1",
                    params![crate::legacy::REBUILD_REPORT_KEY],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .and_then(|value| serde_json::from_str(&value).ok()),
        })
    }
    pub fn remove_setting(&self, key: &str) -> Result<(), AppError> {
        self.open()?
            .execute("DELETE FROM settings WHERE key=?1", params![key])?;
        Ok(())
    }

    pub fn update_preferences(&self, language: &str, inactivity: u32) -> Result<(), AppError> {
        self.set_setting("language", if language == "en" { "en" } else { "pl" })?;
        self.set_setting("inactivity_minutes", &inactivity.clamp(15, 240).to_string())
    }
    pub fn inactivity_minutes(&self) -> Result<u32, AppError> {
        Ok(self
            .get_setting("inactivity_minutes")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(60)
            .clamp(15, 240))
    }

    pub fn insert_battle(&self, b: &mut Battle, attach_session: bool) -> Result<bool, AppError> {
        let inactivity_threshold = i64::from(self.inactivity_minutes()?) * 60_000;
        let mut c = self.open()?;
        let tx = c.transaction()?;
        let duplicate: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM battles WHERE arena_id=?1 OR (battle_time=?2 AND vehicle_key=?3))",
            params![b.arena_id, b.battle_time, b.vehicle_key],
            |row| row.get(0),
        )?;
        if duplicate {
            return Ok(false);
        }
        if attach_session {
            Self::attach_session(&tx, b, inactivity_threshold)?;
        }
        let changed = Self::insert_battle_tx(&tx, b)?;
        if changed == 1
            && let Some(id) = &b.session_id
        {
            tx.execute("UPDATE sessions SET last_battle_at=CASE WHEN last_battle_at IS NULL OR last_battle_at<?1 THEN ?1 ELSE last_battle_at END WHERE id=?2 AND status='active'",params![b.battle_time,id])?;
        }
        tx.commit()?;
        Ok(changed == 1)
    }

    fn attach_session(
        tx: &rusqlite::Transaction<'_>,
        b: &mut Battle,
        inactivity_threshold: i64,
    ) -> Result<(), AppError> {
        let mut active=tx.query_row("SELECT id,started_at,last_battle_at FROM sessions WHERE status='active' ORDER BY started_at DESC LIMIT 1",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,Option<i64>>(2)?))).optional()?;
        if let Some((id, started_at, last_battle_at)) = &active {
            let last = last_battle_at.unwrap_or(*started_at);
            if b.battle_time.saturating_sub(last) >= inactivity_threshold {
                tx.execute(
                    "UPDATE sessions SET ended_at=?1,status='timed_out' WHERE id=?2 AND status='active'",
                    params![last.saturating_add(inactivity_threshold), id],
                )?;
                active = None;
            }
        }
        let session = match active {
            Some((id, started_at, _)) => (id, started_at),
            None => {
                let id = Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO sessions(id,started_at,status) VALUES(?1,?2,'active')",
                    params![id, b.battle_time],
                )?;
                (id, b.battle_time)
            }
        };
        if b.battle_time >= session.1.saturating_sub(300_000) {
            b.session_id = Some(session.0);
        }
        Ok(())
    }

    fn insert_battle_tx(tx: &rusqlite::Transaction<'_>, b: &Battle) -> Result<usize, AppError> {
        Ok(tx.execute(
            "INSERT OR IGNORE INTO battles(arena_id,battle_time,vehicle_key,wg_tank_id,tank_name,map_name,battle_type,source_kind,data_quality,won,damage,assist,blocked,damage_received,frags,spots,shots,hits,penetrations,survived,xp,duration_seconds,session_id,arena_unique_id,parser_version,game_version,data_completeness,replay_source,parser_error_code,defense_points,moving_avg_damage,damage_rating,feature_schema_version,map_key,gameplay_id,battle_time_source,player_team,winner_team_raw,assist_spotting,assist_tracking,assist_stun,original_xp,outcome,battle_pace_context,battle_pace_reason,battle_pace_resolver_version,finish_reason_raw,battle_type_raw,bonus_type_raw,initial_team1_size,initial_team2_size,destroyed_team1_score,destroyed_team2_score,direct_enemy_hits,direct_team_hits,piercing_enemy_hits,explosion_hits,imported_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,?38,?39,?40,?41,?42,?43,?44,?45,?46,?47,?48,?49,?50,?51,?52,?53,?54,?55,?56,?57,?58)",
            params![
                b.arena_id,b.battle_time,b.vehicle_key,b.wg_tank_id,b.tank_name,b.map_name,b.battle_type,
                b.source_kind.as_str(),b.data_quality.as_str(),b.outcome.legacy_won().map(i64::from),b.damage,b.assist,b.blocked,
                b.damage_received,b.frags,b.spots,b.shots,b.hits,b.penetrations,b.survived.map(i64::from),b.xp,
                b.duration_seconds,b.session_id,b.arena_unique_id,
                b.parser_version,b.game_version,b.data_completeness,b.replay_source,b.parser_error_code,
                b.defense_points,b.moving_avg_damage,b.damage_rating,b.feature_schema_version,b.map_key,
                b.gameplay_id,b.battle_time_source,b.player_team,b.winner_team_raw,b.assist_spotting,
                b.assist_tracking,b.assist_stun,b.original_xp,b.outcome.as_str(),
                b.battle_pace.context.map(BattlePaceContext::as_str),
                b.battle_pace.reason.map(BattlePaceReason::as_str),
                b.battle_pace.resolver_version,b.battle_pace.finish_reason_raw,
                b.battle_pace.battle_type_raw,b.battle_pace.bonus_type_raw,
                b.battle_pace.initial_team1_size,b.battle_pace.initial_team2_size,
                b.battle_pace.destroyed_team1_score,b.battle_pace.destroyed_team2_score,
                b.direct_enemy_hits,b.direct_team_hits,b.piercing_enemy_hits,b.explosion_hits,
                Utc::now().timestamp_millis()
            ],
        )?)
    }

    fn insert_team_relative_facts_tx(
        tx: &rusqlite::Transaction<'_>,
        battle_id: &str,
        facts: &TeamRelativeFactsV1,
    ) -> Result<usize, AppError> {
        Ok(tx.execute(
            "INSERT INTO team_relative_facts(
               battle_id,fact_schema_version,common_reason,team_player_count,
               team_damage_total,damage_greater_count,damage_equal_count,damage_reason,
               team_frags_total,frags_reason,xp_greater_count,xp_equal_count,xp_reason
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                battle_id,
                TEAM_RELATIVE_FACT_SCHEMA_VERSION,
                facts.common_reason.map(TeamRelativeCommonReason::as_str),
                facts.team_player_count,
                facts.team_damage_total,
                facts.damage_greater_count,
                facts.damage_equal_count,
                facts.damage_reason.map(TeamRelativeDamageReason::as_str),
                facts.team_frags_total,
                facts.frags_reason.map(TeamRelativeFragsReason::as_str),
                facts.xp_greater_count,
                facts.xp_equal_count,
                facts.xp_reason.map(TeamRelativeXpReason::as_str),
            ],
        )?)
    }

    fn read_team_relative_facts_on(
        connection: &Connection,
        battle_id: &str,
    ) -> Result<Option<TeamRelativeFactsV1>, AppError> {
        let stored = connection
            .query_row(
                "SELECT fact_schema_version,common_reason,team_player_count,
                        team_damage_total,damage_greater_count,damage_equal_count,damage_reason,
                        team_frags_total,frags_reason,xp_greater_count,xp_equal_count,xp_reason
                 FROM team_relative_facts WHERE battle_id=?1",
                params![battle_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                        row.get::<_, Option<i64>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            schema,
            common_reason,
            team_player_count,
            team_damage_total,
            damage_greater_count,
            damage_equal_count,
            damage_reason,
            team_frags_total,
            frags_reason,
            xp_greater_count,
            xp_equal_count,
            xp_reason,
        )) = stored
        else {
            return Ok(None);
        };
        if schema != TEAM_RELATIVE_FACT_SCHEMA_VERSION {
            return Err(AppError::UnsupportedDatabaseSchema);
        }
        Ok(Some(TeamRelativeFactsV1 {
            common_reason: parse_team_relative_reason(
                common_reason,
                TeamRelativeCommonReason::parse,
            )?,
            team_player_count,
            team_damage_total,
            damage_greater_count,
            damage_equal_count,
            damage_reason: parse_team_relative_reason(
                damage_reason,
                TeamRelativeDamageReason::parse,
            )?,
            team_frags_total,
            frags_reason: parse_team_relative_reason(frags_reason, TeamRelativeFragsReason::parse)?,
            xp_greater_count,
            xp_equal_count,
            xp_reason: parse_team_relative_reason(xp_reason, TeamRelativeXpReason::parse)?,
        }))
    }

    #[cfg(test)]
    pub(crate) fn team_relative_facts(
        &self,
        battle_id: &str,
    ) -> Result<Option<TeamRelativeFactsV1>, AppError> {
        Self::read_team_relative_facts_on(&self.open_read_only()?, battle_id)
    }

    pub fn upsert_replay(
        &self,
        b: &mut Battle,
        file: &ReplayFileState,
        attach_session: bool,
    ) -> Result<ReplayUpsertOutcome, AppError> {
        self.upsert_replay_with_team_relative_facts(b, file, attach_session, None)
    }

    pub(crate) fn upsert_replay_with_team_relative_facts(
        &self,
        b: &mut Battle,
        file: &ReplayFileState,
        attach_session: bool,
        facts: Option<&TeamRelativeFactsV1>,
    ) -> Result<ReplayUpsertOutcome, AppError> {
        self.upsert_imported(b, file, attach_session, facts, &ImportExtras::default())
    }

    /// Stores one parsed replay with its v10 extras in one transaction.
    pub(crate) fn upsert_imported(
        &self,
        b: &mut Battle,
        file: &ReplayFileState,
        attach_session: bool,
        facts: Option<&TeamRelativeFactsV1>,
        extras: &ImportExtras,
    ) -> Result<ReplayUpsertOutcome, AppError> {
        let inactivity_threshold = i64::from(self.inactivity_minutes()?) * 60_000;
        let mut connection = self.open()?;
        let tx = connection.transaction()?;
        let outcome = Self::upsert_replay_tx(
            &tx,
            b,
            file,
            attach_session,
            facts,
            extras,
            inactivity_threshold,
        )?;
        tx.commit()?;
        Ok(outcome)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn upsert_replay_tx(
        tx: &rusqlite::Transaction<'_>,
        b: &mut Battle,
        file: &ReplayFileState,
        attach_session: bool,
        facts: Option<&TeamRelativeFactsV1>,
        extras: &ImportExtras,
        inactivity_threshold: i64,
    ) -> Result<ReplayUpsertOutcome, AppError> {
        let unique_match: Option<ExistingReplayIdentity> = if let Some(arena_unique_id) =
            b.arena_unique_id
        {
            tx
                .query_row(
                    "SELECT arena_id,session_id,data_quality,arena_unique_id,battle_pace_context,parser_version FROM battles WHERE arena_unique_id=?1",
                    params![arena_unique_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )
                .optional()?
        } else {
            None
        };
        let source_match: Option<ExistingReplayIdentity> = tx
            .query_row(
                "SELECT b.arena_id,b.session_id,b.data_quality,b.arena_unique_id,b.battle_pace_context,b.parser_version FROM replay_files f JOIN battles b ON b.arena_id=f.battle_id WHERE f.source_path_key=?1",
                params![file.source_path_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .optional()?;
        if unique_match
            .as_ref()
            .zip(source_match.as_ref())
            .is_some_and(|(by_unique, by_source)| by_unique.0 != by_source.0)
        {
            return Ok(ReplayUpsertOutcome::Duplicate);
        }
        let mut existing = unique_match.or(source_match);
        if existing.is_none() {
            existing = tx
                .query_row(
                    "SELECT arena_id,session_id,data_quality,arena_unique_id,battle_pace_context,parser_version FROM battles WHERE battle_time=?1 AND vehicle_key=?2 AND COALESCE(map_name,'')=COALESCE(?3,'') ORDER BY imported_at LIMIT 1",
                    params![b.battle_time, b.vehicle_key, b.map_name],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )
                .optional()?;
        }

        let outcome = if let Some((
            id,
            session_id,
            quality,
            stored_unique_id,
            stored_pace,
            _stored_parser_version,
        )) = existing
        {
            if stored_unique_id
                .zip(b.arena_unique_id)
                .is_some_and(|(stored, incoming)| stored != incoming)
            {
                return Ok(ReplayUpsertOutcome::Duplicate);
            }
            let is_upgrade = quality != "full" && b.data_quality == DataQuality::Full;
            let adds_identity = stored_unique_id.is_none() && b.arena_unique_id.is_some();
            let incoming_pace = b.battle_pace.context.map(BattlePaceContext::as_str);
            let changes_pace = incoming_pace.is_some() && stored_pace.as_deref() != incoming_pace;
            b.arena_id = id;
            b.session_id = session_id;
            let existing_facts = Self::read_team_relative_facts_on(tx, &b.arena_id)?;
            let replaces_partial_facts = quality != "full"
                && b.data_quality == DataQuality::Full
                && existing_facts.as_ref().is_some_and(|stored| {
                    stored.common_reason == Some(TeamRelativeCommonReason::PartialReplay)
                });
            if let (Some(stored), Some(incoming)) = (existing_facts.as_ref(), facts)
                && stored != incoming
                && !replaces_partial_facts
            {
                return Ok(ReplayUpsertOutcome::Duplicate);
            }
            if b.data_quality == DataQuality::Full {
                if is_upgrade || adds_identity || changes_pace {
                    tx.execute(
                    "UPDATE battles SET arena_unique_id=COALESCE(?2,arena_unique_id),wg_tank_id=?3,source_kind=?4,data_quality='full',parser_version=?5,feature_schema_version=COALESCE(?6,feature_schema_version),game_version=COALESCE(?7,game_version),map_key=COALESCE(?8,map_key),gameplay_id=COALESCE(?9,gameplay_id),battle_time_source=COALESCE(?10,battle_time_source),data_completeness='results',replay_source='replay_results',parser_error_code=?11,won=?12,player_team=COALESCE(?13,player_team),winner_team_raw=COALESCE(?14,winner_team_raw),damage=COALESCE(?15,damage),assist=COALESCE(?16,assist),assist_spotting=COALESCE(?17,assist_spotting),assist_tracking=COALESCE(?18,assist_tracking),assist_stun=COALESCE(?19,assist_stun),blocked=COALESCE(?20,blocked),damage_received=COALESCE(?21,damage_received),frags=COALESCE(?22,frags),spots=COALESCE(?23,spots),shots=COALESCE(?24,shots),hits=COALESCE(?25,hits),penetrations=COALESCE(?26,penetrations),survived=COALESCE(?27,survived),xp=COALESCE(?28,xp),original_xp=COALESCE(?29,original_xp),duration_seconds=COALESCE(?30,duration_seconds),defense_points=COALESCE(?31,defense_points),moving_avg_damage=COALESCE(?32,moving_avg_damage),damage_rating=COALESCE(?33,damage_rating),outcome=?34,battle_pace_context=?35,battle_pace_reason=?36,battle_pace_resolver_version=?37,finish_reason_raw=?38,battle_type_raw=?39,bonus_type_raw=?40,initial_team1_size=?41,initial_team2_size=?42,destroyed_team1_score=?43,destroyed_team2_score=?44,direct_enemy_hits=COALESCE(?45,direct_enemy_hits),direct_team_hits=COALESCE(?46,direct_team_hits),piercing_enemy_hits=COALESCE(?47,piercing_enemy_hits),explosion_hits=COALESCE(?48,explosion_hits) WHERE arena_id=?1",
                    params![
                        b.arena_id,b.arena_unique_id,b.wg_tank_id,b.source_kind.as_str(),b.parser_version,
                        b.feature_schema_version,b.game_version,b.map_key,b.gameplay_id,b.battle_time_source,
                        b.parser_error_code,b.outcome.legacy_won().map(i64::from),b.player_team,b.winner_team_raw,b.damage,b.assist,
                        b.assist_spotting,b.assist_tracking,b.assist_stun,b.blocked,b.damage_received,b.frags,
                        b.spots,b.shots,b.hits,b.penetrations,b.survived.map(i64::from),b.xp,b.original_xp,
                        b.duration_seconds,b.defense_points,b.moving_avg_damage,b.damage_rating,b.outcome.as_str(),
                        b.battle_pace.context.map(BattlePaceContext::as_str),
                        b.battle_pace.reason.map(BattlePaceReason::as_str),
                        b.battle_pace.resolver_version,b.battle_pace.finish_reason_raw,
                        b.battle_pace.battle_type_raw,b.battle_pace.bonus_type_raw,
                        b.battle_pace.initial_team1_size,b.battle_pace.initial_team2_size,
                        b.battle_pace.destroyed_team1_score,b.battle_pace.destroyed_team2_score,
                        b.direct_enemy_hits,b.direct_team_hits,b.piercing_enemy_hits,b.explosion_hits
                    ],
                    )?;
                    if is_upgrade {
                        Self::write_foundations_tx(tx, &b.arena_id, extras)?;
                    }
                }
            } else if quality == "full" {
                tx.execute(
                    "UPDATE battles SET parser_error_code=?2 WHERE arena_id=?1",
                    params![b.arena_id, b.parser_error_code],
                )?;
            } else {
                tx.execute("UPDATE battles SET arena_unique_id=COALESCE(?2,arena_unique_id),wg_tank_id=COALESCE(?3,wg_tank_id),parser_version=COALESCE(?4,parser_version),game_version=COALESCE(?5,game_version),map_key=COALESCE(?6,map_key),gameplay_id=COALESCE(?7,gameplay_id),battle_time_source=COALESCE(?8,battle_time_source),parser_error_code=?9,battle_pace_context=?10,battle_pace_reason=?11,battle_pace_resolver_version=?12,finish_reason_raw=?13,battle_type_raw=?14,bonus_type_raw=?15,initial_team1_size=?16,initial_team2_size=?17,destroyed_team1_score=?18,destroyed_team2_score=?19 WHERE arena_id=?1",params![b.arena_id,b.arena_unique_id,b.wg_tank_id,b.parser_version,b.game_version,b.map_key,b.gameplay_id,b.battle_time_source,b.parser_error_code,b.battle_pace.context.map(BattlePaceContext::as_str),b.battle_pace.reason.map(BattlePaceReason::as_str),b.battle_pace.resolver_version,b.battle_pace.finish_reason_raw,b.battle_pace.battle_type_raw,b.battle_pace.bonus_type_raw,b.battle_pace.initial_team1_size,b.battle_pace.initial_team2_size,b.battle_pace.destroyed_team1_score,b.battle_pace.destroyed_team2_score])?;
            }
            if replaces_partial_facts {
                tx.execute(
                    "DELETE FROM team_relative_facts WHERE battle_id=?1",
                    params![b.arena_id],
                )?;
            }
            if let Some(facts) = facts
                && (existing_facts.is_none() || replaces_partial_facts)
                && Self::insert_team_relative_facts_tx(tx, &b.arena_id, facts)? != 1
            {
                return Err(AppError::Database(rusqlite::Error::ExecuteReturnedResults));
            }
            if is_upgrade || adds_identity || changes_pace {
                ReplayUpsertOutcome::Updated
            } else {
                ReplayUpsertOutcome::Duplicate
            }
        } else {
            let legacy_session = Self::claim_legacy_battle_tx(tx, b)?;
            if let Some(session_id) = legacy_session {
                b.session_id = session_id;
            } else if attach_session {
                Self::attach_session(tx, b, inactivity_threshold)?;
            }
            if Self::insert_battle_tx(tx, b)? != 1 {
                return Err(AppError::Database(rusqlite::Error::ExecuteReturnedResults));
            }
            Self::write_foundations_tx(tx, &b.arena_id, extras)?;
            if let Some(facts) = facts
                && Self::insert_team_relative_facts_tx(tx, &b.arena_id, facts)? != 1
            {
                return Err(AppError::Database(rusqlite::Error::ExecuteReturnedResults));
            }
            if let Some(id) = &b.session_id {
                tx.execute("UPDATE sessions SET last_battle_at=CASE WHEN last_battle_at IS NULL OR last_battle_at<?1 THEN ?1 ELSE last_battle_at END WHERE id=?2 AND status='active'",params![b.battle_time,id])?;
            }
            ReplayUpsertOutcome::Inserted
        };
        tx.execute("INSERT INTO replay_files(source_path_key,battle_id,file_size,file_mtime_ms,content_fingerprint,block_count,last_processed_at,packet_stream_bytes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(source_path_key) DO UPDATE SET battle_id=excluded.battle_id,file_size=excluded.file_size,file_mtime_ms=excluded.file_mtime_ms,content_fingerprint=excluded.content_fingerprint,block_count=excluded.block_count,last_processed_at=excluded.last_processed_at,packet_stream_bytes=excluded.packet_stream_bytes",params![file.source_path_key,b.arena_id,file.file_size,file.file_mtime_ms,file.content_fingerprint,file.block_count,Utc::now().timestamp_millis(),extras.packet_stream_bytes])?;
        tx.execute(
            "DELETE FROM replay_file_skips WHERE source_path_key=?1",
            params![file.source_path_key],
        )?;
        Ok(outcome)
    }

    /// A battle rebuilt from a replay takes the id and manually ended session of its legacy row
    /// (v9 → v10, D5), matched by `arena_unique_id` or, without one, by the legacy id. A legacy
    /// row with the same id but another `arena_unique_id` is a conflict: it is not reused.
    /// Returns the session to keep (`Some(None)` for a legacy battle without one).
    fn claim_legacy_battle_tx(
        tx: &rusqlite::Transaction<'_>,
        b: &mut Battle,
    ) -> Result<Option<Option<String>>, AppError> {
        type LegacyRow = (String, Option<i64>, Option<String>);
        let read = |row: &Row<'_>| -> rusqlite::Result<LegacyRow> {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        };
        let by_unique = match b.arena_unique_id {
            Some(unique) => tx
                .query_row(
                    "SELECT arena_id,arena_unique_id,session_id FROM legacy_battles WHERE arena_unique_id=?1 AND state='pending'",
                    params![unique],
                    read,
                )
                .optional()?,
            None => None,
        };
        let candidate = match by_unique {
            Some(found) => Some(found),
            None => tx
                .query_row(
                    "SELECT arena_id,arena_unique_id,session_id FROM legacy_battles WHERE arena_id=?1 AND state='pending'",
                    params![b.arena_id],
                    read,
                )
                .optional()?,
        };
        let Some((legacy_id, legacy_unique, session_id)) = candidate else {
            return Ok(None);
        };
        let taken: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM battles WHERE arena_id=?1)",
            params![legacy_id],
            |row| row.get(0),
        )?;
        let conflicting = legacy_unique
            .zip(b.arena_unique_id)
            .is_some_and(|(legacy, incoming)| legacy != incoming);
        if conflicting || taken {
            tx.execute(
                "UPDATE legacy_battles SET state='conflict' WHERE arena_id=?1",
                params![legacy_id],
            )?;
            return Ok(None);
        }
        tx.execute(
            "UPDATE legacy_battles SET state='matched' WHERE arena_id=?1",
            params![legacy_id],
        )?;
        b.arena_id = legacy_id;
        Ok(Some(session_id))
    }

    /// Own-player columns, tier and the anonymous roster with team aggregates (parser 9, D17).
    fn write_foundations_tx(
        tx: &rusqlite::Transaction<'_>,
        battle_id: &str,
        extras: &ImportExtras,
    ) -> Result<(), AppError> {
        if let Some(own) = &extras.own {
            tx.execute(
                "UPDATE battles SET vehicle_tier=?2,vehicle_catalog_fingerprint=?3,life_time_seconds=?4,death_reason_raw=?5,max_health=?6,health_remaining=?7,direct_hits_received=?8,piercings_received=?9,no_damage_hits_received=?10,explosion_hits_received=?11,potential_damage_received=?12,damage_from_invisibles=?13,assist_smoke=?14,assist_inspire=?15,stun_duration=?16,stun_num=?17,capture_points=?18,mileage=?19,arena_type_id=?20,own_platoon_size=?21 WHERE arena_id=?1",
                params![
                    battle_id,
                    extras.vehicle_tier,
                    extras.catalog_fingerprint,
                    own.life_time_seconds,
                    own.death_reason_raw,
                    own.max_health,
                    own.health_remaining,
                    own.direct_hits_received,
                    own.piercings_received,
                    own.no_damage_hits_received,
                    own.explosion_hits_received,
                    own.potential_damage_received,
                    own.damage_from_invisibles,
                    own.assist_smoke,
                    own.assist_inspire,
                    own.stun_duration,
                    own.stun_num,
                    own.capture_points,
                    own.mileage,
                    own.arena_type_id,
                    own.own_platoon_size,
                ],
            )?;
        } else {
            tx.execute(
                "UPDATE battles SET vehicle_tier=?2,vehicle_catalog_fingerprint=?3 WHERE arena_id=?1",
                params![battle_id, extras.vehicle_tier, extras.catalog_fingerprint],
            )?;
        }
        tx.execute(
            "DELETE FROM battle_roster WHERE battle_id=?1",
            params![battle_id],
        )?;
        tx.execute(
            "DELETE FROM battle_team_stats WHERE battle_id=?1",
            params![battle_id],
        )?;
        let Some(roster) = &extras.roster else {
            return Ok(());
        };
        let mut insert_row = tx.prepare_cached(
            "INSERT INTO battle_roster(battle_id,slot,side,is_own_player,type_comp_descr,vehicle_tier,vehicle_class,platoon_group,damage_dealt,kills,spotted,shots,direct_enemy_hits,piercing_enemy_hits,explosion_hits,assist_radio,assist_track,assist_stun,damage_blocked,damage_received,potential_damage_received,max_health,life_time,death_reason_raw,killer_slot,capture_points,defense_points,mileage)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28)",
        )?;
        for row in &roster.rows {
            insert_row.execute(params![
                battle_id,
                row.slot,
                row.side.as_str(),
                row.is_own_player,
                row.type_comp_descr,
                row.vehicle_tier,
                row.vehicle_class.map(VehicleClass::as_str),
                row.platoon_group,
                row.damage_dealt,
                row.kills,
                row.spotted,
                row.shots,
                row.direct_enemy_hits,
                row.piercing_enemy_hits,
                row.explosion_hits,
                row.assist_radio,
                row.assist_track,
                row.assist_stun,
                row.damage_blocked,
                row.damage_received,
                row.potential_damage_received,
                row.max_health,
                row.life_time,
                row.death_reason_raw,
                row.killer_slot,
                row.capture_points,
                row.defense_points,
                row.mileage,
            ])?;
        }
        let mut insert_team = tx.prepare_cached(
            "INSERT INTO battle_team_stats(battle_id,side,fact_schema_version,players,damage_total,kills_total,spotted_total,shots_total,direct_enemy_hits_total,piercing_enemy_hits_total,assist_radio_total,assist_track_total,assist_stun_total,blocked_total,damage_received_total,max_health_total,survivors,life_time_total,tier_min,tier_max,tier_sum,class_light,class_medium,class_heavy,class_td,class_spg,unresolved_vehicles,platoons,platooned_players)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29)",
        )?;
        for team in &roster.teams {
            insert_team.execute(params![
                battle_id,
                team.side.as_str(),
                team.fact_schema_version,
                team.players,
                team.damage_total,
                team.kills_total,
                team.spotted_total,
                team.shots_total,
                team.direct_enemy_hits_total,
                team.piercing_enemy_hits_total,
                team.assist_radio_total,
                team.assist_track_total,
                team.assist_stun_total,
                team.blocked_total,
                team.damage_received_total,
                team.max_health_total,
                team.survivors,
                team.life_time_total,
                team.tier_min,
                team.tier_max,
                team.tier_sum,
                team.class_light,
                team.class_medium,
                team.class_heavy,
                team.class_td,
                team.class_spg,
                team.unresolved_vehicles,
                team.platoons,
                team.platooned_players,
            ])?;
        }
        Ok(())
    }

    /// A replay file rejected by the import gate (not a random battle) is remembered by its key,
    /// size and modification time so the startup scan does not parse it again. A legacy battle it
    /// belongs to counts as "not random" in the rebuild report.
    pub(crate) fn record_skipped_replay_tx(
        tx: &rusqlite::Transaction<'_>,
        file: &ReplayFileState,
        battle: &Battle,
    ) -> Result<(), AppError> {
        tx.execute(
            "INSERT INTO replay_file_skips(source_path_key,file_size,file_mtime_ms,reason,last_processed_at) VALUES(?1,?2,?3,'not_random',?4)
             ON CONFLICT(source_path_key) DO UPDATE SET file_size=excluded.file_size,file_mtime_ms=excluded.file_mtime_ms,reason=excluded.reason,last_processed_at=excluded.last_processed_at",
            params![file.source_path_key, file.file_size, file.file_mtime_ms, Utc::now().timestamp_millis()],
        )?;
        tx.execute(
            "UPDATE legacy_battles SET state='not_random' WHERE state='pending' AND (arena_unique_id=?1 OR (?1 IS NULL AND arena_id=?2))",
            params![battle.arena_unique_id, battle.arena_id],
        )?;
        Ok(())
    }

    pub fn wn8_candidates(&self) -> Result<Vec<Battle>, AppError> {
        self.query_battles(
            &format!("SELECT {BATTLE_COLUMNS} FROM battles ORDER BY battle_time DESC LIMIT 500"),
            &[],
        )
    }

    fn battle(row: &Row<'_>) -> rusqlite::Result<Battle> {
        Ok(Battle {
            arena_id: row.get(0)?,
            arena_unique_id: row.get(23)?,
            battle_time: row.get(1)?,
            battle_time_source: row.get(35)?,
            vehicle_key: row.get(2)?,
            wg_tank_id: row.get(3)?,
            tank_name: row.get(4)?,
            map_name: row.get(5)?,
            map_key: row.get(33)?,
            battle_type: row.get(6)?,
            gameplay_id: row.get(34)?,
            source_kind: SourceKind::parse(&row.get::<_, String>(7)?),
            data_quality: DataQuality::parse(&row.get::<_, String>(8)?),
            parser_version: row.get(24)?,
            feature_schema_version: row.get(32)?,
            game_version: row.get(25)?,
            data_completeness: row.get(26)?,
            replay_source: row.get(27)?,
            parser_error_code: row.get(28)?,
            outcome: BattleOutcome::parse(&row.get::<_, String>(42)?),
            battle_pace: BattlePaceRecord {
                context: row
                    .get::<_, Option<String>>(43)?
                    .as_deref()
                    .and_then(BattlePaceContext::parse),
                reason: row
                    .get::<_, Option<String>>(44)?
                    .as_deref()
                    .and_then(BattlePaceReason::parse),
                resolver_version: row.get(45)?,
                finish_reason_raw: row.get(46)?,
                battle_type_raw: row.get(47)?,
                bonus_type_raw: row.get(48)?,
                initial_team1_size: row.get(49)?,
                initial_team2_size: row.get(50)?,
                destroyed_team1_score: row.get(51)?,
                destroyed_team2_score: row.get(52)?,
            },
            player_team: row.get(36)?,
            winner_team_raw: row.get(37)?,
            damage: row.get(10)?,
            assist: row.get(11)?,
            assist_spotting: row.get(38)?,
            assist_tracking: row.get(39)?,
            assist_stun: row.get(40)?,
            blocked: row.get(12)?,
            damage_received: row.get(13)?,
            frags: row.get(14)?,
            spots: row.get(15)?,
            shots: row.get(16)?,
            hits: row.get(17)?,
            direct_enemy_hits: row.get(53)?,
            direct_team_hits: row.get(54)?,
            penetrations: row.get(18)?,
            piercing_enemy_hits: row.get(55)?,
            explosion_hits: row.get(56)?,
            defense_points: row.get(29)?,
            moving_avg_damage: row.get(30)?,
            damage_rating: row.get(31)?,
            survived: row.get::<_, Option<i64>>(19)?.map(|v| v == 1),
            xp: row.get(20)?,
            original_xp: row.get(41)?,
            duration_seconds: row.get(21)?,
            session_id: row.get(22)?,
        })
    }

    fn challenge(row: &Row<'_>) -> rusqlite::Result<ChallengePresentation> {
        let status = ChallengeStatus::parse(&row.get::<_, String>(4)?)
            .ok_or(rusqlite::Error::InvalidQuery)?;
        let challenge_type =
            ChallengeType::parse(&row.get::<_, String>(5)?).ok_or(rusqlite::Error::InvalidQuery)?;
        let contribution_metric_kind =
            ChallengeContributionMetricKind::parse(&row.get::<_, String>(22)?)
                .ok_or(rusqlite::Error::InvalidQuery)?;
        let baseline_window = BaselineWindowKind::parse(&row.get::<_, String>(26)?)
            .ok_or(rusqlite::Error::InvalidQuery)?;
        let baseline_confidence = BaselineConfidence::parse(&row.get::<_, String>(27)?)
            .ok_or(rusqlite::Error::InvalidQuery)?;
        Ok(ChallengePresentation {
            id: row.get(0)?,
            schema_version: row.get(1)?,
            engine_version: row.get(2)?,
            suggestion_version: row.get(3)?,
            status,
            challenge_type,
            vehicle_id: row.get(6)?,
            vehicle_name: row.get(7)?,
            vehicle_class: VehicleClass::parse(&row.get::<_, String>(8)?),
            vehicle_class_source: VehicleClassSource::parse(&row.get::<_, String>(9)?),
            vehicle_class_resolution_state: VehicleClassResolutionState::parse(
                &row.get::<_, String>(10)?,
            ),
            vehicle_class_resolver_version: row.get(11)?,
            vehicle_class_catalog_schema_version: row.get(12)?,
            compact_descriptor_protocol_source: CompactDescriptorProtocolSource::parse(
                &row.get::<_, String>(13)?,
            ),
            created_at: row.get(14)?,
            started_at: row.get(15)?,
            eligible_after: row.get(16)?,
            finished_at: row.get(17)?,
            target_battles: row_usize(row, 18)?,
            required_successes: row_usize(row, 19)?,
            frozen_baseline_value: row.get(20)?,
            threshold_value: row.get(21)?,
            contribution_metric_kind,
            contribution_floor: row.get(23)?,
            baseline_version: row.get(24)?,
            baseline_cutoff: row.get(25)?,
            baseline_window,
            baseline_confidence,
            baseline_eligible_battles: row_usize(row, 28)?,
            completed_battles: row_usize(row, 29)?,
            successful_battles: row_usize(row, 30)?,
            battles: Vec::new(),
        })
    }

    fn challenge_battles(
        connection: &Connection,
        challenge_id: &str,
    ) -> Result<Vec<ChallengeBattlePresentation>, AppError> {
        let mut statement = connection.prepare(
            "SELECT ordinal,battle_time,metric_value,contribution_value,survived,success
             FROM challenge_battles WHERE challenge_id=?1 ORDER BY ordinal",
        )?;
        Ok(statement
            .query_map(params![challenge_id], |row| {
                Ok(ChallengeBattlePresentation {
                    ordinal: row_usize(row, 0)?,
                    battle_time: row.get(1)?,
                    metric_value: row.get(2)?,
                    contribution_value: row.get(3)?,
                    survived: row.get::<_, Option<i64>>(4)?.map(|value| value == 1),
                    success: row.get::<_, i64>(5)? == 1,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn hydrate_challenge(
        connection: &Connection,
        mut challenge: ChallengePresentation,
    ) -> Result<ChallengePresentation, AppError> {
        challenge.battles = Self::challenge_battles(connection, &challenge.id)?;
        Ok(challenge)
    }
    fn query_battles(
        &self,
        sql: &str,
        args: &[&dyn rusqlite::ToSql],
    ) -> Result<Vec<Battle>, AppError> {
        let c = self.open()?;
        let mut s = c.prepare(sql)?;
        Ok(s.query_map(args, Self::battle)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn cleanup_battles(connection: &Connection) -> Result<Vec<CleanupBattle>, AppError> {
        let mut statement =
            connection.prepare(&format!("SELECT {BATTLE_COLUMNS},imported_at FROM battles"))?;
        Ok(statement
            .query_map([], |row| {
                Ok(CleanupBattle {
                    battle: Self::battle(row)?,
                    imported_at: row.get(57)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn integrity_report_on(connection: &Connection) -> Result<IntegrityReport, AppError> {
        let integrity_check = {
            let mut statement = connection.prepare("PRAGMA integrity_check")?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?
        };
        let foreign_key_check = {
            let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
            statement
                .query_map([], |row| {
                    let table: String = row.get(0)?;
                    let parent: String = row.get(2)?;
                    let foreign_key: i64 = row.get(3)?;
                    Ok(format!("{table}->{parent}#{foreign_key}"))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        let orphaned_records = connection.query_row(
            "SELECT
                (SELECT COUNT(*) FROM battles b LEFT JOIN sessions s ON s.id=b.session_id WHERE b.session_id IS NOT NULL AND s.id IS NULL)
              + (SELECT COUNT(*) FROM replay_files r LEFT JOIN battles b ON b.arena_id=r.battle_id WHERE b.arena_id IS NULL)
              + (SELECT COUNT(*) FROM challenge_battles cb LEFT JOIN challenges c ON c.id=cb.challenge_id WHERE c.id IS NULL)",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let empty_ended_sessions = connection.query_row(
            "SELECT COUNT(*) FROM sessions s WHERE s.status!='active' AND NOT EXISTS(SELECT 1 FROM battles b WHERE b.session_id=s.id)",
            [],
            |row| row.get::<_, i64>(0),
        )? as usize;
        let plan = duplicate_plan(&Self::cleanup_battles(connection)?);
        Ok(IntegrityReport {
            integrity_check,
            foreign_key_check,
            orphaned_records,
            empty_ended_sessions,
            certain_duplicate_records: plan
                .groups
                .iter()
                .map(|group| group.len().saturating_sub(1))
                .sum(),
            ambiguous_candidate_groups: plan.ambiguous_candidate_groups,
        })
    }

    pub fn integrity_report(&self) -> Result<IntegrityReport, AppError> {
        Self::integrity_report_on(&*self.open()?)
    }

    fn update_cleanup_battle(
        transaction: &rusqlite::Transaction<'_>,
        battle: &Battle,
    ) -> Result<(), AppError> {
        transaction.execute(
            "UPDATE battles SET battle_time=?2,vehicle_key=?3,wg_tank_id=?4,tank_name=?5,map_name=?6,battle_type=?7,source_kind=?8,data_quality=?9,won=?10,damage=?11,assist=?12,blocked=?13,damage_received=?14,frags=?15,spots=?16,shots=?17,hits=?18,penetrations=?19,survived=?20,xp=?21,duration_seconds=?22,session_id=?23,arena_unique_id=?24,parser_version=?25,game_version=?26,data_completeness=?27,replay_source=?28,parser_error_code=?29,defense_points=?30,moving_avg_damage=?31,damage_rating=?32,feature_schema_version=?33,map_key=?34,gameplay_id=?35,battle_time_source=?36,player_team=?37,winner_team_raw=?38,assist_spotting=?39,assist_tracking=?40,assist_stun=?41,original_xp=?42,outcome=?43,battle_pace_context=?44,battle_pace_reason=?45,battle_pace_resolver_version=?46,finish_reason_raw=?47,battle_type_raw=?48,bonus_type_raw=?49,initial_team1_size=?50,initial_team2_size=?51,destroyed_team1_score=?52,destroyed_team2_score=?53,direct_enemy_hits=?54,direct_team_hits=?55,piercing_enemy_hits=?56,explosion_hits=?57 WHERE arena_id=?1",
            params![
                battle.arena_id,
                battle.battle_time,
                battle.vehicle_key,
                battle.wg_tank_id,
                battle.tank_name,
                battle.map_name,
                battle.battle_type,
                battle.source_kind.as_str(),
                battle.data_quality.as_str(),
                battle.outcome.legacy_won().map(i64::from),
                battle.damage,
                battle.assist,
                battle.blocked,
                battle.damage_received,
                battle.frags,
                battle.spots,
                battle.shots,
                battle.hits,
                battle.penetrations,
                battle.survived.map(i64::from),
                battle.xp,
                battle.duration_seconds,
                battle.session_id,
                battle.arena_unique_id,
                battle.parser_version,
                battle.game_version,
                battle.data_completeness,
                battle.replay_source,
                battle.parser_error_code,
                battle.defense_points,
                battle.moving_avg_damage,
                battle.damage_rating,
                battle.feature_schema_version,
                battle.map_key,
                battle.gameplay_id,
                battle.battle_time_source,
                battle.player_team,
                battle.winner_team_raw,
                battle.assist_spotting,
                battle.assist_tracking,
                battle.assist_stun,
                battle.original_xp,
                battle.outcome.as_str(),
                battle.battle_pace.context.map(BattlePaceContext::as_str),
                battle.battle_pace.reason.map(BattlePaceReason::as_str),
                battle.battle_pace.resolver_version,
                battle.battle_pace.finish_reason_raw,
                battle.battle_pace.battle_type_raw,
                battle.battle_pace.bonus_type_raw,
                battle.battle_pace.initial_team1_size,
                battle.battle_pace.initial_team2_size,
                battle.battle_pace.destroyed_team1_score,
                battle.battle_pace.destroyed_team2_score,
                battle.direct_enemy_hits,
                battle.direct_team_hits,
                battle.piercing_enemy_hits,
                battle.explosion_hits,
            ],
        )?;
        Ok(())
    }

    pub fn cleanup_legacy_data(&self) -> Result<CleanupReport, AppError> {
        self.cleanup_legacy_data_with(|| Ok(()))
    }

    fn cleanup_legacy_data_with<F>(&self, midpoint: F) -> Result<CleanupReport, AppError>
    where
        F: FnOnce() -> Result<(), AppError>,
    {
        let mut connection = self.open()?;
        let before = Self::integrity_report_on(&connection)?;
        if !before.is_healthy() {
            return Err(AppError::DatabaseIntegrity);
        }
        let transaction = connection.transaction()?;
        let rows = Self::cleanup_battles(&transaction)?;
        let plan = duplicate_plan(&rows);
        let mut merged_battles = 0_usize;
        for group in plan.groups {
            let canonical_index = *group
                .iter()
                .max_by_key(|index| {
                    let row = &rows[**index];
                    (
                        quality_rank(&row.battle.data_quality),
                        metric_richness(&row.battle),
                        row.battle.parser_version.unwrap_or(-1),
                        row.imported_at,
                        row.battle.arena_id.clone(),
                    )
                })
                .ok_or(AppError::DatabaseIntegrity)?;
            let mut canonical = rows[canonical_index].battle.clone();
            for index in &group {
                if *index != canonical_index {
                    merge_battle_data(&mut canonical, &rows[*index].battle);
                }
            }
            for index in &group {
                if *index == canonical_index {
                    continue;
                }
                let duplicate_id = &rows[*index].battle.arena_id;
                transaction.execute(
                    "UPDATE replay_files SET battle_id=?1 WHERE battle_id=?2",
                    params![canonical.arena_id, duplicate_id],
                )?;
                transaction.execute(
                    "DELETE FROM battles WHERE arena_id=?1",
                    params![duplicate_id],
                )?;
                merged_battles += 1;
            }
            Self::update_cleanup_battle(&transaction, &canonical)?;
        }
        midpoint()?;
        let deleted_empty_sessions = transaction.execute(
            "DELETE FROM sessions WHERE status!='active' AND NOT EXISTS(SELECT 1 FROM battles WHERE battles.session_id=sessions.id)",
            [],
        )?;
        transaction.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_battles_arena_unique_id ON battles(arena_unique_id) WHERE arena_unique_id IS NOT NULL;")?;
        let after = Self::integrity_report_on(&transaction)?;
        if !after.is_healthy() || after.certain_duplicate_records != 0 {
            return Err(AppError::DatabaseIntegrity);
        }
        transaction.commit()?;
        Ok(CleanupReport {
            before,
            after,
            merged_battles,
            deleted_empty_sessions,
        })
    }

    pub fn recent_battles(&self, limit: usize) -> Result<Vec<Battle>, AppError> {
        self.query_battles(
            &format!("SELECT {BATTLE_COLUMNS} FROM battles ORDER BY battle_time DESC LIMIT ?1"),
            &[&(limit.clamp(1, 500) as i64)],
        )
    }

    pub fn attempts_conversion_session_battles(
        &self,
        session_id: &str,
    ) -> Result<Vec<AttemptsConversionBattle>, AppError> {
        let connection = self.open_read_only()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
            params![session_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::SessionNotFound);
        }
        let mut statement = connection.prepare(
            "SELECT arena_id,battle_time,vehicle_key,wg_tank_id,data_quality,
                    data_completeness,replay_source,parser_version,feature_schema_version,
                    gameplay_id,battle_type_raw,bonus_type_raw,initial_team1_size,
                    initial_team2_size,shots,hits,direct_enemy_hits,direct_team_hits,
                    penetrations,piercing_enemy_hits,explosion_hits,damage
             FROM battles WHERE session_id=?1
             ORDER BY battle_time DESC,arena_id DESC",
        )?;
        Ok(statement
            .query_map(params![session_id], attempts_conversion_battle)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn attempts_conversion_recent_battles(
        &self,
        limit: usize,
    ) -> Result<Vec<AttemptsConversionBattle>, AppError> {
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(
            "SELECT arena_id,battle_time,vehicle_key,wg_tank_id,data_quality,
                    data_completeness,replay_source,parser_version,feature_schema_version,
                    gameplay_id,battle_type_raw,bonus_type_raw,initial_team1_size,
                    initial_team2_size,shots,hits,direct_enemy_hits,direct_team_hits,
                    penetrations,piercing_enemy_hits,explosion_hits,damage
             FROM battles ORDER BY battle_time DESC,arena_id DESC LIMIT ?1",
        )?;
        Ok(statement
            .query_map(
                params![limit.clamp(1, 100) as i64],
                attempts_conversion_battle,
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub(crate) fn team_relative_results_session_rows(
        &self,
        session_id: &str,
    ) -> Result<Vec<TeamRelativeResultsReadRow>, AppError> {
        let connection = self.open_read_only()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
            params![session_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::SessionNotFound);
        }
        let mut statement = connection.prepare(&format!(
            "SELECT {TEAM_RELATIVE_RESULTS_COLUMNS}
             FROM battles b
             LEFT JOIN team_relative_facts f ON f.battle_id=b.arena_id
             WHERE b.session_id=?1
             ORDER BY b.battle_time DESC,b.arena_id DESC"
        ))?;
        Ok(statement
            .query_map(params![session_id], team_relative_results_row)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub(crate) fn team_relative_results_recent_rows(
        &self,
        limit: usize,
    ) -> Result<Vec<TeamRelativeResultsReadRow>, AppError> {
        debug_assert!(matches!(limit, 10 | 30 | 100));
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(&format!(
            "SELECT {TEAM_RELATIVE_RESULTS_COLUMNS}
             FROM battles b
             LEFT JOIN team_relative_facts f ON f.battle_id=b.arena_id
             ORDER BY b.battle_time DESC,b.arena_id DESC
             LIMIT ?1"
        ))?;
        Ok(statement
            .query_map(params![limit as i64], team_relative_results_row)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn performance_trend_battles(
        &self,
        vehicle_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<Battle>, AppError> {
        let mut sql = format!(
            "SELECT {BATTLE_COLUMNS} FROM battles b
             WHERE {STATS_ELIGIBLE_SQL} AND b.battle_time_source='metadata'"
        );
        let mut values = Vec::<Value>::new();
        if let Some(vehicle_id) = vehicle_id {
            sql.push_str(" AND b.wg_tank_id=?");
            values.push(Value::Integer(vehicle_id));
        }
        sql.push_str(" ORDER BY b.battle_time DESC,b.arena_id DESC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 200) as i64));
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(&sql)?;
        Ok(statement
            .query_map(params_from_iter(values.iter()), Self::battle)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn battle_history_rows(
        &self,
        filter: &BattleHistoryFilter,
        cursor: Option<&BattleHistoryCursor>,
        limit: usize,
    ) -> Result<Vec<HistoryBattleRow>, AppError> {
        let mut sql = format!(
            "SELECT {BATTLE_COLUMNS},s.started_at,s.status FROM battles b LEFT JOIN sessions s ON s.id=b.session_id WHERE 1=1"
        );
        let mut values = Vec::<Value>::new();
        if let Some(vehicle_id) = filter.vehicle_id {
            sql.push_str(" AND b.wg_tank_id=?");
            values.push(Value::Integer(vehicle_id));
        }
        if let Some(map_key) = &filter.map_key {
            sql.push_str(" AND b.map_key=?");
            values.push(Value::Text(map_key.clone()));
        }
        match filter.data_quality {
            HistoryDataQualityFilter::All => {}
            HistoryDataQualityFilter::Full => sql.push_str(" AND b.data_quality='full'"),
            HistoryDataQualityFilter::Partial => sql.push_str(" AND b.data_quality='partial'"),
            HistoryDataQualityFilter::Ambiguous => sql.push_str(" AND b.data_quality='ambiguous'"),
        }
        if let Some(date_from) = filter.date_from {
            sql.push_str(" AND b.battle_time>=?");
            values.push(Value::Integer(date_from));
        }
        if let Some(date_to) = filter.date_to {
            sql.push_str(" AND b.battle_time<=?");
            values.push(Value::Integer(date_to));
        }
        if let Some(cursor) = cursor {
            sql.push_str(" AND (b.battle_time<? OR (b.battle_time=? AND b.arena_id<?))");
            values.push(Value::Integer(cursor.battle_time));
            values.push(Value::Integer(cursor.battle_time));
            values.push(Value::Text(cursor.battle_id.clone()));
        }
        sql.push_str(" ORDER BY b.battle_time DESC,b.arena_id DESC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 500) as i64));
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(&sql)?;
        Ok(statement
            .query_map(params_from_iter(values.iter()), |row| {
                Ok(HistoryBattleRow {
                    battle: Self::battle(row)?,
                    session_started_at: row.get(57)?,
                    session_status: row.get(58)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn all_history_battles(&self) -> Result<Vec<Battle>, AppError> {
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(&format!(
            "SELECT {BATTLE_COLUMNS} FROM battles ORDER BY battle_time DESC,arena_id DESC"
        ))?;
        Ok(statement
            .query_map([], Self::battle)?
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// Tier per vehicle from the newest battle that has one (spec 25 §5.2, tank header).
    pub(crate) fn vehicle_tiers(&self) -> Result<HashMap<i64, u8>, AppError> {
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(
            "SELECT wg_tank_id,vehicle_tier FROM battles WHERE wg_tank_id IS NOT NULL AND vehicle_tier IS NOT NULL ORDER BY battle_time ASC,arena_id ASC",
        )?;
        Ok(statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<HashMap<_, _>, _>>()?)
    }

    pub(crate) fn distinct_vehicle_class_inputs(
        &self,
    ) -> Result<Vec<(i64, Option<String>)>, AppError> {
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(
            "SELECT DISTINCT wg_tank_id,vehicle_key FROM battles WHERE wg_tank_id IS NOT NULL AND wg_tank_id>0",
        )?;
        Ok(statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn challenge_by_id_on(
        connection: &Connection,
        challenge_id: &str,
    ) -> Result<Option<ChallengePresentation>, AppError> {
        let challenge = connection
            .query_row(
                &format!("SELECT {CHALLENGE_COLUMNS} FROM challenges WHERE id=?1"),
                params![challenge_id],
                Self::challenge,
            )
            .optional()?;
        challenge
            .map(|value| Self::hydrate_challenge(connection, value))
            .transpose()
    }

    fn active_challenge_on(
        connection: &Connection,
    ) -> Result<Option<ChallengePresentation>, AppError> {
        let challenge = connection
            .query_row(
                &format!(
                    "SELECT {CHALLENGE_COLUMNS} FROM challenges WHERE status='active' AND active_slot=1"
                ),
                [],
                Self::challenge,
            )
            .optional()?;
        challenge
            .map(|value| Self::hydrate_challenge(connection, value))
            .transpose()
    }

    pub fn active_challenge(&self) -> Result<Option<ChallengePresentation>, AppError> {
        Self::active_challenge_on(&self.open_read_only()?)
    }

    pub fn latest_finished_challenge(&self) -> Result<Option<ChallengePresentation>, AppError> {
        let connection = self.open_read_only()?;
        let challenge = connection
            .query_row(
                &format!("SELECT {CHALLENGE_COLUMNS} FROM challenges WHERE status!='active' ORDER BY finished_at DESC,id DESC LIMIT 1"),
                [],
                Self::challenge,
            )
            .optional()?;
        challenge
            .map(|value| Self::hydrate_challenge(&connection, value))
            .transpose()
    }

    pub fn start_challenge(
        &self,
        source_session_id: &str,
        suggestion: &ChallengeSuggestion,
        now: i64,
    ) -> Result<ChallengePresentation, AppError> {
        if suggestion.challenge_engine_version != CHALLENGE_ENGINE_VERSION
            || suggestion.suggestion_version.is_empty()
            || suggestion.vehicle_id <= 0
            || suggestion.vehicle_class == VehicleClass::Unknown
            || suggestion.vehicle_class_resolution_state != VehicleClassResolutionState::Resolved
            || suggestion.target_battles != 5
            || !(2..=4).contains(&suggestion.required_successes)
            || suggestion.baseline_eligible_battles < 30
            || suggestion.baseline_metric_eligible_battles < 30
            || !suggestion.frozen_baseline_value.is_finite()
            || suggestion.frozen_baseline_value <= 0.0
            || !suggestion.evidence_delta.is_finite()
            || !suggestion.evidence_normalized_strength.is_finite()
            || suggestion.expected_snapshot_sha256.len() != 64
            || !suggestion
                .expected_snapshot_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || !valid_suggestion_contract(suggestion)
        {
            return Err(AppError::InvalidChallenge);
        }
        let eligible_after = now
            .div_euclid(1_000)
            .checked_add(1)
            .and_then(|second| second.checked_mul(1_000))
            .ok_or(AppError::InvalidChallenge)?;
        let id = Uuid::new_v4().to_string();
        let mut connection = self.open()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
            params![source_session_id],
            |row| row.get(0),
        )?;
        if !session_exists {
            return Err(AppError::SessionNotFound);
        }
        let active_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM challenges WHERE active_slot=1)",
            [],
            |row| row.get(0),
        )?;
        if active_exists {
            return Err(AppError::ChallengeAlreadyActive);
        }
        transaction.execute(
            "INSERT INTO challenges(
              id,schema_version,engine_version,suggestion_version,suggestion_id,source_session_id,status,active_slot,challenge_type,
              vehicle_id,vehicle_name_snapshot,vehicle_class_snapshot,vehicle_class_source_snapshot,vehicle_class_resolution_state_snapshot,
              vehicle_class_resolver_version,vehicle_class_catalog_schema_version,compact_descriptor_protocol_source,
              created_at,started_at,eligible_after,finished_at,target_battles,required_successes,frozen_baseline_value,threshold_value,
              contribution_metric_kind,contribution_floor,frozen_damage_baseline,frozen_survival_baseline,frozen_combined_contribution_baseline,
              frozen_spotting_assist_baseline,frozen_tracking_assist_baseline,frozen_stun_assist_baseline,baseline_version,baseline_cutoff,
              baseline_window,baseline_confidence,baseline_eligible_battles,baseline_metric_eligible_battles,insight_version,formula_version,
              expected_snapshot_version,expected_snapshot_sha256,evidence_metric,evidence_delta,evidence_normalized_strength,completed_battles,successful_battles
            ) VALUES(
              ?1,?2,?3,?4,?5,?6,'active',1,?7,?8,?9,?10,?11,?12,?13,?14,?15,
              ?16,?16,?17,NULL,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37,?38,?39,?40,?41,?42,0,0
            )",
            params![
                id,
                CHALLENGE_SCHEMA_VERSION,
                suggestion.challenge_engine_version,
                suggestion.suggestion_version,
                suggestion.suggestion_id,
                source_session_id,
                suggestion.challenge_type.as_str(),
                suggestion.vehicle_id,
                suggestion.vehicle_name,
                suggestion.vehicle_class.as_str(),
                suggestion.vehicle_class_source.as_str(),
                suggestion.vehicle_class_resolution_state.as_str(),
                suggestion.vehicle_class_resolver_version,
                suggestion.vehicle_class_catalog_schema_version,
                suggestion.compact_descriptor_protocol_source.as_str(),
                now,
                eligible_after,
                suggestion.target_battles as i64,
                suggestion.required_successes as i64,
                suggestion.frozen_baseline_value,
                suggestion.threshold_value,
                suggestion.contribution_metric_kind.as_str(),
                suggestion.contribution_floor,
                suggestion.frozen_damage_baseline,
                suggestion.frozen_survival_baseline,
                suggestion.frozen_combined_contribution_baseline,
                suggestion.frozen_spotting_assist_baseline,
                suggestion.frozen_tracking_assist_baseline,
                suggestion.frozen_stun_assist_baseline,
                suggestion.baseline_version,
                suggestion.baseline_cutoff,
                suggestion.baseline_window.as_str(),
                suggestion.baseline_confidence.as_str(),
                suggestion.baseline_eligible_battles as i64,
                suggestion.baseline_metric_eligible_battles as i64,
                suggestion.insight_version,
                suggestion.formula_version,
                suggestion.expected_snapshot_version,
                suggestion.expected_snapshot_sha256,
                suggestion.evidence_metric.as_str(),
                suggestion.evidence_delta,
                suggestion.evidence_normalized_strength,
            ],
        )?;
        transaction.commit()?;
        Self::challenge_by_id_on(&self.open_read_only()?, &id)?.ok_or(AppError::InvalidChallenge)
    }

    fn apply_challenge_battle(
        transaction: &rusqlite::Transaction<'_>,
        challenge: &mut ChallengePresentation,
        battle: &Battle,
        now: i64,
        already_counted: bool,
    ) -> Result<bool, AppError> {
        let evaluation = if already_counted {
            ChallengeBattleEvaluation::NotEligible(
                crate::challenges::ChallengeNotEligibleReason::AlreadyCounted,
            )
        } else {
            evaluate_battle_for_challenge(challenge, battle)
        };
        let ChallengeBattleEvaluation::Eligible {
            metric_value,
            contribution_value,
            survived,
            success,
        } = evaluation
        else {
            return Ok(false);
        };
        if challenge.completed_battles >= challenge.target_battles {
            return Ok(false);
        }
        let ordinal = challenge.completed_battles + 1;
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO challenge_battles(challenge_id,battle_id,ordinal,battle_time,counted_at,metric_value,contribution_value,survived,success)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                challenge.id,
                battle.arena_id,
                ordinal as i64,
                battle.battle_time,
                now,
                metric_value,
                contribution_value,
                survived.map(i64::from),
                i64::from(success),
            ],
        )?;
        if inserted == 0 {
            return Ok(false);
        }
        challenge.completed_battles += 1;
        if success {
            challenge.successful_battles += 1;
        }
        Ok(true)
    }

    fn persist_challenge_progress(
        transaction: &rusqlite::Transaction<'_>,
        challenge: &mut ChallengePresentation,
        now: i64,
    ) -> Result<(), AppError> {
        let counts = transaction.query_row(
            "SELECT COUNT(*),COALESCE(SUM(success),0) FROM challenge_battles WHERE challenge_id=?1",
            params![challenge.id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, i64>(1)? as usize,
                ))
            },
        )?;
        challenge.completed_battles = counts.0;
        challenge.successful_battles = counts.1;
        if challenge.completed_battles > challenge.target_battles
            || challenge.successful_battles > challenge.completed_battles
        {
            return Err(AppError::DatabaseIntegrity);
        }
        if challenge.completed_battles == challenge.target_battles {
            challenge.status = if challenge.successful_battles >= challenge.required_successes {
                ChallengeStatus::Completed
            } else {
                ChallengeStatus::Failed
            };
            challenge.finished_at = Some(now);
            transaction.execute(
                "UPDATE challenges SET status=?2,active_slot=NULL,finished_at=?3,completed_battles=?4,successful_battles=?5 WHERE id=?1 AND status='active'",
                params![challenge.id, challenge.status.as_str(), now, counts.0 as i64, counts.1 as i64],
            )?;
        } else {
            transaction.execute(
                "UPDATE challenges SET completed_battles=?2,successful_battles=?3 WHERE id=?1 AND status='active'",
                params![challenge.id, counts.0 as i64, counts.1 as i64],
            )?;
        }
        Ok(())
    }

    pub fn process_battle_for_active_challenge(
        &self,
        battle_id: &str,
    ) -> Result<Option<ChallengePresentation>, AppError> {
        let is_possible_candidate = self.open_read_only()?.query_row(
            &format!(
                "SELECT EXISTS(
               SELECT 1 FROM challenges c JOIN battles b ON b.arena_id=?1
               WHERE c.status='active' AND c.active_slot=1
                 AND b.wg_tank_id=c.vehicle_id AND b.battle_time>=c.eligible_after
                 AND b.battle_time_source='metadata' AND {STATS_ELIGIBLE_SQL}
             )"
            ),
            params![battle_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !is_possible_candidate {
            return Ok(None);
        }
        let mut connection = self.open()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(mut challenge) = Self::active_challenge_on(&transaction)? else {
            transaction.commit()?;
            return Ok(None);
        };
        let battle = transaction
            .query_row(
                &format!("SELECT {BATTLE_COLUMNS} FROM battles WHERE arena_id=?1"),
                params![battle_id],
                Self::battle,
            )
            .optional()?;
        let Some(battle) = battle else {
            transaction.commit()?;
            return Ok(Some(challenge));
        };
        let already_counted = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM challenge_battles WHERE challenge_id=?1 AND battle_id=?2)",
            params![challenge.id, battle.arena_id],
            |row| row.get::<_, bool>(0),
        )?;
        let now = Utc::now().timestamp_millis();
        if Self::apply_challenge_battle(
            &transaction,
            &mut challenge,
            &battle,
            now,
            already_counted,
        )? {
            Self::persist_challenge_progress(&transaction, &mut challenge, now)?;
        }
        let id = challenge.id.clone();
        transaction.commit()?;
        Self::challenge_by_id_on(&self.open_read_only()?, &id)
    }

    pub fn catch_up_active_challenge(&self) -> Result<Option<ChallengePresentation>, AppError> {
        let mut challenge_id = None::<String>;
        let mut cursor = None::<(i64, String)>;
        loop {
            let mut connection = self.open()?;
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let Some(mut challenge) = Self::active_challenge_on(&transaction)? else {
                transaction.commit()?;
                return Ok(None);
            };
            if challenge_id
                .as_deref()
                .is_some_and(|expected| expected != challenge.id)
            {
                transaction.commit()?;
                return Ok(Some(challenge));
            }
            challenge_id.get_or_insert_with(|| challenge.id.clone());

            let candidates = {
                let mut sql = format!(
                    "SELECT {BATTLE_COLUMNS} FROM battles b
                     WHERE b.wg_tank_id=? AND b.battle_time>=? AND b.battle_time_source='metadata'
                       AND {STATS_ELIGIBLE_SQL}
                       AND NOT EXISTS(SELECT 1 FROM challenge_battles cb WHERE cb.challenge_id=? AND cb.battle_id=b.arena_id)"
                );
                let mut values = vec![
                    Value::Integer(challenge.vehicle_id),
                    Value::Integer(challenge.eligible_after),
                    Value::Text(challenge.id.clone()),
                ];
                if let Some((battle_time, battle_id)) = cursor.as_ref() {
                    sql.push_str(" AND (b.battle_time>? OR (b.battle_time=? AND b.arena_id>?))");
                    values.push(Value::Integer(*battle_time));
                    values.push(Value::Integer(*battle_time));
                    values.push(Value::Text(battle_id.clone()));
                }
                sql.push_str(" ORDER BY b.battle_time,b.arena_id LIMIT ?");
                values.push(Value::Integer(CHALLENGE_CATCH_UP_PAGE_SIZE as i64));
                let mut statement = transaction.prepare(&sql)?;
                statement
                    .query_map(params_from_iter(values.iter()), Self::battle)?
                    .collect::<Result<Vec<_>, _>>()?
            };
            if candidates.is_empty() {
                let id = challenge.id.clone();
                transaction.commit()?;
                return Self::challenge_by_id_on(&self.open_read_only()?, &id);
            }
            let page_is_full = candidates.len() == CHALLENGE_CATCH_UP_PAGE_SIZE;
            let next_cursor = candidates
                .last()
                .map(|battle| (battle.battle_time, battle.arena_id.clone()));
            let now = Utc::now().timestamp_millis();
            let mut changed = false;
            for battle in candidates {
                if challenge.completed_battles >= challenge.target_battles {
                    break;
                }
                changed |= Self::apply_challenge_battle(
                    &transaction,
                    &mut challenge,
                    &battle,
                    now,
                    false,
                )?;
            }
            if changed {
                Self::persist_challenge_progress(&transaction, &mut challenge, now)?;
            }
            let id = challenge.id.clone();
            let terminal = challenge.status != ChallengeStatus::Active;
            transaction.commit()?;
            let presentation = Self::challenge_by_id_on(&self.open_read_only()?, &id)?;
            if terminal || !page_is_full {
                return Ok(presentation);
            }
            cursor = next_cursor;
        }
    }

    pub fn abandon_active_challenge(&self) -> Result<ChallengePresentation, AppError> {
        let mut connection = self.open()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(challenge) = Self::active_challenge_on(&transaction)? else {
            return Err(AppError::NoActiveChallenge);
        };
        let now = Utc::now().timestamp_millis();
        let changed = transaction.execute(
            "UPDATE challenges SET status='abandoned',active_slot=NULL,finished_at=?2 WHERE id=?1 AND status='active' AND active_slot=1",
            params![challenge.id, now],
        )?;
        if changed != 1 {
            return Err(AppError::NoActiveChallenge);
        }
        let id = challenge.id;
        transaction.commit()?;
        Self::challenge_by_id_on(&self.open_read_only()?, &id)?.ok_or(AppError::InvalidChallenge)
    }

    pub fn challenge_history_page(
        &self,
        offset: usize,
        page_size: usize,
    ) -> Result<ChallengeHistoryPage, AppError> {
        let connection = self.open_read_only()?;
        let limit = page_size.clamp(1, 50);
        let mut statement = connection.prepare(&format!(
            "SELECT {CHALLENGE_COLUMNS} FROM challenges WHERE status!='active' ORDER BY started_at DESC,id DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let mut items = statement
            .query_map(params![(limit + 1) as i64, offset as i64], Self::challenge)?
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = items.len() > limit;
        items.truncate(limit);
        if !items.is_empty() {
            let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
            let placeholders = (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(",");
            let mut child_statement = connection.prepare(&format!(
                "SELECT challenge_id,ordinal,battle_time,metric_value,contribution_value,survived,success FROM challenge_battles WHERE challenge_id IN ({placeholders}) ORDER BY challenge_id,ordinal"
            ))?;
            let mut grouped = BTreeMap::<String, Vec<ChallengeBattlePresentation>>::new();
            for row in child_statement.query_map(params_from_iter(ids.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ChallengeBattlePresentation {
                        ordinal: row_usize(row, 1)?,
                        battle_time: row.get(2)?,
                        metric_value: row.get(3)?,
                        contribution_value: row.get(4)?,
                        survived: row.get::<_, Option<i64>>(5)?.map(|value| value == 1),
                        success: row.get::<_, i64>(6)? == 1,
                    },
                ))
            })? {
                let (id, battle) = row?;
                grouped.entry(id).or_default().push(battle);
            }
            for item in &mut items {
                item.battles = grouped.remove(&item.id).unwrap_or_default();
            }
        }
        Ok(ChallengeHistoryPage {
            items,
            next_offset: has_more.then_some(offset + limit),
        })
    }

    pub fn challenge_diagnostic_state(&self) -> Result<ChallengeDiagnosticState, AppError> {
        let connection = self.open_read_only()?;
        let active = Self::active_challenge_on(&connection)?;
        let mut counts = BTreeMap::new();
        let mut statement =
            connection.prepare("SELECT status,COUNT(*) FROM challenges GROUP BY status")?;
        for row in statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })? {
            let (status, count) = row?;
            counts.insert(status, count);
        }
        Ok(ChallengeDiagnosticState {
            active_challenge: active.is_some(),
            active_challenge_type: active.as_ref().map(|item| item.challenge_type),
            active_challenge_vehicle_class: active.as_ref().map(|item| item.vehicle_class),
            active_challenge_completed_battles: active.as_ref().map(|item| item.completed_battles),
            active_challenge_target_battles: active.as_ref().map(|item| item.target_battles),
            active_challenge_successful_battles: active
                .as_ref()
                .map(|item| item.successful_battles),
            active_challenge_required_successes: active
                .as_ref()
                .map(|item| item.required_successes),
            challenge_counts_by_status: counts,
        })
    }

    pub fn vehicle_history_rows(&self, vehicle_id: i64) -> Result<Vec<HistoryBattleRow>, AppError> {
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(&format!(
            "SELECT {BATTLE_COLUMNS},s.started_at,s.status FROM battles b LEFT JOIN sessions s ON s.id=b.session_id WHERE b.wg_tank_id=?1 ORDER BY b.battle_time DESC,b.arena_id DESC"
        ))?;
        Ok(statement
            .query_map(params![vehicle_id], |row| {
                Ok(HistoryBattleRow {
                    battle: Self::battle(row)?,
                    session_started_at: row.get(57)?,
                    session_status: row.get(58)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn baseline_candidate_rows(
        &self,
        cutoff: i64,
        vehicle_id: Option<i64>,
        excluded_session_id: Option<&str>,
        cursor: Option<&BattleHistoryCursor>,
        limit: usize,
    ) -> Result<Vec<Battle>, AppError> {
        let mut sql = format!("SELECT {BATTLE_COLUMNS} FROM battles b WHERE b.battle_time<?");
        let mut values = vec![Value::Integer(cutoff)];
        if let Some(vehicle_id) = vehicle_id {
            sql.push_str(" AND b.wg_tank_id=?");
            values.push(Value::Integer(vehicle_id));
        }
        if let Some(session_id) = excluded_session_id {
            sql.push_str(" AND (b.session_id IS NULL OR b.session_id!=?)");
            values.push(Value::Text(session_id.to_owned()));
        }
        if let Some(cursor) = cursor {
            sql.push_str(" AND (b.battle_time<? OR (b.battle_time=? AND b.arena_id<?))");
            values.push(Value::Integer(cursor.battle_time));
            values.push(Value::Integer(cursor.battle_time));
            values.push(Value::Text(cursor.battle_id.clone()));
        }
        sql.push_str(" ORDER BY b.battle_time DESC,b.arena_id DESC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 500) as i64));
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(&sql)?;
        Ok(statement
            .query_map(params_from_iter(values.iter()), Self::battle)?
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub fn pending_replays_between(
        &self,
        vehicle: &str,
        after: i64,
        before: i64,
    ) -> Result<Vec<Battle>, AppError> {
        self.query_battles(
            &format!("SELECT {BATTLE_COLUMNS} FROM battles WHERE vehicle_key=?1 AND data_quality='partial' AND battle_time BETWEEN ?2 AND ?3 ORDER BY battle_time"),
            &[&vehicle, &after, &before],
        )
    }
    /// Reference values for per-battle feedback (P4): the newest 100 other battles on the same
    /// vehicle, or the newest 100 other battles at all when fewer than 5 of the former count for
    /// statistics. Computed as SQL aggregates instead of loading full rows.
    pub fn feedback_baseline_for_battle(&self, b: &Battle) -> Result<FeedbackBaseline, AppError> {
        let connection = self.open_read_only()?;
        let aggregate = |filter: &str, args: &[&dyn rusqlite::ToSql]| {
            connection.query_row(
                &format!(
                    "SELECT COUNT(CASE WHEN {STATS_ELIGIBLE_SQL} THEN 1 END),
                            COALESCE(SUM(CASE WHEN {STATS_ELIGIBLE_SQL} THEN b.damage END),0),
                            COUNT(CASE WHEN {STATS_ELIGIBLE_SQL} THEN b.damage END),
                            COALESCE(SUM(CASE WHEN {STATS_ELIGIBLE_SQL} THEN b.assist END),0),
                            COUNT(CASE WHEN {STATS_ELIGIBLE_SQL} THEN b.assist END)
                     FROM (SELECT * FROM battles WHERE {filter}
                           ORDER BY battle_time DESC, arena_id DESC LIMIT 100) b"
                ),
                args,
                |row| {
                    Ok(FeedbackBaseline::from_sums(
                        row_usize(row, 0)?,
                        (row.get(1)?, row_usize(row, 2)?),
                        (row.get(3)?, row_usize(row, 4)?),
                    ))
                },
            )
        };
        let same = aggregate(
            "vehicle_key=?1 AND arena_id!=?2",
            &[&b.vehicle_key, &b.arena_id],
        )?;
        if same.battles >= 5 {
            return Ok(same);
        }
        Ok(aggregate("arena_id!=?1", &[&b.arena_id])?)
    }

    fn session_row(r: &Row<'_>) -> rusqlite::Result<Session> {
        Ok(Session {
            id: r.get(0)?,
            started_at: r.get(1)?,
            last_battle_at: r.get(2)?,
            ended_at: r.get(3)?,
            status: r.get(4)?,
            battle_count: r.get::<_, i64>(5)? as usize,
            analyzed_battle_count: r.get::<_, i64>(6)? as usize,
        })
    }
    fn session_query(
        &self,
        where_sql: &str,
        args: &[&dyn rusqlite::ToSql],
    ) -> Result<Option<Session>, AppError> {
        let c = self.open()?;
        Ok(c.query_row(&format!("SELECT s.id,s.started_at,s.last_battle_at,s.ended_at,s.status,COUNT(b.arena_id),COUNT(CASE WHEN {STATS_ELIGIBLE_SQL} THEN 1 END) FROM sessions s LEFT JOIN battles b ON b.session_id=s.id {where_sql} GROUP BY s.id ORDER BY s.started_at DESC LIMIT 1"),args,Self::session_row).optional()?)
    }
    pub fn active_session(&self) -> Result<Option<Session>, AppError> {
        self.session_query("WHERE s.status='active'", &[])
    }
    pub fn session(&self, id: &str) -> Result<Session, AppError> {
        self.session_query("WHERE s.id=?1", &[&id])?
            .ok_or(AppError::SessionNotFound)
    }
    pub fn recent_sessions(&self, limit: usize) -> Result<Vec<Session>, AppError> {
        let c = self.open()?;
        let mut s=c.prepare(&format!("SELECT s.id,s.started_at,s.last_battle_at,s.ended_at,s.status,COUNT(b.arena_id),COUNT(CASE WHEN {STATS_ELIGIBLE_SQL} THEN 1 END) FROM sessions s LEFT JOIN battles b ON b.session_id=s.id GROUP BY s.id ORDER BY s.started_at DESC LIMIT ?1"))?;
        Ok(
            s.query_map(params![limit.clamp(1, 100) as i64], Self::session_row)?
                .collect::<Result<Vec<_>, _>>()?,
        )
    }
    pub fn start_session(&self) -> Result<Session, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();
        self.open()?.execute(
            "INSERT OR IGNORE INTO sessions(id,started_at,status)VALUES(?1,?2,'active')",
            params![id, now],
        )?;
        self.active_session()?.ok_or(AppError::NoActiveSession)
    }
    pub fn end_active_session(&self, status: &str) -> Result<Session, AppError> {
        let s = self.active_session()?.ok_or(AppError::NoActiveSession)?;
        let ended_at = Utc::now().timestamp_millis();
        let status = if status == "timed_out" {
            "timed_out"
        } else {
            "ended"
        };
        let connection = self.open()?;
        if s.battle_count == 0
            && connection.execute(
                "DELETE FROM sessions WHERE id=?1 AND status='active' AND NOT EXISTS(SELECT 1 FROM battles WHERE battles.session_id=sessions.id)",
                params![&s.id],
            )? == 1
        {
            let mut ended = s;
            ended.ended_at = Some(ended_at);
            ended.status = status.into();
            return Ok(ended);
        }
        connection.execute(
            "UPDATE sessions SET ended_at=?1,status=?2 WHERE id=?3",
            params![ended_at, status, &s.id],
        )?;
        drop(connection);
        self.session(&s.id)
    }
    pub fn auto_end_stale(&self, minutes: u32) -> Result<Option<Session>, AppError> {
        self.auto_end_stale_at(minutes, Utc::now().timestamp_millis(), || Ok(()))
    }
    pub(crate) fn auto_end_stale_at<F>(
        &self,
        minutes: u32,
        now: i64,
        before_end: F,
    ) -> Result<Option<Session>, AppError>
    where
        F: FnOnce() -> Result<(), AppError>,
    {
        let Some(s) = self.active_session()? else {
            return Ok(None);
        };
        let last_activity = s.last_battle_at.unwrap_or(s.started_at);
        let threshold = i64::from(minutes.clamp(15, 240)) * 60_000;
        if now.saturating_sub(last_activity) >= threshold {
            before_end()?;
            let connection = self.open()?;
            let ended_at = last_activity.saturating_add(threshold);
            if s.battle_count == 0
                && connection.execute(
                    "DELETE FROM sessions WHERE id=?1 AND status='active' AND COALESCE(last_battle_at,started_at)=?2 AND NOT EXISTS(SELECT 1 FROM battles WHERE battles.session_id=sessions.id)",
                    params![&s.id, last_activity],
                )? == 1
            {
                let mut ended = s;
                ended.ended_at = Some(ended_at);
                ended.status = "timed_out".into();
                return Ok(Some(ended));
            }
            let changed = connection.execute(
                "UPDATE sessions SET ended_at=?1,status='timed_out' WHERE id=?2 AND status='active' AND COALESCE(last_battle_at,started_at)=?3",
                params![ended_at, &s.id, last_activity],
            )?;
            drop(connection);
            return if changed == 1 {
                self.session(&s.id).map(Some)
            } else {
                Ok(None)
            };
        }
        Ok(None)
    }
    pub fn battle_by_id(&self, id: &str) -> Result<Option<Battle>, AppError> {
        Ok(self
            .query_battles(
                &format!("SELECT {BATTLE_COLUMNS} FROM battles WHERE arena_id=?1"),
                &[&id],
            )?
            .into_iter()
            .next())
    }

    /// Remaining and maximum health of the player's vehicle and whether no team mate dealt more
    /// damage, for the Battle screen.
    pub fn battle_own_extras(&self, battle_id: &str) -> Result<OwnBattleExtras, AppError> {
        let connection = self.open_read_only()?;
        Ok(connection
            .query_row(
                "SELECT b.health_remaining,b.max_health,t.damage_greater_count
                 FROM battles b LEFT JOIN team_relative_facts t ON t.battle_id=b.arena_id
                 WHERE b.arena_id=?1",
                [battle_id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, Option<i64>>(2)?.map(|greater| greater == 0),
                    ))
                },
            )
            .optional()?
            .unwrap_or((None, None, None)))
    }

    /// Both teams' totals from `battle_team_stats` (D17); `None` unless both sides are stored.
    /// Reads only aggregate columns, never `battle_roster`.
    pub fn team_comparison(&self, battle_id: &str) -> Result<Option<TeamComparison>, AppError> {
        let connection = self.open_read_only()?;
        let mut statement = connection.prepare(
            "SELECT side,damage_total,spotted_total,
                    CASE WHEN assist_radio_total IS NULL OR assist_track_total IS NULL THEN NULL
                         ELSE assist_radio_total+assist_track_total END,
                    kills_total,survivors,tier_min,tier_max,platoons
             FROM battle_team_stats WHERE battle_id=?1",
        )?;
        let mut own = None;
        let mut enemy = None;
        for row in statement.query_map([battle_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                TeamTotals {
                    damage: row.get(1)?,
                    spotted: row.get(2)?,
                    assist: row.get(3)?,
                    kills: row.get(4)?,
                    survivors: row.get(5)?,
                    tier_min: row.get(6)?,
                    tier_max: row.get(7)?,
                    platoons: row.get(8)?,
                },
            ))
        })? {
            let (side, totals) = row?;
            match side.as_str() {
                "own" => own = Some(totals),
                "enemy" => enemy = Some(totals),
                _ => {}
            }
        }
        Ok(own
            .zip(enemy)
            .map(|(own, enemy)| TeamComparison { own, enemy }))
    }

    pub fn session_battles(&self, id: &str) -> Result<Vec<Battle>, AppError> {
        self.query_battles(
            &format!(
                "SELECT {BATTLE_COLUMNS} FROM battles WHERE session_id=?1 ORDER BY battle_time"
            ),
            &[&id],
        )
    }
    /// Every stored battle on one map, newest first (E5-flare, "You on <map>").
    pub fn battles_on_map(&self, map_key: &str) -> Result<Vec<Battle>, AppError> {
        self.query_battles(
            &format!(
                "SELECT {BATTLE_COLUMNS} FROM battles WHERE map_key=?1 ORDER BY battle_time DESC,arena_id DESC"
            ),
            &[&map_key],
        )
    }
    pub fn battles_for_sessions(&self, session_ids: &[String]) -> Result<Vec<Battle>, AppError> {
        if session_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = (1..=session_ids.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let connection = self.open()?;
        let mut statement = connection.prepare(&format!(
            "SELECT {BATTLE_COLUMNS} FROM battles WHERE session_id IN ({placeholders}) ORDER BY session_id,battle_time"
        ))?;
        Ok(statement
            .query_map(params_from_iter(session_ids.iter()), Self::battle)?
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub fn historical_battles_excluding_session(
        &self,
        id: &str,
        limit: usize,
    ) -> Result<Vec<Battle>, AppError> {
        self.query_battles(&format!("SELECT {BATTLE_COLUMNS} FROM battles WHERE session_id IS NULL OR session_id!=?1 ORDER BY battle_time DESC LIMIT ?2"),&[&id,&(limit.clamp(1,1000) as i64)])
    }
    pub fn wipe(&self) -> Result<(), AppError> {
        let mut connection = self.open()?;
        connection.pragma_update(None, "secure_delete", "ON")?;
        let transaction = connection.transaction()?;
        for table in [
            "challenge_battles",
            "challenges",
            "battle_roster",
            "battle_team_stats",
            "team_relative_facts",
            "replay_files",
            "replay_file_skips",
            "legacy_battles",
            "battles",
        ] {
            transaction.execute(&format!("DELETE FROM {table}"), [])?;
        }
        transaction.execute("DELETE FROM sessions", [])?;
        transaction.execute(
            "DELETE FROM settings WHERE key NOT IN ('language','inactivity_minutes','onboarding_complete')",
            [],
        )?;
        transaction.commit()?;
        connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);VACUUM;")?;
        Ok(())
    }
}

fn avg(values: impl Iterator<Item = Option<i64>>) -> Option<f64> {
    let v = values.flatten().collect::<Vec<_>>();
    (!v.is_empty()).then(|| v.iter().map(|value| *value as f64).sum::<f64>() / v.len() as f64)
}
pub fn metrics(b: &[Battle]) -> SessionMetrics {
    let full = b
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .collect::<Vec<_>>();
    let outcomes = full
        .iter()
        .filter(|battle| battle.outcome.is_known())
        .collect::<Vec<_>>();
    let damage_sample_size = full.iter().filter(|x| x.damage.is_some()).count();
    let assist_sample_size = full.iter().filter(|x| x.assist.is_some()).count();
    let xp_sample_size = full.iter().filter(|x| x.original_xp.is_some()).count();
    let survival = full.iter().filter_map(|x| x.survived).collect::<Vec<_>>();
    SessionMetrics {
        battles: b.len(),
        analyzed_battles: full.len(),
        wins: outcomes
            .iter()
            .filter(|battle| battle.outcome == BattleOutcome::Victory)
            .count(),
        win_rate: (!outcomes.is_empty()).then(|| {
            outcomes
                .iter()
                .filter(|battle| battle.outcome == BattleOutcome::Victory)
                .count() as f64
                / outcomes.len() as f64
                * 100.0
        }),
        outcome_sample_size: outcomes.len(),
        average_damage: avg(full.iter().map(|x| x.damage)),
        damage_sample_size,
        average_assist: avg(full.iter().map(|x| x.assist)),
        assist_sample_size,
        average_blocked: avg(full.iter().map(|x| x.blocked)),
        average_frags: avg(full.iter().map(|x| x.frags)),
        // D8 / audit M11: XP averages use XP without premium.
        average_xp: avg(full.iter().map(|x| x.original_xp)),
        xp_sample_size,
        survival_rate: (!survival.is_empty()).then(|| {
            survival.iter().filter(|x| **x).count() as f64 / survival.len() as f64 * 100.0
        }),
        survival_sample_size: survival.len(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tempfile::tempdir;

    /// A separate connection for test setup and inspection, so a test never holds the shared
    /// write connection while it calls `Database` methods.
    fn raw_connection(database: &Database) -> Connection {
        let connection = Connection::open(&database.path).unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")
            .unwrap();
        connection
    }

    /// Runs only the pre-v10 chain on a legacy file (tests of the old chain, which v2.0 uses on
    /// the working copy during the v10 rebuild).
    fn legacy_v9(path: PathBuf) -> Database {
        let mut connection = Connection::open(&path).unwrap();
        Database::migrate_legacy_to_v9(&mut connection).unwrap();
        Database::with_path(path)
    }

    /// A v9 database with `present + missing` battles. The first `present` have a replay file in
    /// `replay_dir`; the others point to a replay that no longer exists. Battle `legacy-0` belongs
    /// to the manually ended session `manual`, `legacy-1` to the timed-out session `auto` and
    /// `legacy-2` to the session `live`, still active when the rebuild starts.
    fn legacy_with_battles(
        path: &std::path::Path,
        replay_dir: &std::path::Path,
        present: usize,
        missing: usize,
    ) -> Database {
        fs::create_dir_all(replay_dir).unwrap();
        let database = legacy_v9(path.to_path_buf());
        raw_session(&database, "manual", "ended");
        raw_session(&database, "auto", "timed_out");
        raw_session(&database, "live", "active");
        raw_connection(&database)
            .execute(
                "UPDATE sessions SET last_battle_at=1700000002000 WHERE id='live'",
                [],
            )
            .unwrap();
        database.set_setting("language", "pl").unwrap();
        let connection = raw_connection(&database);
        for index in 0..present + missing {
            let id = format!("legacy-{index}");
            let session = match index {
                0 => Some("manual"),
                1 => Some("auto"),
                2 => Some("live"),
                _ => None,
            };
            connection
                .execute(
                    "INSERT INTO battles(arena_id,battle_time,vehicle_key,source_kind,data_quality,arena_unique_id,session_id,imported_at)
                     VALUES(?1,?2,'v','replay','full',?3,?4,1)",
                    params![id, 1_700_000_000_000_i64 + index as i64, 5_000 + index as i64, session],
                )
                .unwrap();
            let key = if index < present {
                let file = replay_dir.join(format!("replay-{index}.wotreplay"));
                fs::write(&file, b"synthetic").unwrap();
                crate::local_tracker::source_path_key(&fs::canonicalize(&file).unwrap())
            } else {
                format!("gone-{index}")
            };
            connection
                .execute(
                    "INSERT INTO replay_files(source_path_key,battle_id,file_size,file_mtime_ms,content_fingerprint,block_count,last_processed_at)
                     VALUES(?1,?2,9,1,'f',2,1)",
                    params![key, id],
                )
                .unwrap();
        }
        drop(connection);
        database
    }

    fn count(path: &std::path::Path, sql: &str) -> i64 {
        Connection::open(path)
            .unwrap()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn v9_with_player_data_is_rebuilt_as_new_v10_and_backup_keeps_everything() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let replay_dir = directory.path().join("replays");
        let legacy = legacy_with_battles(&path, &replay_dir, 20, 1);
        let suggestion =
            challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank);
        let challenge = legacy
            .start_challenge("manual", &suggestion, 1_700_000_100_000)
            .unwrap();
        raw_connection(&legacy)
            .execute(
                "INSERT INTO challenge_battles(challenge_id,battle_id,ordinal,battle_time,counted_at,metric_value,contribution_value,survived,success)
                 VALUES(?1,'legacy-0',1,1,2,1500,1500,1,1)",
                params![challenge.id],
            )
            .unwrap();
        drop(legacy);

        let database = Database::initialize_with(
            path.clone(),
            &crate::legacy::LegacySource {
                replay_dir: Some(replay_dir.clone()),
            },
        )
        .unwrap();

        let backup = crate::schema::backup_path(&path, 9);
        assert_eq!(count(&backup, "PRAGMA user_version"), 9);
        assert_eq!(count(&backup, "SELECT COUNT(*) FROM battles"), 21);
        assert_eq!(count(&backup, "SELECT COUNT(*) FROM sessions"), 3);
        assert_eq!(
            count(
                &backup,
                "SELECT COUNT(*) FROM sessions WHERE id='live' AND status='active'"
            ),
            1
        );
        assert_eq!(count(&path, "PRAGMA user_version"), 10);
        assert_eq!(count(&path, "SELECT COUNT(*) FROM battles"), 0);
        // Every finished session moves (author's correction of 2026-11-12); the session active
        // at the rebuild is closed as timed out at its last activity and moves too.
        assert_eq!(
            count(
                &path,
                "SELECT COUNT(*) FROM sessions WHERE id='manual' AND status='ended' AND ended_at=1700000060000"
            ),
            1
        );
        assert_eq!(
            count(
                &path,
                "SELECT COUNT(*) FROM sessions WHERE id='auto' AND status='timed_out' AND ended_at=1700000060000"
            ),
            1
        );
        assert_eq!(
            count(
                &path,
                "SELECT COUNT(*) FROM sessions WHERE id='live' AND status='timed_out' AND ended_at=1700000002000 AND last_battle_at=1700000002000"
            ),
            1
        );
        assert_eq!(count(&path, "SELECT COUNT(*) FROM sessions"), 3);
        assert!(database.active_session().unwrap().is_none());
        assert_eq!(count(&path, "SELECT COUNT(*) FROM challenges"), 1);
        assert_eq!(count(&path, "SELECT COUNT(*) FROM challenge_battles"), 1);
        assert_eq!(count(&path, "SELECT COUNT(*) FROM legacy_battles"), 21);
        assert_eq!(
            count(
                &path,
                "SELECT COUNT(*) FROM legacy_battles WHERE arena_id='legacy-0' AND session_id='manual' AND arena_unique_id=5000"
            ),
            1
        );
        assert_eq!(
            count(
                &path,
                "SELECT COUNT(*) FROM legacy_battles WHERE arena_id='legacy-1' AND session_id='auto'"
            ),
            1
        );
        assert_eq!(
            count(
                &path,
                "SELECT COUNT(*) FROM legacy_battles WHERE arena_id='legacy-2' AND session_id='live'"
            ),
            1
        );
        assert_eq!(
            count(
                &path,
                "SELECT COUNT(*) FROM legacy_battles WHERE arena_id='legacy-3' AND session_id IS NULL"
            ),
            1
        );
        assert_eq!(
            database.get_setting("language").unwrap().as_deref(),
            Some("pl")
        );
        let active = database.active_challenge().unwrap().unwrap();
        assert_eq!(active.id, challenge.id);
        assert_eq!(active.battles.len(), 1);
        let report = database.rebuild_report().unwrap().unwrap();
        assert_eq!(report.legacy_battles, 21);
        assert_eq!(report.missing_at_start, 1);
        assert_eq!(report.legacy_version, 9);
        assert_eq!(report.state, crate::legacy::RebuildState::Pending);
        assert_eq!(
            database.diagnostics_read_only().unwrap().rebuild_report,
            Some(report)
        );
        for leftover in ["legacy-work.sqlite3", "v10-build.sqlite3"] {
            assert!(
                !crate::schema::sibling(&path, leftover).exists(),
                "{leftover}"
            );
        }
        assert!(database.integrity_report().unwrap().is_healthy());

        // v10 → v10 is idempotent: nothing is rebuilt again and the backup is unchanged.
        drop(database);
        let reopened = Database::initialize(path.clone()).unwrap();
        assert_eq!(count(&path, "SELECT COUNT(*) FROM legacy_battles"), 21);
        assert_eq!(count(&backup, "SELECT COUNT(*) FROM battles"), 21);
        assert!(reopened.rebuild_report().unwrap().is_some());
    }

    #[test]
    fn p8_gate_refuses_the_rebuild_above_five_percent_and_leaves_v9_untouched() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let replay_dir = directory.path().join("replays");
        drop(legacy_with_battles(&path, &replay_dir, 47, 3));
        let source = crate::legacy::LegacySource {
            replay_dir: Some(replay_dir),
        };
        assert!(matches!(
            Database::initialize_with(path.clone(), &source),
            Err(AppError::LegacyRebuildBlocked {
                missing: 3,
                total: 50
            })
        ));
        assert_eq!(count(&path, "PRAGMA user_version"), 9);
        assert_eq!(count(&path, "SELECT COUNT(*) FROM battles"), 50);
        assert_eq!(count(&path, "SELECT COUNT(*) FROM sessions"), 3);
        for leftover in ["legacy-work.sqlite3", "v10-build.sqlite3"] {
            assert!(
                !crate::schema::sibling(&path, leftover).exists(),
                "{leftover}"
            );
        }
        // Refused again on the next start; nothing changes.
        assert!(Database::initialize_with(path.clone(), &source).is_err());
        assert_eq!(count(&path, "SELECT COUNT(*) FROM battles"), 50);
    }

    #[test]
    fn p8_gate_allows_exactly_five_percent_and_counts_all_missing_without_a_source() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let replay_dir = directory.path().join("replays");
        drop(legacy_with_battles(&path, &replay_dir, 38, 2));
        let blocked = Database::initialize(path.clone());
        assert!(matches!(
            blocked,
            Err(AppError::LegacyRebuildBlocked {
                missing: 40,
                total: 40
            })
        ));
        let database = Database::initialize_with(
            path.clone(),
            &crate::legacy::LegacySource {
                replay_dir: Some(replay_dir),
            },
        )
        .unwrap();
        assert_eq!(
            database.rebuild_report().unwrap().unwrap().missing_at_start,
            2
        );
    }

    /// Adds `count` partial rows (early alpha: no replay file row, no `arenaUniqueID`) to the
    /// session `session`, or to none.
    fn add_partial_rows(database: &Database, count: usize, session: Option<&str>) {
        let connection = raw_connection(database);
        for index in 0..count {
            connection
                .execute(
                    "INSERT INTO battles(arena_id,battle_time,vehicle_key,source_kind,data_quality,session_id,imported_at)
                     VALUES(?1,?2,'v','replay','partial',?3,1)",
                    params![
                        format!("partial-{}-{index}", session.unwrap_or("none")),
                        1_600_000_000_000_i64 + index as i64,
                        session
                    ],
                )
                .unwrap();
        }
    }

    /// Author's decision of 2026-11-13 (spec 20 §9, P8): the gate counts full battles only.
    /// Partial rows are in neither the numerator nor the denominator and stay in the backup.
    #[test]
    fn p8_gate_ignores_partial_rows_and_starts_when_every_full_battle_has_a_replay() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let replay_dir = directory.path().join("replays");
        let legacy = legacy_with_battles(&path, &replay_dir, 10, 0);
        add_partial_rows(&legacy, 40, None);
        drop(legacy);
        let database = Database::initialize_with(
            path.clone(),
            &crate::legacy::LegacySource {
                replay_dir: Some(replay_dir),
            },
        )
        .unwrap();
        assert_eq!(count(&path, "PRAGMA user_version"), 10);
        // Only full battles are mapped for the import; partial rows stay in the backup.
        assert_eq!(count(&path, "SELECT COUNT(*) FROM legacy_battles"), 10);
        assert_eq!(
            count(
                &crate::schema::backup_path(&path, 9),
                "SELECT COUNT(*) FROM battles"
            ),
            50
        );
        let report = database.rebuild_report().unwrap().unwrap();
        assert_eq!(report.legacy_battles, 10);
        assert_eq!(report.missing_at_start, 0);
        assert_eq!(report.skipped_partial, 40);
    }

    #[test]
    fn p8_gate_still_refuses_above_five_percent_of_full_battles_whatever_the_partial_rows() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let replay_dir = directory.path().join("replays");
        // 1 of 10 full battles (10%) has no replay; among all 110 rows that would be under 5%
        // if the 100 partial rows counted as present, and far above if they counted as missing.
        let legacy = legacy_with_battles(&path, &replay_dir, 9, 1);
        add_partial_rows(&legacy, 100, None);
        let connection = raw_connection(&legacy);
        for index in 0..100 {
            let file = replay_dir.join(format!("partial-{index}.wotreplay"));
            fs::write(&file, b"synthetic").unwrap();
            connection
                .execute(
                    "INSERT INTO replay_files(source_path_key,battle_id,file_size,file_mtime_ms,content_fingerprint,block_count,last_processed_at)
                     VALUES(?1,?2,9,1,'f',1,1)",
                    params![
                        crate::local_tracker::source_path_key(&fs::canonicalize(&file).unwrap()),
                        format!("partial-none-{index}")
                    ],
                )
                .unwrap();
        }
        drop(connection);
        drop(legacy);
        let source = crate::legacy::LegacySource {
            replay_dir: Some(replay_dir),
        };
        assert!(matches!(
            Database::initialize_with(path.clone(), &source),
            Err(AppError::LegacyRebuildBlocked {
                missing: 1,
                total: 10
            })
        ));
        assert_eq!(count(&path, "PRAGMA user_version"), 9);
        assert_eq!(count(&path, "SELECT COUNT(*) FROM battles"), 110);
    }

    /// Author's decision of 2026-11-13: a session that cannot get a battle back does not move
    /// to v10. At the rebuild: sessions without any full battle. A session that is the source
    /// of a challenge always moves, so challenges stay as they were.
    #[test]
    fn sessions_without_a_full_battle_do_not_move_to_v10() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let replay_dir = directory.path().join("replays");
        let legacy = legacy_with_battles(&path, &replay_dir, 5, 0);
        raw_session(&legacy, "only-partial", "ended");
        raw_session(&legacy, "no-battles", "timed_out");
        raw_session(&legacy, "challenge-source", "ended");
        add_partial_rows(&legacy, 3, Some("only-partial"));
        add_partial_rows(&legacy, 2, Some("challenge-source"));
        add_partial_rows(&legacy, 1, Some("manual"));
        let suggestion =
            challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank);
        legacy
            .start_challenge("challenge-source", &suggestion, 1_700_000_100_000)
            .unwrap();
        drop(legacy);
        let database = Database::initialize_with(
            path.clone(),
            &crate::legacy::LegacySource {
                replay_dir: Some(replay_dir),
            },
        )
        .unwrap();
        let ids = {
            let connection = Connection::open(&path).unwrap();
            let mut statement = connection
                .prepare("SELECT id FROM sessions ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(ids, ["auto", "challenge-source", "live", "manual"]);
        assert_eq!(
            count(
                &crate::schema::backup_path(&path, 9),
                "SELECT COUNT(*) FROM sessions"
            ),
            6
        );
        assert_eq!(count(&path, "SELECT COUNT(*) FROM challenges"), 1);
        let report = database.rebuild_report().unwrap().unwrap();
        assert_eq!(report.skipped_empty_sessions, 2);
        assert_eq!(report.skipped_partial, 6);
        assert!(database.integrity_report().unwrap().is_healthy());
    }

    /// After the import: a session whose full battles did not come back (no replay file or not
    /// a random battle) is removed and counted; sessions with a battle, challenge sources, the
    /// active session and sessions started after the rebuild stay.
    #[test]
    fn finishing_the_rebuild_removes_legacy_sessions_left_without_a_battle() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let replay_dir = directory.path().join("replays");
        let legacy = legacy_with_battles(&path, &replay_dir, 5, 0);
        raw_session(&legacy, "challenge-source", "ended");
        raw_connection(&legacy)
            .execute(
                "UPDATE battles SET session_id='challenge-source' WHERE arena_id='legacy-3'",
                [],
            )
            .unwrap();
        let suggestion =
            challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank);
        legacy
            .start_challenge("challenge-source", &suggestion, 1_700_000_100_000)
            .unwrap();
        drop(legacy);
        let database = Database::initialize_with(
            path.clone(),
            &crate::legacy::LegacySource {
                replay_dir: Some(replay_dir),
            },
        )
        .unwrap();
        assert_eq!(count(&path, "SELECT COUNT(*) FROM sessions"), 4);
        // The import brought back only the battle of `manual`; a new session started since.
        let mut returned = battle("legacy-0");
        returned.session_id = Some("manual".to_owned());
        database.insert_battle(&mut returned, false).unwrap();
        raw_connection(&database)
            .execute(
                "UPDATE legacy_battles SET state='matched' WHERE arena_id='legacy-0'",
                [],
            )
            .unwrap();
        raw_session(&database, "new-live", "active");

        database.finish_legacy_rebuild().unwrap();

        let ids = {
            let connection = Connection::open(&path).unwrap();
            let mut statement = connection
                .prepare("SELECT id FROM sessions ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(ids, ["challenge-source", "manual", "new-live"]);
        let report = database.rebuild_report().unwrap().unwrap();
        assert_eq!(report.state, crate::legacy::RebuildState::Done);
        assert_eq!(report.skipped_empty_sessions, 2);
        assert_eq!(report.recalculated, 1);
        assert_eq!(report.without_replay, 4);
        assert_eq!(report.skipped_partial, 0);
        // A second call changes nothing.
        database.finish_legacy_rebuild().unwrap();
        assert_eq!(count(&path, "SELECT COUNT(*) FROM sessions"), 3);
        assert!(database.integrity_report().unwrap().is_healthy());
    }

    #[test]
    fn interrupted_rebuild_reuses_the_backup_and_cleans_temporary_files() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        let replay_dir = directory.path().join("replays");
        drop(legacy_with_battles(&path, &replay_dir, 2, 0));
        let backup = crate::schema::backup_path(&path, 9);
        {
            let connection = Connection::open(&path).unwrap();
            crate::schema::backup_into(&connection, &backup).unwrap();
        }
        // A crash after the backup left a half-built file and a working copy behind; the original
        // lost a battle afterwards (e.g. its write-ahead log was removed).
        fs::write(crate::schema::sibling(&path, "v10-build.sqlite3"), b"half").unwrap();
        fs::write(
            crate::schema::sibling(&path, "legacy-work.sqlite3"),
            b"half",
        )
        .unwrap();
        Connection::open(&path)
            .unwrap()
            .execute("DELETE FROM battles WHERE arena_id='legacy-1'", [])
            .unwrap();
        let database = Database::initialize_with(
            path.clone(),
            &crate::legacy::LegacySource {
                replay_dir: Some(replay_dir),
            },
        )
        .unwrap();
        assert_eq!(count(&path, "SELECT COUNT(*) FROM legacy_battles"), 2);
        assert_eq!(
            database.rebuild_report().unwrap().unwrap().legacy_battles,
            2
        );
        for leftover in ["legacy-work.sqlite3", "v10-build.sqlite3"] {
            assert!(
                !crate::schema::sibling(&path, leftover).exists(),
                "{leftover}"
            );
        }
    }

    #[test]
    fn empty_and_newer_databases() {
        let directory = tempdir().unwrap();
        let empty = directory.path().join("empty.sqlite3");
        fs::write(&empty, b"").unwrap();
        Database::initialize(empty.clone()).unwrap();
        assert_eq!(count(&empty, "PRAGMA user_version"), 10);
        assert_eq!(count(&empty, "SELECT COUNT(*) FROM settings"), 0);

        let newer = directory.path().join("newer.sqlite3");
        Database::initialize(newer.clone()).unwrap();
        Connection::open(&newer)
            .unwrap()
            .pragma_update(None, "user_version", 11)
            .unwrap();
        assert!(matches!(
            Database::initialize(newer.clone()),
            Err(AppError::DatabaseTooNew)
        ));
        assert_eq!(count(&newer, "PRAGMA user_version"), 11);
        assert!(!crate::schema::backup_path(&newer, 11).exists());
    }

    fn battle(id: &str) -> Battle {
        Battle {
            arena_id: id.into(),
            arena_unique_id: None,
            battle_time: Utc::now().timestamp_millis(),
            battle_time_source: None,
            vehicle_key: "tank".into(),
            wg_tank_id: None,
            tank_name: None,
            map_name: None,
            map_key: None,
            battle_type: Some("random".into()),
            gameplay_id: None,
            source_kind: SourceKind::Replay,
            data_quality: DataQuality::Partial,
            parser_version: None,
            feature_schema_version: None,
            game_version: None,
            data_completeness: None,
            replay_source: None,
            parser_error_code: None,
            outcome: BattleOutcome::Unknown,
            battle_pace: crate::models::strict_random_pace(),
            player_team: None,
            winner_team_raw: None,
            damage: None,
            assist: None,
            assist_spotting: None,
            assist_tracking: None,
            assist_stun: None,
            blocked: None,
            damage_received: None,
            frags: None,
            spots: None,
            shots: None,
            hits: None,
            direct_enemy_hits: None,
            direct_team_hits: None,
            penetrations: None,
            piercing_enemy_hits: None,
            explosion_hits: None,
            defense_points: None,
            moving_avg_damage: None,
            damage_rating: None,
            survived: None,
            xp: None,
            original_xp: None,
            duration_seconds: None,
            session_id: None,
        }
    }

    fn raw_insert(database: &Database, battle: &Battle) {
        let mut connection = raw_connection(database);
        let transaction = connection.transaction().unwrap();
        assert_eq!(Database::insert_battle_tx(&transaction, battle).unwrap(), 1);
        transaction.commit().unwrap();
    }

    fn raw_session(database: &Database, id: &str, status: &str) {
        raw_connection(database)
            .execute(
                "INSERT INTO sessions(id,started_at,ended_at,status) VALUES(?1,?2,?3,?4)",
                params![
                    id,
                    1_700_000_000_000_i64,
                    (status != "active").then_some(1_700_000_060_000_i64),
                    status
                ],
            )
            .unwrap();
    }

    fn eligible_team_facts() -> TeamRelativeFactsV1 {
        TeamRelativeFactsV1 {
            common_reason: None,
            team_player_count: Some(15),
            team_damage_total: Some(15_000),
            damage_greater_count: Some(1),
            damage_equal_count: Some(2),
            damage_reason: None,
            team_frags_total: Some(5),
            frags_reason: None,
            xp_greater_count: Some(2),
            xp_equal_count: Some(1),
            xp_reason: None,
        }
    }

    pub(crate) fn full_replay_battle(id: &str) -> Battle {
        let mut value = battle(id);
        value.data_quality = DataQuality::Full;
        value.parser_version = Some(7);
        value.feature_schema_version = Some(3);
        value.data_completeness = Some("results".into());
        value.replay_source = Some("replay_results".into());
        value.damage = Some(3_000);
        value.frags = Some(2);
        value.original_xp = Some(900);
        value.xp = Some(900);
        value
    }

    fn drop_battle_pace_columns(connection: &Connection) {
        for column in [
            "destroyed_team2_score",
            "destroyed_team1_score",
            "initial_team2_size",
            "initial_team1_size",
            "bonus_type_raw",
            "battle_type_raw",
            "finish_reason_raw",
            "battle_pace_resolver_version",
            "battle_pace_reason",
            "battle_pace_context",
        ] {
            connection
                .execute_batch(&format!("ALTER TABLE battles DROP COLUMN {column};"))
                .unwrap();
        }
    }

    fn legacy_pair(first_id: &str, second_id: &str) -> (Battle, Battle) {
        let mut first = battle(first_id);
        first.battle_time = 1_700_000_000_000;
        first.map_name = Some("Map".into());
        first.battle_type = Some("ctf".into());
        let mut second = first.clone();
        second.arena_id = second_id.into();
        (first, second)
    }

    fn challenge_suggestion(
        challenge_type: ChallengeType,
        vehicle_class: VehicleClass,
    ) -> ChallengeSuggestion {
        ChallengeSuggestion {
            suggestion_id: "b".repeat(64),
            challenge_type,
            vehicle_id: 42,
            vehicle_name: "Synthetic vehicle".into(),
            vehicle_class,
            target_battles: 5,
            required_successes: 4,
            frozen_baseline_value: 2_000.0,
            threshold_value: (challenge_type != ChallengeType::EffectiveSurvival).then_some(1_400),
            contribution_metric_kind: ChallengeContributionMetricKind::Damage,
            contribution_floor: (challenge_type == ChallengeType::EffectiveSurvival)
                .then_some(1_000),
            evidence_metric: if challenge_type == ChallengeType::EffectiveSurvival {
                crate::models::ChallengeEvidenceMetric::Survival
            } else {
                crate::models::ChallengeEvidenceMetric::Damage
            },
            evidence_delta: -20.0,
            evidence_normalized_strength: 2.0,
            baseline_confidence: BaselineConfidence::Useful,
            baseline_eligible_battles: 40,
            baseline_metric_eligible_battles: 40,
            baseline_version: "personal-baseline-v1".into(),
            baseline_cutoff: 1_700_000_000_000,
            baseline_window: BaselineWindowKind::Last100,
            insight_version: "session-deviation-v1".into(),
            challenge_engine_version: CHALLENGE_ENGINE_VERSION.into(),
            suggestion_version: "vehicle-challenge-suggestion-v1".into(),
            formula_version: "wn8-standard-v1".into(),
            expected_snapshot_version: "synthetic-v1".into(),
            expected_snapshot_sha256: "a".repeat(64),
            vehicle_class_source: VehicleClassSource::WgClientPackedCatalog,
            vehicle_class_resolution_state: VehicleClassResolutionState::Resolved,
            vehicle_class_resolver_version: "vehicle-class-resolver-v1".into(),
            vehicle_class_catalog_schema_version: 1,
            compact_descriptor_protocol_source:
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            frozen_damage_baseline: Some(2_000.0),
            frozen_survival_baseline: Some(40.0),
            frozen_combined_contribution_baseline: None,
            frozen_spotting_assist_baseline: Some(300.0),
            frozen_tracking_assist_baseline: Some(100.0),
            frozen_stun_assist_baseline: None,
        }
    }

    /// Spec 20 D17 / spec 25 §5.3: the Battle screen reads only team totals; the DTO carries no
    /// roster field, and a battle without both sides stored has no comparison.
    #[test]
    fn team_comparison_reads_only_aggregates_of_both_sides() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("teams.db");
        let database = Database::initialize(path.clone()).unwrap();
        let mut value = qualifying_battle("teams-1", 1_000, Some(2_000));
        database.insert_battle(&mut value, false).unwrap();
        assert_eq!(
            database.battle_by_id("teams-1").unwrap().unwrap().arena_id,
            "teams-1"
        );
        assert!(database.battle_by_id("missing").unwrap().is_none());
        assert!(database.team_comparison("teams-1").unwrap().is_none());
        assert_eq!(
            database.battle_own_extras("teams-1").unwrap(),
            (None, None, None)
        );
        assert_eq!(
            database.battle_own_extras("missing").unwrap(),
            (None, None, None)
        );
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE battles SET health_remaining=312,max_health=1950 WHERE arena_id='teams-1'",
                [],
            )
            .unwrap();
        assert_eq!(
            database.battle_own_extras("teams-1").unwrap(),
            (Some(312), Some(1_950), None)
        );
        let insert = "INSERT INTO battle_team_stats(battle_id,side,fact_schema_version,players,
              damage_total,kills_total,spotted_total,assist_radio_total,assist_track_total,
              survivors,tier_min,tier_max,tier_sum,class_light,class_medium,class_heavy,class_td,
              class_spg,unresolved_vehicles,platoons,platooned_players)
            VALUES('teams-1',?1,1,15,?2,?3,?4,?5,?6,?7,10,10,150,3,5,4,2,1,0,?8,?9)";
        connection
            .execute(
                insert,
                rusqlite::params!["own", 21_700, 15, 18, 6_000, 2_900, 9, 1, 2],
            )
            .unwrap();
        assert!(
            database.team_comparison("teams-1").unwrap().is_none(),
            "one side is not a comparison"
        );
        connection
            .execute(
                insert,
                rusqlite::params!["enemy", 14_400, 6, 11, 5_000, 2_300, 0, 1, 3],
            )
            .unwrap();
        let teams = database.team_comparison("teams-1").unwrap().unwrap();
        assert_eq!(teams.own.damage, Some(21_700));
        assert_eq!(teams.own.assist, Some(8_900));
        assert_eq!(teams.enemy.spotted, Some(11));
        assert_eq!(teams.enemy.survivors, Some(0));
        assert_eq!(
            (teams.own.tier_min, teams.own.tier_max),
            (Some(10), Some(10))
        );
        assert_eq!(teams.own.platoons, 1);
        let json = serde_json::to_value(&teams).unwrap();
        let mut keys = json["own"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        assert_eq!(
            keys,
            [
                "assist",
                "damage",
                "kills",
                "platoons",
                "spotted",
                "survivors",
                "tierMax",
                "tierMin"
            ]
        );
        for forbidden in [
            "slot",
            "typeCompDescr",
            "platoonGroup",
            "killerSlot",
            "isOwnPlayer",
            "roster",
        ] {
            assert!(!json.to_string().contains(forbidden), "{forbidden}");
        }
    }

    /// P4: the per-battle feedback baseline is two SQL aggregates instead of up to two
    /// `SELECT`s of 100 full rows, with the same result as the old row-based computation.
    #[test]
    fn feedback_baseline_aggregates_match_the_row_based_baseline() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("feedback.db")).unwrap();
        let mut all = Vec::new();
        // 105 battles on "tank-a": the oldest five have huge damage and fall outside the 100.
        for index in 0..105_i64 {
            let mut value =
                qualifying_battle(&format!("a-{index:03}"), 1_000 + index, Some(1_000 + index));
            value.vehicle_key = "tank-a".into();
            value.assist = Some(100 + index);
            if index < 5 {
                value.damage = Some(1_000_000);
            }
            if index % 10 == 7 {
                value.data_quality = DataQuality::Partial;
                value.feature_schema_version = None;
            }
            if index % 10 == 3 {
                value.damage = None;
            }
            all.push(value);
        }
        // Two battles on "tank-b": too few, so the baseline falls back to all other battles.
        for index in 0..2_i64 {
            let mut value = qualifying_battle(&format!("b-{index}"), 5_000 + index, Some(77));
            value.vehicle_key = "tank-b".into();
            value.assist = None;
            all.push(value);
        }
        for value in &mut all {
            database.insert_battle(value, false).unwrap();
        }
        let newest_first = |mut rows: Vec<Battle>| {
            rows.sort_by(|left, right| {
                right
                    .battle_time
                    .cmp(&left.battle_time)
                    .then_with(|| right.arena_id.cmp(&left.arena_id))
            });
            rows.truncate(100);
            rows
        };
        let target_a = all.iter().find(|b| b.arena_id == "a-104").unwrap().clone();
        let same_a = newest_first(
            all.iter()
                .filter(|b| b.vehicle_key == "tank-a" && b.arena_id != "a-104")
                .cloned()
                .collect(),
        );
        assert_eq!(
            database.feedback_baseline_for_battle(&target_a).unwrap(),
            FeedbackBaseline::from_battles(&same_a)
        );
        let target_b = all.iter().find(|b| b.arena_id == "b-1").unwrap().clone();
        let fallback = newest_first(
            all.iter()
                .filter(|b| b.arena_id != "b-1")
                .cloned()
                .collect(),
        );
        let expected_b = FeedbackBaseline::from_battles(&fallback);
        assert!(expected_b.battles >= 5);
        assert_eq!(
            database.feedback_baseline_for_battle(&target_b).unwrap(),
            expected_b
        );
        let empty = Database::initialize(directory.path().join("empty.db")).unwrap();
        assert_eq!(
            empty.feedback_baseline_for_battle(&target_a).unwrap(),
            FeedbackBaseline::default()
        );
    }

    fn qualifying_battle(id: &str, battle_time: i64, damage: Option<i64>) -> Battle {
        let mut value = battle(id);
        value.battle_time = battle_time;
        value.battle_time_source = Some("metadata".into());
        value.wg_tank_id = Some(42);
        value.data_quality = DataQuality::Full;
        value.feature_schema_version = Some(1);
        value.damage = damage;
        value.survived = Some(true);
        value
    }

    fn attempts_conversion_battle(id: &str, battle_time: i64) -> Battle {
        let mut value = battle(id);
        value.battle_time = battle_time;
        value.vehicle_key = "tank".into();
        value.wg_tank_id = Some(42);
        value.data_quality = DataQuality::Full;
        value.parser_version = Some(6);
        value.feature_schema_version = Some(2);
        value.data_completeness = Some("results".into());
        value.replay_source = Some("replay_results".into());
        value.gameplay_id = Some("ctf".into());
        value.damage = Some(500);
        value.shots = Some(10);
        value.hits = Some(6);
        value.direct_enemy_hits = Some(5);
        value.direct_team_hits = Some(1);
        value.penetrations = Some(4);
        value.piercing_enemy_hits = Some(4);
        value.explosion_hits = Some(0);
        value.battle_pace = BattlePaceRecord {
            battle_type_raw: Some(1),
            bonus_type_raw: Some(1),
            initial_team1_size: Some(15),
            initial_team2_size: Some(15),
            ..Default::default()
        };
        value
    }

    #[test]
    fn attempts_conversion_read_models_preserve_nulls_zeroes_and_stable_order() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("attempts-read.db")).unwrap();
        raw_session(&database, "attempts-session", "ended");
        let mut first = attempts_conversion_battle("a", 100);
        first.session_id = Some("attempts-session".into());
        first.direct_team_hits = Some(0);
        first.explosion_hits = None;
        let mut second = attempts_conversion_battle("b", 100);
        second.session_id = Some("attempts-session".into());
        raw_insert(&database, &first);
        raw_insert(&database, &second);

        let session = database
            .attempts_conversion_session_battles("attempts-session")
            .unwrap();
        assert_eq!(
            session
                .iter()
                .map(|battle| battle.arena_id.as_str())
                .collect::<Vec<_>>(),
            vec!["b", "a"]
        );
        assert_eq!(session[1].direct_team_hits, Some(0));
        assert_eq!(session[1].explosion_hits, None);
        let recent = database.attempts_conversion_recent_battles(10).unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(database.recent_battles(10).unwrap().len(), 2);
        assert!(matches!(
            database.attempts_conversion_session_battles("missing"),
            Err(AppError::SessionNotFound)
        ));
    }

    #[test]
    fn attempts_conversion_recent_window_does_not_backfill_older_rows() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("attempts-window.db")).unwrap();
        for index in 0..15 {
            let mut value =
                attempts_conversion_battle(&format!("window-{index:02}"), 1_000 - index);
            if index < 5 {
                value.data_quality = DataQuality::Partial;
            }
            raw_insert(&database, &value);
        }
        let selected = database.attempts_conversion_recent_battles(10).unwrap();
        assert_eq!(selected.len(), 10);
        assert_eq!(
            selected
                .iter()
                .filter(|battle| battle.data_quality == DataQuality::Partial)
                .count(),
            5
        );
        assert!(!selected.iter().any(|battle| battle.arena_id == "window-10"));
    }

    #[test]
    fn attempts_conversion_windows_are_exact_and_reads_do_not_mutate_membership() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("attempts-windows.db")).unwrap();
        raw_session(&database, "attempts-windows-session", "ended");
        for index in 0..105 {
            let mut value =
                attempts_conversion_battle(&format!("window-{index:03}"), 10_000 - index);
            if index < 3 {
                value.session_id = Some("attempts-windows-session".into());
            }
            raw_insert(&database, &value);
        }
        let before = database
            .session("attempts-windows-session")
            .unwrap()
            .battle_count;
        assert_eq!(
            database
                .attempts_conversion_recent_battles(10)
                .unwrap()
                .len(),
            10
        );
        assert_eq!(
            database
                .attempts_conversion_recent_battles(30)
                .unwrap()
                .len(),
            30
        );
        assert_eq!(
            database
                .attempts_conversion_recent_battles(100)
                .unwrap()
                .len(),
            100
        );
        assert_eq!(
            database
                .attempts_conversion_session_battles("attempts-windows-session")
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            database
                .session("attempts-windows-session")
                .unwrap()
                .battle_count,
            before
        );
        assert!(matches!(
            database.attempts_conversion_session_battles("x' OR 1=1 --"),
            Err(AppError::SessionNotFound)
        ));
    }

    #[test]
    fn challenge_schema_remains_integral_in_v10_and_enforces_one_active_slot() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("challenge-schema.db");
        let database = Database::initialize(path.clone()).unwrap();
        raw_session(&database, "source", "ended");
        let suggestion =
            challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank);
        let first = database
            .start_challenge("source", &suggestion, 1_700_000_100_123)
            .unwrap();
        assert_eq!(first.eligible_after, 1_700_000_101_000);
        assert!(matches!(
            database.start_challenge("source", &suggestion, 1_700_000_200_000),
            Err(AppError::ChallengeAlreadyActive)
        ));
        assert!(database.integrity_report().unwrap().is_healthy());
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            10
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM challenges WHERE status='active'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn challenge_migration_rolls_back_on_conflicting_schema() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("challenge-migration-rollback.db");
        let database = Database::initialize(path.clone()).unwrap();
        drop(database);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE challenge_battles;
                 DROP TABLE challenges;
                 DROP INDEX idx_battles_challenge_candidates;
                 CREATE TABLE challenges(bad TEXT) STRICT;
                 PRAGMA user_version=4;",
            )
            .unwrap();
        drop(connection);

        assert!(Database::initialize(path.clone()).is_err());
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            4
        );
        let created_child: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='challenge_battles')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!created_child);
    }

    #[test]
    fn challenge_v4_upgrade_is_idempotent_and_preserves_sessions_and_battles() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("challenge-v4-upgrade.db");
        let database = legacy_v9(path.clone());
        raw_session(&database, "legacy-session", "ended");
        let mut stored = qualifying_battle("legacy-battle", 1_700_000_100_000, Some(2_000));
        stored.session_id = Some("legacy-session".into());
        raw_insert(&database, &stored);
        drop(database);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE challenge_battles;
                 DROP TABLE challenges;
                 DROP INDEX idx_battles_challenge_candidates;
                 PRAGMA user_version=4;",
            )
            .unwrap();
        drop(connection);

        for _ in 0..2 {
            let reopened = legacy_v9(path.clone());
            assert_eq!(reopened.session("legacy-session").unwrap().battle_count, 1);
            assert_eq!(reopened.recent_battles(10).unwrap()[0].damage, Some(2_000));
            assert!(reopened.integrity_report().unwrap().is_healthy());
            drop(reopened);
        }
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            9
        );
    }

    #[test]
    fn abandoning_preserves_progress_allows_a_new_goal_and_updates_safe_diagnostics() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("challenge-abandon.db")).unwrap();
        raw_session(&database, "source", "ended");
        let suggestion =
            challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank);
        let first = database
            .start_challenge("source", &suggestion, 1_700_000_100_123)
            .unwrap();
        raw_insert(
            &database,
            &qualifying_battle("counted", first.eligible_after, Some(2_000)),
        );
        database
            .process_battle_for_active_challenge("counted")
            .unwrap();
        let abandoned = database.abandon_active_challenge().unwrap();
        assert_eq!(abandoned.status, ChallengeStatus::Abandoned);
        assert_eq!(
            (abandoned.completed_battles, abandoned.successful_battles),
            (1, 1)
        );

        let second = database
            .start_challenge("source", &suggestion, 1_700_000_200_123)
            .unwrap();
        assert_eq!(second.status, ChallengeStatus::Active);
        let diagnostics = database.challenge_diagnostic_state().unwrap();
        assert!(diagnostics.active_challenge);
        assert_eq!(
            diagnostics.active_challenge_type,
            Some(ChallengeType::DamageConsistency)
        );
        assert_eq!(
            diagnostics.challenge_counts_by_status.get("abandoned"),
            Some(&1)
        );
        database.abandon_active_challenge().unwrap();
        assert_eq!(
            database.challenge_history_page(0, 10).unwrap().items.len(),
            2
        );
        assert!(database.integrity_report().unwrap().is_healthy());
    }

    #[test]
    fn challenge_progress_is_future_only_exactly_once_and_persists_after_restart() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("challenge-progress.db");
        let database = Database::initialize(path.clone()).unwrap();
        raw_session(&database, "source", "ended");
        let challenge = database
            .start_challenge(
                "source",
                &challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank),
                1_700_000_100_123,
            )
            .unwrap();

        let before = qualifying_battle("before", challenge.eligible_after - 1, Some(2_000));
        raw_insert(&database, &before);
        database
            .process_battle_for_active_challenge(&before.arena_id)
            .unwrap();
        let missing = qualifying_battle("missing", challenge.eligible_after, None);
        raw_insert(&database, &missing);
        database
            .process_battle_for_active_challenge(&missing.arena_id)
            .unwrap();
        let wrong = Battle {
            wg_tank_id: Some(43),
            ..qualifying_battle("wrong", challenge.eligible_after + 1, Some(2_000))
        };
        raw_insert(&database, &wrong);
        database
            .process_battle_for_active_challenge(&wrong.arena_id)
            .unwrap();

        let mut partial = qualifying_battle("partial", challenge.eligible_after + 2, Some(2_000));
        partial.data_quality = DataQuality::Partial;
        partial.feature_schema_version = None;
        raw_insert(&database, &partial);
        database
            .process_battle_for_active_challenge(&partial.arena_id)
            .unwrap();
        raw_connection(&database)
            .execute(
                "UPDATE battles SET data_quality='full',feature_schema_version=1,
                 outcome='draw',won=0,player_team=1,winner_team_raw=0
                 WHERE arena_id='partial'",
                [],
            )
            .unwrap();
        database
            .process_battle_for_active_challenge(&partial.arena_id)
            .unwrap();
        database
            .process_battle_for_active_challenge(&partial.arena_id)
            .unwrap();
        let active = database.active_challenge().unwrap().unwrap();
        assert_eq!(
            (active.completed_battles, active.successful_battles),
            (1, 1)
        );
        assert_eq!(active.battles[0].ordinal, 1);
        assert_eq!(
            database
                .recent_battles(10)
                .unwrap()
                .into_iter()
                .find(|battle| battle.arena_id == "partial")
                .unwrap()
                .outcome,
            BattleOutcome::Draw
        );

        drop(database);
        let reopened = Database::initialize(path).unwrap();
        let active = reopened.active_challenge().unwrap().unwrap();
        assert_eq!(
            (active.completed_battles, active.successful_battles),
            (1, 1)
        );
        assert_eq!(active.threshold_value, Some(1_400));
        assert_eq!(active.vehicle_class, VehicleClass::HeavyTank);
        assert_eq!(active.baseline_cutoff, 1_700_000_000_000);
    }

    #[test]
    fn newer_parser_still_upgrades_an_existing_partial_replay() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("partial-upgrade.db")).unwrap();
        let replay_state = ReplayFileState {
            source_path_key: "PARTIAL_SOURCE_KEY".into(),
            file_size: 500,
            file_mtime_ms: 1_700_000_200_000,
            content_fingerprint: "PARTIAL_CONTENT_FINGERPRINT".into(),
            block_count: 1,
        };
        let mut partial = qualifying_battle("partial-upgrade", 1_700_000_100_000, None);
        partial.data_quality = DataQuality::Partial;
        partial.parser_version = Some(4);
        partial.feature_schema_version = None;
        partial.frags = None;
        assert_eq!(
            database
                .upsert_replay(&mut partial, &replay_state, false)
                .unwrap(),
            ReplayUpsertOutcome::Inserted
        );

        let mut full = qualifying_battle("ignored-new-id", 1_700_000_100_000, Some(2_000));
        full.parser_version = Some(5);
        full.frags = Some(2);
        assert_eq!(
            database
                .upsert_replay(&mut full, &replay_state, false)
                .unwrap(),
            ReplayUpsertOutcome::Updated
        );
        let stored = database.recent_battles(1).unwrap().remove(0);
        assert_eq!(stored.arena_id, "partial-upgrade");
        assert_eq!(stored.data_quality, DataQuality::Full);
        assert_eq!(stored.parser_version, Some(5));
        assert_eq!(stored.frags, Some(2));
        assert_eq!(database.recent_battles(10).unwrap().len(), 1);
    }

    #[test]
    fn catch_up_uses_stable_order_finishes_and_never_counts_again() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("challenge-catch-up.db");
        let database = Database::initialize(path.clone()).unwrap();
        raw_session(&database, "source", "ended");
        let challenge = database
            .start_challenge(
                "source",
                &challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank),
                1_700_000_100_123,
            )
            .unwrap();
        for index in (0..7).rev() {
            raw_insert(
                &database,
                &qualifying_battle(
                    &format!("future-{index}"),
                    challenge.eligible_after + index * 1_000,
                    Some(if index < 4 { 2_000 } else { 100 }),
                ),
            );
        }
        drop(database);
        let reopened = Database::initialize(path).unwrap();
        let finished = reopened.catch_up_active_challenge().unwrap().unwrap();
        assert_eq!(finished.status, ChallengeStatus::Completed);
        assert_eq!(
            (finished.completed_battles, finished.successful_battles),
            (5, 4)
        );
        assert_eq!(
            finished
                .battles
                .iter()
                .map(|battle| battle.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        assert!(reopened.catch_up_active_challenge().unwrap().is_none());
        assert!(reopened.active_challenge().unwrap().is_none());
        assert_eq!(
            reopened
                .latest_finished_challenge()
                .unwrap()
                .unwrap()
                .completed_battles,
            5
        );
    }

    #[test]
    fn catch_up_pages_past_500_metric_ineligible_rows() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("challenge-catch-up-pages.db")).unwrap();
        raw_session(&database, "source", "ended");
        let challenge = database
            .start_challenge(
                "source",
                &challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank),
                1_700_000_100_123,
            )
            .unwrap();

        let mut connection = raw_connection(&database);
        let transaction = connection.transaction().unwrap();
        for index in 0..CHALLENGE_CATCH_UP_PAGE_SIZE {
            let battle = qualifying_battle(
                &format!("missing-{index:03}"),
                challenge.eligible_after + index as i64,
                None,
            );
            assert_eq!(
                Database::insert_battle_tx(&transaction, &battle).unwrap(),
                1
            );
        }
        for index in 0..5 {
            let mut battle = qualifying_battle(
                &format!("eligible-{index}"),
                challenge.eligible_after + CHALLENGE_CATCH_UP_PAGE_SIZE as i64 + index,
                Some(2_000),
            );
            battle.feature_schema_version = Some(if index % 2 == 0 { 1 } else { 2 });
            assert_eq!(
                Database::insert_battle_tx(&transaction, &battle).unwrap(),
                1
            );
        }
        transaction.commit().unwrap();

        let finished = database.catch_up_active_challenge().unwrap().unwrap();
        assert_eq!(finished.status, ChallengeStatus::Completed);
        assert_eq!(
            (finished.completed_battles, finished.successful_battles),
            (5, 5)
        );
        assert_eq!(finished.battles.len(), 5);
        assert!(
            finished
                .battles
                .iter()
                .all(|battle| battle.metric_value == 2_000)
        );
        let connection = raw_connection(&database);
        let battle_ids = connection
            .prepare(
                "SELECT battle_id FROM challenge_battles WHERE challenge_id=?1 ORDER BY ordinal",
            )
            .unwrap()
            .query_map(params![finished.id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            battle_ids,
            vec![
                "eligible-0",
                "eligible-1",
                "eligible-2",
                "eligible-3",
                "eligible-4",
            ]
        );
        assert!(database.catch_up_active_challenge().unwrap().is_none());
    }

    #[test]
    fn failed_and_abandoned_goals_are_terminal_for_live_and_catch_up() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("challenge-terminal.db")).unwrap();
        raw_session(&database, "source", "ended");
        let suggestion =
            challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank);
        let failed_goal = database
            .start_challenge("source", &suggestion, 1_700_000_100_123)
            .unwrap();
        for index in 0..6 {
            raw_insert(
                &database,
                &qualifying_battle(
                    &format!("failure-{index}"),
                    failed_goal.eligible_after + index,
                    Some(if index < 5 { 100 } else { 2_000 }),
                ),
            );
        }
        let failed = database.catch_up_active_challenge().unwrap().unwrap();
        assert_eq!(failed.status, ChallengeStatus::Failed);
        assert_eq!(
            (failed.completed_battles, failed.successful_battles),
            (5, 0)
        );
        assert!(database.catch_up_active_challenge().unwrap().is_none());
        assert!(
            database
                .process_battle_for_active_challenge("failure-5")
                .unwrap()
                .is_none()
        );

        let abandoned_goal = database
            .start_challenge("source", &suggestion, 1_700_000_200_123)
            .unwrap();
        database.abandon_active_challenge().unwrap();
        raw_insert(
            &database,
            &qualifying_battle("after-abandon", abandoned_goal.eligible_after, Some(2_000)),
        );
        assert!(database.catch_up_active_challenge().unwrap().is_none());
        assert!(
            database
                .process_battle_for_active_challenge("after-abandon")
                .unwrap()
                .is_none()
        );
        let history = database.challenge_history_page(0, 10).unwrap();
        assert_eq!(history.items.len(), 2);
        assert_eq!(history.items[0].status, ChallengeStatus::Abandoned);
        assert_eq!(history.items[1].status, ChallengeStatus::Failed);
    }

    #[test]
    fn concurrent_start_and_progress_keep_single_active_and_contiguous_ordinals() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("challenge-race.db")).unwrap();
        raw_session(&database, "source", "ended");
        let suggestion =
            challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank);
        let left_db = database.clone();
        let left_suggestion = suggestion.clone();
        let left = std::thread::spawn(move || {
            left_db.start_challenge("source", &left_suggestion, 1_700_000_100_123)
        });
        let right_db = database.clone();
        let right = std::thread::spawn(move || {
            right_db.start_challenge("source", &suggestion, 1_700_000_100_123)
        });
        let results = [left.join().unwrap(), right.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(AppError::ChallengeAlreadyActive)))
                .count(),
            1
        );
        let eligible_after = database.active_challenge().unwrap().unwrap().eligible_after;
        for index in 0..2 {
            raw_insert(
                &database,
                &qualifying_battle(
                    &format!("race-{index}"),
                    eligible_after + index,
                    Some(2_000),
                ),
            );
        }
        let first_db = database.clone();
        let first =
            std::thread::spawn(move || first_db.process_battle_for_active_challenge("race-0"));
        let second_db = database.clone();
        let second =
            std::thread::spawn(move || second_db.process_battle_for_active_challenge("race-1"));
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
        let active = database.active_challenge().unwrap().unwrap();
        assert_eq!(active.completed_battles, 2);
        assert_eq!(
            active
                .battles
                .iter()
                .map(|battle| battle.ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }
    #[test]
    fn dedupe_and_auto_session() {
        let d = tempdir().unwrap();
        let db = Database::initialize(d.path().join("a.db")).unwrap();
        let mut b = battle("x");
        assert!(db.insert_battle(&mut b, true).unwrap());
        assert!(!db.insert_battle(&mut b, true).unwrap());
        assert!(b.session_id.is_some());
    }
    #[test]
    fn missing_metrics_stay_missing() {
        let d = tempdir().unwrap();
        let db = Database::initialize(d.path().join("a.db")).unwrap();
        let mut b = battle("x");
        db.insert_battle(&mut b, false).unwrap();
        assert_eq!(db.recent_battles(1).unwrap()[0].damage, None);
        assert_eq!(metrics(&[b]).average_damage, None);
    }

    #[test]
    fn wn8_candidates_preserve_replay_rows_and_missing_fields_for_coverage() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("wn8.db")).unwrap();
        let mut full_replay = battle("full-replay");
        full_replay.data_quality = DataQuality::Full;
        full_replay.feature_schema_version = Some(1);
        full_replay.wg_tank_id = Some(33345);
        full_replay.damage = Some(1_000);
        full_replay.frags = Some(1);
        full_replay.spots = Some(1);
        full_replay.outcome = BattleOutcome::Victory;
        full_replay.defense_points = None;
        raw_insert(&database, &full_replay);
        raw_insert(&database, &battle("partial"));

        let candidates = database.wn8_candidates().unwrap();
        assert_eq!(candidates.len(), 2);
        let replay = candidates
            .iter()
            .find(|battle| battle.arena_id == "full-replay")
            .unwrap();
        assert_eq!(replay.source_kind, SourceKind::Replay);
        assert_eq!(replay.data_quality, DataQuality::Full);
        assert_eq!(replay.defense_points, None);
        assert!(candidates.iter().any(|battle| battle.arena_id == "partial"));
    }

    #[test]
    fn damage_sample_size_counts_only_present_values() {
        let mut current = (0..30)
            .map(|index| battle(&format!("partial-{index}")))
            .collect::<Vec<_>>();
        current[0].data_quality = DataQuality::Full;
        current[0].feature_schema_version = Some(3);
        current[0].damage = Some(2_000);
        let baseline = (0..10)
            .map(|index| {
                let mut value = battle(&format!("baseline-{index}"));
                value.data_quality = DataQuality::Full;
                value.feature_schema_version = Some(3);
                value.damage = Some(1_000);
                value
            })
            .collect::<Vec<_>>();

        let current_metrics = metrics(&current);
        let baseline_metrics = metrics(&baseline);
        let serialized = serde_json::to_value(&current_metrics).unwrap();
        assert_eq!(serialized["damageSampleSize"], 1);
        let feedback = crate::analysis::session_feedback(&current_metrics, Some(&baseline_metrics));
        assert!(
            !feedback
                .iter()
                .any(|item| item.code == "session_damage_up" || item.code == "session_damage_down")
        );
        assert!(
            feedback
                .iter()
                .all(|item| item.confidence != crate::models::Confidence::High)
        );
    }

    /// D4: the Rust predicate and its SQL form select exactly the same battles.
    #[test]
    fn stats_eligibility_sql_matches_the_rust_predicate() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("d4.sqlite3")).unwrap();
        let mut expected = std::collections::BTreeSet::new();
        let qualities = [
            DataQuality::Full,
            DataQuality::Partial,
            DataQuality::Ambiguous,
        ];
        let schemas = [None, Some(1), Some(3), Some(4)];
        let modes = [None, Some(1), Some(2), Some(7)];
        let mut index = 0;
        for quality in qualities {
            for schema in schemas {
                for battle_type in modes {
                    for bonus_type in modes {
                        index += 1;
                        let mut value = battle(&format!("case-{index:03}"));
                        value.battle_time = 1_000 + index;
                        value.data_quality = quality.clone();
                        value.feature_schema_version = schema;
                        value.battle_pace.battle_type_raw = battle_type;
                        value.battle_pace.bonus_type_raw = bonus_type;
                        if stats_eligibility(&value) {
                            expected.insert(value.arena_id.clone());
                        }
                        database.insert_battle(&mut value, false).unwrap();
                    }
                }
            }
        }
        let connection = database.open_read_only().unwrap();
        let mut statement = connection
            .prepare(&format!(
                "SELECT b.arena_id FROM battles b WHERE {STATS_ELIGIBLE_SQL}"
            ))
            .unwrap();
        let selected = statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<std::collections::BTreeSet<_>, _>>()
            .unwrap();
        assert_eq!(expected.len(), 2, "full x schema 1 and 3 x mode 1/1");
        assert_eq!(selected, expected);
    }

    /// D4: a non-random battle in a session changes no session metric and no analyzed count.
    #[test]
    fn non_random_battle_changes_no_session_metric() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("nr.sqlite3")).unwrap();
        let session = database.start_session().unwrap();
        let now = Utc::now().timestamp_millis();
        let eligible = |id: &str, time: i64, damage: i64| {
            let mut value = battle(id);
            value.battle_time = time;
            value.data_quality = DataQuality::Full;
            value.feature_schema_version = Some(3);
            value.outcome = BattleOutcome::Victory;
            value.damage = Some(damage);
            value.assist = Some(300);
            value.xp = Some(1_000);
            value.survived = Some(true);
            value
        };
        for (index, damage) in [1_000, 2_000].into_iter().enumerate() {
            let mut value = eligible(&format!("random-{index}"), now + index as i64, damage);
            database.insert_battle(&mut value, true).unwrap();
        }
        let before_battles = database.session_battles(&session.id).unwrap();
        let before = metrics(&before_battles);
        let before_session = database.recent_sessions(1).unwrap().remove(0);

        let mut other = eligible("training", now + 10, 9_000);
        other.outcome = BattleOutcome::Defeat;
        other.survived = Some(false);
        other.battle_pace.battle_type_raw = Some(2);
        other.battle_pace.bonus_type_raw = Some(2);
        database.insert_battle(&mut other, true).unwrap();
        let after_battles = database.session_battles(&session.id).unwrap();
        let after = metrics(&after_battles);
        let after_session = database.recent_sessions(1).unwrap().remove(0);

        assert_eq!(after_battles.len(), 3);
        assert_eq!(after.battles, 3);
        assert_eq!(after.analyzed_battles, before.analyzed_battles);
        assert_eq!(after.average_damage, before.average_damage);
        assert_eq!(after.average_assist, before.average_assist);
        assert_eq!(after.average_xp, before.average_xp);
        assert_eq!(after.win_rate, before.win_rate);
        assert_eq!(after.survival_rate, before.survival_rate);
        assert_eq!(
            after_session.analyzed_battle_count,
            before_session.analyzed_battle_count
        );
        assert_eq!(after_session.battle_count, 3);
    }

    #[test]
    fn partial_values_never_enter_session_aggregates() {
        let mut full = battle("full");
        full.data_quality = DataQuality::Full;
        full.feature_schema_version = Some(3);
        full.damage = Some(1_000);
        full.assist = Some(200);
        full.xp = Some(900);
        full.original_xp = Some(900);
        full.survived = Some(true);
        let mut partial = battle("partial");
        partial.damage = Some(0);
        partial.assist = Some(0);
        partial.xp = Some(0);
        partial.survived = Some(false);

        let result = metrics(&[full, partial]);

        assert_eq!(result.battles, 2);
        assert_eq!(result.analyzed_battles, 1);
        assert_eq!(result.average_damage, Some(1_000.0));
        assert_eq!(result.average_assist, Some(200.0));
        assert_eq!(result.average_xp, Some(900.0));
        assert_eq!(result.survival_rate, Some(100.0));
        assert_eq!(result.damage_sample_size, 1);
        assert_eq!(result.assist_sample_size, 1);
        assert_eq!(result.xp_sample_size, 1);
        assert_eq!(result.survival_sample_size, 1);
    }

    #[test]
    fn session_win_rate_counts_draws_as_zero_and_excludes_unknown_results() {
        let full = |id: &str, outcome: BattleOutcome| {
            let mut value = battle(id);
            value.data_quality = DataQuality::Full;
            value.feature_schema_version = Some(3);
            value.outcome = outcome;
            value
        };

        let victory = full("victory", BattleOutcome::Victory);
        let defeat = full("defeat", BattleOutcome::Defeat);
        let draw = full("draw", BattleOutcome::Draw);
        let unknown = full("unknown", BattleOutcome::Unknown);

        let victory_only = metrics(std::slice::from_ref(&victory));
        assert_eq!(victory_only.win_rate, Some(100.0));
        assert_eq!(victory_only.outcome_sample_size, 1);

        let defeat_only = metrics(std::slice::from_ref(&defeat));
        assert_eq!(defeat_only.win_rate, Some(0.0));
        assert_eq!(defeat_only.outcome_sample_size, 1);

        let draw_only = metrics(std::slice::from_ref(&draw));
        assert_eq!(draw_only.win_rate, Some(0.0));
        assert_eq!(draw_only.outcome_sample_size, 1);

        let victory_and_draw = metrics(&[victory.clone(), draw.clone()]);
        assert_eq!(victory_and_draw.win_rate, Some(50.0));
        assert_eq!(victory_and_draw.outcome_sample_size, 2);

        let all_known = metrics(&[victory.clone(), defeat, draw]);
        assert!((all_known.win_rate.unwrap() - 100.0 / 3.0).abs() < 1e-12);
        assert_eq!(all_known.outcome_sample_size, 3);

        let with_unknown = metrics(&[victory, unknown]);
        assert_eq!(with_unknown.battles, 2);
        assert_eq!(with_unknown.analyzed_battles, 2);
        assert_eq!(with_unknown.win_rate, Some(100.0));
        assert_eq!(with_unknown.outcome_sample_size, 1);
    }

    #[test]
    fn sqlite_round_trips_every_outcome_across_restart() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("outcome-round-trip.db");
        let database = Database::initialize(path.clone()).unwrap();
        for (id, outcome) in [
            ("victory", BattleOutcome::Victory),
            ("defeat", BattleOutcome::Defeat),
            ("draw", BattleOutcome::Draw),
            ("unknown", BattleOutcome::Unknown),
        ] {
            let mut value = battle(id);
            value.data_quality = DataQuality::Full;
            value.outcome = outcome;
            raw_insert(&database, &value);
        }
        drop(database);

        let reopened = Database::initialize(path).unwrap();
        let outcomes = reopened
            .recent_battles(10)
            .unwrap()
            .into_iter()
            .map(|battle| (battle.arena_id, battle.outcome))
            .collect::<HashMap<_, _>>();
        assert_eq!(outcomes["victory"], BattleOutcome::Victory);
        assert_eq!(outcomes["defeat"], BattleOutcome::Defeat);
        assert_eq!(outcomes["draw"], BattleOutcome::Draw);
        assert_eq!(outcomes["unknown"], BattleOutcome::Unknown);
    }

    #[test]
    fn sqlite_outcome_and_legacy_won_invariant_detects_every_invalid_pair() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("outcome-won-invariant.db");
        let database = Database::initialize(path).unwrap();
        for (id, outcome) in [
            ("victory", BattleOutcome::Victory),
            ("defeat", BattleOutcome::Defeat),
            ("draw", BattleOutcome::Draw),
            ("unknown", BattleOutcome::Unknown),
        ] {
            let mut value = battle(id);
            value.data_quality = DataQuality::Full;
            value.outcome = outcome;
            raw_insert(&database, &value);
        }

        let connection = raw_connection(&database);
        let violation_count = || {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM battles
                     WHERE NOT (
                       (outcome='victory' AND won=1)
                       OR (outcome IN ('defeat','draw') AND won=0)
                       OR (outcome='unknown' AND won IS NULL)
                     )",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap()
        };
        assert_eq!(violation_count(), 0);

        for (outcome, won) in [
            ("victory", Some(0_i64)),
            ("defeat", Some(1_i64)),
            ("draw", Some(1_i64)),
            ("unknown", Some(0_i64)),
            ("unknown", Some(1_i64)),
        ] {
            connection
                .execute(
                    "UPDATE battles SET outcome=?1,won=?2 WHERE arena_id='victory'",
                    params![outcome, won],
                )
                .unwrap();
            assert_eq!(violation_count(), 1, "{outcome} + {won:?}");
            connection
                .execute(
                    "UPDATE battles SET outcome='victory',won=1 WHERE arena_id='victory'",
                    [],
                )
                .unwrap();
            assert_eq!(violation_count(), 0);
        }
    }

    #[test]
    fn session_metrics_refresh_after_partial_is_upgraded_without_counting_remaining_partial() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("session-refresh.db");
        let database = Database::initialize(path.clone()).unwrap();
        let base_time = Utc::now().timestamp_millis();
        let state = |key: &str, size: i64| ReplayFileState {
            source_path_key: key.into(),
            file_size: size,
            file_mtime_ms: base_time + size,
            content_fingerprint: format!("fingerprint-{key}-{size}"),
            block_count: if size > 1 { 2 } else { 1 },
        };

        let mut battle_a = battle("battle-a");
        battle_a.battle_time = base_time;
        battle_a.data_quality = DataQuality::Full;
        battle_a.feature_schema_version = Some(3);
        battle_a.damage = Some(6_568);
        battle_a.assist = Some(318);
        battle_a.xp = Some(1_377);
        battle_a.original_xp = Some(1_377);
        battle_a.survived = Some(true);
        database
            .upsert_replay(&mut battle_a, &state("a", 2), true)
            .unwrap();

        let mut battle_b = battle("battle-b");
        battle_b.battle_time = base_time + 1_000;
        let partial_state = state("b", 1);
        database
            .upsert_replay(&mut battle_b, &partial_state, true)
            .unwrap();
        let session_id = battle_b.session_id.clone().unwrap();

        let mut completed_b = battle_b.clone();
        completed_b.data_quality = DataQuality::Full;
        completed_b.feature_schema_version = Some(3);
        completed_b.damage = Some(3_022);
        completed_b.assist = Some(100);
        completed_b.xp = Some(1_000);
        completed_b.original_xp = Some(1_000);
        completed_b.survived = Some(false);
        assert_eq!(
            database
                .upsert_replay(&mut completed_b, &state("b", 2), true)
                .unwrap(),
            ReplayUpsertOutcome::Updated
        );

        let mut battle_c = battle("battle-c");
        battle_c.battle_time = base_time + 2_000;
        database
            .upsert_replay(&mut battle_c, &state("c", 1), true)
            .unwrap();

        let session = database.active_session().unwrap().unwrap();
        assert_eq!(session.id, session_id);
        assert_eq!(session.battle_count, 3);
        assert_eq!(session.analyzed_battle_count, 2);
        let current_metrics = metrics(&database.session_battles(&session_id).unwrap());
        assert_eq!(current_metrics.battles, 3);
        assert_eq!(current_metrics.analyzed_battles, 2);
        assert_eq!(current_metrics.average_damage, Some(4_795.0));
        assert_eq!(current_metrics.damage_sample_size, 2);
        assert_eq!(current_metrics.assist_sample_size, 2);
        assert_eq!(current_metrics.xp_sample_size, 2);
        assert_eq!(current_metrics.survival_sample_size, 2);
        let serialized = serde_json::to_value(&current_metrics).unwrap();
        assert_eq!(serialized["analyzedBattles"], 2);
        assert_eq!(
            crate::analysis::session_feedback(&current_metrics, None)[0].sample_size,
            2
        );

        drop(database);
        let reopened = Database::initialize(path).unwrap();
        let reopened_session = reopened.active_session().unwrap().unwrap();
        assert_eq!(reopened_session.battle_count, 3);
        assert_eq!(reopened_session.analyzed_battle_count, 2);
        let reopened_metrics = metrics(&reopened.session_battles(&session_id).unwrap());
        assert_eq!(reopened_metrics, current_metrics);
    }

    #[test]
    fn partial_to_full_draw_is_one_persistent_battle_and_one_session_count() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("draw-upsert.db");
        let database = Database::initialize(path.clone()).unwrap();
        let base_time = Utc::now().timestamp_millis();
        let state = |size: i64, blocks: i64| ReplayFileState {
            source_path_key: "same-replay".into(),
            file_size: size,
            file_mtime_ms: base_time + size,
            content_fingerprint: format!("draw-{size}-{blocks}"),
            block_count: blocks,
        };

        let mut partial = battle("draw-battle");
        partial.battle_time = base_time;
        assert_eq!(
            database
                .upsert_replay(&mut partial, &state(100, 1), true)
                .unwrap(),
            ReplayUpsertOutcome::Inserted
        );
        let session_id = partial.session_id.clone().unwrap();

        let mut completed = partial.clone();
        completed.data_quality = DataQuality::Full;
        completed.feature_schema_version = Some(1);
        completed.outcome = BattleOutcome::Draw;
        completed.player_team = Some(1);
        completed.winner_team_raw = Some(0);
        completed.damage = Some(1_234);
        assert_eq!(
            database
                .upsert_replay(&mut completed, &state(200, 2), true)
                .unwrap(),
            ReplayUpsertOutcome::Updated
        );

        let stored = database.session_battles(&session_id).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].outcome, BattleOutcome::Draw);
        assert_eq!(database.session(&session_id).unwrap().battle_count, 1);
        assert_eq!(metrics(&stored).win_rate, Some(0.0));

        drop(database);
        let reopened = Database::initialize(path).unwrap();
        let stored = reopened.session_battles(&session_id).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].outcome, BattleOutcome::Draw);
        assert_eq!(reopened.session(&session_id).unwrap().battle_count, 1);
    }

    #[test]
    fn cleanup_prefers_full_over_partial_for_same_arena_unique_id() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("cleanup.db")).unwrap();
        raw_connection(&database)
            .execute_batch("DROP INDEX idx_battles_arena_unique_id;")
            .unwrap();
        let (mut partial, mut full) = legacy_pair("partial", "full");
        partial.arena_unique_id = Some(42);
        partial.parser_version = Some(1);
        full.arena_unique_id = Some(42);
        full.data_quality = DataQuality::Full;
        full.parser_version = Some(2);
        full.damage = Some(3_000);
        raw_insert(&database, &partial);
        raw_insert(&database, &full);
        let connection = raw_connection(&database);
        connection.execute("INSERT INTO replay_files VALUES('partial-path','partial',1,1,'partial-fingerprint',1,1,NULL)", []).unwrap();
        connection
            .execute(
                "INSERT INTO replay_files VALUES('full-path','full',2,2,'full-fingerprint',2,2,NULL)",
                [],
            )
            .unwrap();
        drop(connection);

        let before = database.integrity_report().unwrap();
        assert_eq!(before.certain_duplicate_records, 1);
        let cleanup = database.cleanup_legacy_data().unwrap();

        assert_eq!(cleanup.merged_battles, 1);
        assert_eq!(cleanup.after.certain_duplicate_records, 0);
        let battles = database.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].data_quality, DataQuality::Full);
        assert_eq!(battles[0].damage, Some(3_000));
        assert_eq!(battles[0].parser_version, Some(2));
        assert_eq!(battles[0].arena_unique_id, Some(42));
        let replay_state: (i64, i64) = raw_connection(&database)
            .query_row(
                "SELECT COUNT(*),COUNT(DISTINCT battle_id) FROM replay_files",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(replay_state, (2, 1));
    }

    #[test]
    fn cleanup_preserves_complementary_metrics_from_full_duplicates() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("cleanup.db")).unwrap();
        let (mut first, mut second) = legacy_pair("first", "second");
        first.data_quality = DataQuality::Full;
        first.parser_version = Some(1);
        first.feature_schema_version = Some(1);
        first.parser_error_code = Some("vehicle_id_conflict".into());
        first.damage = Some(6_568);
        first.xp = Some(1_377);
        first.map_key = Some("synthetic_map_key".into());
        first.assist_spotting = Some(200);
        first.direct_enemy_hits = Some(12);
        first.piercing_enemy_hits = Some(9);
        second.data_quality = DataQuality::Full;
        second.wg_tank_id = Some(33_345);
        second.parser_version = Some(2);
        second.assist = Some(318);
        second.gameplay_id = Some("ctf".into());
        second.battle_time_source = Some("metadata".into());
        second.player_team = Some(1);
        second.winner_team_raw = Some(1);
        second.assist_tracking = Some(100);
        second.assist_stun = Some(18);
        second.original_xp = Some(1_377);
        second.direct_team_hits = Some(1);
        second.explosion_hits = Some(3);
        second.survived = Some(true);
        raw_insert(&database, &first);
        raw_insert(&database, &second);

        database.cleanup_legacy_data().unwrap();

        let battles = database.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].data_quality, DataQuality::Full);
        assert_eq!(battles[0].damage, Some(6_568));
        assert_eq!(battles[0].xp, Some(1_377));
        assert_eq!(battles[0].assist, Some(318));
        assert_eq!(battles[0].survived, Some(true));
        assert_eq!(battles[0].parser_version, Some(2));
        assert_eq!(battles[0].feature_schema_version, Some(1));
        assert_eq!(battles[0].wg_tank_id, None);
        assert_eq!(
            battles[0].parser_error_code.as_deref(),
            Some("vehicle_id_conflict")
        );
        assert_eq!(battles[0].map_key.as_deref(), Some("synthetic_map_key"));
        assert_eq!(battles[0].gameplay_id.as_deref(), Some("ctf"));
        assert_eq!(battles[0].battle_time_source.as_deref(), Some("metadata"));
        assert_eq!(battles[0].player_team, Some(1));
        assert_eq!(battles[0].winner_team_raw, Some(1));
        assert_eq!(battles[0].assist_spotting, Some(200));
        assert_eq!(battles[0].assist_tracking, Some(100));
        assert_eq!(battles[0].assist_stun, Some(18));
        assert_eq!(battles[0].original_xp, Some(1_377));
        assert_eq!(battles[0].direct_enemy_hits, Some(12));
        assert_eq!(battles[0].direct_team_hits, Some(1));
        assert_eq!(battles[0].piercing_enemy_hits, Some(9));
        assert_eq!(battles[0].explosion_hits, Some(3));
    }

    #[test]
    fn ambiguous_candidates_are_reported_but_never_merged() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("cleanup.db")).unwrap();
        let (mut first, mut second) = legacy_pair("first", "second");
        first.arena_unique_id = Some(100);
        second.arena_unique_id = Some(200);
        raw_insert(&database, &first);
        raw_insert(&database, &second);

        let before = database.integrity_report().unwrap();
        assert_eq!(before.certain_duplicate_records, 0);
        assert_eq!(before.ambiguous_candidate_groups, 1);
        let cleanup = database.cleanup_legacy_data().unwrap();

        assert_eq!(cleanup.merged_battles, 0);
        assert_eq!(cleanup.after.ambiguous_candidate_groups, 1);
        assert_eq!(database.recent_battles(10).unwrap().len(), 2);
    }

    #[test]
    fn legacy_candidates_without_map_are_left_ambiguous() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("cleanup.db")).unwrap();
        let (mut first, mut second) = legacy_pair("first", "second");
        first.map_name = None;
        second.map_name = None;
        raw_insert(&database, &first);
        raw_insert(&database, &second);

        let report = database.integrity_report().unwrap();
        let cleanup = database.cleanup_legacy_data().unwrap();

        assert_eq!(report.certain_duplicate_records, 0);
        assert_eq!(report.ambiguous_candidate_groups, 1);
        assert_eq!(cleanup.merged_battles, 0);
        assert_eq!(database.recent_battles(10).unwrap().len(), 2);
    }

    #[test]
    fn cleanup_is_idempotent() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("cleanup.db")).unwrap();
        let (first, second) = legacy_pair("first", "second");
        raw_insert(&database, &first);
        raw_insert(&database, &second);

        let first_run = database.cleanup_legacy_data().unwrap();
        let second_run = database.cleanup_legacy_data().unwrap();

        assert_eq!(first_run.merged_battles, 1);
        assert_eq!(second_run.merged_battles, 0);
        assert_eq!(second_run.deleted_empty_sessions, 0);
        assert_eq!(second_run.before, second_run.after);
    }

    #[test]
    fn cleanup_rolls_back_every_change_when_midpoint_fails() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("cleanup.db")).unwrap();
        let (first, second) = legacy_pair("first", "second");
        raw_insert(&database, &first);
        raw_insert(&database, &second);
        raw_session(&database, "empty-ended", "ended");

        let result = database.cleanup_legacy_data_with(|| Err(AppError::Tracker));

        assert!(matches!(result, Err(AppError::Tracker)));
        assert_eq!(database.recent_battles(10).unwrap().len(), 2);
        assert!(database.session("empty-ended").is_ok());
        assert_eq!(
            database
                .integrity_report()
                .unwrap()
                .certain_duplicate_records,
            1
        );
    }

    #[test]
    fn cleanup_removes_only_empty_non_active_sessions() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("cleanup.db")).unwrap();
        raw_session(&database, "empty-ended", "ended");
        raw_session(&database, "with-battle", "ended");
        raw_session(&database, "empty-active", "active");
        let mut stored = battle("stored");
        stored.session_id = Some("with-battle".into());
        raw_insert(&database, &stored);

        let cleanup = database.cleanup_legacy_data().unwrap();

        assert_eq!(cleanup.deleted_empty_sessions, 1);
        assert!(matches!(
            database.session("empty-ended"),
            Err(AppError::SessionNotFound)
        ));
        assert_eq!(database.session("empty-active").unwrap().battle_count, 0);
        assert_eq!(database.session("with-battle").unwrap().battle_count, 1);
    }

    #[test]
    fn ending_an_empty_active_session_does_not_create_history() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("sessions.db")).unwrap();
        let active = database.start_session().unwrap();

        let ended = database.end_active_session("ended").unwrap();

        assert_eq!(ended.id, active.id);
        assert_eq!(ended.status, "ended");
        assert!(database.active_session().unwrap().is_none());
        assert!(database.recent_sessions(10).unwrap().is_empty());
    }

    #[test]
    fn timing_out_an_empty_active_session_does_not_create_history() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("sessions.db")).unwrap();
        let active = database.start_session().unwrap();

        let ended = database
            .auto_end_stale_at(15, active.started_at + 16 * 60_000, || Ok(()))
            .unwrap()
            .unwrap();

        assert_eq!(ended.id, active.id);
        assert_eq!(ended.status, "timed_out");
        assert!(database.active_session().unwrap().is_none());
        assert!(database.recent_sessions(10).unwrap().is_empty());
    }

    #[test]
    fn healthy_database_reports_real_pragma_results() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("healthy.db")).unwrap();

        let report = database.integrity_report().unwrap();

        assert_eq!(report.integrity_check, ["ok"]);
        assert!(report.foreign_key_check.is_empty());
        assert_eq!(report.orphaned_records, 0);
        assert!(report.is_healthy());
    }

    #[test]
    fn foreign_key_check_reports_orphans_and_reopen_rejects_them() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("orphan.db");
        let database = Database::initialize(path.clone()).unwrap();
        let mut orphan = battle("orphan");
        orphan.session_id = Some("missing-session".into());
        let mut connection = raw_connection(&database);
        connection
            .pragma_update(None, "foreign_keys", "OFF")
            .unwrap();
        let transaction = connection.transaction().unwrap();
        assert_eq!(
            Database::insert_battle_tx(&transaction, &orphan).unwrap(),
            1
        );
        transaction.commit().unwrap();
        drop(connection);

        let report = database.integrity_report().unwrap();

        assert_eq!(report.orphaned_records, 1);
        assert_eq!(report.foreign_key_check.len(), 1);
        assert!(!report.is_healthy());
        assert!(matches!(
            Database::initialize(path),
            Err(AppError::DatabaseIntegrity)
        ));
    }

    #[test]
    fn cleanup_on_database_copy_preserves_source_and_valid_full_data() {
        let directory = tempdir().unwrap();
        let source_path = directory.path().join("source.sqlite3");
        let copy_path = directory.path().join("source.copy.sqlite3");
        let source = Database::initialize(source_path.clone()).unwrap();
        let (mut first, mut second) = legacy_pair("first", "second");
        first.data_quality = DataQuality::Full;
        first.damage = Some(6_568);
        second.data_quality = DataQuality::Full;
        second.assist = Some(318);
        raw_insert(&source, &first);
        raw_insert(&source, &second);
        raw_connection(&source)
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        fs::copy(&source_path, &copy_path).unwrap();

        let copy = Database::initialize(copy_path).unwrap();
        copy.cleanup_legacy_data().unwrap();

        assert_eq!(source.recent_battles(10).unwrap().len(), 2);
        let cleaned = copy.recent_battles(10).unwrap();
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0].damage, Some(6_568));
        assert_eq!(cleaned[0].assist, Some(318));
    }

    #[test]
    fn duplicate_does_not_create_an_empty_session() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("a.db")).unwrap();
        let mut original = battle("same");
        assert!(database.insert_battle(&mut original, false).unwrap());
        let mut duplicate = battle("same");
        assert!(!database.insert_battle(&mut duplicate, true).unwrap());
        assert!(database.active_session().unwrap().is_none());
    }

    #[test]
    fn active_session_survives_database_reopen() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("a.db");
        let database = Database::initialize(path.clone()).unwrap();
        let mut first = battle("first");
        database.insert_battle(&mut first, true).unwrap();
        let session_id = first.session_id.clone().unwrap();
        drop(database);

        let reopened = Database::initialize(path).unwrap();
        let active = reopened.active_session().unwrap().unwrap();
        assert_eq!(active.id, session_id);
        assert_eq!(active.battle_count, 1);
        assert_eq!(active.status, "active");
    }

    #[test]
    fn a_battle_after_timeout_starts_a_new_session() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("a.db")).unwrap();
        database.set_setting("inactivity_minutes", "15").unwrap();
        let mut first = battle("first");
        first.battle_time = 1_700_000_000_000;
        database.insert_battle(&mut first, true).unwrap();
        let first_session = first.session_id.clone().unwrap();
        let mut second = battle("second");
        second.battle_time = first.battle_time + 16 * 60_000;
        database.insert_battle(&mut second, true).unwrap();
        assert_ne!(second.session_id.as_deref(), Some(first_session.as_str()));
        assert_eq!(
            database.session(&first_session).unwrap().status,
            "timed_out"
        );
        assert_eq!(database.active_session().unwrap().unwrap().battle_count, 1);
    }

    #[test]
    fn auto_end_waits_for_the_whole_inactivity_limit() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("limit.db")).unwrap();
        let mut played = battle("played");
        played.battle_time = 1_700_000_000_000;
        database.insert_battle(&mut played, true).unwrap();
        let ended = database
            .auto_end_stale_at(15, played.battle_time + 14 * 60_000, || Ok(()))
            .unwrap();
        assert!(ended.is_none());
        assert!(database.active_session().unwrap().is_some());
        let ended = database
            .auto_end_stale_at(15, played.battle_time + 15 * 60_000, || Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(ended.ended_at, Some(played.battle_time + 15 * 60_000));
    }

    fn file_state(key: &str) -> ReplayFileState {
        ReplayFileState {
            source_path_key: key.into(),
            file_size: 10,
            file_mtime_ms: 1,
            content_fingerprint: "f".into(),
            block_count: 2,
        }
    }

    fn legacy_row(database: &Database, id: &str, unique: Option<i64>, session: Option<&str>) {
        raw_connection(database)
            .execute(
                "INSERT INTO legacy_battles(arena_id,arena_unique_id,session_id) VALUES(?1,?2,?3)",
                params![id, unique, session],
            )
            .unwrap();
    }

    fn legacy_state(database: &Database, id: &str) -> String {
        raw_connection(database)
            .query_row(
                "SELECT state FROM legacy_battles WHERE arena_id=?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn legacy_battle_is_claimed_only_with_the_same_identity_and_a_free_id() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("claim.db")).unwrap();
        raw_session(&database, "manual", "ended");

        // Same provisional id, other arena: a conflict, the battle keeps its own id.
        legacy_row(&database, "provisional-a", Some(1), Some("manual"));
        let mut other = full_replay_battle("provisional-a");
        other.arena_unique_id = Some(2);
        database
            .upsert_replay(&mut other, &file_state("a"), false)
            .unwrap();
        assert_eq!(legacy_state(&database, "provisional-a"), "conflict");
        assert_eq!(other.session_id, None);

        // Matching arena whose legacy id is already used by another row: a conflict too.
        legacy_row(&database, "taken", Some(3), Some("manual"));
        raw_insert(&database, &full_replay_battle("taken"));
        let mut incoming = full_replay_battle("incoming-b");
        incoming.arena_unique_id = Some(3);
        incoming.battle_time += 1;
        database
            .upsert_replay(&mut incoming, &file_state("b"), false)
            .unwrap();
        assert_eq!(legacy_state(&database, "taken"), "conflict");
        assert_eq!(incoming.arena_id, "incoming-b");

        // Same arena, free id: claimed with its manual session.
        legacy_row(&database, "legacy-c", Some(4), Some("manual"));
        let mut claimed = full_replay_battle("provisional-c");
        claimed.arena_unique_id = Some(4);
        claimed.battle_time += 2;
        database
            .upsert_replay(&mut claimed, &file_state("c"), true)
            .unwrap();
        assert_eq!(legacy_state(&database, "legacy-c"), "matched");
        assert_eq!(claimed.arena_id, "legacy-c");
        assert_eq!(claimed.session_id.as_deref(), Some("manual"));

        // Legacy row without arena id matched by its id, without a session.
        legacy_row(&database, "partial-d", None, None);
        let mut partial = full_replay_battle("partial-d");
        partial.battle_time += 3;
        database
            .upsert_replay(&mut partial, &file_state("d"), true)
            .unwrap();
        assert_eq!(legacy_state(&database, "partial-d"), "matched");
        assert_eq!(partial.session_id, None);
        assert!(database.active_session().unwrap().is_none());
    }

    #[test]
    fn auto_end_with_battles_uses_the_shared_connection_without_deadlock() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("d6.db")).unwrap();
        let mut played = battle("played");
        played.battle_time = 1_700_000_000_000;
        database.insert_battle(&mut played, true).unwrap();
        let ended = database
            .auto_end_stale_at(15, played.battle_time + 16 * 60_000, || Ok(()))
            .unwrap()
            .unwrap();
        assert_eq!(ended.status, "timed_out");
        assert_eq!(ended.battle_count, 1);
    }

    /// D6: one write connection shared by every clone; writers on several threads are
    /// serialized, and a nested lock on one thread is an error instead of a deadlock.
    #[test]
    fn one_write_connection_serializes_threads_and_refuses_nested_use() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("d6.db")).unwrap();
        let handles = (0..4)
            .map(|thread| {
                let database = database.clone();
                std::thread::spawn(move || {
                    for index in 0..25 {
                        database
                            .set_setting(&format!("k-{thread}-{index}"), "v")
                            .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }
        let count: i64 = raw_connection(&database)
            .query_row(
                "SELECT COUNT(*) FROM settings WHERE key LIKE 'k-%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 100);
        let held = database.open().unwrap();
        let nested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            database.get_setting("k-0-0").map(|_| ())
        }));
        assert!(nested.is_err() || nested.unwrap().is_err());
        drop(held);
        assert_eq!(database.get_setting("k-0-0").unwrap().as_deref(), Some("v"));
    }

    #[test]
    fn stale_timer_cannot_end_a_replacement_session() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("a.db")).unwrap();
        let mut old_battle = battle("old");
        old_battle.battle_time = 1_700_000_000_000;
        database.insert_battle(&mut old_battle, true).unwrap();
        let old_session_id = old_battle.session_id.clone().unwrap();
        let now = old_battle.battle_time + 16 * 60_000;
        let replacement_id = std::cell::RefCell::new(None);

        let ended = database
            .auto_end_stale_at(15, now, || {
                database.end_active_session("ended")?;
                let replacement = database.start_session()?;
                *replacement_id.borrow_mut() = Some(replacement.id);
                Ok(())
            })
            .unwrap();

        assert!(ended.is_none());
        assert_eq!(database.session(&old_session_id).unwrap().status, "ended");
        let replacement_id = replacement_id.into_inner().unwrap();
        assert_eq!(
            database.active_session().unwrap().unwrap().id,
            replacement_id
        );
    }

    #[test]
    fn migrates_v1_without_retaining_raw_payload() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        {
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL) STRICT;
              CREATE TABLE sessions(id TEXT PRIMARY KEY NOT NULL,started_at INTEGER NOT NULL,last_battle_at INTEGER,ended_at INTEGER,status TEXT NOT NULL) STRICT;
              CREATE TABLE battles(arena_id TEXT PRIMARY KEY NOT NULL,battle_time INTEGER NOT NULL,tank_id INTEGER NOT NULL,tank_name TEXT,map_name TEXT,battle_type TEXT,won INTEGER,damage INTEGER NOT NULL,assist INTEGER NOT NULL,blocked INTEGER NOT NULL,damage_received INTEGER NOT NULL,frags INTEGER NOT NULL,spots INTEGER NOT NULL,shots INTEGER NOT NULL,hits INTEGER NOT NULL,penetrations INTEGER NOT NULL,survived INTEGER,duration_seconds INTEGER,wn8 REAL,wnx REAL,session_id TEXT,raw_json TEXT NOT NULL,imported_at INTEGER NOT NULL) STRICT;") .unwrap();
            connection.execute("INSERT INTO battles VALUES('legacy',1700000000000,42,'Tank','Map','random',1,1000,200,300,400,1,2,10,8,7,1,400,1500,NULL,NULL,'RAW_SECRET_SHOULD_DISAPPEAR',1700000000000)", []).unwrap();
        }
        let database = legacy_v9(path.clone());
        let migrated = database.recent_battles(10).unwrap();
        assert_eq!(migrated.len(), 1);
        let connection = Connection::open(&path).unwrap();
        let source_kind: String = connection
            .query_row("SELECT source_kind FROM battles", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            source_kind, "tomato",
            "legacy chain keeps the old value; v10 never reads it"
        );
        let columns = connection
            .prepare("PRAGMA table_info(battles)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns.iter().any(|column| column == "raw_json"));
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            9
        );
        drop(connection);
        assert!(
            !String::from_utf8_lossy(&fs::read(path).unwrap())
                .contains("RAW_SECRET_SHOULD_DISAPPEAR")
        );
    }

    #[test]
    fn migrates_v2_to_v9_without_losing_battles() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("v2.db");
        {
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch("CREATE TABLE settings(key TEXT PRIMARY KEY NOT NULL,value TEXT NOT NULL) STRICT;
              CREATE TABLE sessions(id TEXT PRIMARY KEY NOT NULL,started_at INTEGER NOT NULL,last_battle_at INTEGER,ended_at INTEGER,status TEXT NOT NULL CHECK(status IN ('active','ended','timed_out'))) STRICT;
              CREATE TABLE battles(
                arena_id TEXT PRIMARY KEY NOT NULL,battle_time INTEGER NOT NULL,vehicle_key TEXT NOT NULL,wg_tank_id INTEGER,
                tank_name TEXT,map_name TEXT,battle_type TEXT,source_kind TEXT NOT NULL CHECK(source_kind IN ('replay','local_hybrid','tomato')),
                data_quality TEXT NOT NULL CHECK(data_quality IN ('full','partial','ambiguous')),won INTEGER CHECK(won IN(0,1) OR won IS NULL),
                damage INTEGER CHECK(damage>=0),assist INTEGER CHECK(assist>=0),blocked INTEGER CHECK(blocked>=0),damage_received INTEGER CHECK(damage_received>=0),
                frags INTEGER CHECK(frags>=0),spots INTEGER CHECK(spots>=0),shots INTEGER CHECK(shots>=0),hits INTEGER CHECK(hits>=0),penetrations INTEGER CHECK(penetrations>=0),
                survived INTEGER CHECK(survived IN(0,1) OR survived IS NULL),xp INTEGER CHECK(xp>=0),duration_seconds INTEGER CHECK(duration_seconds>=0),
                wn8 REAL,wn8_estimated INTEGER NOT NULL DEFAULT 0 CHECK(wn8_estimated IN(0,1)),wnx REAL,session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,imported_at INTEGER NOT NULL
              ) STRICT;
              INSERT INTO battles(arena_id,battle_time,vehicle_key,source_kind,data_quality,imported_at) VALUES('v2-battle',1700000000000,'tank','replay','partial',1700000000000);
              PRAGMA user_version=2;")
                .unwrap();
        }

        let database = legacy_v9(path.clone());
        let battles = database.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].arena_id, "v2-battle");
        assert_eq!(battles[0].arena_unique_id, None);
        assert_eq!(battles[0].feature_schema_version, None);
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            9
        );
        let replay_files_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='replay_files')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(replay_files_exists);
    }

    #[test]
    fn migrates_v3_to_v9_preserving_legacy_data_and_null_feature_version() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("v3.db");
        let database = legacy_v9(path.clone());
        let mut legacy = battle("v3-battle");
        legacy.data_quality = DataQuality::Full;
        legacy.damage = Some(2_345);
        legacy.assist = Some(321);
        raw_insert(&database, &legacy);
        drop(database);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "DROP TABLE challenge_battles;
                 DROP TABLE challenges;
                 DROP INDEX idx_battles_challenge_candidates;",
            )
            .unwrap();
        for column in [
            "outcome",
            "original_xp",
            "assist_stun",
            "assist_tracking",
            "assist_spotting",
            "winner_team_raw",
            "player_team",
            "battle_time_source",
            "gameplay_id",
            "map_key",
            "feature_schema_version",
        ] {
            connection
                .execute_batch(&format!("ALTER TABLE battles DROP COLUMN {column};"))
                .unwrap();
        }
        connection.pragma_update(None, "user_version", 3).unwrap();
        drop(connection);

        let migrated = legacy_v9(path.clone());
        let battles = migrated.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].arena_id, "v3-battle");
        assert_eq!(battles[0].damage, Some(2_345));
        assert_eq!(battles[0].assist, Some(321));
        assert_eq!(battles[0].feature_schema_version, None);
        assert_eq!(battles[0].map_key, None);
        assert_eq!(battles[0].assist_spotting, None);
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            9
        );
    }

    #[test]
    fn migrates_v5_outcomes_from_raw_teams_and_leaves_ambiguous_false_unknown() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("v5-outcomes.db");
        let database = legacy_v9(path.clone());

        for (id, outcome, player_team, winner_team) in [
            ("victory", BattleOutcome::Victory, Some(1), Some(1)),
            ("defeat", BattleOutcome::Defeat, Some(1), Some(2)),
            ("draw", BattleOutcome::Draw, Some(1), Some(0)),
            ("legacy-false", BattleOutcome::Defeat, None, None),
            ("conflict", BattleOutcome::Victory, Some(1), Some(2)),
        ] {
            let mut value = battle(id);
            value.data_quality = DataQuality::Full;
            value.outcome = outcome;
            value.player_team = player_team;
            value.winner_team_raw = winner_team;
            raw_insert(&database, &value);
        }
        drop(database);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("ALTER TABLE battles DROP COLUMN outcome; PRAGMA user_version=5;")
            .unwrap();
        drop(connection);

        for _ in 0..2 {
            let migrated = legacy_v9(path.clone());
            let outcomes = migrated
                .recent_battles(10)
                .unwrap()
                .into_iter()
                .map(|battle| (battle.arena_id, battle.outcome))
                .collect::<HashMap<_, _>>();
            assert_eq!(outcomes["victory"], BattleOutcome::Victory);
            assert_eq!(outcomes["defeat"], BattleOutcome::Defeat);
            assert_eq!(outcomes["draw"], BattleOutcome::Draw);
            assert_eq!(outcomes["legacy-false"], BattleOutcome::Unknown);
            assert_eq!(outcomes["conflict"], BattleOutcome::Unknown);
            drop(migrated);
        }
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            9
        );
        let pairs = connection
            .prepare("SELECT arena_id,outcome,won FROM battles ORDER BY arena_id")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    (row.get::<_, String>(1)?, row.get::<_, Option<i64>>(2)?),
                ))
            })
            .unwrap()
            .collect::<Result<HashMap<_, _>, _>>()
            .unwrap();
        assert_eq!(pairs["victory"], ("victory".to_owned(), Some(1)));
        assert_eq!(pairs["defeat"], ("defeat".to_owned(), Some(0)));
        assert_eq!(pairs["draw"], ("draw".to_owned(), Some(0)));
        assert_eq!(pairs["legacy-false"], ("unknown".to_owned(), None));
        assert_eq!(pairs["conflict"], ("unknown".to_owned(), None));
    }

    #[test]
    fn outcome_migration_rolls_back_column_and_version_when_backfill_fails() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("v5-outcome-rollback.db");
        let database = Database::initialize(path.clone()).unwrap();
        raw_insert(&database, &battle("legacy"));
        drop(database);

        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "ALTER TABLE battles DROP COLUMN outcome;
                 PRAGMA user_version=5;
                 CREATE TRIGGER block_outcome_backfill
                 BEFORE UPDATE ON battles
                 BEGIN
                   SELECT RAISE(ABORT,'controlled migration failure');
                 END;",
            )
            .unwrap();
        drop(connection);

        assert!(Database::initialize(path.clone()).is_err());
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            5
        );
        let columns = connection
            .prepare("PRAGMA table_info(battles)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns.iter().any(|column| column == "outcome"));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM battles", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn migrates_v6_to_v9_without_backfill_and_preserves_outcome_and_challenges() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("v6-battle-pace.db");
        let database = legacy_v9(path.clone());
        let mut legacy = battle("legacy-v6");
        legacy.data_quality = DataQuality::Full;
        legacy.outcome = BattleOutcome::Victory;
        raw_insert(&database, &legacy);
        raw_session(&database, "challenge-source", "ended");
        let challenge = database
            .start_challenge(
                "challenge-source",
                &challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank),
                1_700_000_100_000,
            )
            .unwrap();
        drop(database);

        let connection = Connection::open(&path).unwrap();
        drop_battle_pace_columns(&connection);
        connection.pragma_update(None, "user_version", 6).unwrap();
        drop(connection);

        for _ in 0..2 {
            let migrated = legacy_v9(path.clone());
            let stored = migrated.recent_battles(10).unwrap();
            assert_eq!(stored.len(), 1);
            assert_eq!(stored[0].outcome, BattleOutcome::Victory);
            assert_eq!(stored[0].battle_pace, BattlePaceRecord::default());
            assert_eq!(
                raw_connection(&migrated)
                    .query_row(
                        "SELECT won FROM battles WHERE arena_id='legacy-v6'",
                        [],
                        |row| row.get::<_, Option<i64>>(0),
                    )
                    .unwrap(),
                Some(1)
            );
            assert_eq!(
                migrated.active_challenge().unwrap().unwrap().id,
                challenge.id
            );
            drop(migrated);
        }

        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            9
        );
        let pace_values = connection
            .query_row(
                "SELECT battle_pace_context,battle_pace_reason,battle_pace_resolver_version,
                        finish_reason_raw,battle_type_raw,bonus_type_raw,initial_team1_size,
                        initial_team2_size,destroyed_team1_score,destroyed_team2_score
                 FROM battles WHERE arena_id='legacy-v6'",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                        row.get::<_, Option<i64>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            pace_values,
            (None, None, None, None, None, None, None, None, None, None)
        );
    }

    #[test]
    fn battle_pace_migration_rolls_back_partial_schema_change() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("v6-battle-pace-rollback.db");
        let database = Database::initialize(path.clone()).unwrap();
        raw_insert(&database, &battle("legacy"));
        drop(database);

        let connection = Connection::open(&path).unwrap();
        drop_battle_pace_columns(&connection);
        connection
            .execute_batch(
                "ALTER TABLE battles ADD COLUMN battle_pace_reason TEXT;
                 PRAGMA user_version=6;",
            )
            .unwrap();
        drop(connection);

        assert!(Database::initialize(path.clone()).is_err());
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            6
        );
        let columns = connection
            .prepare("PRAGMA table_info(battles)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns.iter().any(|column| column == "battle_pace_context"));
        assert!(columns.iter().any(|column| column == "battle_pace_reason"));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM battles", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn sqlite_round_trips_all_pace_contexts_and_reasons_across_restart() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("battle-pace-round-trip.db");
        let database = Database::initialize(path.clone()).unwrap();
        for (index, context) in [
            BattlePaceContext::ObviousSteamroll,
            BattlePaceContext::BorderlineSteamroll,
            BattlePaceContext::Standard,
            BattlePaceContext::Unknown,
        ]
        .into_iter()
        .enumerate()
        {
            let mut value = battle(&format!("context-{index}"));
            value.data_quality = DataQuality::Full;
            value.battle_pace = BattlePaceRecord {
                context: Some(context),
                resolver_version: Some("battle-pace-context-v1".into()),
                finish_reason_raw: Some(1),
                battle_type_raw: Some(1),
                bonus_type_raw: Some(1),
                initial_team1_size: Some(15),
                initial_team2_size: Some(15),
                destroyed_team1_score: Some(if index == 0 { 15 } else { 4 }),
                destroyed_team2_score: Some(if index == 0 { 1 } else { 15 }),
                ..Default::default()
            };
            raw_insert(&database, &value);
        }
        let reasons = [
            BattlePaceReason::MissingDuration,
            BattlePaceReason::MissingFinalRoster,
            BattlePaceReason::MissingTeamAssignment,
            BattlePaceReason::MissingFinishReason,
            BattlePaceReason::UnsupportedMode,
            BattlePaceReason::UnsupportedFinishReason,
            BattlePaceReason::UnsupportedSchema,
            BattlePaceReason::InvalidTeamSize,
            BattlePaceReason::AsymmetricTeams,
            BattlePaceReason::InvalidDestroyedScore,
            BattlePaceReason::ConflictingData,
        ];
        for (index, reason) in reasons.into_iter().enumerate() {
            let mut value = battle(&format!("reason-{index}"));
            value.data_quality = DataQuality::Full;
            value.battle_pace = BattlePaceRecord {
                context: Some(BattlePaceContext::Unknown),
                reason: Some(reason),
                resolver_version: Some("battle-pace-context-v1".into()),
                ..Default::default()
            };
            raw_insert(&database, &value);
        }
        drop(database);

        let reopened = Database::initialize(path.clone()).unwrap();
        let stored = reopened.recent_battles(100).unwrap();
        for context in [
            BattlePaceContext::ObviousSteamroll,
            BattlePaceContext::BorderlineSteamroll,
            BattlePaceContext::Standard,
            BattlePaceContext::Unknown,
        ] {
            assert!(
                stored
                    .iter()
                    .any(|battle| battle.battle_pace.context == Some(context))
            );
        }
        for reason in reasons {
            assert!(
                stored
                    .iter()
                    .any(|battle| battle.battle_pace.reason == Some(reason))
            );
        }
        let connection = raw_connection(&reopened);
        assert!(
            connection
                .execute(
                    "UPDATE battles SET battle_pace_context='invented' WHERE arena_id='context-0'",
                    [],
                )
                .is_err()
        );
        assert!(
            connection
                .execute(
                    "UPDATE battles SET battle_pace_reason='invented' WHERE arena_id='reason-0'",
                    [],
                )
                .is_err()
        );
    }

    #[test]
    fn migrates_v7_to_v9_without_backfill_or_existing_data_changes() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("v7-to-v8.db");
        let database = legacy_v9(path.clone());
        let mut legacy = battle("legacy-v7");
        legacy.data_quality = DataQuality::Full;
        legacy.parser_version = Some(5);
        legacy.feature_schema_version = Some(1);
        legacy.damage = Some(1_234);
        legacy.hits = Some(7);
        legacy.penetrations = Some(5);
        raw_insert(&database, &legacy);
        drop(database);

        let connection = Connection::open(&path).unwrap();
        for column in [
            "explosion_hits",
            "piercing_enemy_hits",
            "direct_team_hits",
            "direct_enemy_hits",
        ] {
            connection
                .execute_batch(&format!("ALTER TABLE battles DROP COLUMN {column};"))
                .unwrap();
        }
        connection.pragma_update(None, "user_version", 7).unwrap();
        drop(connection);

        for _ in 0..2 {
            let migrated = legacy_v9(path.clone());
            let stored = migrated.recent_battles(10).unwrap();
            assert_eq!(stored.len(), 1);
            assert_eq!(stored[0].arena_id, "legacy-v7");
            assert_eq!(stored[0].damage, Some(1_234));
            assert_eq!(stored[0].hits, Some(7));
            assert_eq!(stored[0].penetrations, Some(5));
            assert_eq!(stored[0].parser_version, Some(5));
            assert_eq!(stored[0].feature_schema_version, Some(1));
            assert_eq!(stored[0].direct_enemy_hits, None);
            assert_eq!(stored[0].direct_team_hits, None);
            assert_eq!(stored[0].piercing_enemy_hits, None);
            assert_eq!(stored[0].explosion_hits, None);
            drop(migrated);
        }
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            9
        );
    }

    #[test]
    fn attempts_conversion_raw_counters_round_trip_values_zero_and_null_after_restart() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("attempts-round-trip.db");
        let database = Database::initialize(path.clone()).unwrap();
        let mut values = battle("values");
        values.data_quality = DataQuality::Full;
        values.parser_version = Some(6);
        values.feature_schema_version = Some(2);
        values.direct_enemy_hits = Some(12);
        values.direct_team_hits = Some(1);
        values.piercing_enemy_hits = Some(9);
        values.explosion_hits = Some(3);
        raw_insert(&database, &values);
        let mut zeros = values.clone();
        zeros.arena_id = "zeros".into();
        zeros.direct_enemy_hits = Some(0);
        zeros.direct_team_hits = Some(0);
        zeros.piercing_enemy_hits = Some(0);
        zeros.explosion_hits = Some(0);
        raw_insert(&database, &zeros);
        let mut nulls = values.clone();
        nulls.arena_id = "nulls".into();
        nulls.direct_enemy_hits = None;
        nulls.direct_team_hits = None;
        nulls.piercing_enemy_hits = None;
        nulls.explosion_hits = None;
        raw_insert(&database, &nulls);
        drop(database);

        let reopened = Database::initialize(path).unwrap();
        let stored = reopened
            .recent_battles(10)
            .unwrap()
            .into_iter()
            .map(|battle| (battle.arena_id.clone(), battle))
            .collect::<HashMap<_, _>>();
        assert_eq!(stored["values"].direct_enemy_hits, Some(12));
        assert_eq!(stored["values"].direct_team_hits, Some(1));
        assert_eq!(stored["values"].piercing_enemy_hits, Some(9));
        assert_eq!(stored["values"].explosion_hits, Some(3));
        assert_eq!(stored["zeros"].direct_enemy_hits, Some(0));
        assert_eq!(stored["zeros"].direct_team_hits, Some(0));
        assert_eq!(stored["zeros"].piercing_enemy_hits, Some(0));
        assert_eq!(stored["zeros"].explosion_hits, Some(0));
        assert_eq!(stored["nulls"].direct_enemy_hits, None);
        assert_eq!(stored["nulls"].direct_team_hits, None);
        assert_eq!(stored["nulls"].piercing_enemy_hits, None);
        assert_eq!(stored["nulls"].explosion_hits, None);
    }

    #[test]
    fn challenge_live_and_catch_up_accept_only_core_feature_schemas_one_two_and_three() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("challenge-schema-bridge.db")).unwrap();
        raw_session(&database, "source", "ended");
        let suggestion =
            challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank);
        let challenge = database
            .start_challenge("source", &suggestion, 1_700_000_100_000)
            .unwrap();
        for (index, schema) in [Some(1), Some(2), None, Some(3)].into_iter().enumerate() {
            let id = format!("live-{index}");
            let mut candidate =
                qualifying_battle(&id, challenge.eligible_after + index as i64, Some(2_000));
            candidate.feature_schema_version = schema;
            raw_insert(&database, &candidate);
            database.process_battle_for_active_challenge(&id).unwrap();
        }
        let mut connection = raw_connection(&database);
        let transaction = connection.transaction().unwrap();
        for (id, schema) in [("invalid-negative", -1), ("invalid-zero", 0)] {
            let mut invalid = qualifying_battle(id, challenge.eligible_after + 50, Some(2_000));
            invalid.feature_schema_version = Some(schema);
            assert_eq!(
                Database::insert_battle_tx(&transaction, &invalid).unwrap(),
                0
            );
        }
        transaction.rollback().unwrap();
        let live = database.active_challenge().unwrap().unwrap();
        assert_eq!(live.completed_battles, 3);
        assert_eq!(live.battles.len(), 3);
        assert_eq!(live.battles[0].battle_time, challenge.eligible_after);
        assert_eq!(live.battles[1].battle_time, challenge.eligible_after + 1);
        assert_eq!(live.battles[2].battle_time, challenge.eligible_after + 3);

        for index in 0..2 {
            let mut candidate = qualifying_battle(
                &format!("catch-up-{index}"),
                challenge.eligible_after + 100 + index,
                Some(2_000),
            );
            candidate.feature_schema_version = Some(if index == 0 { 2 } else { 1 });
            raw_insert(&database, &candidate);
        }
        let caught_up = database.catch_up_active_challenge().unwrap().unwrap();
        assert_eq!(caught_up.completed_battles, 5);
        assert_eq!(caught_up.battles.len(), 5);
        assert!(database.catch_up_active_challenge().unwrap().is_none());
    }

    #[test]
    fn partial_v8_schema_is_rejected_without_completing_the_migration() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("partial-v8.db");
        let database = Database::initialize(path.clone()).unwrap();
        drop(database);
        let connection = Connection::open(&path).unwrap();
        for column in [
            "explosion_hits",
            "piercing_enemy_hits",
            "direct_team_hits",
            "direct_enemy_hits",
        ] {
            connection
                .execute_batch(&format!("ALTER TABLE battles DROP COLUMN {column};"))
                .unwrap();
        }
        connection
            .execute_batch(
                "ALTER TABLE battles ADD COLUMN direct_enemy_hits INTEGER CHECK(direct_enemy_hits>=0);
                 PRAGMA user_version=7;",
            )
            .unwrap();
        drop(connection);

        assert!(Database::initialize(path.clone()).is_err());
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            7
        );
        let columns = connection
            .prepare("PRAGMA table_info(battles)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.iter().any(|column| column == "direct_enemy_hits"));
        assert!(!columns.iter().any(|column| column == "direct_team_hits"));
        assert!(!columns.iter().any(|column| column == "piercing_enemy_hits"));
        assert!(!columns.iter().any(|column| column == "explosion_hits"));
    }

    #[test]
    fn fresh_database_uses_schema_v10() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("fresh.db");
        let database = Database::initialize(path.clone()).unwrap();
        drop(database);

        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            10
        );
        let columns = connection
            .prepare("PRAGMA table_info(battles)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for required in [
            "feature_schema_version",
            "map_key",
            "gameplay_id",
            "battle_time_source",
            "player_team",
            "winner_team_raw",
            "assist_spotting",
            "assist_tracking",
            "assist_stun",
            "original_xp",
            "outcome",
            "battle_pace_context",
            "battle_pace_reason",
            "battle_pace_resolver_version",
            "finish_reason_raw",
            "battle_type_raw",
            "bonus_type_raw",
            "initial_team1_size",
            "initial_team2_size",
            "destroyed_team1_score",
            "destroyed_team2_score",
            "direct_enemy_hits",
            "direct_team_hits",
            "piercing_enemy_hits",
            "explosion_hits",
        ] {
            assert!(columns.iter().any(|column| column == required));
        }
        for column in [
            "direct_enemy_hits",
            "direct_team_hits",
            "piercing_enemy_hits",
            "explosion_hits",
        ] {
            let (not_null, default_value): (i64, Option<String>) = connection
                .query_row(
                    "SELECT \"notnull\",dflt_value FROM pragma_table_info('battles') WHERE name=?1",
                    params![column],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(not_null, 0);
            assert_eq!(default_value, None);
        }
    }

    #[test]
    fn team_relative_schema_is_strict_nullable_and_cascades() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("team-relative-schema.db")).unwrap();
        let connection = raw_connection(&database);
        let columns = connection
            .prepare("PRAGMA table_info(team_relative_facts)")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            columns
                .iter()
                .map(|column| column.0.as_str())
                .collect::<Vec<_>>(),
            [
                "battle_id",
                "fact_schema_version",
                "common_reason",
                "team_player_count",
                "team_damage_total",
                "damage_greater_count",
                "damage_equal_count",
                "damage_reason",
                "team_frags_total",
                "frags_reason",
                "xp_greater_count",
                "xp_equal_count",
                "xp_reason",
            ]
        );
        assert_eq!(columns[0].1, "TEXT");
        assert_eq!(columns[1].1, "INTEGER");
        assert_eq!(columns[1].2, 1);
        assert_eq!(columns[1].3, None);
        for column in columns.iter().skip(2) {
            assert_eq!(column.2, 0, "{}", column.0);
            assert_eq!(column.3, None, "{}", column.0);
        }
        let foreign_key = connection
            .query_row(
                "SELECT \"table\",\"from\",\"to\",on_delete
                 FROM pragma_foreign_key_list('team_relative_facts')",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            foreign_key,
            (
                "battles".into(),
                "battle_id".into(),
                "arena_id".into(),
                "CASCADE".into()
            )
        );

        let mut value = full_replay_battle("cascade");
        let state = ReplayFileState {
            source_path_key: "cascade-source".into(),
            file_size: 10,
            file_mtime_ms: 20,
            content_fingerprint: "cascade-fingerprint".into(),
            block_count: 2,
        };
        database
            .upsert_replay_with_team_relative_facts(
                &mut value,
                &state,
                false,
                Some(&eligible_team_facts()),
            )
            .unwrap();
        assert!(database.team_relative_facts("cascade").unwrap().is_some());
        raw_connection(&database)
            .execute("DELETE FROM battles WHERE arena_id='cascade'", [])
            .unwrap();
        assert!(database.team_relative_facts("cascade").unwrap().is_none());
    }

    #[test]
    fn sqlite_rejects_every_hybrid_or_invalid_team_relative_state() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("team-relative-constraints.db")).unwrap();
        for id in [
            "schema",
            "reason",
            "common-with-fact",
            "eligible-no-team",
            "damage-incomplete",
            "damage-hybrid",
            "rank-range",
            "rank-sum",
            "frags-range",
        ] {
            raw_insert(&database, &battle(id));
        }
        let connection = raw_connection(&database);
        for statement in [
            "INSERT INTO team_relative_facts(battle_id,fact_schema_version,common_reason)
             VALUES('schema',2,'partial_replay')",
            "INSERT INTO team_relative_facts(battle_id,fact_schema_version,common_reason)
             VALUES('reason',1,'invented')",
            "INSERT INTO team_relative_facts(battle_id,fact_schema_version,common_reason,team_damage_total)
             VALUES('common-with-fact',1,'partial_replay',0)",
            "INSERT INTO team_relative_facts(battle_id,fact_schema_version)
             VALUES('eligible-no-team',1)",
            "INSERT INTO team_relative_facts(
               battle_id,fact_schema_version,team_player_count,team_damage_total,
               damage_greater_count,team_frags_total,xp_greater_count,xp_equal_count)
             VALUES('damage-incomplete',1,15,1,0,0,0,1)",
            "INSERT INTO team_relative_facts(
               battle_id,fact_schema_version,team_player_count,team_damage_total,
               damage_greater_count,damage_equal_count,damage_reason,
               team_frags_total,xp_greater_count,xp_equal_count)
             VALUES('damage-hybrid',1,15,1,0,1,'damage_conflict',0,0,1)",
            "INSERT INTO team_relative_facts(
               battle_id,fact_schema_version,team_player_count,damage_reason,
               team_frags_total,xp_greater_count,xp_equal_count)
             VALUES('rank-range',1,15,'damage_missing',0,15,1)",
            "INSERT INTO team_relative_facts(
               battle_id,fact_schema_version,team_player_count,damage_reason,
               team_frags_total,xp_greater_count,xp_equal_count)
             VALUES('rank-sum',1,15,'damage_missing',0,14,2)",
            "INSERT INTO team_relative_facts(
               battle_id,fact_schema_version,team_player_count,damage_reason,
               team_frags_total,xp_greater_count,xp_equal_count)
             VALUES('frags-range',1,15,'damage_missing',16,0,1)",
        ] {
            assert!(connection.execute_batch(statement).is_err(), "{statement}");
        }
    }

    #[test]
    fn team_relative_failure_states_round_trip_without_false_zeroes() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("team-relative-states.db")).unwrap();
        let mut states = Vec::new();
        states.push(TeamRelativeFactsV1::common_ineligible(
            TeamRelativeCommonReason::UnsupportedMode,
        ));

        let mut damage_failure = eligible_team_facts();
        damage_failure.team_damage_total = None;
        damage_failure.damage_greater_count = None;
        damage_failure.damage_equal_count = None;
        damage_failure.damage_reason = Some(TeamRelativeDamageReason::Missing);
        states.push(damage_failure);

        let mut frags_failure = eligible_team_facts();
        frags_failure.team_frags_total = None;
        frags_failure.frags_reason = Some(TeamRelativeFragsReason::Conflict);
        states.push(frags_failure);

        let mut xp_failure = eligible_team_facts();
        xp_failure.xp_greater_count = None;
        xp_failure.xp_equal_count = None;
        xp_failure.xp_reason = Some(TeamRelativeXpReason::OutOfRange);
        states.push(xp_failure);

        let mut all_failures = eligible_team_facts();
        all_failures.team_damage_total = None;
        all_failures.damage_greater_count = None;
        all_failures.damage_equal_count = None;
        all_failures.damage_reason = Some(TeamRelativeDamageReason::Missing);
        all_failures.team_frags_total = None;
        all_failures.frags_reason = Some(TeamRelativeFragsReason::Missing);
        all_failures.xp_greater_count = None;
        all_failures.xp_equal_count = None;
        all_failures.xp_reason = Some(TeamRelativeXpReason::Missing);
        states.push(all_failures);

        let mut zero_totals = eligible_team_facts();
        zero_totals.team_damage_total = Some(0);
        zero_totals.damage_greater_count = Some(0);
        zero_totals.damage_equal_count = Some(15);
        zero_totals.team_frags_total = Some(0);
        zero_totals.xp_greater_count = Some(0);
        zero_totals.xp_equal_count = Some(15);
        states.push(zero_totals);

        for (index, facts) in states.into_iter().enumerate() {
            let id = format!("state-{index}");
            let mut value = full_replay_battle(&id);
            let state = ReplayFileState {
                source_path_key: format!("state-source-{index}"),
                file_size: 10,
                file_mtime_ms: 20,
                content_fingerprint: format!("state-fingerprint-{index}"),
                block_count: 2,
            };
            database
                .upsert_replay_with_team_relative_facts(&mut value, &state, false, Some(&facts))
                .unwrap();
            assert_eq!(database.team_relative_facts(&id).unwrap(), Some(facts));
        }
    }

    #[test]
    fn normal_import_is_atomic_exactly_once_and_partial_upgrades_facts() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("team-relative-import.db")).unwrap();
        let state = ReplayFileState {
            source_path_key: "import-source".into(),
            file_size: 10,
            file_mtime_ms: 20,
            content_fingerprint: "import-partial".into(),
            block_count: 1,
        };
        let mut partial = battle("import");
        partial.parser_version = Some(7);
        partial.data_completeness = Some("metadata_only".into());
        partial.replay_source = Some("replay_metadata".into());
        let partial_facts =
            TeamRelativeFactsV1::common_ineligible(TeamRelativeCommonReason::PartialReplay);
        assert_eq!(
            database
                .upsert_replay_with_team_relative_facts(
                    &mut partial,
                    &state,
                    true,
                    Some(&partial_facts),
                )
                .unwrap(),
            ReplayUpsertOutcome::Inserted
        );
        assert_eq!(
            database.team_relative_facts(&partial.arena_id).unwrap(),
            Some(partial_facts)
        );

        let mut full = full_replay_battle("provisional");
        full.battle_time = partial.battle_time;
        full.vehicle_key = partial.vehicle_key.clone();
        full.map_name = partial.map_name.clone();
        let mut completed_state = state.clone();
        completed_state.block_count = 2;
        completed_state.content_fingerprint = "import-full".into();
        assert_eq!(
            database
                .upsert_replay_with_team_relative_facts(
                    &mut full,
                    &completed_state,
                    true,
                    Some(&eligible_team_facts()),
                )
                .unwrap(),
            ReplayUpsertOutcome::Updated
        );
        assert_eq!(full.arena_id, partial.arena_id);
        assert_eq!(
            database.team_relative_facts(&full.arena_id).unwrap(),
            Some(eligible_team_facts())
        );
        assert_eq!(
            database
                .upsert_replay_with_team_relative_facts(
                    &mut full,
                    &completed_state,
                    true,
                    Some(&eligible_team_facts()),
                )
                .unwrap(),
            ReplayUpsertOutcome::Duplicate
        );
        assert_eq!(database.recent_battles(10).unwrap().len(), 1);
        assert_eq!(database.active_session().unwrap().unwrap().battle_count, 1);
        assert_eq!(
            raw_connection(&database)
                .query_row("SELECT COUNT(*) FROM team_relative_facts", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            1
        );
    }

    #[test]
    fn invalid_team_relative_facts_roll_back_battle_source_and_session_atomically() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("team-relative-rollback.db")).unwrap();
        let mut value = full_replay_battle("rollback");
        let state = ReplayFileState {
            source_path_key: "rollback-source".into(),
            file_size: 10,
            file_mtime_ms: 20,
            content_fingerprint: "rollback-fingerprint".into(),
            block_count: 2,
        };
        let mut invalid = eligible_team_facts();
        invalid.team_player_count = None;
        assert!(
            database
                .upsert_replay_with_team_relative_facts(&mut value, &state, true, Some(&invalid),)
                .is_err()
        );
        let connection = raw_connection(&database);
        for table in ["battles", "sessions", "replay_files", "team_relative_facts"] {
            assert_eq!(
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .unwrap(),
                0,
                "{table}"
            );
        }
    }

    #[test]
    fn migration_eight_to_nine_is_additive_empty_and_idempotent() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("v8-to-v9.db");
        let database = legacy_v9(path.clone());
        raw_insert(&database, &full_replay_battle("legacy"));
        raw_session(&database, "legacy-session", "ended");
        let before = (
            database.recent_battles(10).unwrap(),
            database.recent_sessions(10).unwrap(),
        );
        drop(database);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("DROP TABLE team_relative_facts; PRAGMA user_version=8;")
            .unwrap();
        drop(connection);

        for _ in 0..2 {
            let migrated = legacy_v9(path.clone());
            assert_eq!(
                (
                    migrated.recent_battles(10).unwrap(),
                    migrated.recent_sessions(10).unwrap()
                ),
                before
            );
            assert_eq!(
                raw_connection(&migrated)
                    .query_row("SELECT COUNT(*) FROM team_relative_facts", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .unwrap(),
                0
            );
            assert!(migrated.integrity_report().unwrap().is_healthy());
        }
        let connection = Connection::open(path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            9
        );
    }

    #[test]
    fn wipe_removes_history_after_restart_but_preserves_ui_preferences() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("wipe.db");
        let database = Database::initialize(path.clone()).unwrap();
        database.set_setting("language", "en").unwrap();
        database.set_setting("inactivity_minutes", "45").unwrap();
        database.set_setting("onboarding_complete", "1").unwrap();
        database
            .set_setting("account_id", "SHOULD_NOT_REMAIN")
            .unwrap();
        database.set_setting("xvm_enabled", "1").unwrap();
        let mut stored = battle("wipe-arena");
        stored.tank_name = Some("WIPE_TANK_MARKER".into());
        let replay_state = ReplayFileState {
            source_path_key: "WIPE_SOURCE_KEY".into(),
            file_size: 123,
            file_mtime_ms: 1_700_000_000_000,
            content_fingerprint: "WIPE_CONTENT_FINGERPRINT".into(),
            block_count: 1,
        };
        database
            .upsert_replay(&mut stored, &replay_state, true)
            .unwrap();
        raw_session(&database, "wipe-source-session", "ended");
        database
            .start_challenge(
                "wipe-source-session",
                &challenge_suggestion(ChallengeType::DamageConsistency, VehicleClass::HeavyTank),
                1_700_000_100_123,
            )
            .unwrap();

        database.wipe().unwrap();
        drop(database);

        let reopened = Database::initialize(path.clone()).unwrap();
        assert!(reopened.recent_battles(10).unwrap().is_empty());
        assert!(reopened.recent_sessions(10).unwrap().is_empty());
        assert!(reopened.active_challenge().unwrap().is_none());
        assert!(
            reopened
                .challenge_history_page(0, 10)
                .unwrap()
                .items
                .is_empty()
        );
        assert_eq!(
            reopened.get_setting("language").unwrap().as_deref(),
            Some("en")
        );
        assert_eq!(
            reopened
                .get_setting("inactivity_minutes")
                .unwrap()
                .as_deref(),
            Some("45")
        );
        assert!(reopened.get_setting("account_id").unwrap().is_none());
        assert!(reopened.get_setting("xvm_enabled").unwrap().is_none());
        assert_eq!(
            reopened
                .get_setting("onboarding_complete")
                .unwrap()
                .as_deref(),
            Some("1")
        );
        let replay_file_count: i64 = raw_connection(&reopened)
            .query_row("SELECT COUNT(*) FROM replay_files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(replay_file_count, 0);
        let bytes = fs::read(path).unwrap();
        let database_image = String::from_utf8_lossy(&bytes);
        assert!(!database_image.contains("SHOULD_NOT_REMAIN"));
        assert!(!database_image.contains("WIPE_TANK_MARKER"));
        assert!(!database_image.contains("wipe-arena"));
        assert!(!database_image.contains("WIPE_SOURCE_KEY"));
        assert!(!database_image.contains("WIPE_CONTENT_FINGERPRINT"));
        assert!(!database_image.contains("wipe-source-session"));
        assert!(!database_image.contains("Synthetic vehicle"));
    }
}
