use crate::{
    expected_values::{ExpectedValuesRow, LoadedExpectedSnapshot},
    models::{
        Battle, DataQuality, Wn8Calculation, Wn8ExclusionCounts, Wn8Result, Wn8Totals,
        Wn8Unavailable, stats_eligibility, supports_core_feature_schema,
    },
};
use std::collections::BTreeMap;

pub const WN8_FORMULA_VERSION: &str = "wn8-standard-v1";

#[derive(Debug, Clone, Copy)]
struct Ratios {
    damage: f64,
    frags: f64,
    spots: f64,
    defense: f64,
    win: f64,
}

#[derive(Debug, Clone, Copy)]
struct NormalizedRatios {
    damage: f64,
    frags: f64,
    spots: f64,
    defense: f64,
    win: f64,
}

#[derive(Debug, Clone, Copy)]
enum ExclusionReason {
    PartialOrIncomplete,
    UnsupportedFeatureSchema,
    NotRandomBattle,
    VehicleIdConflict,
    MissingVehicleId,
    VehicleNotInExpectedValues,
    MissingDamage,
    MissingFrags,
    MissingSpots,
    MissingDefense,
    MissingResult,
    InvalidNumericInput,
}

impl Wn8ExclusionCounts {
    fn record(&mut self, reason: ExclusionReason) {
        match reason {
            ExclusionReason::PartialOrIncomplete => self.partial_or_incomplete += 1,
            ExclusionReason::UnsupportedFeatureSchema => self.unsupported_feature_schema += 1,
            ExclusionReason::NotRandomBattle => self.not_random_battle += 1,
            ExclusionReason::VehicleIdConflict => self.vehicle_id_conflict += 1,
            ExclusionReason::MissingVehicleId => self.missing_vehicle_id += 1,
            ExclusionReason::VehicleNotInExpectedValues => {
                self.vehicle_not_in_expected_values += 1;
            }
            ExclusionReason::MissingDamage => self.missing_damage += 1,
            ExclusionReason::MissingFrags => self.missing_frags += 1,
            ExclusionReason::MissingSpots => self.missing_spots += 1,
            ExclusionReason::MissingDefense => self.missing_defense += 1,
            ExclusionReason::MissingResult => self.missing_result += 1,
            ExclusionReason::InvalidNumericInput => self.invalid_numeric_input += 1,
        }
    }
}

fn expected_is_valid(expected: &ExpectedValuesRow) -> bool {
    let values = [
        expected.exp_damage,
        expected.exp_frags,
        expected.exp_spots,
        expected.exp_defense,
        expected.exp_win_rate,
    ];
    values.iter().all(|value| value.is_finite() && *value > 0.0) && expected.exp_win_rate <= 100.0
}

fn qualify<'a>(
    battle: &'a Battle,
    snapshot: &'a LoadedExpectedSnapshot,
) -> Result<(i64, i64, i64, i64, f64, &'a ExpectedValuesRow), ExclusionReason> {
    if battle.data_quality != DataQuality::Full {
        return Err(ExclusionReason::PartialOrIncomplete);
    }
    if !supports_core_feature_schema(battle.feature_schema_version) {
        return Err(ExclusionReason::UnsupportedFeatureSchema);
    }
    if !stats_eligibility(battle) {
        return Err(ExclusionReason::NotRandomBattle);
    }
    if battle.parser_error_code.as_deref() == Some("vehicle_id_conflict") {
        return Err(ExclusionReason::VehicleIdConflict);
    }
    let vehicle_id = battle.wg_tank_id.ok_or(ExclusionReason::MissingVehicleId)?;
    let expected = snapshot
        .rows
        .get(&vehicle_id)
        .ok_or(ExclusionReason::VehicleNotInExpectedValues)?;
    let damage = battle.damage.ok_or(ExclusionReason::MissingDamage)?;
    let frags = battle.frags.ok_or(ExclusionReason::MissingFrags)?;
    let spots = battle.spots.ok_or(ExclusionReason::MissingSpots)?;
    let defense = battle
        .defense_points
        .ok_or(ExclusionReason::MissingDefense)?;
    let win_contribution = battle
        .outcome
        .win_contribution()
        .ok_or(ExclusionReason::MissingResult)?;
    if [damage, frags, spots, defense]
        .iter()
        .any(|value| *value < 0)
        || !expected_is_valid(expected)
    {
        return Err(ExclusionReason::InvalidNumericInput);
    }
    Ok((damage, frags, spots, defense, win_contribution, expected))
}

