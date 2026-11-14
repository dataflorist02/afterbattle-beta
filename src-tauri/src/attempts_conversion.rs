use std::collections::BTreeMap;

use thiserror::Error;

use crate::{
    models::{DataQuality, VehicleClass, VehicleClassResolution, VehicleClassResolutionState},
    vehicle_class_catalog::{LoadedVehicleClassCatalog, unavailable_resolution},
};

pub const ATTEMPTS_CONVERSION_VERSION: &str = "attempts-conversion-v2";
const MAX_COUNTER: i64 = 2_000;
const MAX_DAMAGE: i64 = 200_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptsConversionBattle {
    pub arena_id: String,
    pub battle_time: i64,
    pub vehicle_key: String,
    pub wg_tank_id: Option<i64>,
    pub data_quality: DataQuality,
    pub data_completeness: Option<String>,
    pub replay_source: Option<String>,
    pub parser_version: Option<i64>,
    pub feature_schema_version: Option<i64>,
    pub gameplay_id: Option<String>,
    pub battle_type_raw: Option<i64>,
    pub bonus_type_raw: Option<i64>,
    pub initial_team1_size: Option<i64>,
    pub initial_team2_size: Option<i64>,
    pub shots: Option<i64>,
    pub direct_hits: Option<i64>,
    pub direct_enemy_hits: Option<i64>,
    pub direct_team_hits: Option<i64>,
    pub piercings: Option<i64>,
    pub piercing_enemy_hits: Option<i64>,
    pub explosion_hits: Option<i64>,
    pub damage_dealt: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttemptsConversionRejectionReason {
    PartialReplay,
    ResultsProvenanceMissing,
    ParserTooOld,
    UnsupportedSchema,
    UnsupportedMode,
    InvalidTeamStructure,
    VehicleClassCatalogUnavailable,
    VehicleClassUnknown,
    VehicleClassConflict,
    UnsupportedVehicleMechanics,
    MissingCounters,
    CounterOutOfRange,
    CounterConflict,
}

impl AttemptsConversionRejectionReason {
    pub const COMMON: [Self; 10] = [
        Self::PartialReplay,
        Self::ResultsProvenanceMissing,
        Self::ParserTooOld,
        Self::UnsupportedSchema,
        Self::UnsupportedMode,
        Self::InvalidTeamStructure,
        Self::VehicleClassCatalogUnavailable,
        Self::VehicleClassUnknown,
        Self::VehicleClassConflict,
        Self::UnsupportedVehicleMechanics,
    ];
    pub const METRIC: [Self; 13] = [
        Self::PartialReplay,
        Self::ResultsProvenanceMissing,
        Self::ParserTooOld,
        Self::UnsupportedSchema,
        Self::UnsupportedMode,
        Self::InvalidTeamStructure,
        Self::VehicleClassCatalogUnavailable,
        Self::VehicleClassUnknown,
        Self::VehicleClassConflict,
        Self::UnsupportedVehicleMechanics,
        Self::MissingCounters,
        Self::CounterOutOfRange,
        Self::CounterConflict,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PartialReplay => "partial_replay",
            Self::ResultsProvenanceMissing => "results_provenance_missing",
            Self::ParserTooOld => "parser_too_old",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::UnsupportedMode => "unsupported_mode",
            Self::InvalidTeamStructure => "invalid_team_structure",
            Self::VehicleClassCatalogUnavailable => "vehicle_class_catalog_unavailable",
            Self::VehicleClassUnknown => "vehicle_class_unknown",
            Self::VehicleClassConflict => "vehicle_class_conflict",
            Self::UnsupportedVehicleMechanics => "unsupported_vehicle_mechanics",
            Self::MissingCounters => "missing_counters",
            Self::CounterOutOfRange => "counter_out_of_range",
            Self::CounterConflict => "counter_conflict",
        }
    }

    pub fn is_common(self) -> bool {
        Self::COMMON.contains(&self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonContextCoverage {
    pub selected_battles: usize,
    pub eligible_battles: usize,
    pub ineligible_battles: usize,
    pub rejection_reason_counts: BTreeMap<AttemptsConversionRejectionReason, usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttemptsConversionRateCoverage {
    pub selected_battles: usize,
    pub common_eligible_battles: usize,
    pub metric_eligible_battles: usize,
    pub metric_ineligible_battles: usize,
    pub contributing_battles: usize,
    pub zero_denominator_battles: usize,
    pub missing_battles: usize,
    pub invalid_battles: usize,
    pub numerator_sum: i64,
    pub denominator_sum: i64,
    pub value: Option<f64>,
    pub rejection_reason_counts: BTreeMap<AttemptsConversionRejectionReason, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptsConversionFactCoverage {
    pub selected_battles: usize,
    pub common_eligible_battles: usize,
    pub fact_eligible_battles: usize,
    pub fact_ineligible_battles: usize,
    pub contributing_battles: usize,
    pub missing_battles: usize,
    pub invalid_battles: usize,
    pub value_sum: Option<i64>,
    pub rejection_reason_counts: BTreeMap<AttemptsConversionRejectionReason, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptsConversionFacts {
    pub direct_team_hits: AttemptsConversionFactCoverage,
    pub explosion_hits: AttemptsConversionFactCoverage,
    pub damage_dealt: AttemptsConversionFactCoverage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttemptsConversionReport {
    pub contract_version: &'static str,
    pub selected_battles: usize,
    pub common_context: CommonContextCoverage,
    pub hit_rate: AttemptsConversionRateCoverage,
    pub penetration_rate: AttemptsConversionRateCoverage,
    pub facts: AttemptsConversionFacts,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AttemptsConversionError {
    #[error("attempts/conversion totals overflow")]
    IntegerOverflow,
    #[error("attempts/conversion ratio is invalid")]
    InvalidRatio,
}

fn resolution_for(
    battle: &AttemptsConversionBattle,
    snapshot: Option<&LoadedVehicleClassCatalog>,
) -> VehicleClassResolution {
    let vehicle_id = battle.wg_tank_id.unwrap_or_default();
    snapshot.map_or_else(
        || unavailable_resolution(vehicle_id),
        |catalog| catalog.resolve(vehicle_id, Some(&battle.vehicle_key)),
    )
}

fn common_reason(
    battle: &AttemptsConversionBattle,
    resolution: &VehicleClassResolution,
) -> Option<AttemptsConversionRejectionReason> {
    if battle.data_quality != DataQuality::Full {
        Some(AttemptsConversionRejectionReason::PartialReplay)
    } else if battle.data_completeness.as_deref() != Some("results")
        || battle.replay_source.as_deref() != Some("replay_results")
    {
        Some(AttemptsConversionRejectionReason::ResultsProvenanceMissing)
    } else if battle.parser_version.unwrap_or_default() < 6 {
        Some(AttemptsConversionRejectionReason::ParserTooOld)
    } else if !matches!(battle.feature_schema_version, Some(2) | Some(3)) {
        Some(AttemptsConversionRejectionReason::UnsupportedSchema)
    } else if battle.battle_type_raw != Some(1)
        || battle.bonus_type_raw != Some(1)
        || battle.gameplay_id.as_deref() != Some("ctf")
    {
        Some(AttemptsConversionRejectionReason::UnsupportedMode)
    } else if battle.initial_team1_size != Some(15) || battle.initial_team2_size != Some(15) {
        Some(AttemptsConversionRejectionReason::InvalidTeamStructure)
    } else if resolution.state == VehicleClassResolutionState::CatalogUnavailable {
        Some(AttemptsConversionRejectionReason::VehicleClassCatalogUnavailable)
    } else if battle.wg_tank_id.unwrap_or_default() <= 0
        || resolution.state == VehicleClassResolutionState::Unknown
    {
        Some(AttemptsConversionRejectionReason::VehicleClassUnknown)
    } else if resolution.state == VehicleClassResolutionState::Conflict {
        Some(AttemptsConversionRejectionReason::VehicleClassConflict)
    } else if resolution.vehicle_class == VehicleClass::Spg {
        Some(AttemptsConversionRejectionReason::UnsupportedVehicleMechanics)
    } else {
        None
    }
}

fn in_range(value: i64, maximum: i64) -> bool {
    (0..=maximum).contains(&value)
}

fn checked_add(total: &mut i64, value: i64) -> Result<(), AttemptsConversionError> {
    *total = total
        .checked_add(value)
        .ok_or(AttemptsConversionError::IntegerOverflow)?;
    Ok(())
}

fn ratio(numerator: i64, denominator: i64) -> Result<Option<f64>, AttemptsConversionError> {
    if denominator == 0 {
        return Ok(None);
    }
    let value = numerator as f64 / denominator as f64;
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(AttemptsConversionError::InvalidRatio);
    }
    Ok(Some(value))
}

fn empty_common(selected_battles: usize) -> CommonContextCoverage {
    CommonContextCoverage {
        selected_battles,
        eligible_battles: 0,
        ineligible_battles: 0,
        rejection_reason_counts: BTreeMap::new(),
    }
}

fn empty_rate(selected_battles: usize) -> AttemptsConversionRateCoverage {
    AttemptsConversionRateCoverage {
        selected_battles,
        common_eligible_battles: 0,
        metric_eligible_battles: 0,
        metric_ineligible_battles: 0,
        contributing_battles: 0,
        zero_denominator_battles: 0,
        missing_battles: 0,
        invalid_battles: 0,
        numerator_sum: 0,
        denominator_sum: 0,
        value: None,
        rejection_reason_counts: BTreeMap::new(),
    }
}

fn empty_fact(selected_battles: usize) -> AttemptsConversionFactCoverage {
    AttemptsConversionFactCoverage {
        selected_battles,
        common_eligible_battles: 0,
        fact_eligible_battles: 0,
        fact_ineligible_battles: 0,
        contributing_battles: 0,
        missing_battles: 0,
        invalid_battles: 0,
        value_sum: None,
        rejection_reason_counts: BTreeMap::new(),
    }
}

fn count_reason(
    counts: &mut BTreeMap<AttemptsConversionRejectionReason, usize>,
    reason: AttemptsConversionRejectionReason,
) {
    *counts.entry(reason).or_default() += 1;
}

fn reject_rate(
    coverage: &mut AttemptsConversionRateCoverage,
    reason: AttemptsConversionRejectionReason,
) {
    coverage.metric_ineligible_battles += 1;
    if reason == AttemptsConversionRejectionReason::MissingCounters {
        coverage.missing_battles += 1;
    } else if !reason.is_common() {
        coverage.invalid_battles += 1;
    }
    count_reason(&mut coverage.rejection_reason_counts, reason);
}

fn reject_fact(
    coverage: &mut AttemptsConversionFactCoverage,
    reason: AttemptsConversionRejectionReason,
) {
    coverage.fact_ineligible_battles += 1;
    if reason == AttemptsConversionRejectionReason::MissingCounters {
        coverage.missing_battles += 1;
    } else if !reason.is_common() {
        coverage.invalid_battles += 1;
    }
    count_reason(&mut coverage.rejection_reason_counts, reason);
}

fn add_rate(
    coverage: &mut AttemptsConversionRateCoverage,
    numerator: i64,
    denominator: i64,
) -> Result<(), AttemptsConversionError> {
    coverage.metric_eligible_battles += 1;
    if denominator == 0 {
        coverage.zero_denominator_battles += 1;
    } else {
        coverage.contributing_battles += 1;
        checked_add(&mut coverage.numerator_sum, numerator)?;
        checked_add(&mut coverage.denominator_sum, denominator)?;
    }
    Ok(())
}

fn add_fact(
    coverage: &mut AttemptsConversionFactCoverage,
    value: i64,
) -> Result<(), AttemptsConversionError> {
    coverage.fact_eligible_battles += 1;
    coverage.contributing_battles += 1;
    let total = coverage.value_sum.get_or_insert(0);
    checked_add(total, value)
}

fn hit_reason(battle: &AttemptsConversionBattle) -> Option<AttemptsConversionRejectionReason> {
    let (Some(shots), Some(enemy)) = (battle.shots, battle.direct_enemy_hits) else {
        return Some(AttemptsConversionRejectionReason::MissingCounters);
    };
    if !in_range(shots, MAX_COUNTER) || !in_range(enemy, MAX_COUNTER) {
        Some(AttemptsConversionRejectionReason::CounterOutOfRange)
    } else if enemy > shots {
        Some(AttemptsConversionRejectionReason::CounterConflict)
    } else {
        None
    }
}

fn penetration_reason(
    battle: &AttemptsConversionBattle,
) -> Option<AttemptsConversionRejectionReason> {
    let (Some(enemy), Some(piercing)) = (battle.direct_enemy_hits, battle.piercing_enemy_hits)
    else {
        return Some(AttemptsConversionRejectionReason::MissingCounters);
    };
    if !in_range(enemy, MAX_COUNTER) || !in_range(piercing, MAX_COUNTER) {
        Some(AttemptsConversionRejectionReason::CounterOutOfRange)
    } else if piercing > enemy
        || battle
            .shots
            .is_some_and(|shots| in_range(shots, MAX_COUNTER) && enemy > shots)
    {
        Some(AttemptsConversionRejectionReason::CounterConflict)
    } else {
        None
    }
}

fn fact_reason(value: Option<i64>, maximum: i64) -> Option<AttemptsConversionRejectionReason> {
    match value {
        None => Some(AttemptsConversionRejectionReason::MissingCounters),
        Some(value) if !in_range(value, maximum) => {
            Some(AttemptsConversionRejectionReason::CounterOutOfRange)
        }
        Some(_) => None,
    }
}

pub fn analyze_with_resolver<F>(
    battles: &[AttemptsConversionBattle],
    mut resolve: F,
) -> Result<AttemptsConversionReport, AttemptsConversionError>
where
    F: FnMut(&AttemptsConversionBattle) -> VehicleClassResolution,
{
    let selected_battles = battles.len();
    let mut common_context = empty_common(selected_battles);
    let mut hit_rate = empty_rate(selected_battles);
    let mut penetration_rate = empty_rate(selected_battles);
    let mut direct_team_hits = empty_fact(selected_battles);
    let mut explosion_hits = empty_fact(selected_battles);
    let mut damage_dealt = empty_fact(selected_battles);

    for battle in battles {
        let resolution = resolve(battle);
        if let Some(reason) = common_reason(battle, &resolution) {
            common_context.ineligible_battles += 1;
            count_reason(&mut common_context.rejection_reason_counts, reason);
            reject_rate(&mut hit_rate, reason);
            reject_rate(&mut penetration_rate, reason);
            reject_fact(&mut direct_team_hits, reason);
            reject_fact(&mut explosion_hits, reason);
            reject_fact(&mut damage_dealt, reason);
            continue;
        }

        common_context.eligible_battles += 1;
        hit_rate.common_eligible_battles += 1;
        penetration_rate.common_eligible_battles += 1;
        direct_team_hits.common_eligible_battles += 1;
        explosion_hits.common_eligible_battles += 1;
        damage_dealt.common_eligible_battles += 1;

        if let Some(reason) = hit_reason(battle) {
            reject_rate(&mut hit_rate, reason);
        } else {
            add_rate(
                &mut hit_rate,
                battle
                    .direct_enemy_hits
                    .expect("hit evaluator checked enemy hits"),
                battle.shots.expect("hit evaluator checked shots"),
            )?;
        }

        if let Some(reason) = penetration_reason(battle) {
            reject_rate(&mut penetration_rate, reason);
        } else {
            add_rate(
                &mut penetration_rate,
                battle
                    .piercing_enemy_hits
                    .expect("penetration evaluator checked piercing hits"),
                battle
                    .direct_enemy_hits
                    .expect("penetration evaluator checked enemy hits"),
            )?;
        }

        for (coverage, value, maximum) in [
            (&mut direct_team_hits, battle.direct_team_hits, MAX_COUNTER),
            (&mut explosion_hits, battle.explosion_hits, MAX_COUNTER),
            (&mut damage_dealt, battle.damage_dealt, MAX_DAMAGE),
        ] {
            if let Some(reason) = fact_reason(value, maximum) {
                reject_fact(coverage, reason);
            } else {
                add_fact(coverage, value.expect("fact evaluator checked value"))?;
            }
        }
    }

    hit_rate.value = ratio(hit_rate.numerator_sum, hit_rate.denominator_sum)?;
    penetration_rate.value = ratio(
        penetration_rate.numerator_sum,
        penetration_rate.denominator_sum,
    )?;
    Ok(AttemptsConversionReport {
        contract_version: ATTEMPTS_CONVERSION_VERSION,
        selected_battles,
        common_context,
        hit_rate,
        penetration_rate,
        facts: AttemptsConversionFacts {
            direct_team_hits,
            explosion_hits,
            damage_dealt,
        },
    })
}

/// The runtime passes exactly one immutable catalog snapshot. The calculation
/// only reads that snapshot and never writes report state or replay data.
pub(crate) fn analyze_with_catalog(
    battles: &[AttemptsConversionBattle],
    snapshot: Option<&LoadedVehicleClassCatalog>,
) -> Result<AttemptsConversionReport, AttemptsConversionError> {
    analyze_with_resolver(battles, |battle| resolution_for(battle, snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        CompactDescriptorProtocolSource, VehicleClassSource, VehicleKeyCrossCheck,
    };

    fn battle(id: &str) -> AttemptsConversionBattle {
        AttemptsConversionBattle {
            arena_id: id.into(),
            battle_time: 1,
            vehicle_key: "tank".into(),
            wg_tank_id: Some(7),
            data_quality: DataQuality::Full,
            data_completeness: Some("results".into()),
            replay_source: Some("replay_results".into()),
            parser_version: Some(6),
            feature_schema_version: Some(2),
            gameplay_id: Some("ctf".into()),
            battle_type_raw: Some(1),
            bonus_type_raw: Some(1),
            initial_team1_size: Some(15),
            initial_team2_size: Some(15),
            shots: Some(10),
            direct_hits: Some(6),
            direct_enemy_hits: Some(6),
            direct_team_hits: Some(0),
            piercings: Some(4),
            piercing_enemy_hits: Some(4),
            explosion_hits: Some(1),
            damage_dealt: Some(1000),
        }
    }

    fn resolution(
        state: VehicleClassResolutionState,
        class: VehicleClass,
    ) -> VehicleClassResolution {
        VehicleClassResolution {
            vehicle_id: 7,
            vehicle_class: class,
            state,
            source: VehicleClassSource::WgClientPackedCatalog,
            catalog_schema_version: Some(1),
            catalog_source_version: Some("synthetic".into()),
            catalog_fingerprint: None,
            protocol_source:
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            vehicle_key_cross_check: VehicleKeyCrossCheck::RawExact,
        }
    }

    fn report(values: &[AttemptsConversionBattle]) -> AttemptsConversionReport {
        analyze_with_resolver(values, |_| {
            resolution(
                VehicleClassResolutionState::Resolved,
                VehicleClass::HeavyTank,
            )
        })
        .unwrap()
    }

    fn reason_count(
        counts: &BTreeMap<AttemptsConversionRejectionReason, usize>,
        reason: AttemptsConversionRejectionReason,
    ) -> usize {
        counts.get(&reason).copied().unwrap_or_default()
    }

    #[test]
    fn golden_missing_piercing_keeps_hit_and_independent_facts() {
        let mut value = battle("golden");
        value.piercing_enemy_hits = None;
        value.damage_dealt = Some(2_000);
        let actual = report(&[value]);
        assert_eq!(actual.contract_version, "attempts-conversion-v2");
        assert_eq!(actual.common_context.eligible_battles, 1);
        assert_eq!(actual.hit_rate.metric_eligible_battles, 1);
        assert_eq!(actual.hit_rate.numerator_sum, 6);
        assert_eq!(actual.hit_rate.denominator_sum, 10);
        assert_eq!(actual.hit_rate.value, Some(0.6));
        assert_eq!(actual.penetration_rate.metric_eligible_battles, 0);
        assert_eq!(
            reason_count(
                &actual.penetration_rate.rejection_reason_counts,
                AttemptsConversionRejectionReason::MissingCounters
            ),
            1
        );
        assert_eq!(actual.facts.direct_team_hits.value_sum, Some(0));
        assert_eq!(actual.facts.explosion_hits.value_sum, Some(1));
        assert_eq!(actual.facts.damage_dealt.value_sum, Some(2_000));
    }

    #[test]
    fn unrelated_missing_facts_do_not_block_rates() {
        for field in ["explosion", "team", "damage"] {
            let mut value = battle(field);
            match field {
                "explosion" => value.explosion_hits = None,
                "team" => value.direct_team_hits = None,
                _ => value.damage_dealt = None,
            }
            let actual = report(&[value]);
            assert_eq!(actual.hit_rate.metric_eligible_battles, 1, "{field}");
            assert_eq!(
                actual.penetration_rate.metric_eligible_battles, 1,
                "{field}"
            );
        }
    }

    #[test]
    fn missing_shots_only_blocks_hit_not_penetration() {
        let mut value = battle("missing-shots");
        value.shots = None;
        let actual = report(&[value]);
        assert_eq!(actual.hit_rate.metric_eligible_battles, 0);
        assert_eq!(actual.penetration_rate.metric_eligible_battles, 1);
        assert_eq!(actual.penetration_rate.value, Some(4.0 / 6.0));
    }

    #[test]
    fn missing_direct_enemy_hits_blocks_both_metrics() {
        let mut value = battle("missing-enemy");
        value.direct_enemy_hits = None;
        let actual = report(&[value]);
        assert_eq!(actual.hit_rate.metric_eligible_battles, 0);
        assert_eq!(actual.penetration_rate.metric_eligible_battles, 0);
        assert_eq!(
            reason_count(
                &actual.hit_rate.rejection_reason_counts,
                AttemptsConversionRejectionReason::MissingCounters
            ),
            1
        );
    }

    #[test]
    fn missing_piercing_with_zero_shots_keeps_hit_zero_denominator() {
        let mut value = battle("zero-shots-missing-piercing");
        value.shots = Some(0);
        value.direct_enemy_hits = Some(0);
        value.piercing_enemy_hits = None;
        let actual = report(&[value]);
        assert_eq!(actual.hit_rate.metric_eligible_battles, 1);
        assert_eq!(actual.hit_rate.zero_denominator_battles, 1);
        assert_eq!(actual.penetration_rate.metric_eligible_battles, 0);
    }

    #[test]
    fn zero_denominators_are_eligible_not_rejections() {
        let mut zero_shots = battle("zero-shots");
        zero_shots.shots = Some(0);
        zero_shots.direct_enemy_hits = Some(0);
        zero_shots.piercing_enemy_hits = Some(0);
        let actual = report(&[zero_shots]);
        assert_eq!(actual.hit_rate.zero_denominator_battles, 1);
        assert_eq!(actual.penetration_rate.zero_denominator_battles, 1);
        assert_eq!(actual.hit_rate.value, None);
        assert_eq!(actual.penetration_rate.value, None);
        assert!(actual.hit_rate.rejection_reason_counts.is_empty());
    }

    #[test]
    fn feature_schema_three_is_exactly_equivalent_to_schema_two() {
        let schema_two = battle("schema-two");
        let mut schema_three = schema_two.clone();
        schema_three.feature_schema_version = Some(3);
        schema_three.parser_version = Some(7);
        assert_eq!(report(&[schema_two]), report(&[schema_three]));
    }

    /// M12: the contract accepts schema 2 or 3; anything else stays unsupported.
    #[test]
    fn other_feature_schemas_are_unsupported() {
        for schema in [None, Some(1), Some(4)] {
            let mut value = battle("schema-other");
            value.feature_schema_version = schema;
            let actual = report(&[value]);
            assert_eq!(actual.common_context.eligible_battles, 0, "{schema:?}");
            assert_eq!(
                actual
                    .common_context
                    .rejection_reason_counts
                    .get(&AttemptsConversionRejectionReason::UnsupportedSchema)
                    .copied(),
                Some(1),
                "{schema:?}"
            );
        }
    }

    #[test]
    fn zero_enemy_hits_is_real_zero_percent_for_hit_and_zero_denominator_for_penetration() {
        let mut value = battle("zero-enemy");
        value.direct_enemy_hits = Some(0);
        value.piercing_enemy_hits = Some(0);
        let actual = report(&[value]);
        assert_eq!(actual.hit_rate.value, Some(0.0));
        assert_eq!(actual.penetration_rate.value, None);
        assert_eq!(actual.penetration_rate.zero_denominator_battles, 1);
    }

    #[test]
    fn metric_conflicts_are_local() {
        let mut hit_conflict = battle("hit-conflict");
        hit_conflict.direct_enemy_hits = Some(11);
        let actual = report(&[hit_conflict]);
        assert_eq!(
            reason_count(
                &actual.hit_rate.rejection_reason_counts,
                AttemptsConversionRejectionReason::CounterConflict
            ),
            1
        );
        assert_eq!(
            reason_count(
                &actual.penetration_rate.rejection_reason_counts,
                AttemptsConversionRejectionReason::CounterConflict
            ),
            1
        );
        let mut penetration_conflict = battle("penetration-conflict");
        penetration_conflict.piercing_enemy_hits = Some(7);
        let actual = report(&[penetration_conflict]);
        assert_eq!(actual.hit_rate.metric_eligible_battles, 1);
        assert_eq!(
            reason_count(
                &actual.penetration_rate.rejection_reason_counts,
                AttemptsConversionRejectionReason::CounterConflict
            ),
            1
        );
    }

    #[test]
    fn fact_ranges_are_independent_and_zero_is_a_fact() {
        let mut value = battle("invalid-explosion");
        value.explosion_hits = Some(2_001);
        value.direct_team_hits = Some(0);
        let actual = report(&[value]);
        assert_eq!(actual.hit_rate.metric_eligible_battles, 1);
        assert_eq!(actual.penetration_rate.metric_eligible_battles, 1);
        assert_eq!(actual.facts.direct_team_hits.value_sum, Some(0));
        assert_eq!(
            reason_count(
                &actual.facts.explosion_hits.rejection_reason_counts,
                AttemptsConversionRejectionReason::CounterOutOfRange
            ),
            1
        );
    }

    #[test]
    fn common_rejection_is_copied_to_each_coverage() {
        let mut value = battle("partial");
        value.data_quality = DataQuality::Partial;
        let actual = report(&[value]);
        assert_eq!(actual.common_context.ineligible_battles, 1);
        for counts in [
            &actual.hit_rate.rejection_reason_counts,
            &actual.penetration_rate.rejection_reason_counts,
            &actual.facts.direct_team_hits.rejection_reason_counts,
            &actual.facts.explosion_hits.rejection_reason_counts,
            &actual.facts.damage_dealt.rejection_reason_counts,
        ] {
            assert_eq!(
                reason_count(counts, AttemptsConversionRejectionReason::PartialReplay),
                1
            );
        }
    }

    #[test]
    fn mixed_session_has_separate_coverages_and_sum_aggregation() {
        let valid = battle("valid");
        let mut missing_piercing = battle("missing-piercing");
        missing_piercing.piercing_enemy_hits = None;
        let mut missing_shots = battle("missing-shots");
        missing_shots.shots = None;
        let actual = report(&[valid, missing_piercing, missing_shots]);
        assert_eq!(actual.common_context.eligible_battles, 3);
        assert_eq!(actual.hit_rate.metric_eligible_battles, 2);
        assert_eq!(actual.penetration_rate.metric_eligible_battles, 2);
        assert_eq!(actual.hit_rate.numerator_sum, 12);
        assert_eq!(actual.hit_rate.denominator_sum, 20);
        assert_eq!(actual.penetration_rate.numerator_sum, 8);
        assert_eq!(actual.penetration_rate.denominator_sum, 12);
    }

    #[test]
    fn overflow_fails_closed() {
        let mut total = i64::MAX;
        assert_eq!(
            checked_add(&mut total, 1),
            Err(AttemptsConversionError::IntegerOverflow)
        );
    }
}
