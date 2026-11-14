use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::attempts_conversion::{
    ATTEMPTS_CONVERSION_VERSION, AttemptsConversionFactCoverage, AttemptsConversionRateCoverage,
    AttemptsConversionRejectionReason, AttemptsConversionReport, CommonContextCoverage,
};

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AttemptsConversionScopeDto {
    Session {
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    Recent {
        window: u16,
    },
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptsConversionRejectionReasonDto {
    pub code: String,
    pub count: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonContextCoverageDto {
    pub eligible_battles: usize,
    pub ineligible_battles: usize,
    pub rejection_reasons: Vec<AttemptsConversionRejectionReasonDto>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptsConversionRateDto {
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
    pub rejection_reasons: Vec<AttemptsConversionRejectionReasonDto>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptsConversionFactDto {
    pub selected_battles: usize,
    pub common_eligible_battles: usize,
    pub fact_eligible_battles: usize,
    pub fact_ineligible_battles: usize,
    pub contributing_battles: usize,
    pub missing_battles: usize,
    pub invalid_battles: usize,
    pub value_sum: Option<i64>,
    pub rejection_reasons: Vec<AttemptsConversionRejectionReasonDto>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptsConversionFactsDto {
    pub direct_team_hits: AttemptsConversionFactDto,
    pub explosion_hits: AttemptsConversionFactDto,
    pub damage_dealt: AttemptsConversionFactDto,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptsConversionReportDto {
    pub contract_version: String,
    pub scope: AttemptsConversionScopeDto,
    pub selected_battles: usize,
    pub common_context: CommonContextCoverageDto,
    pub hit_rate: AttemptsConversionRateDto,
    pub penetration_rate: AttemptsConversionRateDto,
    pub facts: AttemptsConversionFactsDto,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AttemptsConversionDtoError {
    #[error("attempts/conversion dto invariant violation")]
    InvariantViolation,
}

fn reasons(
    counts: &std::collections::BTreeMap<AttemptsConversionRejectionReason, usize>,
    catalog: &[AttemptsConversionRejectionReason],
) -> Vec<AttemptsConversionRejectionReasonDto> {
    catalog
        .iter()
        .map(|reason| AttemptsConversionRejectionReasonDto {
            code: reason.as_str().to_owned(),
            count: counts.get(reason).copied().unwrap_or_default(),
        })
        .collect()
}

fn reason_total(reasons: &[AttemptsConversionRejectionReasonDto]) -> Option<usize> {
    reasons
        .iter()
        .try_fold(0usize, |total, reason| total.checked_add(reason.count))
}

fn metric_reason_count(
    reasons: &[AttemptsConversionRejectionReasonDto],
    reason: AttemptsConversionRejectionReason,
) -> Result<usize, AttemptsConversionDtoError> {
    let mut matching = reasons
        .iter()
        .filter(|candidate| candidate.code == reason.as_str());
    let count = matching
        .next()
        .map(|candidate| candidate.count)
        .ok_or(AttemptsConversionDtoError::InvariantViolation)?;
    if matching.next().is_some() {
        return Err(AttemptsConversionDtoError::InvariantViolation);
    }
    Ok(count)
}

fn metric_reason_totals(
    reasons: &[AttemptsConversionRejectionReasonDto],
) -> Result<(usize, usize), AttemptsConversionDtoError> {
    let missing = metric_reason_count(reasons, AttemptsConversionRejectionReason::MissingCounters)?;
    let out_of_range = metric_reason_count(
        reasons,
        AttemptsConversionRejectionReason::CounterOutOfRange,
    )?;
    let conflict =
        metric_reason_count(reasons, AttemptsConversionRejectionReason::CounterConflict)?;
    let invalid = out_of_range
        .checked_add(conflict)
        .ok_or(AttemptsConversionDtoError::InvariantViolation)?;
    Ok((missing, invalid))
}

fn common_to_dto(
    selected_battles: usize,
    coverage: CommonContextCoverage,
) -> Result<CommonContextCoverageDto, AttemptsConversionDtoError> {
    if coverage.selected_battles != selected_battles
        || coverage
            .eligible_battles
            .checked_add(coverage.ineligible_battles)
            != Some(selected_battles)
        || coverage
            .rejection_reason_counts
            .keys()
            .any(|reason| !reason.is_common())
    {
        return Err(AttemptsConversionDtoError::InvariantViolation);
    }
    let rejection_reasons = reasons(
        &coverage.rejection_reason_counts,
        &AttemptsConversionRejectionReason::COMMON,
    );
    if reason_total(&rejection_reasons) != Some(coverage.ineligible_battles) {
        return Err(AttemptsConversionDtoError::InvariantViolation);
    }
    Ok(CommonContextCoverageDto {
        eligible_battles: coverage.eligible_battles,
        ineligible_battles: coverage.ineligible_battles,
        rejection_reasons,
    })
}

fn rate_to_dto(
    selected_battles: usize,
    common_eligible_battles: usize,
    coverage: AttemptsConversionRateCoverage,
) -> Result<AttemptsConversionRateDto, AttemptsConversionDtoError> {
    let invariant = coverage.selected_battles != selected_battles
        || coverage.common_eligible_battles != common_eligible_battles
        || coverage
            .metric_eligible_battles
            .checked_add(coverage.metric_ineligible_battles)
            != Some(selected_battles)
        || coverage
            .contributing_battles
            .checked_add(coverage.zero_denominator_battles)
            != Some(coverage.metric_eligible_battles)
        || coverage.numerator_sum < 0
        || coverage.denominator_sum < 0
        || coverage.numerator_sum > coverage.denominator_sum;
    if invariant {
        return Err(AttemptsConversionDtoError::InvariantViolation);
    }
    let rejection_reasons = reasons(
        &coverage.rejection_reason_counts,
        &AttemptsConversionRejectionReason::METRIC,
    );
    let expected = if coverage.denominator_sum == 0 {
        None
    } else {
        Some(coverage.numerator_sum as f64 / coverage.denominator_sum as f64)
    };
    let value_valid = match (coverage.value, expected) {
        (None, None) => true,
        (Some(actual), Some(expected)) => {
            actual.is_finite()
                && (0.0..=1.0).contains(&actual)
                && (actual - expected).abs() <= 1e-12
        }
        _ => false,
    };
    let (missing_reasons, invalid_reasons) = metric_reason_totals(&rejection_reasons)?;
    if !value_valid
        || reason_total(&rejection_reasons) != Some(coverage.metric_ineligible_battles)
        || coverage.missing_battles != missing_reasons
        || coverage.invalid_battles != invalid_reasons
    {
        return Err(AttemptsConversionDtoError::InvariantViolation);
    }
    Ok(AttemptsConversionRateDto {
        selected_battles,
        common_eligible_battles,
        metric_eligible_battles: coverage.metric_eligible_battles,
        metric_ineligible_battles: coverage.metric_ineligible_battles,
        contributing_battles: coverage.contributing_battles,
        zero_denominator_battles: coverage.zero_denominator_battles,
        missing_battles: coverage.missing_battles,
        invalid_battles: coverage.invalid_battles,
        numerator_sum: coverage.numerator_sum,
        denominator_sum: coverage.denominator_sum,
        value: coverage.value,
        rejection_reasons,
    })
}

fn fact_to_dto(
    selected_battles: usize,
    common_eligible_battles: usize,
    coverage: AttemptsConversionFactCoverage,
) -> Result<AttemptsConversionFactDto, AttemptsConversionDtoError> {
    let invariant = coverage.selected_battles != selected_battles
        || coverage.common_eligible_battles != common_eligible_battles
        || coverage
            .fact_eligible_battles
            .checked_add(coverage.fact_ineligible_battles)
            != Some(selected_battles)
        || coverage.contributing_battles != coverage.fact_eligible_battles
        || coverage.value_sum.is_some() != (coverage.contributing_battles > 0)
        || coverage.value_sum.is_some_and(|sum| sum < 0);
    if invariant {
        return Err(AttemptsConversionDtoError::InvariantViolation);
    }
    let rejection_reasons = reasons(
        &coverage.rejection_reason_counts,
        &AttemptsConversionRejectionReason::METRIC,
    );
    let (missing_reasons, invalid_reasons) = metric_reason_totals(&rejection_reasons)?;
    if reason_total(&rejection_reasons) != Some(coverage.fact_ineligible_battles)
        || coverage.missing_battles != missing_reasons
        || coverage.invalid_battles != invalid_reasons
    {
        return Err(AttemptsConversionDtoError::InvariantViolation);
    }
    Ok(AttemptsConversionFactDto {
        selected_battles,
        common_eligible_battles,
        fact_eligible_battles: coverage.fact_eligible_battles,
        fact_ineligible_battles: coverage.fact_ineligible_battles,
        contributing_battles: coverage.contributing_battles,
        missing_battles: coverage.missing_battles,
        invalid_battles: coverage.invalid_battles,
        value_sum: coverage.value_sum,
        rejection_reasons,
    })
}

pub fn report_to_dto(
    scope: AttemptsConversionScopeDto,
    report: AttemptsConversionReport,
) -> Result<AttemptsConversionReportDto, AttemptsConversionDtoError> {
    if report.contract_version != ATTEMPTS_CONVERSION_VERSION {
        return Err(AttemptsConversionDtoError::InvariantViolation);
    }
    let selected_battles = report.selected_battles;
    let common_context = common_to_dto(selected_battles, report.common_context)?;
    let hit_rate = rate_to_dto(
        selected_battles,
        common_context.eligible_battles,
        report.hit_rate,
    )?;
    let penetration_rate = rate_to_dto(
        selected_battles,
        common_context.eligible_battles,
        report.penetration_rate,
    )?;
    let facts = AttemptsConversionFactsDto {
        direct_team_hits: fact_to_dto(
            selected_battles,
            common_context.eligible_battles,
            report.facts.direct_team_hits,
        )?,
        explosion_hits: fact_to_dto(
            selected_battles,
            common_context.eligible_battles,
            report.facts.explosion_hits,
        )?,
        damage_dealt: fact_to_dto(
            selected_battles,
            common_context.eligible_battles,
            report.facts.damage_dealt,
        )?,
    };
    Ok(AttemptsConversionReportDto {
        contract_version: ATTEMPTS_CONVERSION_VERSION.to_owned(),
        scope,
        selected_battles,
        common_context,
        hit_rate,
        penetration_rate,
        facts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        attempts_conversion::{AttemptsConversionBattle, analyze_with_resolver},
        models::{
            CompactDescriptorProtocolSource, DataQuality, VehicleClass, VehicleClassResolution,
            VehicleClassResolutionState, VehicleClassSource, VehicleKeyCrossCheck,
        },
    };

    fn battle() -> AttemptsConversionBattle {
        AttemptsConversionBattle {
            arena_id: "a".into(),
            battle_time: 1,
            vehicle_key: "v".into(),
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
            direct_hits: None,
            direct_enemy_hits: Some(6),
            direct_team_hits: Some(0),
            piercings: None,
            piercing_enemy_hits: None,
            explosion_hits: Some(1),
            damage_dealt: Some(2_000),
        }
    }
    fn resolved() -> VehicleClassResolution {
        VehicleClassResolution {
            vehicle_id: 7,
            vehicle_class: VehicleClass::HeavyTank,
            state: VehicleClassResolutionState::Resolved,
            source: VehicleClassSource::WgClientPackedCatalog,
            catalog_schema_version: Some(1),
            catalog_source_version: Some("test".into()),
            catalog_fingerprint: None,
            protocol_source:
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            vehicle_key_cross_check: VehicleKeyCrossCheck::RawExact,
        }
    }
    fn dto() -> AttemptsConversionReportDto {
        report_to_dto(
            AttemptsConversionScopeDto::Recent { window: 10 },
            analyze_with_resolver(&[battle()], |_| resolved()).unwrap(),
        )
        .unwrap()
    }

    fn metric_counts(
        entries: &[(AttemptsConversionRejectionReason, usize)],
    ) -> std::collections::BTreeMap<AttemptsConversionRejectionReason, usize> {
        entries.iter().copied().collect()
    }

    fn rejected_rate(
        rejection_reason_counts: std::collections::BTreeMap<
            AttemptsConversionRejectionReason,
            usize,
        >,
        missing_battles: usize,
        invalid_battles: usize,
    ) -> AttemptsConversionRateCoverage {
        let selected_battles = rejection_reason_counts.values().sum();
        AttemptsConversionRateCoverage {
            selected_battles,
            common_eligible_battles: selected_battles,
            metric_eligible_battles: 0,
            metric_ineligible_battles: selected_battles,
            contributing_battles: 0,
            zero_denominator_battles: 0,
            missing_battles,
            invalid_battles,
            numerator_sum: 0,
            denominator_sum: 0,
            value: None,
            rejection_reason_counts,
        }
    }

    fn rejected_fact(
        rejection_reason_counts: std::collections::BTreeMap<
            AttemptsConversionRejectionReason,
            usize,
        >,
        missing_battles: usize,
        invalid_battles: usize,
    ) -> AttemptsConversionFactCoverage {
        let selected_battles = rejection_reason_counts.values().sum();
        AttemptsConversionFactCoverage {
            selected_battles,
            common_eligible_battles: selected_battles,
            fact_eligible_battles: 0,
            fact_ineligible_battles: selected_battles,
            contributing_battles: 0,
            missing_battles,
            invalid_battles,
            value_sum: None,
            rejection_reason_counts,
        }
    }

    #[test]
    fn maps_exact_v2_shape_and_reason_catalogs() {
        let value = serde_json::to_value(dto()).unwrap();
        assert_eq!(value["contractVersion"], "attempts-conversion-v2");
        assert!(value.get("rawSums").is_none());
        assert_eq!(
            value["commonContext"]["rejectionReasons"]
                .as_array()
                .unwrap()
                .len(),
            10
        );
        assert_eq!(
            value["hitRate"]["rejectionReasons"]
                .as_array()
                .unwrap()
                .len(),
            13
        );
        assert_eq!(value["facts"]["damageDealt"]["valueSum"], 2_000);
    }

    #[test]
    fn serializes_public_session_and_recent_scope_fields_exactly() {
        let engine = analyze_with_resolver(&[battle()], |_| resolved()).unwrap();
        let session = report_to_dto(
            AttemptsConversionScopeDto::Session {
                session_id: "session-wire".to_owned(),
            },
            engine.clone(),
        )
        .unwrap();
        let session_json = serde_json::to_value(session).unwrap();
        assert_eq!(session_json["contractVersion"], "attempts-conversion-v2");
        assert_eq!(session_json["scope"]["type"], "session");
        assert_eq!(session_json["scope"]["sessionId"], "session-wire");
        assert!(session_json["scope"].get("session_id").is_none());

        let recent =
            report_to_dto(AttemptsConversionScopeDto::Recent { window: 30 }, engine).unwrap();
        let recent_json = serde_json::to_value(recent).unwrap();
        assert_eq!(recent_json["contractVersion"], "attempts-conversion-v2");
        assert_eq!(recent_json["scope"]["type"], "recent");
        assert_eq!(recent_json["scope"]["window"], 30);
        assert!(recent_json["scope"].get("sessionId").is_none());
        assert!(recent_json["scope"].get("session_id").is_none());
    }

    #[test]
    fn rejects_malformed_report_invariants() {
        let mut report = dto();
        report.hit_rate.numerator_sum = 11;
        assert!(serde_json::to_value(report).is_ok());
        let raw = analyze_with_resolver(&[battle()], |_| resolved()).unwrap();
        let mut invalid = raw.clone();
        invalid.hit_rate.value = Some(0.2);
        assert_eq!(
            report_to_dto(AttemptsConversionScopeDto::Recent { window: 10 }, invalid),
            Err(AttemptsConversionDtoError::InvariantViolation)
        );
        let mut wrong_contract = raw;
        wrong_contract.contract_version = "attempts-conversion-v1";
        assert_eq!(
            report_to_dto(
                AttemptsConversionScopeDto::Recent { window: 10 },
                wrong_contract
            ),
            Err(AttemptsConversionDtoError::InvariantViolation)
        );
    }

    #[test]
    fn rate_dto_requires_exact_missing_and_invalid_reason_counts() {
        let valid = rejected_rate(
            metric_counts(&[
                (AttemptsConversionRejectionReason::MissingCounters, 1),
                (AttemptsConversionRejectionReason::CounterOutOfRange, 1),
                (AttemptsConversionRejectionReason::CounterConflict, 2),
            ]),
            1,
            3,
        );
        assert!(rate_to_dto(4, 4, valid).is_ok());

        let cases = [
            (
                metric_counts(&[(AttemptsConversionRejectionReason::MissingCounters, 1)]),
                0,
                0,
            ),
            (
                metric_counts(&[(AttemptsConversionRejectionReason::PartialReplay, 1)]),
                1,
                0,
            ),
            (
                metric_counts(&[(AttemptsConversionRejectionReason::CounterConflict, 1)]),
                0,
                0,
            ),
            (
                metric_counts(&[(AttemptsConversionRejectionReason::CounterOutOfRange, 1)]),
                0,
                0,
            ),
            (
                metric_counts(&[(AttemptsConversionRejectionReason::PartialReplay, 1)]),
                0,
                1,
            ),
        ];
        for (counts, missing_battles, invalid_battles) in cases {
            let selected_battles = counts.values().sum();
            assert_eq!(
                rate_to_dto(
                    selected_battles,
                    selected_battles,
                    rejected_rate(counts, missing_battles, invalid_battles),
                ),
                Err(AttemptsConversionDtoError::InvariantViolation)
            );
        }

        let combined_invalid = rejected_rate(
            metric_counts(&[
                (AttemptsConversionRejectionReason::CounterOutOfRange, 2),
                (AttemptsConversionRejectionReason::CounterConflict, 3),
            ]),
            0,
            5,
        );
        assert!(rate_to_dto(5, 5, combined_invalid).is_ok());
    }

    #[test]
    fn fact_dto_requires_exact_missing_and_invalid_reason_counts() {
        let valid = rejected_fact(
            metric_counts(&[
                (AttemptsConversionRejectionReason::MissingCounters, 1),
                (AttemptsConversionRejectionReason::CounterOutOfRange, 1),
                (AttemptsConversionRejectionReason::CounterConflict, 1),
            ]),
            1,
            2,
        );
        assert!(fact_to_dto(3, 3, valid).is_ok());

        let cases = [
            (
                metric_counts(&[(AttemptsConversionRejectionReason::MissingCounters, 1)]),
                0,
                0,
            ),
            (
                metric_counts(&[(AttemptsConversionRejectionReason::PartialReplay, 1)]),
                1,
                0,
            ),
            (
                metric_counts(&[(AttemptsConversionRejectionReason::CounterOutOfRange, 1)]),
                0,
                0,
            ),
            (
                metric_counts(&[(AttemptsConversionRejectionReason::CounterConflict, 1)]),
                0,
                0,
            ),
            (
                metric_counts(&[(AttemptsConversionRejectionReason::PartialReplay, 1)]),
                0,
                1,
            ),
        ];
        for (counts, missing_battles, invalid_battles) in cases {
            let selected_battles = counts.values().sum();
            assert_eq!(
                fact_to_dto(
                    selected_battles,
                    selected_battles,
                    rejected_fact(counts, missing_battles, invalid_battles),
                ),
                Err(AttemptsConversionDtoError::InvariantViolation)
            );
        }
    }

    #[test]
    fn report_mapper_applies_fact_reason_invariants_to_every_fact() {
        for fact_selector in 0..3 {
            let mut report = analyze_with_resolver(&[battle()], |_| resolved()).unwrap();
            let fact = match fact_selector {
                0 => &mut report.facts.direct_team_hits,
                1 => &mut report.facts.explosion_hits,
                2 => &mut report.facts.damage_dealt,
                _ => unreachable!(),
            };
            fact.fact_eligible_battles = 0;
            fact.fact_ineligible_battles = 1;
            fact.contributing_battles = 0;
            fact.missing_battles = 0;
            fact.value_sum = None;
            fact.rejection_reason_counts =
                metric_counts(&[(AttemptsConversionRejectionReason::MissingCounters, 1)]);
            assert_eq!(
                report_to_dto(AttemptsConversionScopeDto::Recent { window: 10 }, report),
                Err(AttemptsConversionDtoError::InvariantViolation)
            );
        }
    }

    #[test]
    fn metric_reason_totals_reject_missing_catalog_entries_and_overflow() {
        let missing_conflict = vec![AttemptsConversionRejectionReasonDto {
            code: AttemptsConversionRejectionReason::MissingCounters
                .as_str()
                .to_owned(),
            count: 0,
        }];
        assert_eq!(
            metric_reason_totals(&missing_conflict),
            Err(AttemptsConversionDtoError::InvariantViolation)
        );

        let overflow = vec![
            AttemptsConversionRejectionReasonDto {
                code: AttemptsConversionRejectionReason::MissingCounters
                    .as_str()
                    .to_owned(),
                count: 0,
            },
            AttemptsConversionRejectionReasonDto {
                code: AttemptsConversionRejectionReason::CounterOutOfRange
                    .as_str()
                    .to_owned(),
                count: usize::MAX,
            },
            AttemptsConversionRejectionReasonDto {
                code: AttemptsConversionRejectionReason::CounterConflict
                    .as_str()
                    .to_owned(),
                count: 1,
            },
        ];
        assert_eq!(
            metric_reason_totals(&overflow),
            Err(AttemptsConversionDtoError::InvariantViolation)
        );
    }
}