fn normalize(ratios: Ratios) -> NormalizedRatios {
    let damage = ((ratios.damage - 0.22) / (1.0 - 0.22)).max(0.0);
    let frags = ((ratios.frags - 0.12) / (1.0 - 0.12))
        .min(damage + 0.2)
        .max(0.0);
    let spots = ((ratios.spots - 0.38) / (1.0 - 0.38))
        .min(damage + 0.1)
        .max(0.0);
    let defense = ((ratios.defense - 0.10) / (1.0 - 0.10))
        .min(damage + 0.1)
        .max(0.0);
    let win = ((ratios.win - 0.71) / (1.0 - 0.71)).max(0.0);
    NormalizedRatios {
        damage,
        frags,
        spots,
        defense,
        win,
    }
}

fn formula(actual: &Wn8Totals, expected: &Wn8Totals) -> Option<f64> {
    let actual_values = [
        actual.damage,
        actual.frags,
        actual.spots,
        actual.defense,
        actual.win_points,
    ];
    let expected_values = [
        expected.damage,
        expected.frags,
        expected.spots,
        expected.defense,
        expected.win_points,
    ];
    if actual_values
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
        || expected_values
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return None;
    }
    let normalized = normalize(Ratios {
        damage: actual.damage / expected.damage,
        frags: actual.frags / expected.frags,
        spots: actual.spots / expected.spots,
        defense: actual.defense / expected.defense,
        win: actual.win_points / expected.win_points,
    });
    let value = 980.0 * normalized.damage
        + 210.0 * normalized.damage * normalized.frags
        + 155.0 * normalized.frags * normalized.spots
        + 75.0 * normalized.defense * normalized.frags
        + 145.0 * normalized.win.min(1.8);
    value.is_finite().then_some(value)
}

fn unavailable(
    snapshot: &LoadedExpectedSnapshot,
    total_battles: usize,
    exclusions: Wn8ExclusionCounts,
) -> Wn8Calculation {
    Wn8Calculation::Unavailable(Wn8Unavailable {
        formula_version: WN8_FORMULA_VERSION.to_owned(),
        snapshot_sha256: snapshot.manifest.sha256.clone(),
        snapshot_source_version: snapshot.manifest.source_version.clone(),
        eligible_battles: 0,
        total_battles,
        coverage_ratio: 0.0,
        exclusions,
    })
}

pub fn calculate_wn8(battles: &[Battle], snapshot: &LoadedExpectedSnapshot) -> Wn8Calculation {
    let mut exclusions = Wn8ExclusionCounts::default();
    let mut damage = 0_i128;
    let mut frags = 0_i128;
    let mut spots = 0_i128;
    let mut defense = 0_i128;
    let mut wins = 0.0_f64;
    let mut expected_counts = BTreeMap::<i64, usize>::new();

    for battle in battles {
        match qualify(battle, snapshot) {
            Ok((
                battle_damage,
                battle_frags,
                battle_spots,
                battle_defense,
                win_contribution,
                expected,
            )) => {
                damage += i128::from(battle_damage);
                frags += i128::from(battle_frags);
                spots += i128::from(battle_spots);
                defense += i128::from(battle_defense);
                wins += win_contribution;
                *expected_counts.entry(expected.vehicle_id).or_default() += 1;
            }
            Err(reason) => exclusions.record(reason),
        }
    }

    let eligible_battles = expected_counts.values().sum::<usize>();
    if eligible_battles == 0 {
        return unavailable(snapshot, battles.len(), exclusions);
    }

    let actual_totals = Wn8Totals {
        damage: damage as f64,
        frags: frags as f64,
        spots: spots as f64,
        defense: defense as f64,
        win_points: wins * 100.0,
    };
    let mut expected_totals = Wn8Totals::default();
    for (vehicle_id, count) in expected_counts {
        let expected = &snapshot.rows[&vehicle_id];
        let count = count as f64;
        expected_totals.damage += expected.exp_damage * count;
        expected_totals.frags += expected.exp_frags * count;
        expected_totals.spots += expected.exp_spots * count;
        expected_totals.defense += expected.exp_defense * count;
        expected_totals.win_points += expected.exp_win_rate * count;
    }

    let Some(value) = formula(&actual_totals, &expected_totals) else {
        exclusions.invalid_numeric_input += eligible_battles;
        return unavailable(snapshot, battles.len(), exclusions);
    };
    Wn8Calculation::Available(Wn8Result {
        value,
        formula_version: WN8_FORMULA_VERSION.to_owned(),
        snapshot_sha256: snapshot.manifest.sha256.clone(),
        snapshot_source_version: snapshot.manifest.source_version.clone(),
        eligible_battles,
        total_battles: battles.len(),
        coverage_ratio: eligible_battles as f64 / battles.len() as f64,
        actual_totals,
        expected_totals,
        exclusions,
    })
}

