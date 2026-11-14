use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Where a battle row came from. v10 stores only replays; `local_hybrid` is read from rows of
/// older builds. The Tomato.gg source was removed in v2.0 (D2) and cannot occur in v10.
pub enum SourceKind {
    Replay,
    LocalHybrid,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Replay => "replay",
            Self::LocalHybrid => "local_hybrid",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "local_hybrid" => Self::LocalHybrid,
            _ => Self::Replay,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataQuality {
    Full,
    Partial,
    Ambiguous,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VehicleClass {
    LightTank,
    MediumTank,
    HeavyTank,
    TankDestroyer,
    Spg,
    Unknown,
}

impl VehicleClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::LightTank => "light_tank",
            Self::MediumTank => "medium_tank",
            Self::HeavyTank => "heavy_tank",
            Self::TankDestroyer => "tank_destroyer",
            Self::Spg => "spg",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "light_tank" => Self::LightTank,
            "medium_tank" => Self::MediumTank,
            "heavy_tank" => Self::HeavyTank,
            "tank_destroyer" => Self::TankDestroyer,
            "spg" => Self::Spg,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn source_tag(self) -> &'static str {
        match self {
            Self::LightTank => "lightTank",
            Self::MediumTank => "mediumTank",
            Self::HeavyTank => "heavyTank",
            Self::TankDestroyer => "AT-SPG",
            Self::Spg => "SPG",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn is_known(self) -> bool {
        self != Self::Unknown
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VehicleClassSource {
    WgClientPackedCatalog,
    Unknown,
}

impl VehicleClassSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WgClientPackedCatalog => "wg_client_packed_catalog",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "wg_client_packed_catalog" => Self::WgClientPackedCatalog,
            _ => Self::Unknown,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompactDescriptorProtocolSource {
    ExternallyDocumentedAndRuntimeValidated,
    Unconfirmed,
}

impl CompactDescriptorProtocolSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ExternallyDocumentedAndRuntimeValidated => {
                "externally_documented_and_runtime_validated"
            }
            Self::Unconfirmed => "unconfirmed",
        }
    }

    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "externally_documented_and_runtime_validated" => {
                Self::ExternallyDocumentedAndRuntimeValidated
            }
            _ => Self::Unconfirmed,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VehicleClassResolutionState {
    Resolved,
    Unknown,
    Conflict,
    CatalogUnavailable,
}

impl VehicleClassResolutionState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Unknown => "unknown",
            Self::Conflict => "conflict",
            Self::CatalogUnavailable => "catalog_unavailable",
        }
    }

    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "resolved" => Self::Resolved,
            "conflict" => Self::Conflict,
            "catalog_unavailable" => Self::CatalogUnavailable,
            _ => Self::Unknown,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VehicleKeyCrossCheck {
    Unavailable,
    RawExact,
    CanonicalExact,
    Conflict,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleClassResolution {
    pub vehicle_id: i64,
    pub vehicle_class: VehicleClass,
    pub state: VehicleClassResolutionState,
    pub source: VehicleClassSource,
    pub catalog_schema_version: Option<u32>,
    pub catalog_source_version: Option<String>,
    pub catalog_fingerprint: Option<String>,
    pub protocol_source: CompactDescriptorProtocolSource,
    pub vehicle_key_cross_check: VehicleKeyCrossCheck,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VehicleClassCatalogAvailability {
    Available,
    Updating,
    Unavailable,
    Error,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleClassCatalogStatus {
    pub availability: VehicleClassCatalogAvailability,
    pub source_version: Option<String>,
    pub vehicle_count: usize,
    pub invalid_record_count: usize,
    pub unresolved_current_vehicle_count: usize,
    pub conflict_current_vehicle_count: usize,
    pub last_attempt_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_error_code: Option<String>,
    pub update_in_progress: bool,
    pub protocol_source: CompactDescriptorProtocolSource,
    pub schema_version: Option<u32>,
    pub vehicles_with_tier: usize,
    pub missing_nations: Vec<String>,
    pub unknown_nations: Vec<String>,
    /// Installed client version when the active catalog is out of date for it (B5).
    pub stale_for_client_version: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VehicleClassCatalogRefreshOutcome {
    Activated,
    Unchanged,
    Unavailable,
    Busy,
    FailedUsingCurrent,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleClassCatalogRefreshResult {
    pub outcome: VehicleClassCatalogRefreshOutcome,
    pub status: VehicleClassCatalogStatus,
}

impl DataQuality {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Partial => "partial",
            Self::Ambiguous => "ambiguous",
        }
    }
    pub fn parse(value: &str) -> Self {
        match value {
            "full" => Self::Full,
            "ambiguous" => Self::Ambiguous,
            _ => Self::Partial,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BattleOutcome {
    Victory,
    Defeat,
    Draw,
    Unknown,
}

impl BattleOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Victory => "victory",
            Self::Defeat => "defeat",
            Self::Draw => "draw",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "victory" => Self::Victory,
            "defeat" => Self::Defeat,
            "draw" => Self::Draw,
            _ => Self::Unknown,
        }
    }

    /// Maps the current random-battle replay contract:
    /// winnerTeam=0 is a draw; 1/2 identifies the winning team.
    pub fn from_replay_teams(player_team: Option<i64>, winner_team: Option<i64>) -> Self {
        match (player_team, winner_team) {
            (_, Some(0)) => Self::Draw,
            (Some(player @ 1..=2), Some(winner @ 1..=2)) if player == winner => Self::Victory,
            (Some(1..=2), Some(1..=2)) => Self::Defeat,
            _ => Self::Unknown,
        }
    }

    /// Reads old rows without guessing whether an unqualified `won=0` meant defeat or draw.
    pub fn from_legacy(
        won: Option<bool>,
        player_team: Option<i64>,
        winner_team: Option<i64>,
    ) -> Self {
        let replay_outcome = Self::from_replay_teams(player_team, winner_team);
        match (replay_outcome, won) {
            (Self::Unknown, Some(true)) if winner_team.is_none() => Self::Victory,
            (Self::Unknown, _) => Self::Unknown,
            (known, None) => known,
            (known, Some(legacy)) if known.legacy_won() == Some(legacy) => known,
            _ => Self::Unknown,
        }
    }

    pub fn legacy_won(self) -> Option<bool> {
        match self {
            Self::Victory => Some(true),
            Self::Defeat | Self::Draw => Some(false),
            Self::Unknown => None,
        }
    }

    pub fn win_contribution(self) -> Option<f64> {
        match self {
            Self::Victory => Some(1.0),
            Self::Defeat | Self::Draw => Some(0.0),
            Self::Unknown => None,
        }
    }

    pub fn is_known(self) -> bool {
        self != Self::Unknown
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BattlePaceContext {
    ObviousSteamroll,
    BorderlineSteamroll,
    Standard,
    Unknown,
}

impl BattlePaceContext {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ObviousSteamroll => "obvious_steamroll",
            Self::BorderlineSteamroll => "borderline_steamroll",
            Self::Standard => "standard",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "obvious_steamroll" => Some(Self::ObviousSteamroll),
            "borderline_steamroll" => Some(Self::BorderlineSteamroll),
            "standard" => Some(Self::Standard),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BattlePaceReason {
    MissingDuration,
    MissingFinalRoster,
    MissingTeamAssignment,
    MissingFinishReason,
    UnsupportedMode,
    UnsupportedFinishReason,
    UnsupportedSchema,
    InvalidTeamSize,
    AsymmetricTeams,
    InvalidDestroyedScore,
    ConflictingData,
}

impl BattlePaceReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingDuration => "missing_duration",
            Self::MissingFinalRoster => "missing_final_roster",
            Self::MissingTeamAssignment => "missing_team_assignment",
            Self::MissingFinishReason => "missing_finish_reason",
            Self::UnsupportedMode => "unsupported_mode",
            Self::UnsupportedFinishReason => "unsupported_finish_reason",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::InvalidTeamSize => "invalid_team_size",
            Self::AsymmetricTeams => "asymmetric_teams",
            Self::InvalidDestroyedScore => "invalid_destroyed_score",
            Self::ConflictingData => "conflicting_data",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "missing_duration" => Some(Self::MissingDuration),
            "missing_final_roster" => Some(Self::MissingFinalRoster),
            "missing_team_assignment" => Some(Self::MissingTeamAssignment),
            "missing_finish_reason" => Some(Self::MissingFinishReason),
            "unsupported_mode" => Some(Self::UnsupportedMode),
            "unsupported_finish_reason" => Some(Self::UnsupportedFinishReason),
            "unsupported_schema" => Some(Self::UnsupportedSchema),
            "invalid_team_size" => Some(Self::InvalidTeamSize),
            "asymmetric_teams" => Some(Self::AsymmetricTeams),
            "invalid_destroyed_score" => Some(Self::InvalidDestroyedScore),
            "conflicting_data" => Some(Self::ConflictingData),
            _ => None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BattlePaceProvenance {
    ReplayMetadata,
    ReplayResults,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BattlePaceTeamSizes {
    pub team1: i64,
    pub team2: i64,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BattlePaceDestroyedScore {
    pub team1: i64,
    pub team2: i64,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BattlePaceModeIdentity {
    pub battle_type_raw: Option<i64>,
    pub bonus_type_raw: Option<i64>,
    pub gameplay_id: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BattlePaceResolution {
    pub context: BattlePaceContext,
    pub reason: Option<BattlePaceReason>,
    pub resolver_version: String,
    pub provenance: BattlePaceProvenance,
    pub duration_seconds: Option<i64>,
    pub initial_team_sizes: Option<BattlePaceTeamSizes>,
    pub destroyed_score: Option<BattlePaceDestroyedScore>,
    pub finish_reason_raw: Option<i64>,
    pub mode_identity: BattlePaceModeIdentity,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BattlePaceRecord {
    pub context: Option<BattlePaceContext>,
    pub reason: Option<BattlePaceReason>,
    pub resolver_version: Option<String>,
    pub finish_reason_raw: Option<i64>,
    pub battle_type_raw: Option<i64>,
    pub bonus_type_raw: Option<i64>,
    pub initial_team1_size: Option<i64>,
    pub initial_team2_size: Option<i64>,
    pub destroyed_team1_score: Option<i64>,
    pub destroyed_team2_score: Option<i64>,
}

impl BattlePaceRecord {
    pub fn from_resolution(value: &BattlePaceResolution) -> Self {
        Self {
            context: Some(value.context),
            reason: value.reason,
            resolver_version: Some(value.resolver_version.clone()),
            finish_reason_raw: value.finish_reason_raw,
            battle_type_raw: value.mode_identity.battle_type_raw,
            bonus_type_raw: value.mode_identity.bonus_type_raw,
            initial_team1_size: value.initial_team_sizes.map(|sizes| sizes.team1),
            initial_team2_size: value.initial_team_sizes.map(|sizes| sizes.team2),
            destroyed_team1_score: value.destroyed_score.map(|score| score.team1),
            destroyed_team2_score: value.destroyed_score.map(|score| score.team2),
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BattlePaceSummary {
    pub total_battles: usize,
    pub obvious_steamrolls: usize,
    pub borderline_steamrolls: usize,
    pub standard_battles: usize,
    pub unknown_battles: usize,
    pub classified_battles: usize,
    pub coverage_ratio: f64,
}

#[cfg(test)]
mod battle_outcome_tests {
    use super::BattleOutcome;

    #[test]
    fn maps_confirmed_replay_team_contract_without_guessing() {
        assert_eq!(
            BattleOutcome::from_replay_teams(Some(1), Some(1)),
            BattleOutcome::Victory
        );
        assert_eq!(
            BattleOutcome::from_replay_teams(Some(1), Some(2)),
            BattleOutcome::Defeat
        );
        assert_eq!(
            BattleOutcome::from_replay_teams(Some(1), Some(0)),
            BattleOutcome::Draw
        );
        assert_eq!(
            BattleOutcome::from_replay_teams(None, Some(1)),
            BattleOutcome::Unknown
        );
        assert_eq!(
            BattleOutcome::from_replay_teams(Some(1), Some(9)),
            BattleOutcome::Unknown
        );
    }

    #[test]
    fn legacy_false_without_raw_teams_remains_unknown_and_conflicts_fail_closed() {
        assert_eq!(
            BattleOutcome::from_legacy(Some(false), None, None),
            BattleOutcome::Unknown
        );
        assert_eq!(
            BattleOutcome::from_legacy(Some(true), None, None),
            BattleOutcome::Victory
        );
        assert_eq!(
            BattleOutcome::from_legacy(Some(false), Some(1), Some(0)),
            BattleOutcome::Draw
        );
        assert_eq!(
            BattleOutcome::from_legacy(Some(true), Some(1), Some(2)),
            BattleOutcome::Unknown
        );
    }

    #[test]
    fn serde_round_trips_all_outcomes_with_stable_wire_values() {
        for (outcome, wire) in [
            (BattleOutcome::Victory, "\"victory\""),
            (BattleOutcome::Defeat, "\"defeat\""),
            (BattleOutcome::Draw, "\"draw\""),
            (BattleOutcome::Unknown, "\"unknown\""),
        ] {
            assert_eq!(serde_json::to_string(&outcome).unwrap(), wire);
            assert_eq!(
                serde_json::from_str::<BattleOutcome>(wire).unwrap(),
                outcome
            );
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Battle {
    /// Stable internal row id. Replay content fingerprints are stored separately and are never
    /// used as the primary identity of a battle.
    pub arena_id: String,
    pub arena_unique_id: Option<i64>,
    pub battle_time: i64,
    pub battle_time_source: Option<String>,
    pub vehicle_key: String,
    pub wg_tank_id: Option<i64>,
    pub tank_name: Option<String>,
    pub map_name: Option<String>,
    pub map_key: Option<String>,
    pub battle_type: Option<String>,
    pub gameplay_id: Option<String>,
    pub source_kind: SourceKind,
    pub data_quality: DataQuality,
    pub parser_version: Option<i64>,
    pub feature_schema_version: Option<i64>,
    pub game_version: Option<String>,
    pub data_completeness: Option<String>,
    pub replay_source: Option<String>,
    pub parser_error_code: Option<String>,
    pub outcome: BattleOutcome,
    pub battle_pace: BattlePaceRecord,
    pub player_team: Option<i64>,
    pub winner_team_raw: Option<i64>,
    pub damage: Option<i64>,
    pub assist: Option<i64>,
    pub assist_spotting: Option<i64>,
    pub assist_tracking: Option<i64>,
    pub assist_stun: Option<i64>,
    pub blocked: Option<i64>,
    pub damage_received: Option<i64>,
    pub frags: Option<i64>,
    pub spots: Option<i64>,
    pub shots: Option<i64>,
    pub hits: Option<i64>,
    pub direct_enemy_hits: Option<i64>,
    pub direct_team_hits: Option<i64>,
    pub penetrations: Option<i64>,
    pub piercing_enemy_hits: Option<i64>,
    pub explosion_hits: Option<i64>,
    pub defense_points: Option<i64>,
    pub moving_avg_damage: Option<i64>,
    pub damage_rating: Option<i64>,
    pub survived: Option<bool>,
    pub xp: Option<i64>,
    pub original_xp: Option<i64>,
    pub duration_seconds: Option<i64>,
    pub session_id: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wn8Totals {
    pub damage: f64,
    pub frags: f64,
    pub spots: f64,
    pub defense: f64,
    pub win_points: f64,
}

impl Default for Wn8Totals {
    fn default() -> Self {
        Self {
            damage: 0.0,
            frags: 0.0,
            spots: 0.0,
            defense: 0.0,
            win_points: 0.0,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Wn8ExclusionCounts {
    pub partial_or_incomplete: usize,
    pub unsupported_feature_schema: usize,
    pub not_random_battle: usize,
    pub vehicle_id_conflict: usize,
    pub missing_vehicle_id: usize,
    pub vehicle_not_in_expected_values: usize,
    pub missing_damage: usize,
    pub missing_frags: usize,
    pub missing_spots: usize,
    pub missing_defense: usize,
    pub missing_result: usize,
    pub invalid_numeric_input: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wn8Result {
    pub value: f64,
    pub formula_version: String,
    pub snapshot_sha256: String,
    pub snapshot_source_version: String,
    pub eligible_battles: usize,
    pub total_battles: usize,
    pub coverage_ratio: f64,
    pub actual_totals: Wn8Totals,
    pub expected_totals: Wn8Totals,
    pub exclusions: Wn8ExclusionCounts,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wn8Unavailable {
    pub formula_version: String,
    pub snapshot_sha256: String,
    pub snapshot_source_version: String,
    pub eligible_battles: usize,
    pub total_battles: usize,
    pub coverage_ratio: f64,
    pub exclusions: Wn8ExclusionCounts,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "details", rename_all = "snake_case")]
pub enum Wn8Calculation {
    Available(Wn8Result),
    Unavailable(Wn8Unavailable),
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackItem {
    pub code: String,
    pub tone: FeedbackTone,
    pub confidence: Confidence,
    pub actual: Option<f64>,
    pub baseline: Option<f64>,
    pub sample_size: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackTone {
    Positive,
    Warning,
    Neutral,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BattleView {
    pub battle: Battle,
    pub feedback: Vec<FeedbackItem>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub started_at: i64,
    pub last_battle_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub status: String,
    pub battle_count: usize,
    pub analyzed_battle_count: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetrics {
    pub battles: usize,
    pub analyzed_battles: usize,
    pub wins: usize,
    pub win_rate: Option<f64>,
    pub outcome_sample_size: usize,
    pub average_damage: Option<f64>,
    pub damage_sample_size: usize,
    pub average_assist: Option<f64>,
    pub assist_sample_size: usize,
    pub average_blocked: Option<f64>,
    pub average_frags: Option<f64>,
    pub average_xp: Option<f64>,
    pub xp_sample_size: usize,
    pub survival_rate: Option<f64>,
    pub survival_sample_size: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session: Session,
    pub metrics: SessionMetrics,
    pub feedback: Vec<FeedbackItem>,
    pub wn8: SessionWn8Presentation,
    pub battles: Vec<BattleView>,
    pub battle_wn8: Vec<BattleWn8Presentation>,
    pub battle_pace: BattlePaceSummary,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryDataQualityFilter {
    All,
    Full,
    Partial,
    Ambiguous,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryWn8Filter {
    All,
    Available,
    Unavailable,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BattleHistoryCursor {
    pub battle_time: i64,
    pub battle_id: String,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BattleHistoryFilter {
    pub vehicle_id: Option<i64>,
    pub data_quality: HistoryDataQualityFilter,
    pub wn8_availability: HistoryWn8Filter,
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    /// Map filter of the History screen (spec 25 §5.5); missing means all maps.
    #[serde(default)]
    pub map_key: Option<String>,
    pub cursor: Option<BattleHistoryCursor>,
    pub page_size: usize,
}

/// One team's totals from `battle_team_stats` for the Battle screen (D17): aggregates only,
/// never any field of `battle_roster`.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TeamTotals {
    pub damage: Option<i64>,
    pub spotted: Option<i64>,
    /// Radio + tracking assist (D8), stun excluded.
    pub assist: Option<i64>,
    pub kills: Option<i64>,
    pub survivors: Option<i64>,
    pub tier_min: Option<i64>,
    pub tier_max: Option<i64>,
    pub platoons: i64,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TeamComparison {
    pub own: TeamTotals,
    pub enemy: TeamTotals,
}

/// `get_battle_view`: one battle with its feedback and the team comparison, if stored.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BattleDetailView {
    pub view: BattleView,
    pub teams: Option<TeamComparison>,
    /// Own columns of v10 kept outside `Battle` (E2 decision 15).
    pub health_remaining: Option<i64>,
    pub max_health: Option<i64>,
    /// `true` when no team mate dealt more damage (team-relative facts).
    pub most_damage_in_team: Option<bool>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BattleHistoryItem {
    pub battle_id: String,
    pub session_id: Option<String>,
    pub session_started_at: Option<i64>,
    pub session_status: Option<String>,
    pub vehicle_id: Option<i64>,
    pub vehicle_key: Option<String>,
    pub tank_name: Option<String>,
    pub map_name: Option<String>,
    pub map_key: Option<String>,
    pub battle_time: i64,
    pub outcome: BattleOutcome,
    pub battle_pace: BattlePaceRecord,
    pub duration_seconds: Option<i64>,
    pub damage: Option<i64>,
    pub assist: Option<i64>,
    pub frags: Option<i64>,
    pub xp: Option<i64>,
    pub survived: Option<bool>,
    pub data_quality: DataQuality,
    pub wn8: BattleWn8Presentation,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BattleHistoryPage {
    pub items: Vec<BattleHistoryItem>,
    pub next_cursor: Option<BattleHistoryCursor>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleHistorySummary {
    pub vehicle_id: i64,
    pub vehicle_key: Option<String>,
    pub tank_name: Option<String>,
    pub vehicle_class: VehicleClassResolution,
    /// Tier stored with the newest battle that has one (E2 `battles.vehicle_tier`, from the
    /// client catalog at import time); `None` when no battle of this vehicle has a tier.
    pub tier: Option<u8>,
    pub total_battles: usize,
    pub session_count: usize,
    pub first_battle_at: i64,
    pub last_battle_at: i64,
    pub win_rate: Option<f64>,
    pub outcome_sample_size: usize,
    pub average_damage: Option<f64>,
    pub average_assist: Option<f64>,
    pub average_frags: Option<f64>,
    pub average_xp: Option<f64>,
    pub survival_rate: Option<f64>,
    pub wn8: SessionWn8Presentation,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleHistoryOverview {
    pub vehicles: Vec<VehicleHistorySummary>,
    pub unidentified_battle_count: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleHistorySession {
    pub session_id: String,
    pub session_started_at: i64,
    pub session_status: String,
    pub battle_count: usize,
    pub win_rate: Option<f64>,
    pub outcome_sample_size: usize,
    pub average_damage: Option<f64>,
    pub average_assist: Option<f64>,
    pub wn8: SessionWn8Presentation,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleHistoryReport {
    pub summary: VehicleHistorySummary,
    pub sessions: Vec<VehicleHistorySession>,
    /// Existing metrics grouped by map for "Where it goes best / worst" (spec 25 §5.2);
    /// maps with at least 4 counted battles, highest average damage first.
    pub maps: Vec<VehicleMapSummary>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleMapSummary {
    pub map_key: String,
    pub map_name: Option<String>,
    pub battles: usize,
    pub average_damage: f64,
    pub survival_rate: Option<f64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BaselineWindowKind {
    Last10,
    Last30,
    Last100,
}

pub(crate) fn supports_core_feature_schema(version: Option<i64>) -> bool {
    matches!(version, Some(1) | Some(2) | Some(3))
}

/// Spec 20, D4: the single definition of "this battle counts towards statistics". A battle
/// counts only when it is a strict random battle (`battleType == 1` and `bonusType == 1`, read
/// as numbers), has full results and an allowed feature schema. Every statistic uses this
/// function or its SQL form [`STATS_ELIGIBLE_SQL`]; partial and non-random battles are stored but
/// never counted.
pub(crate) fn stats_eligibility(battle: &Battle) -> bool {
    battle.data_quality == DataQuality::Full
        && supports_core_feature_schema(battle.feature_schema_version)
        && battle.battle_pace.battle_type_raw == Some(1)
        && battle.battle_pace.bonus_type_raw == Some(1)
}

/// Test fixture: battle-mode identity of a strict random battle (`battleType` 1, `bonusType` 1).
#[cfg(test)]
pub(crate) fn strict_random_pace() -> BattlePaceRecord {
    BattlePaceRecord {
        battle_type_raw: Some(1),
        bonus_type_raw: Some(1),
        ..BattlePaceRecord::default()
    }
}

/// SQL form of [`stats_eligibility`] for queries over `battles` aliased as `b`.
pub(crate) const STATS_ELIGIBLE_SQL: &str = "(b.data_quality='full' AND b.feature_schema_version IN (1,2,3) AND b.battle_type_raw=1 AND b.bonus_type_raw=1)";

impl BaselineWindowKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Last10 => "last10",
            Self::Last30 => "last30",
            Self::Last100 => "last100",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "last10" => Some(Self::Last10),
            "last30" => Some(Self::Last30),
            "last100" => Some(Self::Last100),
            _ => None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BaselineScope {
    Player,
    Vehicle,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BaselineConfidence {
    Insufficient,
    Preliminary,
    Useful,
    Reliable,
    Stable,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BaselineMetric {
    pub value: Option<f64>,
    pub eligible_battles: usize,
    pub candidate_battles: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedBaselineMetrics {
    pub damage: Option<f64>,
    pub frags: Option<f64>,
    pub spots: Option<f64>,
    pub defense: Option<f64>,
    pub win: Option<f64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BaselineWindowResult {
    pub baseline_version: String,
    pub scope: BaselineScope,
    pub window: BaselineWindowKind,
    pub cutoff: i64,
    pub vehicle_id: Option<i64>,
    pub candidate_battles: usize,
    pub eligible_battles: usize,
    pub coverage_ratio: f64,
    pub confidence: BaselineConfidence,
    pub first_battle_at: Option<i64>,
    pub last_battle_at: Option<i64>,
    pub wn8: SessionWn8Presentation,
    pub win_rate: BaselineMetric,
    pub survival_rate: BaselineMetric,
    pub average_damage: BaselineMetric,
    pub average_assist: BaselineMetric,
    pub average_frags: BaselineMetric,
    pub average_xp: BaselineMetric,
    pub normalized: NormalizedBaselineMetrics,
    pub formula_version: String,
    pub snapshot_source_version: Option<String>,
    pub snapshot_sha256: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerBaselinePresentation {
    pub baseline_version: String,
    pub cutoff: i64,
    pub windows: Vec<BaselineWindowResult>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleBaselinePresentation {
    pub baseline_version: String,
    pub cutoff: i64,
    pub vehicle_id: i64,
    pub vehicle_key: Option<String>,
    pub tank_name: Option<String>,
    pub windows: Vec<BaselineWindowResult>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BaselineComparisonState {
    Above,
    Equal,
    Below,
    Unavailable,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BaselineDelta {
    pub current_value: Option<f64>,
    pub baseline_value: Option<f64>,
    pub absolute_delta: Option<f64>,
    pub relative_delta_percent: Option<f64>,
    pub comparison_state: BaselineComparisonState,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedBaselineDeltas {
    pub damage: BaselineDelta,
    pub frags: BaselineDelta,
    pub spots: BaselineDelta,
    pub defense: BaselineDelta,
    pub win: BaselineDelta,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetricBattles {
    pub wn8: usize,
    pub damage: usize,
    pub survival: usize,
    pub assist: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VehicleSessionBaselineComparison {
    pub vehicle_id: i64,
    pub vehicle_key: Option<String>,
    pub tank_name: Option<String>,
    pub session_battles: usize,
    /// Session battles on this vehicle that count for statistics (D4), with or without XVM.
    pub session_eligible_battles: usize,
    /// Per-metric session counts (D7): WN8 needs expected values, the others do not.
    pub session_metric_battles: SessionMetricBattles,
    pub baseline: BaselineWindowResult,
    pub wn8: BaselineDelta,
    pub damage: BaselineDelta,
    pub win_rate: BaselineDelta,
    pub survival_rate: BaselineDelta,
    pub assist: BaselineDelta,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionBaselineKind {
    SingleVehicle,
    MultiVehicle,
    NoComparableBattles,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionInsightState {
    Available,
    InsufficientBaseline,
    InsufficientSession,
    Unavailable,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionInsightMetric {
    Wn8,
    Damage,
    Survival,
    Assist,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionInsightDirection {
    Positive,
    Negative,
    Neutral,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionInsightDeltaUnit {
    Percent,
    PercentagePoints,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionInsightEvidenceTextKey {
    Wn8AboveNorm,
    Wn8BelowNorm,
    DamageAboveNorm,
    DamageBelowNorm,
    SurvivalAboveNorm,
    SurvivalBelowNorm,
    AssistAboveNorm,
    AssistBelowNorm,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionInsightUnavailableReason {
    NoComparableBattles,
    BaselineTooSmall,
    SessionTooSmall,
    NoComparableMetrics,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionInsight {
    pub metric: SessionInsightMetric,
    pub direction: SessionInsightDirection,
    pub current_value: f64,
    pub baseline_value: f64,
    pub delta_value: f64,
    pub delta_unit: SessionInsightDeltaUnit,
    pub normalized_strength: f64,
    pub evidence_text_key: SessionInsightEvidenceTextKey,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionInsightSummary {
    pub version: String,
    pub state: SessionInsightState,
    pub confidence: BaselineConfidence,
    pub vehicle_id: Option<i64>,
    pub vehicle_key: Option<String>,
    pub tank_name: Option<String>,
    pub baseline_eligible_battles: usize,
    pub session_eligible_battles: usize,
    pub strongest_positive: Option<SessionInsight>,
    pub strongest_negative: Option<SessionInsight>,
    pub supporting_signals: Vec<SessionInsight>,
    pub unavailable_reason: Option<SessionInsightUnavailableReason>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionInsightPresentation {
    pub version: String,
    pub state: SessionInsightState,
    pub summaries: Vec<SessionInsightSummary>,
    pub unavailable_reason: Option<SessionInsightUnavailableReason>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionBaselineComparison {
    pub baseline_version: String,
    pub session_id: String,
    pub cutoff: i64,
    pub kind: SessionBaselineKind,
    pub session_battles: usize,
    pub global_baseline: BaselineWindowResult,
    pub global_wn8: BaselineDelta,
    pub global_normalized: NormalizedBaselineDeltas,
    pub vehicles: Vec<VehicleSessionBaselineComparison>,
    pub insights: SessionInsightPresentation,
}

impl BaselineConfidence {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Insufficient => "insufficient",
            Self::Preliminary => "preliminary",
            Self::Useful => "useful",
            Self::Reliable => "reliable",
            Self::Stable => "stable",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "insufficient" => Some(Self::Insufficient),
            "preliminary" => Some(Self::Preliminary),
            "useful" => Some(Self::Useful),
            "reliable" => Some(Self::Reliable),
            "stable" => Some(Self::Stable),
            _ => None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeType {
    DamageConsistency,
    EffectiveSurvival,
    SpottingAssistConsistency,
    StunAssistConsistency,
}

impl ChallengeType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::DamageConsistency => "damage_consistency",
            Self::EffectiveSurvival => "effective_survival",
            Self::SpottingAssistConsistency => "spotting_assist_consistency",
            Self::StunAssistConsistency => "stun_assist_consistency",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "damage_consistency" => Some(Self::DamageConsistency),
            "effective_survival" => Some(Self::EffectiveSurvival),
            "spotting_assist_consistency" => Some(Self::SpottingAssistConsistency),
            "stun_assist_consistency" => Some(Self::StunAssistConsistency),
            _ => None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeStatus {
    Active,
    Completed,
    Failed,
    Abandoned,
}

impl ChallengeStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Abandoned => "abandoned",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeContributionMetricKind {
    Damage,
    CombinedContribution,
    SpottingTrackingAssist,
    StunAssist,
}

impl ChallengeContributionMetricKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Damage => "damage",
            Self::CombinedContribution => "combined_contribution",
            Self::SpottingTrackingAssist => "spotting_tracking_assist",
            Self::StunAssist => "stun_assist",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "damage" => Some(Self::Damage),
            "combined_contribution" => Some(Self::CombinedContribution),
            "spotting_tracking_assist" => Some(Self::SpottingTrackingAssist),
            "stun_assist" => Some(Self::StunAssist),
            _ => None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeEvidenceMetric {
    Damage,
    Survival,
    SpottingTrackingAssist,
    StunAssist,
}

impl ChallengeEvidenceMetric {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Damage => "damage",
            Self::Survival => "survival",
            Self::SpottingTrackingAssist => "spotting_tracking_assist",
            Self::StunAssist => "stun_assist",
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeSuggestionUnavailableReason {
    InsufficientData,
    NoSupportedNegativeSignal,
    VehicleClassUnavailable,
    ActiveChallengeExists,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeSuggestion {
    pub suggestion_id: String,
    pub challenge_type: ChallengeType,
    pub vehicle_id: i64,
    pub vehicle_name: String,
    pub vehicle_class: VehicleClass,
    pub target_battles: usize,
    pub required_successes: usize,
    pub frozen_baseline_value: f64,
    pub threshold_value: Option<i64>,
    pub contribution_metric_kind: ChallengeContributionMetricKind,
    pub contribution_floor: Option<i64>,
    pub evidence_metric: ChallengeEvidenceMetric,
    pub evidence_delta: f64,
    pub evidence_normalized_strength: f64,
    pub baseline_confidence: BaselineConfidence,
    pub baseline_eligible_battles: usize,
    pub baseline_metric_eligible_battles: usize,
    pub baseline_version: String,
    pub baseline_cutoff: i64,
    pub baseline_window: BaselineWindowKind,
    pub insight_version: String,
    pub challenge_engine_version: String,
    pub suggestion_version: String,
    pub formula_version: String,
    pub expected_snapshot_version: String,
    pub expected_snapshot_sha256: String,
    pub vehicle_class_source: VehicleClassSource,
    pub vehicle_class_resolution_state: VehicleClassResolutionState,
    pub vehicle_class_resolver_version: String,
    pub vehicle_class_catalog_schema_version: u32,
    pub compact_descriptor_protocol_source: CompactDescriptorProtocolSource,
    pub frozen_damage_baseline: Option<f64>,
    pub frozen_survival_baseline: Option<f64>,
    pub frozen_combined_contribution_baseline: Option<f64>,
    pub frozen_spotting_assist_baseline: Option<f64>,
    pub frozen_tracking_assist_baseline: Option<f64>,
    pub frozen_stun_assist_baseline: Option<f64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeSuggestionResponse {
    pub suggestions: Vec<ChallengeSuggestion>,
    pub unavailable_reason: Option<ChallengeSuggestionUnavailableReason>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeBattlePresentation {
    pub ordinal: usize,
    pub battle_time: i64,
    pub metric_value: i64,
    pub contribution_value: Option<i64>,
    pub survived: Option<bool>,
    pub success: bool,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChallengePresentation {
    pub id: String,
    pub schema_version: u32,
    pub engine_version: String,
    pub suggestion_version: String,
    pub status: ChallengeStatus,
    pub challenge_type: ChallengeType,
    pub vehicle_id: i64,
    pub vehicle_name: String,
    pub vehicle_class: VehicleClass,
    pub vehicle_class_source: VehicleClassSource,
    pub vehicle_class_resolution_state: VehicleClassResolutionState,
    pub vehicle_class_resolver_version: String,
    pub vehicle_class_catalog_schema_version: u32,
    pub compact_descriptor_protocol_source: CompactDescriptorProtocolSource,
    pub created_at: i64,
    pub started_at: i64,
    pub eligible_after: i64,
    pub finished_at: Option<i64>,
    pub target_battles: usize,
    pub required_successes: usize,
    pub frozen_baseline_value: f64,
    pub threshold_value: Option<i64>,
    pub contribution_metric_kind: ChallengeContributionMetricKind,
    pub contribution_floor: Option<i64>,
    pub baseline_version: String,
    pub baseline_cutoff: i64,
    pub baseline_window: BaselineWindowKind,
    pub baseline_confidence: BaselineConfidence,
    pub baseline_eligible_battles: usize,
    pub completed_battles: usize,
    pub successful_battles: usize,
    pub battles: Vec<ChallengeBattlePresentation>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeHistoryPage {
    pub items: Vec<ChallengePresentation>,
    pub next_offset: Option<usize>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeDiagnosticState {
    pub active_challenge: bool,
    pub active_challenge_type: Option<ChallengeType>,
    pub active_challenge_vehicle_class: Option<VehicleClass>,
    pub active_challenge_completed_battles: Option<usize>,
    pub active_challenge_target_battles: Option<usize>,
    pub active_challenge_successful_battles: Option<usize>,
    pub active_challenge_required_successes: Option<usize>,
    pub challenge_counts_by_status: BTreeMap<String, usize>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntegrityReport {
    pub integrity_check: Vec<String>,
    pub foreign_key_check: Vec<String>,
    pub orphaned_records: usize,
    pub empty_ended_sessions: usize,
    pub certain_duplicate_records: usize,
    pub ambiguous_candidate_groups: usize,
}

impl IntegrityReport {
    pub fn is_healthy(&self) -> bool {
        self.integrity_check == ["ok"]
            && self.foreign_key_check.is_empty()
            && self.orphaned_records == 0
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleanupReport {
    pub before: IntegrityReport,
    pub after: IntegrityReport,
    pub merged_battles: usize,
    pub deleted_empty_sessions: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParserState {
    Ready,
    Waiting,
    ReplayOnly,
    Unsupported,
    Error,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceStatus {
    pub configured: bool,
    pub active: bool,
    pub paused: bool,
    pub game_root_display: Option<String>,
    pub last_replay_at: Option<i64>,
    pub parser_state: ParserState,
    pub client_version: Option<String>,
    pub partial_mode: bool,
    pub message: Option<String>,
    /// Progress of the background full import (D14); `None` when no import runs.
    #[serde(default)]
    pub import_progress: Option<ImportProgress>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub done: usize,
    pub total: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedStatus {
    pub xvm_enabled: bool,
    pub xvm_last_updated_at: Option<i64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedValuesAvailability {
    Unavailable,
    Available,
    Updating,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedValuesStatus {
    pub availability: ExpectedValuesAvailability,
    pub source: Option<String>,
    pub source_version: Option<String>,
    pub sha256: Option<String>,
    pub downloaded_at: Option<i64>,
    pub vehicle_count: Option<usize>,
    /// XVM rows skipped as invalid in the active snapshot (M8).
    pub skipped_row_count: Option<usize>,
    pub last_attempt_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_error_code: Option<String>,
    pub update_in_progress: bool,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedValuesUpdateOutcome {
    NotDue,
    Unchanged,
    Activated,
    OfflineUsingCurrent,
    RejectedUsingCurrent,
    Unavailable,
    Busy,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedValuesRefreshResult {
    pub outcome: ExpectedValuesUpdateOutcome,
    pub status: ExpectedValuesStatus,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Wn8PresentationState {
    Available,
    Unavailable,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Wn8UnavailableReason {
    PartialOrIncomplete,
    UnsupportedFeatureSchema,
    VehicleIdConflict,
    MissingVehicleId,
    VehicleNotInExpectedValues,
    MissingDamage,
    MissingFrags,
    MissingSpots,
    MissingDefense,
    MissingResult,
    InvalidNumericInput,
    ExpectedValuesUnavailable,
    NoEligibleBattles,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionWn8Presentation {
    pub state: Wn8PresentationState,
    pub value: Option<f64>,
    pub eligible_battles: usize,
    pub total_battles: usize,
    pub coverage_ratio: f64,
    pub exclusions: Wn8ExclusionCounts,
    pub formula_version: String,
    pub snapshot_source_version: Option<String>,
    pub snapshot_sha256: Option<String>,
    pub unavailable_reason: Option<Wn8UnavailableReason>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BattleWn8Presentation {
    pub battle_id: String,
    pub state: Wn8PresentationState,
    pub value: Option<f64>,
    pub exclusion_reason: Option<Wn8UnavailableReason>,
    pub formula_version: String,
    pub snapshot_source_version: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wn8Presentation {
    pub session: Option<SessionWn8Presentation>,
    pub battles: Vec<BattleWn8Presentation>,
    pub expected_values: ExpectedValuesStatus,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSessionWn8Presentation {
    pub session_id: String,
    pub wn8: SessionWn8Presentation,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceTrendRange {
    Last10,
    Last30,
    Last100,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceTrendMetric {
    Wn8,
    Damage,
    Survival,
    WinRate,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    Up,
    Down,
    Neutral,
    Unavailable,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "vehicleId", rename_all = "snake_case")]
pub enum TrendScope {
    Global,
    Vehicle(i64),
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceTrendPoint {
    pub ordinal: usize,
    pub battle_time: i64,
    pub value: Option<f64>,
    pub sample_size: usize,
    pub coverage: f64,
    pub vehicle_id: Option<i64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceMetricTrend {
    pub metric: PerformanceTrendMetric,
    pub current_value: Option<f64>,
    pub previous_value: Option<f64>,
    pub absolute_delta: Option<f64>,
    pub relative_delta: Option<f64>,
    pub percentage_point_delta: Option<f64>,
    pub direction: TrendDirection,
    pub current_sample_size: usize,
    pub previous_sample_size: usize,
    pub current_coverage: f64,
    pub previous_coverage: f64,
    pub meaningful: bool,
    pub points: Vec<PerformanceTrendPoint>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceTrendSignal {
    pub metric: PerformanceTrendMetric,
    pub direction: TrendDirection,
    pub current_value: f64,
    pub previous_value: f64,
    pub delta: f64,
    pub normalized_strength: f64,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceTrendReport {
    pub version: String,
    pub range: PerformanceTrendRange,
    pub scope: TrendScope,
    pub current_battle_count: usize,
    pub previous_battle_count: usize,
    pub metrics: Vec<PerformanceMetricTrend>,
    pub highlight: Option<PerformanceTrendSignal>,
    pub lowlight: Option<PerformanceTrendSignal>,
    pub generated_at: i64,
}

impl Default for SourceStatus {
    fn default() -> Self {
        Self {
            configured: false,
            active: false,
            paused: false,
            game_root_display: None,
            last_replay_at: None,
            parser_state: ParserState::Waiting,
            client_version: None,
            partial_mode: true,
            message: None,
            import_progress: None,
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub configured: bool,
    pub onboarding_complete: bool,
    pub inactivity_minutes: u32,
    pub source_status: SourceStatus,
    pub advanced_status: AdvancedStatus,
    pub active_session: Option<Session>,
    pub recent_battles: Vec<BattleView>,
    pub recent_sessions: Vec<Session>,
    pub wn8_presentation: Wn8Presentation,
    pub active_challenge: Option<ChallengePresentation>,
    pub latest_finished_challenge: Option<ChallengePresentation>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDiagnostics {
    pub schema_version: i64,
    pub integrity_check: Vec<String>,
    pub foreign_key_violation_count: usize,
    pub settings: usize,
    pub sessions: usize,
    pub battles: usize,
    pub full_battles: usize,
    pub partial_battles: usize,
    pub ambiguous_battles: usize,
    pub replay_files: usize,
    pub active_session_exists: bool,
    pub parser_error_counts: BTreeMap<String, usize>,
    /// Numbers of the one-time v9 → v10 rebuild, when this database was rebuilt (D5).
    #[serde(default)]
    pub rebuild_report: Option<crate::legacy::RebuildReport>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticTrackerStatus {
    pub configured: bool,
    pub active: bool,
    pub paused: bool,
    pub parser_state: String,
    pub client_version: Option<String>,
    pub partial_mode: bool,
    pub client_region: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticExpectedValuesStatus {
    pub availability: String,
    pub source: Option<String>,
    pub source_version: Option<String>,
    pub sha256: Option<String>,
    pub vehicle_count: Option<usize>,
    /// XVM rows skipped as invalid in the active snapshot (M8).
    pub skipped_row_count: Option<usize>,
    pub last_attempt_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_error_code: Option<String>,
    pub update_in_progress: bool,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub schema_version: u32,
    pub app_version: String,
    pub release_channel: String,
    pub generated_at: i64,
    pub language: String,
    pub operating_system: String,
    pub architecture: String,
    pub tracker: DiagnosticTrackerStatus,
    pub expected_values: DiagnosticExpectedValuesStatus,
    pub vehicle_class_catalog_available: bool,
    pub vehicle_class_catalog_schema_version: u32,
    pub vehicle_class_resolver_version: String,
    pub vehicle_class_protocol_source: CompactDescriptorProtocolSource,
    pub vehicle_class_catalog_vehicle_count: usize,
    pub vehicle_class_invalid_record_count: usize,
    pub vehicle_class_unresolved_current_vehicle_count: usize,
    pub vehicle_class_conflict_current_vehicle_count: usize,
    pub vehicle_class_update_in_progress: bool,
    pub vehicle_class_last_error_code: Option<String>,
    pub active_challenge: bool,
    pub challenge_schema_version: u32,
    pub challenge_engine_version: String,
    pub active_challenge_type: Option<ChallengeType>,
    pub active_challenge_vehicle_class: Option<VehicleClass>,
    pub active_challenge_completed_battles: Option<usize>,
    pub active_challenge_target_battles: Option<usize>,
    pub active_challenge_successful_battles: Option<usize>,
    pub active_challenge_required_successes: Option<usize>,
    pub challenge_counts_by_status: BTreeMap<String, usize>,
    pub challenge_last_error_code: Option<String>,
    pub database: DatabaseDiagnostics,
    pub recent_error_codes: Vec<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalImportResult {
    pub scanned: usize,
    pub imported: usize,
    pub duplicates: usize,
    pub partial: usize,
}
