use crate::models::{
    BattleOutcome, BattlePaceContext, BattlePaceDestroyedScore, BattlePaceModeIdentity,
    BattlePaceProvenance, BattlePaceReason, BattlePaceResolution, BattlePaceSummary,
    BattlePaceTeamSizes, DataQuality, supports_core_feature_schema,
};

pub const BATTLE_PACE_RESOLVER_VERSION: &str = "battle-pace-context-v1";
const RANDOM_BATTLE_TYPE: i64 = 1;
const RANDOM_BONUS_TYPE: i64 = 1;
const EXTERMINATION_FINISH_REASON: i64 = 1;
const RANDOM_TEAM_SIZE: i64 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattlePaceInput {
    pub data_quality: DataQuality,
    pub feature_schema_version: Option<i64>,
    pub battle_type_raw: Option<i64>,
    pub bonus_type_raw: Option<i64>,
    pub gameplay_id: Option<String>,
    pub duration_seconds: Option<i64>,
    pub finish_reason_raw: Option<i64>,
    pub initial_team_sizes: Option<BattlePaceTeamSizes>,
    pub destroyed_score: Option<BattlePaceDestroyedScore>,
    pub player_team: Option<i64>,
    pub winner_team_raw: Option<i64>,
    pub outcome: BattleOutcome,
    pub extraction_error: Option<BattlePaceReason>,
    pub provenance: BattlePaceProvenance,
}

fn resolution(
    input: &BattlePaceInput,
    context: BattlePaceContext,
    reason: Option<BattlePaceReason>,
) -> BattlePaceResolution {
    BattlePaceResolution {
        context,
        reason,
        resolver_version: BATTLE_PACE_RESOLVER_VERSION.to_owned(),
        provenance: input.provenance,
        duration_seconds: input.duration_seconds,
        initial_team_sizes: input.initial_team_sizes,
        destroyed_score: input.destroyed_score,
        finish_reason_raw: input.finish_reason_raw,
        mode_identity: BattlePaceModeIdentity {
            battle_type_raw: input.battle_type_raw,
            bonus_type_raw: input.bonus_type_raw,
            gameplay_id: input.gameplay_id.clone(),
        },
    }
}

fn unknown(input: &BattlePaceInput, reason: BattlePaceReason) -> BattlePaceResolution {
    resolution(input, BattlePaceContext::Unknown, Some(reason))
}

pub fn resolve_battle_pace(input: &BattlePaceInput) -> BattlePaceResolution {
    if input.data_quality != DataQuality::Full {
        return unknown(input, BattlePaceReason::MissingFinalRoster);
    }
    if !supports_core_feature_schema(input.feature_schema_version) {
        return unknown(input, BattlePaceReason::UnsupportedSchema);
    }
    if let Some(reason) = input.extraction_error {
        return unknown(input, reason);
    }
    if input.battle_type_raw != Some(RANDOM_BATTLE_TYPE)
        || input.bonus_type_raw != Some(RANDOM_BONUS_TYPE)
        || input.gameplay_id.as_deref() != Some("ctf")
    {
        return unknown(input, BattlePaceReason::UnsupportedMode);
    }

    let Some(team_sizes) = input.initial_team_sizes else {
        return unknown(input, BattlePaceReason::MissingTeamAssignment);
    };
    if team_sizes.team1 != team_sizes.team2 {
        return unknown(input, BattlePaceReason::AsymmetricTeams);
    }
    if team_sizes.team1 != RANDOM_TEAM_SIZE {
        return unknown(input, BattlePaceReason::InvalidTeamSize);
    }

    let Some(duration) = input.duration_seconds else {
        return unknown(input, BattlePaceReason::MissingDuration);
    };
    if duration < 0 {
        return unknown(input, BattlePaceReason::ConflictingData);
    }
    let Some(finish_reason) = input.finish_reason_raw else {
        return unknown(input, BattlePaceReason::MissingFinishReason);
    };
    if finish_reason != EXTERMINATION_FINISH_REASON {
        return unknown(input, BattlePaceReason::UnsupportedFinishReason);
    }

    let Some(score) = input.destroyed_score else {
        return unknown(input, BattlePaceReason::MissingFinalRoster);
    };
    if !(0..=RANDOM_TEAM_SIZE).contains(&score.team1)
        || !(0..=RANDOM_TEAM_SIZE).contains(&score.team2)
    {
        return unknown(input, BattlePaceReason::InvalidDestroyedScore);
    }
    let winner = match (
        score.team1 == RANDOM_TEAM_SIZE,
        score.team2 == RANDOM_TEAM_SIZE,
    ) {
        (true, false) => 1,
        (false, true) => 2,
        _ => return unknown(input, BattlePaceReason::ConflictingData),
    };
    if input.winner_team_raw != Some(winner) {
        return unknown(input, BattlePaceReason::ConflictingData);
    }
    let expected_outcome =
        BattleOutcome::from_replay_teams(input.player_team, input.winner_team_raw);
    if !expected_outcome.is_known() || expected_outcome != input.outcome {
        return unknown(input, BattlePaceReason::ConflictingData);
    }

    let loser_score = if winner == 1 {
        score.team2
    } else {
        score.team1
    };
    let context = if duration < 180 && loser_score <= 2 {
        BattlePaceContext::ObviousSteamroll
    } else if duration <= 330 && loser_score <= 4 {
        BattlePaceContext::BorderlineSteamroll
    } else {
        BattlePaceContext::Standard
    };
    resolution(input, context, None)
}