pub fn calculate_battle_wn8(battle: &Battle, snapshot: &LoadedExpectedSnapshot) -> Wn8Calculation {
    calculate_wn8(std::slice::from_ref(battle), snapshot)
}

/// The legacy downloader persisted an estimated, defense-free value in SQLite. It is retired;
/// expected-value updates and exact WN8 calculation are now separate, explicit operations.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expected_values::{
            CACHE_SCHEMA_VERSION, ExpectedSnapshotManifest, ExpectedValuesSource,
            SnapshotValidationStatus, VALIDATOR_VERSION, XVM_WARGAMING_SOURCE_URL,
        },
        models::{BattleOutcome, SourceKind},
        storage::Database,
    };
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn expected(vehicle_id: i64, scale: f64) -> ExpectedValuesRow {
        ExpectedValuesRow {
            vehicle_id,
            exp_damage: 1_000.0 * scale,
            exp_frags: 1.0 * scale,
            exp_spots: 1.0 * scale,
            exp_defense: 1.0 * scale,
            exp_win_rate: 50.0,
        }
    }

    fn snapshot(rows: Vec<ExpectedValuesRow>) -> LoadedExpectedSnapshot {
        LoadedExpectedSnapshot {
            manifest: ExpectedSnapshotManifest {
                cache_schema_version: CACHE_SCHEMA_VERSION,
                validator_version: VALIDATOR_VERSION,
                source: ExpectedValuesSource::XvmWargaming,
                source_url: XVM_WARGAMING_SOURCE_URL.to_owned(),
                downloaded_at: 1_000,
                source_version: "2026-07-10".to_owned(),
                source_date: "2026-07-10".to_owned(),
                sha256: "a".repeat(64),
                byte_length: 1_000,
                vehicle_count: rows.len(),
                skipped_row_count: 0,
                validation_status: SnapshotValidationStatus::Validated,
            },
            rows: rows
                .into_iter()
                .map(|row| (row.vehicle_id, row))
                .collect::<HashMap<_, _>>(),
        }
    }

    fn battle(id: &str, vehicle_id: i64) -> Battle {
        Battle {
            arena_id: id.to_owned(),
            arena_unique_id: None,
            battle_time: 1_000,
            battle_time_source: Some("metadata".to_owned()),
            vehicle_key: "tank".to_owned(),
            wg_tank_id: Some(vehicle_id),
            tank_name: None,
            map_name: None,
            map_key: None,
            battle_type: Some("random".to_owned()),
            gameplay_id: Some("ctf".to_owned()),
            source_kind: SourceKind::Replay,
            data_quality: DataQuality::Full,
            parser_version: Some(4),
            feature_schema_version: Some(1),
            game_version: Some("test".to_owned()),
            data_completeness: Some("results".to_owned()),
            replay_source: Some("replay_results".to_owned()),
            parser_error_code: None,
            outcome: BattleOutcome::Victory,
            battle_pace: crate::models::strict_random_pace(),
            player_team: Some(1),
            winner_team_raw: Some(1),
            damage: Some(1_000),
            assist: None,
            assist_spotting: None,
            assist_tracking: None,
            assist_stun: None,
            blocked: None,
            damage_received: None,
            frags: Some(1),
            spots: Some(1),
            shots: None,
            hits: None,
            direct_enemy_hits: None,
            direct_team_hits: None,
            penetrations: None,
            piercing_enemy_hits: None,
            explosion_hits: None,
            defense_points: Some(1),
            moving_avg_damage: None,
            damage_rating: None,
            survived: None,
            xp: None,
            original_xp: None,
            duration_seconds: None,
            session_id: None,
        }
    }

    fn available(calculation: Wn8Calculation) -> Wn8Result {
        match calculation {
            Wn8Calculation::Available(result) => result,
            Wn8Calculation::Unavailable(_) => panic!("WN8 unexpectedly unavailable"),
        }
    }

    fn unavailable_result(calculation: Wn8Calculation) -> Wn8Unavailable {
        match calculation {
            Wn8Calculation::Unavailable(result) => result,
            Wn8Calculation::Available(_) => panic!("WN8 unexpectedly available"),
        }
    }

    #[test]
    fn equal_actual_and_expected_totals_produce_1565() {
        let mut row = expected(33345, 1.0);
        row.exp_win_rate = 100.0;
        let result = available(calculate_battle_wn8(
            &battle("one", 33345),
            &snapshot(vec![row]),
        ));
        assert_eq!(result.value, 1565.0);
        assert_eq!(result.formula_version, WN8_FORMULA_VERSION);
        assert_eq!(result.actual_totals, result.expected_totals);
    }

    #[test]
    fn zero_actual_totals_produce_zero() {
        let mut zero = battle("zero", 1);
        zero.damage = Some(0);
        zero.frags = Some(0);
        zero.spots = Some(0);
        zero.defense_points = Some(0);
        zero.outcome = BattleOutcome::Defeat;
        assert_eq!(
            available(calculate_battle_wn8(
                &zero,
                &snapshot(vec![expected(1, 1.0)])
            ))
            .value,
            0.0
        );
    }

    #[test]
    fn draw_is_wn8_eligible_with_zero_win_points_while_unknown_is_excluded() {
        let snapshot = snapshot(vec![expected(1, 1.0)]);
        let mut draw = battle("draw", 1);
        draw.outcome = BattleOutcome::Draw;
        let draw_result = available(calculate_battle_wn8(&draw, &snapshot));
        assert_eq!(draw_result.eligible_battles, 1);
        assert_eq!(draw_result.coverage_ratio, 1.0);
        assert_eq!(draw_result.actual_totals.win_points, 0.0);
        assert_eq!(draw_result.actual_totals.damage, 1_000.0);
        assert_eq!(draw_result.expected_totals.win_points, 50.0);

        let mut unknown = draw;
        unknown.arena_id = "unknown".to_owned();
        unknown.outcome = BattleOutcome::Unknown;
        let unavailable = unavailable_result(calculate_battle_wn8(&unknown, &snapshot));
        assert_eq!(unavailable.exclusions.missing_result, 1);
        assert_eq!(unavailable.eligible_battles, 0);
    }

    #[test]
    fn victory_draw_and_defeat_aggregate_use_exact_outcome_contributions() {
        let snapshot = snapshot(vec![expected(1, 1.0)]);
        let victory = battle("victory", 1);
        let mut draw = battle("draw", 1);
        draw.outcome = BattleOutcome::Draw;
        let mut defeat = battle("defeat", 1);
        defeat.outcome = BattleOutcome::Defeat;

        let victory_and_draw =
            available(calculate_wn8(&[victory.clone(), draw.clone()], &snapshot));
        assert_eq!(victory_and_draw.eligible_battles, 2);
        assert_eq!(victory_and_draw.actual_totals.win_points, 100.0);
        assert_eq!(victory_and_draw.expected_totals.win_points, 100.0);

        let defeat_and_draw = available(calculate_wn8(&[defeat.clone(), draw.clone()], &snapshot));
        assert_eq!(defeat_and_draw.eligible_battles, 2);
        assert_eq!(defeat_and_draw.actual_totals.win_points, 0.0);
        assert_eq!(defeat_and_draw.expected_totals.win_points, 100.0);

        let result = available(calculate_wn8(&[victory, draw, defeat], &snapshot));
        assert_eq!(result.eligible_battles, 3);
        assert_eq!(result.actual_totals.win_points, 100.0);
        assert_eq!(result.expected_totals.win_points, 150.0);
        assert_eq!(result.actual_totals.damage, 3_000.0);
    }

    #[test]
    fn draw_without_expected_values_reports_vehicle_gap_not_missing_result() {
        let mut draw = battle("draw-no-expected", 2);
        draw.outcome = BattleOutcome::Draw;
        let result = unavailable_result(calculate_battle_wn8(
            &draw,
            &snapshot(vec![expected(1, 1.0)]),
        ));
        assert_eq!(result.exclusions.vehicle_not_in_expected_values, 1);
        assert_eq!(result.exclusions.missing_result, 0);
    }

    #[test]
    fn normalization_thresholds_are_exact() {
        let at_threshold = normalize(Ratios {
            damage: 0.22,
            frags: 0.12,
            spots: 0.38,
            defense: 0.10,
            win: 0.71,
        });
        assert_eq!(at_threshold.damage, 0.0);
        assert_eq!(at_threshold.frags, 0.0);
        assert_eq!(at_threshold.spots, 0.0);
        assert_eq!(at_threshold.defense, 0.0);
        assert_eq!(at_threshold.win, 0.0);
        let above = normalize(Ratios {
            damage: 0.23,
            frags: 0.13,
            spots: 0.39,
            defense: 0.11,
            win: 0.72,
        });
        assert!(above.damage > 0.0);
        assert!(above.frags > 0.0);
        assert!(above.spots > 0.0);
        assert!(above.defense > 0.0);
        assert!(above.win > 0.0);
    }

    #[test]
    fn frag_spot_and_defense_caps_follow_damage() {
        let normalized = normalize(Ratios {
            damage: 0.22,
            frags: 10.0,
            spots: 10.0,
            defense: 10.0,
            win: 1.0,
        });
        assert_eq!(normalized.frags, 0.2);
        assert_eq!(normalized.spots, 0.1);
        assert_eq!(normalized.defense, 0.1);
    }

    #[test]
    fn win_component_is_capped_at_one_point_eight() {
        let actual = Wn8Totals {
            win_points: 10_000.0,
            ..Wn8Totals::default()
        };
        let expected_totals = Wn8Totals {
            damage: 1.0,
            frags: 1.0,
            spots: 1.0,
            defense: 1.0,
            win_points: 1.0,
        };
        assert_eq!(formula(&actual, &expected_totals), Some(145.0 * 1.8));
    }

    #[test]
    fn aggregate_uses_totals_not_average_of_per_battle_wn8() {
        let snapshot = snapshot(vec![expected(1, 1.0), expected(2, 3.0)]);
        let first = battle("first", 1);
        let mut second = battle("second", 2);
        second.damage = Some(1_000);
        second.frags = Some(1);
        second.spots = Some(1);
        second.defense_points = Some(1);
        let aggregate = available(calculate_wn8(&[first.clone(), second.clone()], &snapshot));
        let mean = (available(calculate_battle_wn8(&first, &snapshot)).value
            + available(calculate_battle_wn8(&second, &snapshot)).value)
            / 2.0;
        assert_ne!(aggregate.value, mean);
    }

    #[test]
    fn order_does_not_change_result_and_single_matches_aggregate() {
        let snapshot = snapshot(vec![expected(1, 1.0), expected(2, 2.0)]);
        let first = battle("first", 1);
        let second = battle("second", 2);
        assert_eq!(
            calculate_wn8(&[first.clone(), second.clone()], &snapshot),
            calculate_wn8(&[second, first.clone()], &snapshot)
        );
        assert_eq!(
            calculate_battle_wn8(&first, &snapshot),
            calculate_wn8(&[first], &snapshot)
        );
    }

    #[test]
    fn replay_full_and_vehicle_33345_qualify_with_snapshot_metadata() {
        let snapshot = snapshot(vec![expected(33345, 1.0)]);
        let result = available(calculate_battle_wn8(&battle("replay", 33345), &snapshot));
        assert_eq!(result.eligible_battles, 1);
        assert_eq!(result.snapshot_sha256, "a".repeat(64));
        assert_eq!(result.snapshot_source_version, "2026-07-10");
    }

    #[test]
    fn each_missing_fact_has_an_explicit_exclusion() {
        let snapshot = snapshot(vec![expected(1, 1.0)]);

        let mut missing_defense = battle("defense", 1);
        missing_defense.defense_points = None;
        assert_eq!(
            unavailable_result(calculate_battle_wn8(&missing_defense, &snapshot))
                .exclusions
                .missing_defense,
            1
        );

        let mut missing_vehicle = battle("vehicle", 1);
        missing_vehicle.wg_tank_id = None;
        assert_eq!(
            unavailable_result(calculate_battle_wn8(&missing_vehicle, &snapshot))
                .exclusions
                .missing_vehicle_id,
            1
        );

        let mut conflict = battle("conflict", 1);
        conflict.parser_error_code = Some("vehicle_id_conflict".to_owned());
        assert_eq!(
            unavailable_result(calculate_battle_wn8(&conflict, &snapshot))
                .exclusions
                .vehicle_id_conflict,
            1
        );

        let no_expected = battle("expected", 2);
        assert_eq!(
            unavailable_result(calculate_battle_wn8(&no_expected, &snapshot))
                .exclusions
                .vehicle_not_in_expected_values,
            1
        );

        for (field, mut battle) in [
            ("damage", battle("damage", 1)),
            ("frags", battle("frags", 1)),
            ("spots", battle("spots", 1)),
            ("result", battle("result", 1)),
        ] {
            match field {
                "damage" => battle.damage = None,
                "frags" => battle.frags = None,
                "spots" => battle.spots = None,
                _ => battle.outcome = BattleOutcome::Unknown,
            }
            let exclusions =
                unavailable_result(calculate_battle_wn8(&battle, &snapshot)).exclusions;
            match field {
                "damage" => assert_eq!(exclusions.missing_damage, 1),
                "frags" => assert_eq!(exclusions.missing_frags, 1),
                "spots" => assert_eq!(exclusions.missing_spots, 1),
                _ => assert_eq!(exclusions.missing_result, 1),
            }
        }
    }

    #[test]
    fn partial_and_unsupported_schema_are_excluded() {
        let snapshot = snapshot(vec![expected(1, 1.0)]);
        let mut partial = battle("partial", 1);
        partial.data_quality = DataQuality::Partial;
        assert_eq!(
            unavailable_result(calculate_battle_wn8(&partial, &snapshot))
                .exclusions
                .partial_or_incomplete,
            1
        );
        for schema in [None, Some(-1), Some(0), Some(4)] {
            let mut unsupported = battle("unsupported", 1);
            unsupported.feature_schema_version = schema;
            assert_eq!(
                unavailable_result(calculate_battle_wn8(&unsupported, &snapshot))
                    .exclusions
                    .unsupported_feature_schema,
                1
            );
        }
    }

    #[test]
    fn core_wn8_is_exactly_equivalent_for_feature_schemas_one_two_and_three() {
        let snapshot = snapshot(vec![expected(1, 1.0), expected(2, 2.0)]);
        let schema_one = [battle("one", 1), battle("two", 2)];
        let mut schema_two = schema_one.clone();
        for battle in &mut schema_two {
            battle.feature_schema_version = Some(2);
            battle.parser_version = Some(6);
            battle.direct_enemy_hits = Some(1);
            battle.direct_team_hits = Some(0);
            battle.piercing_enemy_hits = Some(1);
            battle.explosion_hits = Some(0);
        }
        let mut schema_three = schema_two.clone();
        for battle in &mut schema_three {
            battle.feature_schema_version = Some(3);
            battle.parser_version = Some(7);
        }
        assert_eq!(
            calculate_battle_wn8(&schema_one[0], &snapshot),
            calculate_battle_wn8(&schema_two[0], &snapshot)
        );
        assert_eq!(
            calculate_wn8(&schema_one, &snapshot),
            calculate_wn8(&schema_two, &snapshot)
        );
        assert_eq!(
            calculate_wn8(&schema_two, &snapshot),
            calculate_wn8(&schema_three, &snapshot)
        );
    }

    #[test]
    fn zero_eligible_is_unavailable_and_partial_coverage_is_exact() {
        let snapshot = snapshot(vec![expected(1, 1.0)]);
        let empty = unavailable_result(calculate_wn8(&[], &snapshot));
        assert_eq!(empty.eligible_battles, 0);
        assert_eq!(empty.coverage_ratio, 0.0);

        let first = battle("first", 1);
        let second = battle("second", 1);
        let mut partial = battle("partial", 1);
        partial.data_quality = DataQuality::Partial;
        let result = available(calculate_wn8(&[first, second, partial], &snapshot));
        assert_eq!(result.eligible_battles, 2);
        assert_eq!(result.total_battles, 3);
        assert_eq!(result.coverage_ratio, 2.0 / 3.0);
        assert_eq!(result.exclusions.partial_or_incomplete, 1);
    }

    #[test]
    fn invalid_expected_denominator_and_negative_actual_are_safe() {
        let mut invalid_expected = expected(1, 1.0);
        invalid_expected.exp_defense = 0.0;
        let result = unavailable_result(calculate_battle_wn8(
            &battle("invalid-expected", 1),
            &snapshot(vec![invalid_expected]),
        ));
        assert_eq!(result.exclusions.invalid_numeric_input, 1);

        let mut negative = battle("negative", 1);
        negative.damage = Some(-1);
        let result = unavailable_result(calculate_battle_wn8(
            &negative,
            &snapshot(vec![expected(1, 1.0)]),
        ));
        assert_eq!(result.exclusions.invalid_numeric_input, 1);

        let mut non_finite_expected = expected(1, 1.0);
        non_finite_expected.exp_damage = f64::INFINITY;
        let result = unavailable_result(calculate_battle_wn8(
            &battle("non-finite", 1),
            &snapshot(vec![non_finite_expected]),
        ));
        assert_eq!(result.exclusions.invalid_numeric_input, 1);

        let actual = Wn8Totals::default();
        let invalid = Wn8Totals {
            damage: f64::NAN,
            frags: 1.0,
            spots: 1.0,
            defense: 1.0,
            win_points: 1.0,
        };
        assert_eq!(formula(&actual, &invalid), None);
    }

    /// Each expected value must be finite and positive and the expected win rate at most 100;
    /// the formula alone would accept a win rate above 100 or a negative frag expectation.
    #[test]
    fn expected_row_validity_is_checked_field_by_field() {
        let valid = |row: ExpectedValuesRow| {
            matches!(
                calculate_battle_wn8(&battle("row", 1), &snapshot(vec![row])),
                Wn8Calculation::Available(_)
            )
        };
        let mut boundary = expected(1, 1.0);
        boundary.exp_win_rate = 100.0;
        assert!(valid(boundary));
        let mut above = expected(1, 1.0);
        above.exp_win_rate = 100.5;
        assert!(!valid(above));
        for field in 0..5 {
            for bad in [-1.0, 0.0, f64::NAN] {
                let mut row = expected(1, 1.0);
                match field {
                    0 => row.exp_damage = bad,
                    1 => row.exp_frags = bad,
                    2 => row.exp_spots = bad,
                    3 => row.exp_defense = bad,
                    _ => row.exp_win_rate = bad,
                }
                assert!(!valid(row), "field {field} = {bad}");
            }
        }
    }

    #[test]
    fn database_restart_with_same_facts_is_bit_identical() {
        let snapshot = snapshot(vec![expected(1, 1.0), expected(2, 2.0)]);
        let directory = tempdir().unwrap();
        let path = directory.path().join("restart.db");
        let database = Database::initialize(path.clone()).unwrap();
        let mut first = battle("one", 1);
        let mut second = battle("two", 2);
        database.insert_battle(&mut first, false).unwrap();
        database.insert_battle(&mut second, false).unwrap();
        let before = calculate_wn8(&database.wn8_candidates().unwrap(), &snapshot);
        drop(database);

        let reopened = Database::initialize(path).unwrap();
        let after = calculate_wn8(&reopened.wn8_candidates().unwrap(), &snapshot);
        assert_eq!(before, after);
    }

    // ---- E0 characterization tests --------------------------------------------------------
    // They pin TODAY's behavior. Tests tagged CHARACTERIZATION(<id>) freeze a known defect from
    // the author's internal 2026-11 audit and must be inverted by the stage that fixes it.

    /// Reference values computed by a control script (a separate Python implementation of the
    /// published WN8 formula) with expected row damage 1000 / frags 1 / spots 1 / defense 1 /
    /// win rate 50. This is not a comparison with an external WN8 calculator; that stays in the
    /// author's manual test.
    #[test]
    fn characterization_wn8_single_battle_matches_control_script_reference() {
        let mut one = battle("reference", 33345);
        one.damage = Some(1_500);
        one.frags = Some(2);
        one.spots = Some(1);
        one.defense_points = Some(0);
        let result = available(calculate_battle_wn8(
            &one,
            &snapshot(vec![expected(33345, 1.0)]),
        ));
        assert!(
            (result.value - 2789.009861932939).abs() < 1e-6,
            "{}",
            result.value
        );
    }

    /// E4 / §6.1: three cases against an independent calculator (a separate implementation of
    /// the published WN8 formula, written for this test, not the engine), with non-round
    /// XVM-like expected values; tolerance ±0.5 point. The comparison with an external website
    /// stays in the author's manual test (§6.4 point 4).
    #[test]
    fn wn8_matches_an_independent_calculator_in_three_cases() {
        let row = ExpectedValuesRow {
            vehicle_id: 4_321,
            exp_damage: 1_834.27,
            exp_frags: 1.0823,
            exp_spots: 1.3417,
            exp_defense: 0.7861,
            exp_win_rate: 52.384,
        };
        let rows = snapshot(vec![row]);
        let make = |id: &str, outcome, damage, frags, spots, defense| {
            let mut value = battle(id, 4_321);
            value.outcome = outcome;
            value.damage = Some(damage);
            value.frags = Some(frags);
            value.spots = Some(spots);
            value.defense_points = Some(defense);
            value
        };
        let victory = make("v1", BattleOutcome::Victory, 2_650, 2, 3, 1);
        let defeat = make("d1", BattleOutcome::Defeat, 740, 0, 1, 0);
        let draw = make("x1", BattleOutcome::Draw, 1_900, 1, 0, 2);
        let second_victory = make("v2", BattleOutcome::Victory, 3_100, 3, 2, 0);
        for (label, battles, reference) in [
            ("victory", vec![victory.clone()], 3_014.563_810_810_882_4),
            ("defeat", vec![defeat.clone()], 230.463_644_238_824_54),
            (
                "four battles",
                vec![victory, defeat, draw, second_victory],
                1_980.498_657_541_108_5,
            ),
        ] {
            let value = available(calculate_wn8(&battles, &rows)).value;
            assert!((value - reference).abs() <= 0.5, "{label}: {value}");
        }
    }

    #[test]
    fn characterization_wn8_aggregate_is_computed_from_summed_totals() {
        let mut won = battle("won", 33345);
        won.damage = Some(1_500);
        won.frags = Some(2);
        won.spots = Some(1);
        won.defense_points = Some(0);
        let mut lost = battle("lost", 33345);
        lost.damage = Some(500);
        lost.frags = Some(0);
        lost.spots = Some(0);
        lost.defense_points = Some(1);
        lost.outcome = BattleOutcome::Defeat;
        let result = available(calculate_wn8(
            &[won, lost],
            &snapshot(vec![expected(33345, 1.0)]),
        ));
        // Totals: damage 2000, frags 2, spots 1, defense 1, win 100 against expected
        // 2000 / 2 / 2 / 2 / 100.
        assert!(
            (result.value - 1398.3333333333333).abs() < 1e-6,
            "{}",
            result.value
        );
        assert_eq!(result.eligible_battles, 2);
    }

    /// M2 (fixed in E1, was a characterization test): WN8 excludes every battle that is not a
    /// strict random battle and reports it as `not_random_battle`; the value equals the
    /// random-only value.
    #[test]
    fn wn8_excludes_non_random_battles() {
        let rows = snapshot(vec![expected(33345, 1.0)]);
        let random = battle("random", 33345);
        let random_only = available(calculate_wn8(std::slice::from_ref(&random), &rows));

        for (battle_type_raw, bonus_type_raw, battle_type) in [
            (Some(2), Some(2), "training"),
            (Some(1), Some(7), "ranked_battles"),
            (Some(22), None, "event"),
            (None, None, "something_else"),
            (None, Some(1), "random"),
        ] {
            let mut other = battle("other", 33345);
            other.damage = Some(3_000);
            other.battle_type = Some(battle_type.to_owned());
            other.battle_pace.battle_type_raw = battle_type_raw;
            other.battle_pace.bonus_type_raw = bonus_type_raw;
            let mixed = available(calculate_wn8(&[random.clone(), other], &rows));
            assert_eq!(mixed.eligible_battles, 1, "{battle_type}");
            assert_eq!(
                mixed.exclusions,
                Wn8ExclusionCounts {
                    not_random_battle: 1,
                    ..Wn8ExclusionCounts::default()
                },
                "{battle_type}"
            );
            assert_eq!(mixed.value, random_only.value, "{battle_type}");
        }
    }
}
