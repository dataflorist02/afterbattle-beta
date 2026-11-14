use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::models::DataQuality;

pub(crate) const TEAM_RELATIVE_FACT_SCHEMA_VERSION: i64 = 1;
#[cfg(test)]
pub(crate) const TEAM_RELATIVE_CONTRACT_VERSION: &str = "team-relative-results-v1";
const EXPECTED_TEAM_SIZE: i64 = 15;
const EXPECTED_ROSTER_SIZE: usize = 30;
const MAX_DAMAGE: i64 = 200_000;
const MAX_TEAM_DAMAGE: i64 = 3_000_000;
const MAX_FRAGS: i64 = 15;
const MAX_XP: i64 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeamRelativeCommonReason {
    PartialReplay,
    ResultsProvenanceMissing,
    ParserTooOld,
    UnsupportedFeatureSchema,
    UnsupportedMode,
    MissingInitialRoster,
    MissingFinalRoster,
    RosterSizeConflict,
    RosterKeyConflict,
    DuplicateRollout,
    InvalidTeamAssignment,
    OwnPlayerMissing,
    OwnPlayerAmbiguous,
    PlayerRowConflict,
    NumericOverflow,
}

impl TeamRelativeCommonReason {
    pub(crate) const ALL: [Self; 15] = [
        Self::PartialReplay,
        Self::ResultsProvenanceMissing,
        Self::ParserTooOld,
        Self::UnsupportedFeatureSchema,
        Self::UnsupportedMode,
        Self::MissingInitialRoster,
        Self::MissingFinalRoster,
        Self::RosterSizeConflict,
        Self::RosterKeyConflict,
        Self::DuplicateRollout,
        Self::InvalidTeamAssignment,
        Self::OwnPlayerMissing,
        Self::OwnPlayerAmbiguous,
        Self::PlayerRowConflict,
        Self::NumericOverflow,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::PartialReplay => "partial_replay",
            Self::ResultsProvenanceMissing => "results_provenance_missing",
            Self::ParserTooOld => "parser_too_old",
            Self::UnsupportedFeatureSchema => "unsupported_feature_schema",
            Self::UnsupportedMode => "unsupported_mode",
            Self::MissingInitialRoster => "missing_initial_roster",
            Self::MissingFinalRoster => "missing_final_roster",
            Self::RosterSizeConflict => "roster_size_conflict",
            Self::RosterKeyConflict => "roster_key_conflict",
            Self::DuplicateRollout => "duplicate_rollout",
            Self::InvalidTeamAssignment => "invalid_team_assignment",
            Self::OwnPlayerMissing => "own_player_missing",
            Self::OwnPlayerAmbiguous => "own_player_ambiguous",
            Self::PlayerRowConflict => "player_row_conflict",
            Self::NumericOverflow => "numeric_overflow",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeamRelativeDamageReason {
    Missing,
    OutOfRange,
    Conflict,
}

impl TeamRelativeDamageReason {
    pub(crate) const ALL: [Self; 3] = [Self::Missing, Self::OutOfRange, Self::Conflict];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "damage_missing",
            Self::OutOfRange => "damage_out_of_range",
            Self::Conflict => "damage_conflict",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeamRelativeFragsReason {
    Missing,
    OutOfRange,
    Conflict,
}

impl TeamRelativeFragsReason {
    pub(crate) const ALL: [Self; 3] = [Self::Missing, Self::OutOfRange, Self::Conflict];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "frags_missing",
            Self::OutOfRange => "frags_out_of_range",
            Self::Conflict => "frags_conflict",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TeamRelativeXpReason {
    Missing,
    OutOfRange,
    Conflict,
}

impl TeamRelativeXpReason {
    pub(crate) const ALL: [Self; 3] = [Self::Missing, Self::OutOfRange, Self::Conflict];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "xp_missing",
            Self::OutOfRange => "xp_out_of_range",
            Self::Conflict => "xp_conflict",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_str() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TeamRelativeFactsV1 {
    pub(crate) common_reason: Option<TeamRelativeCommonReason>,
    pub(crate) team_player_count: Option<i64>,
    pub(crate) team_damage_total: Option<i64>,
    pub(crate) damage_greater_count: Option<i64>,
    pub(crate) damage_equal_count: Option<i64>,
    pub(crate) damage_reason: Option<TeamRelativeDamageReason>,
    pub(crate) team_frags_total: Option<i64>,
    pub(crate) frags_reason: Option<TeamRelativeFragsReason>,
    pub(crate) xp_greater_count: Option<i64>,
    pub(crate) xp_equal_count: Option<i64>,
    pub(crate) xp_reason: Option<TeamRelativeXpReason>,
}

impl TeamRelativeFactsV1 {
    pub(crate) fn common_ineligible(reason: TeamRelativeCommonReason) -> Self {
        Self {
            common_reason: Some(reason),
            team_player_count: None,
            team_damage_total: None,
            damage_greater_count: None,
            damage_equal_count: None,
            damage_reason: None,
            team_frags_total: None,
            frags_reason: None,
            xp_greater_count: None,
            xp_equal_count: None,
            xp_reason: None,
        }
    }

    fn eligible() -> Self {
        Self {
            common_reason: None,
            team_player_count: Some(EXPECTED_TEAM_SIZE),
            team_damage_total: None,
            damage_greater_count: None,
            damage_equal_count: None,
            damage_reason: None,
            team_frags_total: None,
            frags_reason: None,
            xp_greater_count: None,
            xp_equal_count: None,
            xp_reason: None,
        }
    }
}

#[derive(Clone)]
pub(crate) struct TeamRelativeParseInput<'a> {
    pub(crate) data_quality: DataQuality,
    pub(crate) data_completeness: Option<&'a str>,
    pub(crate) replay_source: Option<&'a str>,
    pub(crate) parser_version: Option<i64>,
    pub(crate) feature_schema_version: Option<i64>,
    pub(crate) metadata: &'a Value,
    pub(crate) results: Option<&'a Value>,
    pub(crate) canonical_damage: Option<i64>,
    pub(crate) canonical_frags: Option<i64>,
    pub(crate) canonical_original_xp: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scalar {
    Missing,
    OutOfRange,
    Value(i64),
}

fn scalar(object: &Map<String, Value>, key: &str, maximum: i64) -> Scalar {
    let Some(value) = object.get(key) else {
        return Scalar::Missing;
    };
    let Some(value) = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
    else {
        return Scalar::OutOfRange;
    };
    if !(0..=maximum).contains(&value) {
        Scalar::OutOfRange
    } else {
        Scalar::Value(value)
    }
}

fn checked_counts(values: &[i64], own: i64) -> Option<(i64, i64)> {
    let greater = i64::try_from(values.iter().filter(|value| **value > own).count()).ok()?;
    let equal = i64::try_from(values.iter().filter(|value| **value == own).count()).ok()?;
    (greater <= 14 && (1..=15).contains(&equal) && greater.checked_add(equal)? <= 15)
        .then_some((greater, equal))
}

type CommonContext<'a> = (
    &'a [Value],
    &'a Map<String, Value>,
    &'a Map<String, Value>,
    &'a Map<String, Value>,
    &'a Map<String, Value>,
    &'a str,
    i64,
);

fn common_context<'a>(
    input: &'a TeamRelativeParseInput<'a>,
) -> Result<CommonContext<'a>, TeamRelativeCommonReason> {
    if input.data_quality != DataQuality::Full {
        return Err(TeamRelativeCommonReason::PartialReplay);
    }
    if input.data_completeness != Some("results") || input.replay_source != Some("replay_results") {
        return Err(TeamRelativeCommonReason::ResultsProvenanceMissing);
    }
    if input.parser_version.unwrap_or_default() < 7 {
        return Err(TeamRelativeCommonReason::ParserTooOld);
    }
    if input.feature_schema_version != Some(3) {
        return Err(TeamRelativeCommonReason::UnsupportedFeatureSchema);
    }
    let sections = input
        .results
        .and_then(Value::as_array)
        .ok_or(TeamRelativeCommonReason::MissingFinalRoster)?;
    let summary = sections
        .first()
        .and_then(Value::as_object)
        .ok_or(TeamRelativeCommonReason::MissingFinalRoster)?;
    let bonus_type = summary
        .get("common")
        .and_then(Value::as_object)
        .and_then(|common| common.get("bonusType"))
        .and_then(Value::as_i64);
    if input.metadata.get("battleType").and_then(Value::as_i64) != Some(1)
        || bonus_type != Some(1)
        || input.metadata.get("gameplayID").and_then(Value::as_str) != Some("ctf")
    {
        return Err(TeamRelativeCommonReason::UnsupportedMode);
    }
    let initial = input
        .metadata
        .get("vehicles")
        .and_then(Value::as_object)
        .ok_or(TeamRelativeCommonReason::MissingInitialRoster)?;
    let final_roster = summary
        .get("vehicles")
        .and_then(Value::as_object)
        .ok_or(TeamRelativeCommonReason::MissingFinalRoster)?;
    if initial.len() != EXPECTED_ROSTER_SIZE || final_roster.len() != EXPECTED_ROSTER_SIZE {
        return Err(TeamRelativeCommonReason::RosterSizeConflict);
    }

    let mut initial_teams = HashMap::with_capacity(EXPECTED_ROSTER_SIZE);
    let mut initial_team_counts = [0_i64; 2];
    for (arena_key, row) in initial {
        let team = row
            .as_object()
            .and_then(|row| row.get("team"))
            .and_then(Value::as_i64)
            .ok_or(TeamRelativeCommonReason::InvalidTeamAssignment)?;
        let index = match team {
            1 => 0,
            2 => 1,
            _ => return Err(TeamRelativeCommonReason::InvalidTeamAssignment),
        };
        initial_team_counts[index] = initial_team_counts[index]
            .checked_add(1)
            .ok_or(TeamRelativeCommonReason::NumericOverflow)?;
        initial_teams.insert(arena_key.as_str(), team);
    }
    if initial_team_counts != [EXPECTED_TEAM_SIZE, EXPECTED_TEAM_SIZE] {
        return Err(TeamRelativeCommonReason::RosterSizeConflict);
    }

    let initial_keys = initial.keys().map(String::as_str).collect::<HashSet<_>>();
    let final_keys = final_roster
        .keys()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    if initial_keys != final_keys {
        return Err(TeamRelativeCommonReason::RosterKeyConflict);
    }

    let mut final_rows = HashMap::with_capacity(EXPECTED_ROSTER_SIZE);
    let mut final_team_counts = [0_i64; 2];
    for (arena_key, rows) in final_roster {
        let rows = rows
            .as_array()
            .ok_or(TeamRelativeCommonReason::MissingFinalRoster)?;
        if rows.len() != 1 {
            return Err(TeamRelativeCommonReason::DuplicateRollout);
        }
        let row = rows
            .first()
            .and_then(Value::as_object)
            .ok_or(TeamRelativeCommonReason::MissingFinalRoster)?;
        let team = row
            .get("team")
            .and_then(Value::as_i64)
            .ok_or(TeamRelativeCommonReason::InvalidTeamAssignment)?;
        let index = match team {
            1 => 0,
            2 => 1,
            _ => return Err(TeamRelativeCommonReason::InvalidTeamAssignment),
        };
        if initial_teams.get(arena_key.as_str()) != Some(&team) {
            return Err(TeamRelativeCommonReason::InvalidTeamAssignment);
        }
        final_team_counts[index] = final_team_counts[index]
            .checked_add(1)
            .ok_or(TeamRelativeCommonReason::NumericOverflow)?;
        final_rows.insert(arena_key.as_str(), row);
    }
    if final_team_counts != [EXPECTED_TEAM_SIZE, EXPECTED_TEAM_SIZE] {
        return Err(TeamRelativeCommonReason::RosterSizeConflict);
    }

    let personal = summary
        .get("personal")
        .and_then(Value::as_object)
        .ok_or(TeamRelativeCommonReason::OwnPlayerMissing)?;
    let personal_rows = personal
        .iter()
        .filter(|(key, value)| key.as_str() != "avatar" && value.is_object())
        .collect::<Vec<_>>();
    let (personal_key, personal_row) = match personal_rows.as_slice() {
        [] => return Err(TeamRelativeCommonReason::OwnPlayerMissing),
        [row] => *row,
        _ => return Err(TeamRelativeCommonReason::OwnPlayerAmbiguous),
    };
    let personal_row = personal_row
        .as_object()
        .ok_or(TeamRelativeCommonReason::OwnPlayerMissing)?;
    let own_account = personal_row
        .get("accountDBID")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .ok_or(TeamRelativeCommonReason::OwnPlayerMissing)?;
    let own_matches = final_rows
        .iter()
        .filter(|(_, row)| {
            row.get("accountDBID")
                .and_then(Value::as_i64)
                .is_some_and(|value| value > 0 && value == own_account)
        })
        .map(|(arena_key, row)| (*arena_key, *row))
        .collect::<Vec<_>>();
    let (own_arena_key, own_row) = match own_matches.as_slice() {
        [] => return Err(TeamRelativeCommonReason::OwnPlayerMissing),
        [row] => *row,
        _ => return Err(TeamRelativeCommonReason::OwnPlayerAmbiguous),
    };
    let own_team = own_row
        .get("team")
        .and_then(Value::as_i64)
        .filter(|value| matches!(value, 1 | 2))
        .ok_or(TeamRelativeCommonReason::InvalidTeamAssignment)?;
    let personal_team = personal_row.get("team").and_then(Value::as_i64);
    let personal_descriptor = personal_row.get("typeCompDescr").and_then(Value::as_i64);
    let final_descriptor = own_row.get("typeCompDescr").and_then(Value::as_i64);
    if initial_teams.get(own_arena_key) != Some(&own_team)
        || personal_team != Some(own_team)
        || personal_descriptor.is_none()
        || final_descriptor.is_none()
        || personal_descriptor != final_descriptor
        || personal_key.is_empty()
    {
        return Err(TeamRelativeCommonReason::PlayerRowConflict);
    }
    Ok((
        sections,
        summary,
        personal_row,
        own_row,
        final_roster,
        own_arena_key,
        own_team,
    ))
}

pub(crate) fn extract_team_relative_facts(
    input: &TeamRelativeParseInput<'_>,
) -> TeamRelativeFactsV1 {
    let (sections, _summary, personal, own_row, final_roster, own_arena_key, own_team) =
        match common_context(input) {
            Ok(context) => context,
            Err(reason) => return TeamRelativeFactsV1::common_ineligible(reason),
        };
    let team_rows = final_roster
        .values()
        .filter_map(Value::as_array)
        .filter_map(|rows| rows.first())
        .filter_map(Value::as_object)
        .filter(|row| row.get("team").and_then(Value::as_i64) == Some(own_team))
        .collect::<Vec<_>>();
    if team_rows.len() != EXPECTED_TEAM_SIZE as usize {
        return TeamRelativeFactsV1::common_ineligible(
            TeamRelativeCommonReason::RosterSizeConflict,
        );
    }

    let mut facts = TeamRelativeFactsV1::eligible();

    let canonical_damage = match scalar(personal, "damageDealt", MAX_DAMAGE) {
        Scalar::Missing => {
            facts.damage_reason = Some(TeamRelativeDamageReason::Missing);
            None
        }
        Scalar::OutOfRange => {
            facts.damage_reason = Some(TeamRelativeDamageReason::OutOfRange);
            None
        }
        Scalar::Value(value) if input.canonical_damage == Some(value) => Some(value),
        Scalar::Value(_) => {
            facts.damage_reason = Some(TeamRelativeDamageReason::Conflict);
            None
        }
    };
    if let Some(canonical_damage) = canonical_damage {
        let mut values = Vec::with_capacity(EXPECTED_TEAM_SIZE as usize);
        let mut failure = None;
        for row in &team_rows {
            match scalar(row, "damageDealt", MAX_DAMAGE) {
                Scalar::Missing => {
                    failure = Some(TeamRelativeDamageReason::Missing);
                    break;
                }
                Scalar::OutOfRange => {
                    failure = Some(TeamRelativeDamageReason::OutOfRange);
                    break;
                }
                Scalar::Value(value) => values.push(value),
            }
        }
        if let Some(reason) = failure {
            facts.damage_reason = Some(reason);
        } else if scalar(own_row, "damageDealt", MAX_DAMAGE) != Scalar::Value(canonical_damage) {
            facts.damage_reason = Some(TeamRelativeDamageReason::Conflict);
        } else {
            let total = values
                .iter()
                .try_fold(0_i64, |total, value| total.checked_add(*value));
            match total {
                None => {
                    return TeamRelativeFactsV1::common_ineligible(
                        TeamRelativeCommonReason::NumericOverflow,
                    );
                }
                Some(total)
                    if total <= MAX_TEAM_DAMAGE
                        && total >= canonical_damage
                        && checked_counts(&values, canonical_damage).is_some() =>
                {
                    let (greater, equal) =
                        checked_counts(&values, canonical_damage).expect("checked above");
                    facts.team_damage_total = Some(total);
                    facts.damage_greater_count = Some(greater);
                    facts.damage_equal_count = Some(equal);
                }
                Some(_) => facts.damage_reason = Some(TeamRelativeDamageReason::Conflict),
            }
        }
    }

    let personal_frags = match scalar(personal, "kills", MAX_FRAGS) {
        Scalar::Missing => {
            facts.frags_reason = Some(TeamRelativeFragsReason::Missing);
            None
        }
        Scalar::OutOfRange => {
            facts.frags_reason = Some(TeamRelativeFragsReason::OutOfRange);
            None
        }
        Scalar::Value(value) if input.canonical_frags == Some(value) => Some(value),
        Scalar::Value(value) => {
            let confirmation = sections
                .get(2)
                .and_then(Value::as_object)
                .and_then(|section| section.get(own_arena_key))
                .and_then(Value::as_object)
                .and_then(|row| row.get("frags"));
            facts.frags_reason = Some(match confirmation {
                Some(raw) if raw.as_i64().is_some_and(|confirmed| confirmed != value) => {
                    TeamRelativeFragsReason::Conflict
                }
                Some(_) => TeamRelativeFragsReason::OutOfRange,
                None => TeamRelativeFragsReason::Conflict,
            });
            None
        }
    };
    if let Some(personal_frags) = personal_frags {
        let mut values = Vec::with_capacity(EXPECTED_TEAM_SIZE as usize);
        let mut failure = None;
        for row in &team_rows {
            match scalar(row, "kills", MAX_FRAGS) {
                Scalar::Missing => {
                    failure = Some(TeamRelativeFragsReason::Missing);
                    break;
                }
                Scalar::OutOfRange => {
                    failure = Some(TeamRelativeFragsReason::OutOfRange);
                    break;
                }
                Scalar::Value(value) => values.push(value),
            }
        }
        if let Some(reason) = failure {
            facts.frags_reason = Some(reason);
        } else if scalar(own_row, "kills", MAX_FRAGS) != Scalar::Value(personal_frags) {
            facts.frags_reason = Some(TeamRelativeFragsReason::Conflict);
        } else {
            match values
                .iter()
                .try_fold(0_i64, |total, value| total.checked_add(*value))
            {
                None => {
                    return TeamRelativeFactsV1::common_ineligible(
                        TeamRelativeCommonReason::NumericOverflow,
                    );
                }
                Some(total) if total <= MAX_FRAGS && total >= personal_frags => {
                    facts.team_frags_total = Some(total);
                }
                Some(_) => facts.frags_reason = Some(TeamRelativeFragsReason::Conflict),
            }
        }
    }

    let canonical_xp = match scalar(personal, "originalXP", MAX_XP) {
        Scalar::Missing => {
            facts.xp_reason = Some(TeamRelativeXpReason::Missing);
            None
        }
        Scalar::OutOfRange => {
            facts.xp_reason = Some(TeamRelativeXpReason::OutOfRange);
            None
        }
        Scalar::Value(value) if input.canonical_original_xp == Some(value) => Some(value),
        Scalar::Value(_) => {
            facts.xp_reason = Some(TeamRelativeXpReason::Conflict);
            None
        }
    };
    if let Some(canonical_xp) = canonical_xp {
        let mut values = Vec::with_capacity(EXPECTED_TEAM_SIZE as usize);
        let mut failure = None;
        for row in &team_rows {
            match scalar(row, "xp", MAX_XP) {
                Scalar::Missing => {
                    failure = Some(TeamRelativeXpReason::Missing);
                    break;
                }
                Scalar::OutOfRange => {
                    failure = Some(TeamRelativeXpReason::OutOfRange);
                    break;
                }
                Scalar::Value(value) => values.push(value),
            }
        }
        if let Some(reason) = failure {
            facts.xp_reason = Some(reason);
        } else if scalar(own_row, "xp", MAX_XP) != Scalar::Value(canonical_xp) {
            facts.xp_reason = Some(TeamRelativeXpReason::Conflict);
        } else if let Some((greater, equal)) = checked_counts(&values, canonical_xp) {
            facts.xp_greater_count = Some(greater);
            facts.xp_equal_count = Some(equal);
        } else {
            facts.xp_reason = Some(TeamRelativeXpReason::Conflict);
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn synthetic_input() -> (Value, Value) {
        let mut initial = Map::new();
        let mut final_roster = Map::new();
        for index in 1..=30_i64 {
            let team = if index <= 15 { 1 } else { 2 };
            let own = index == 15;
            let damage = if own {
                3_000
            } else if team == 1 {
                800
            } else {
                900
            };
            let kills = if own {
                2
            } else if team == 1 && index <= 3 {
                1
            } else {
                0
            };
            let xp = if own { 900 } else { 700 + index };
            initial.insert(index.to_string(), json!({"team": team}));
            final_roster.insert(
                index.to_string(),
                json!([{
                    "accountDBID": if own { 7001 } else { 10_000 + index },
                    "team": team,
                    "typeCompDescr": if own { 33345 } else { 100_000 + index },
                    "damageDealt": damage,
                    "kills": kills,
                    "xp": xp,
                    "deathReason": -1
                }]),
            );
        }
        let metadata = json!({
            "battleType": 1,
            "gameplayID": "ctf",
            "vehicles": initial
        });
        let results = json!([{
            "personal": {
                "33345": {
                    "accountDBID": 7001,
                    "team": 1,
                    "typeCompDescr": 33345,
                    "damageDealt": 3000,
                    "kills": 2,
                    "originalXP": 900
                },
                "avatar": {}
            },
            "vehicles": final_roster,
            "common": {"bonusType": 1}
        }, {
            "15": {"frags": 99}
        }, {
            "15": {"frags": 2}
        }]);
        (metadata, results)
    }

    fn facts(metadata: &Value, results: &Value) -> TeamRelativeFactsV1 {
        facts_with_canonical(metadata, results, Some(3_000), Some(2), Some(900))
    }

    fn facts_with_canonical(
        metadata: &Value,
        results: &Value,
        damage: Option<i64>,
        frags: Option<i64>,
        xp: Option<i64>,
    ) -> TeamRelativeFactsV1 {
        extract_team_relative_facts(&TeamRelativeParseInput {
            data_quality: DataQuality::Full,
            data_completeness: Some("results"),
            replay_source: Some("replay_results"),
            parser_version: Some(7),
            feature_schema_version: Some(3),
            metadata,
            results: Some(results),
            canonical_damage: damage,
            canonical_frags: frags,
            canonical_original_xp: xp,
        })
    }

    #[test]
    fn golden_random_fifteen_per_team_produces_anonymous_facts() {
        let (metadata, results) = synthetic_input();
        let actual = facts(&metadata, &results);
        assert_eq!(actual.common_reason, None);
        assert_eq!(actual.team_player_count, Some(15));
        assert_eq!(actual.team_damage_total, Some(14_200));
        assert_eq!(actual.damage_greater_count, Some(0));
        assert_eq!(actual.damage_equal_count, Some(1));
        assert_eq!(actual.team_frags_total, Some(5));
        assert_eq!(actual.xp_greater_count, Some(0));
        assert_eq!(actual.xp_equal_count, Some(1));
        let debug = format!("{actual:?}");
        for forbidden in ["account", "arena", "nickname", "roster", "33345", "7001"] {
            assert!(!debug.contains(forbidden));
        }
    }

    #[test]
    fn metric_failures_are_independent_and_zero_is_not_missing() {
        let (metadata, mut results) = synthetic_input();
        results[0]["personal"]["33345"]["damageDealt"] = json!(0);
        results[0]["vehicles"]["15"][0]["damageDealt"] = json!(0);
        for index in 1..=15 {
            results[0]["vehicles"][index.to_string()][0]["damageDealt"] = json!(0);
        }
        results[0]["vehicles"]["1"][0]
            .as_object_mut()
            .unwrap()
            .remove("kills");
        results[0]["vehicles"]["15"][0]["xp"] = json!(901);
        let actual = extract_team_relative_facts(&TeamRelativeParseInput {
            data_quality: DataQuality::Full,
            data_completeness: Some("results"),
            replay_source: Some("replay_results"),
            parser_version: Some(7),
            feature_schema_version: Some(3),
            metadata: &metadata,
            results: Some(&results),
            canonical_damage: Some(0),
            canonical_frags: Some(2),
            canonical_original_xp: Some(900),
        });
        assert_eq!(actual.team_damage_total, Some(0));
        assert_eq!(actual.damage_greater_count, Some(0));
        assert_eq!(actual.damage_equal_count, Some(15));
        assert_eq!(actual.damage_reason, None);
        assert_eq!(actual.team_frags_total, None);
        assert_eq!(actual.frags_reason, Some(TeamRelativeFragsReason::Missing));
        assert_eq!(actual.xp_greater_count, None);
        assert_eq!(actual.xp_reason, Some(TeamRelativeXpReason::Conflict));
    }

    #[test]
    fn r1_frags_are_ignored_and_r2_conflict_is_local() {
        let (metadata, mut results) = synthetic_input();
        results[1]["15"]["frags"] = json!(0);
        assert_eq!(facts(&metadata, &results).team_frags_total, Some(5));

        results[2]["15"]["frags"] = json!(3);
        let actual = extract_team_relative_facts(&TeamRelativeParseInput {
            data_quality: DataQuality::Full,
            data_completeness: Some("results"),
            replay_source: Some("replay_results"),
            parser_version: Some(7),
            feature_schema_version: Some(3),
            metadata: &metadata,
            results: Some(&results),
            canonical_damage: Some(3_000),
            canonical_frags: None,
            canonical_original_xp: Some(900),
        });
        assert_eq!(actual.frags_reason, Some(TeamRelativeFragsReason::Conflict));
        assert_eq!(actual.team_frags_total, None);
        assert_eq!(actual.team_damage_total, Some(14_200));
        assert_eq!(actual.xp_greater_count, Some(0));
    }

    #[test]
    fn common_reasons_follow_fail_closed_structure() {
        let (metadata, results) = synthetic_input();
        let partial = extract_team_relative_facts(&TeamRelativeParseInput {
            data_quality: DataQuality::Partial,
            data_completeness: None,
            replay_source: Some("replay_metadata"),
            parser_version: Some(7),
            feature_schema_version: None,
            metadata: &metadata,
            results: None,
            canonical_damage: None,
            canonical_frags: None,
            canonical_original_xp: None,
        });
        assert_eq!(
            partial,
            TeamRelativeFactsV1::common_ineligible(TeamRelativeCommonReason::PartialReplay)
        );

        let mut unsupported = metadata.clone();
        unsupported["gameplayID"] = json!("assault");
        assert_eq!(
            facts(&unsupported, &results).common_reason,
            Some(TeamRelativeCommonReason::UnsupportedMode)
        );

        let mut key_mismatch = results.clone();
        let row = key_mismatch[0]["vehicles"]
            .as_object_mut()
            .unwrap()
            .remove("1")
            .unwrap();
        key_mismatch[0]["vehicles"]
            .as_object_mut()
            .unwrap()
            .insert("31".into(), row);
        assert_eq!(
            facts(&metadata, &key_mismatch).common_reason,
            Some(TeamRelativeCommonReason::RosterKeyConflict)
        );

        let mut duplicate = results.clone();
        let row = duplicate[0]["vehicles"]["1"][0].clone();
        duplicate[0]["vehicles"]["1"]
            .as_array_mut()
            .unwrap()
            .push(row);
        assert_eq!(
            facts(&metadata, &duplicate).common_reason,
            Some(TeamRelativeCommonReason::DuplicateRollout)
        );
    }

    #[test]
    fn exact_account_join_does_not_fall_back_to_descriptor() {
        let (metadata, mut results) = synthetic_input();
        results[0]["vehicles"]["15"][0]["accountDBID"] = json!(0);
        assert_eq!(
            facts(&metadata, &results).common_reason,
            Some(TeamRelativeCommonReason::OwnPlayerMissing)
        );

        let (metadata, mut results) = synthetic_input();
        results[0]["vehicles"]["14"][0]["accountDBID"] = json!(7001);
        assert_eq!(
            facts(&metadata, &results).common_reason,
            Some(TeamRelativeCommonReason::OwnPlayerAmbiguous)
        );
    }

    #[test]
    fn damage_missing_out_of_range_and_conflict_do_not_block_other_metrics() {
        let (metadata, mut missing) = synthetic_input();
        missing[0]["personal"]["33345"]
            .as_object_mut()
            .unwrap()
            .remove("damageDealt");
        let actual = facts_with_canonical(&metadata, &missing, None, Some(2), Some(900));
        assert_eq!(
            actual.damage_reason,
            Some(TeamRelativeDamageReason::Missing)
        );
        assert_eq!(actual.team_frags_total, Some(5));
        assert_eq!(actual.xp_greater_count, Some(0));

        let (_, mut invalid) = synthetic_input();
        invalid[0]["vehicles"]["1"][0]["damageDealt"] = json!(-1);
        let actual = facts(&metadata, &invalid);
        assert_eq!(
            actual.damage_reason,
            Some(TeamRelativeDamageReason::OutOfRange)
        );
        assert_eq!(actual.team_frags_total, Some(5));

        let (_, mut mismatch) = synthetic_input();
        mismatch[0]["vehicles"]["15"][0]["damageDealt"] = json!(2_999);
        let actual = facts(&metadata, &mismatch);
        assert_eq!(
            actual.damage_reason,
            Some(TeamRelativeDamageReason::Conflict)
        );
        assert_eq!(actual.team_frags_total, Some(5));
        assert_eq!(actual.xp_greater_count, Some(0));
    }

    #[test]
    fn frag_zero_missing_out_of_range_and_own_mismatch_are_exact() {
        let (metadata, mut zero) = synthetic_input();
        zero[0]["personal"]["33345"]["kills"] = json!(0);
        zero[2]["15"]["frags"] = json!(0);
        for index in 1..=15 {
            zero[0]["vehicles"][index.to_string()][0]["kills"] = json!(0);
        }
        let actual = facts_with_canonical(&metadata, &zero, Some(3_000), Some(0), Some(900));
        assert_eq!(actual.team_frags_total, Some(0));
        assert_eq!(actual.frags_reason, None);

        let (_, mut missing) = synthetic_input();
        missing[0]["personal"]["33345"]
            .as_object_mut()
            .unwrap()
            .remove("kills");
        let actual = facts_with_canonical(&metadata, &missing, Some(3_000), None, Some(900));
        assert_eq!(actual.frags_reason, Some(TeamRelativeFragsReason::Missing));
        assert_eq!(actual.team_damage_total, Some(14_200));

        let (_, mut invalid) = synthetic_input();
        invalid[0]["vehicles"]["1"][0]["kills"] = json!(16);
        assert_eq!(
            facts(&metadata, &invalid).frags_reason,
            Some(TeamRelativeFragsReason::OutOfRange)
        );

        let (_, mut mismatch) = synthetic_input();
        mismatch[0]["vehicles"]["15"][0]["kills"] = json!(1);
        assert_eq!(
            facts(&metadata, &mismatch).frags_reason,
            Some(TeamRelativeFragsReason::Conflict)
        );
    }

    #[test]
    fn xp_competition_rank_supports_ties_and_metric_local_failures() {
        let (metadata, mut tied) = synthetic_input();
        tied[0]["vehicles"]["1"][0]["xp"] = json!(950);
        tied[0]["vehicles"]["2"][0]["xp"] = json!(900);
        tied[0]["vehicles"]["3"][0]["xp"] = json!(900);
        let actual = facts(&metadata, &tied);
        assert_eq!(actual.xp_greater_count, Some(1));
        assert_eq!(actual.xp_equal_count, Some(3));

        let (_, mut missing) = synthetic_input();
        missing[0]["vehicles"]["1"][0]
            .as_object_mut()
            .unwrap()
            .remove("xp");
        let actual = facts(&metadata, &missing);
        assert_eq!(actual.xp_reason, Some(TeamRelativeXpReason::Missing));
        assert_eq!(actual.team_damage_total, Some(14_200));
        assert_eq!(actual.team_frags_total, Some(5));

        let (_, mut invalid) = synthetic_input();
        invalid[0]["vehicles"]["1"][0]["xp"] = json!(100_001);
        assert_eq!(
            facts(&metadata, &invalid).xp_reason,
            Some(TeamRelativeXpReason::OutOfRange)
        );
    }

    #[test]
    fn roster_failures_use_the_frozen_common_reason_order() {
        let (metadata, results) = synthetic_input();

        let mut missing_initial = metadata.clone();
        missing_initial.as_object_mut().unwrap().remove("vehicles");
        assert_eq!(
            facts(&missing_initial, &results).common_reason,
            Some(TeamRelativeCommonReason::MissingInitialRoster)
        );

        let mut missing_final = results.clone();
        missing_final[0].as_object_mut().unwrap().remove("vehicles");
        assert_eq!(
            facts(&metadata, &missing_final).common_reason,
            Some(TeamRelativeCommonReason::MissingFinalRoster)
        );

        let mut short_initial = metadata.clone();
        short_initial["vehicles"]
            .as_object_mut()
            .unwrap()
            .remove("1");
        assert_eq!(
            facts(&short_initial, &results).common_reason,
            Some(TeamRelativeCommonReason::RosterSizeConflict)
        );

        let mut short_final = results.clone();
        short_final[0]["vehicles"]
            .as_object_mut()
            .unwrap()
            .remove("1");
        assert_eq!(
            facts(&metadata, &short_final).common_reason,
            Some(TeamRelativeCommonReason::RosterSizeConflict)
        );

        let mut invalid_team = metadata.clone();
        invalid_team["vehicles"]["1"]["team"] = json!(3);
        assert_eq!(
            facts(&invalid_team, &results).common_reason,
            Some(TeamRelativeCommonReason::InvalidTeamAssignment)
        );
    }

    #[test]
    fn player_structure_conflict_and_duplicate_descriptors_are_distinct() {
        let (metadata, mut conflict) = synthetic_input();
        conflict[0]["personal"]["33345"]["team"] = json!(2);
        assert_eq!(
            facts(&metadata, &conflict).common_reason,
            Some(TeamRelativeCommonReason::PlayerRowConflict)
        );

        let (_, mut duplicate_descriptor) = synthetic_input();
        duplicate_descriptor[0]["vehicles"]["14"][0]["typeCompDescr"] = json!(33345);
        assert_eq!(facts(&metadata, &duplicate_descriptor).common_reason, None);
        duplicate_descriptor[0]["vehicles"]["14"][0]["team"] = json!(1);
        assert_eq!(facts(&metadata, &duplicate_descriptor).common_reason, None);
    }

    #[test]
    fn provenance_version_schema_mode_class_and_outcome_boundaries_fail_closed() {
        let (metadata, results) = synthetic_input();
        let base = TeamRelativeParseInput {
            data_quality: DataQuality::Full,
            data_completeness: Some("results"),
            replay_source: Some("replay_results"),
            parser_version: Some(7),
            feature_schema_version: Some(3),
            metadata: &metadata,
            results: Some(&results),
            canonical_damage: Some(3_000),
            canonical_frags: Some(2),
            canonical_original_xp: Some(900),
        };
        let provenance = extract_team_relative_facts(&TeamRelativeParseInput {
            replay_source: Some("replay_metadata"),
            ..base.clone()
        });
        assert_eq!(
            provenance.common_reason,
            Some(TeamRelativeCommonReason::ResultsProvenanceMissing)
        );
        let parser = extract_team_relative_facts(&TeamRelativeParseInput {
            parser_version: Some(6),
            ..base.clone()
        });
        assert_eq!(
            parser.common_reason,
            Some(TeamRelativeCommonReason::ParserTooOld)
        );
        let schema = extract_team_relative_facts(&TeamRelativeParseInput {
            feature_schema_version: Some(2),
            ..base
        });
        assert_eq!(
            schema.common_reason,
            Some(TeamRelativeCommonReason::UnsupportedFeatureSchema)
        );
        // Neither outcome nor vehicle class is an input to this contract. A structurally
        // valid CTF replay therefore remains eligible for SPGs, draws, and unknown outcomes.
        assert_eq!(facts(&metadata, &results).common_reason, None);
    }

    #[test]
    fn reason_catalogs_are_exact_and_stable() {
        assert_eq!(TEAM_RELATIVE_CONTRACT_VERSION, "team-relative-results-v1");
        assert_eq!(
            TeamRelativeCommonReason::ALL.map(TeamRelativeCommonReason::as_str),
            [
                "partial_replay",
                "results_provenance_missing",
                "parser_too_old",
                "unsupported_feature_schema",
                "unsupported_mode",
                "missing_initial_roster",
                "missing_final_roster",
                "roster_size_conflict",
                "roster_key_conflict",
                "duplicate_rollout",
                "invalid_team_assignment",
                "own_player_missing",
                "own_player_ambiguous",
                "player_row_conflict",
                "numeric_overflow",
            ]
        );
    }
}