pub fn summarize_battle_pace<'a>(
    contexts: impl IntoIterator<Item = Option<&'a BattlePaceContext>>,
) -> BattlePaceSummary {
    let mut summary = BattlePaceSummary {
        total_battles: 0,
        obvious_steamrolls: 0,
        borderline_steamrolls: 0,
        standard_battles: 0,
        unknown_battles: 0,
        classified_battles: 0,
        coverage_ratio: 0.0,
    };
    for context in contexts {
        summary.total_battles += 1;
        match context {
            Some(BattlePaceContext::ObviousSteamroll) => summary.obvious_steamrolls += 1,
            Some(BattlePaceContext::BorderlineSteamroll) => summary.borderline_steamrolls += 1,
            Some(BattlePaceContext::Standard) => summary.standard_battles += 1,
            Some(BattlePaceContext::Unknown) | None => summary.unknown_battles += 1,
        }
    }
    summary.classified_battles =
        summary.obvious_steamrolls + summary.borderline_steamrolls + summary.standard_battles;
    if summary.total_battles > 0 {
        summary.coverage_ratio = summary.classified_battles as f64 / summary.total_battles as f64;
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(duration: i64, winner_score: i64, loser_score: i64) -> BattlePaceInput {
        BattlePaceInput {
            data_quality: DataQuality::Full,
            feature_schema_version: Some(1),
            battle_type_raw: Some(1),
            bonus_type_raw: Some(1),
            gameplay_id: Some("ctf".into()),
            duration_seconds: Some(duration),
            finish_reason_raw: Some(1),
            initial_team_sizes: Some(BattlePaceTeamSizes {
                team1: 15,
                team2: 15,
            }),
            destroyed_score: Some(BattlePaceDestroyedScore {
                team1: winner_score,
                team2: loser_score,
            }),
            player_team: Some(1),
            winner_team_raw: Some(1),
            outcome: BattleOutcome::Victory,
            extraction_error: None,
            provenance: BattlePaceProvenance::ReplayResults,
        }
    }

    fn assert_context(
        duration: i64,
        winner_score: i64,
        loser_score: i64,
        expected: BattlePaceContext,
    ) {
        assert_eq!(
            resolve_battle_pace(&input(duration, winner_score, loser_score)).context,
            expected
        );
    }

    #[test]
    fn exact_obvious_and_borderline_boundaries_are_frozen() {
        for (duration, loser_score, expected) in [
            (179, 0, BattlePaceContext::ObviousSteamroll),
            (179, 1, BattlePaceContext::ObviousSteamroll),
            (179, 2, BattlePaceContext::ObviousSteamroll),
            (179, 3, BattlePaceContext::BorderlineSteamroll),
            (180, 2, BattlePaceContext::BorderlineSteamroll),
            (205, 0, BattlePaceContext::BorderlineSteamroll),
            (330, 4, BattlePaceContext::BorderlineSteamroll),
            (331, 4, BattlePaceContext::Standard),
            (250, 5, BattlePaceContext::Standard),
            (500, 0, BattlePaceContext::Standard),
        ] {
            assert_context(duration, 15, loser_score, expected);
        }
    }

    #[test]
    fn invalid_elimination_score_fails_closed() {
        let resolved = resolve_battle_pace(&input(100, 14, 0));
        assert_eq!(resolved.context, BattlePaceContext::Unknown);
        assert_eq!(resolved.reason, Some(BattlePaceReason::ConflictingData));
    }

    #[test]
    fn unsupported_team_sizes_and_asymmetry_fail_closed() {
        for (team1, team2, reason) in [
            (12, 12, BattlePaceReason::InvalidTeamSize),
            (7, 7, BattlePaceReason::InvalidTeamSize),
            (15, 14, BattlePaceReason::AsymmetricTeams),
        ] {
            let mut value = input(200, 15, 0);
            value.initial_team_sizes = Some(BattlePaceTeamSizes { team1, team2 });
            let resolved = resolve_battle_pace(&value);
            assert_eq!(resolved.context, BattlePaceContext::Unknown);
            assert_eq!(resolved.reason, Some(reason));
        }
    }

    #[test]
    fn unsupported_finish_reasons_and_draws_are_unknown() {
        for finish_reason in [2, 3] {
            let mut value = input(191, 15, 0);
            value.finish_reason_raw = Some(finish_reason);
            let resolved = resolve_battle_pace(&value);
            assert_eq!(resolved.context, BattlePaceContext::Unknown);
            assert_eq!(
                resolved.reason,
                Some(BattlePaceReason::UnsupportedFinishReason)
            );
        }
        let mut draw = input(900, 15, 15);
        draw.winner_team_raw = Some(0);
        draw.outcome = BattleOutcome::Draw;
        assert_eq!(
            resolve_battle_pace(&draw).context,
            BattlePaceContext::Unknown
        );
    }

    #[test]
    fn incomplete_and_unsupported_inputs_are_unknown_with_reasons() {
        let mut partial = input(200, 15, 0);
        partial.data_quality = DataQuality::Partial;
        assert_eq!(
            resolve_battle_pace(&partial).reason,
            Some(BattlePaceReason::MissingFinalRoster)
        );

        let mut missing_duration = input(200, 15, 0);
        missing_duration.duration_seconds = None;
        assert_eq!(
            resolve_battle_pace(&missing_duration).reason,
            Some(BattlePaceReason::MissingDuration)
        );

        let mut missing_roster = input(200, 15, 0);
        missing_roster.destroyed_score = None;
        assert_eq!(
            resolve_battle_pace(&missing_roster).reason,
            Some(BattlePaceReason::MissingFinalRoster)
        );

        for schema in [None, Some(-1), Some(0), Some(4)] {
            let mut unsupported_schema = input(200, 15, 0);
            unsupported_schema.feature_schema_version = schema;
            assert_eq!(
                resolve_battle_pace(&unsupported_schema).reason,
                Some(BattlePaceReason::UnsupportedSchema)
            );
        }

        let mut unsupported_mode = input(200, 15, 0);
        unsupported_mode.battle_type_raw = Some(2);
        assert_eq!(
            resolve_battle_pace(&unsupported_mode).reason,
            Some(BattlePaceReason::UnsupportedMode)
        );
    }

    #[test]
    fn core_battle_pace_is_exactly_equivalent_for_feature_schemas_one_two_and_three() {
        let mut cases = vec![
            input(179, 15, 2),
            input(330, 15, 4),
            input(500, 15, 5),
            input(200, 15, 0),
        ];
        cases[3].duration_seconds = None;
        let mut schema_one_contexts = Vec::new();
        let mut schema_two_contexts = Vec::new();
        for schema_one in cases {
            let mut schema_two = schema_one.clone();
            schema_two.feature_schema_version = Some(2);
            let mut schema_three = schema_one.clone();
            schema_three.feature_schema_version = Some(3);
            let first = resolve_battle_pace(&schema_one);
            let second = resolve_battle_pace(&schema_two);
            let third = resolve_battle_pace(&schema_three);
            assert_eq!(first, second);
            assert_eq!(first, third);
            schema_one_contexts.push(Some(first.context));
            schema_two_contexts.push(Some(second.context));
        }
        assert_eq!(
            summarize_battle_pace(schema_one_contexts.iter().map(Option::as_ref)),
            summarize_battle_pace(schema_two_contexts.iter().map(Option::as_ref))
        );
    }

    #[test]
    fn winner_conflicts_fail_closed() {
        let mut value = input(200, 15, 0);
        value.winner_team_raw = Some(2);
        value.outcome = BattleOutcome::Defeat;
        let resolved = resolve_battle_pace(&value);
        assert_eq!(resolved.context, BattlePaceContext::Unknown);
        assert_eq!(resolved.reason, Some(BattlePaceReason::ConflictingData));
    }

    #[test]
    fn team_orientation_and_player_outcome_do_not_change_severity() {
        let victory = resolve_battle_pace(&input(179, 15, 2));
        let mut defeat = input(179, 15, 2);
        defeat.player_team = Some(2);
        defeat.outcome = BattleOutcome::Defeat;
        let defeat = resolve_battle_pace(&defeat);

        let mut reversed = input(179, 15, 2);
        reversed.destroyed_score = Some(BattlePaceDestroyedScore {
            team1: 2,
            team2: 15,
        });
        reversed.player_team = Some(2);
        reversed.winner_team_raw = Some(2);
        let reversed = resolve_battle_pace(&reversed);

        assert_eq!(victory.context, BattlePaceContext::ObviousSteamroll);
        assert_eq!(defeat.context, victory.context);
        assert_eq!(reversed.context, victory.context);
    }

    #[test]
    fn summary_keeps_unknown_out_of_coverage_without_affecting_total() {
        let values = [
            Some(BattlePaceContext::ObviousSteamroll),
            Some(BattlePaceContext::BorderlineSteamroll),
            Some(BattlePaceContext::Standard),
            Some(BattlePaceContext::Unknown),
            None,
        ];
        let summary = summarize_battle_pace(values.iter().map(Option::as_ref));
        assert_eq!(summary.total_battles, 5);
        assert_eq!(summary.classified_battles, 3);
        assert_eq!(summary.unknown_battles, 2);
        assert_eq!(summary.coverage_ratio, 0.6);
    }
}
