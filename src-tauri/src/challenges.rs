use crate::models::{
    BaselineConfidence, Battle, ChallengeContributionMetricKind, ChallengeEvidenceMetric,
    ChallengePresentation, ChallengeSuggestion, ChallengeSuggestionResponse,
    ChallengeSuggestionUnavailableReason, ChallengeType, DataQuality, SessionBaselineComparison,
    SessionInsightDirection, SessionInsightMetric, SessionInsightState, VehicleClass,
    VehicleClassResolution, VehicleClassResolutionState, VehicleSessionBaselineComparison,
    stats_eligibility, supports_core_feature_schema,
};
use crate::session_insights::SESSION_INSIGHT_VERSION;
use crate::vehicle_class_catalog::VEHICLE_CLASS_RESOLVER_VERSION;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const CHALLENGE_ENGINE_VERSION: &str = "vehicle-challenge-v1";
pub const CHALLENGE_SCHEMA_VERSION: u32 = 1;
pub const CHALLENGE_SUGGESTION_VERSION: &str = "vehicle-challenge-suggestion-v1";
const TARGET_BATTLES: usize = 5;
const CONSISTENCY_SUCCESSES: usize = 4;
const MIN_BASELINE_BATTLES: usize = 30;
const MIN_SESSION_BATTLES: usize = 3;
const ASSIST_SIGNAL_PERCENT: f64 = 15.0;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct MetricSample {
    pub value: Option<f64>,
    pub eligible: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ChallengeVehicleInput {
    pub comparison: VehicleSessionBaselineComparison,
    pub resolution: VehicleClassResolution,
    pub baseline_damage: MetricSample,
    pub baseline_survival: MetricSample,
    pub baseline_spotting: MetricSample,
    pub baseline_tracking: MetricSample,
    pub baseline_spotting_tracking: MetricSample,
    pub baseline_stun: MetricSample,
    pub baseline_combined: MetricSample,
    pub session_damage: MetricSample,
    pub session_survival: MetricSample,
    pub session_spotting_tracking: MetricSample,
    pub session_stun: MetricSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChallengeNotEligibleReason {
    WrongVehicle,
    BeforeEligibleAfter,
    NotFull,
    UnsupportedSchema,
    NotRandomBattle,
    MissingRequiredMetric,
    ChallengeNotActive,
    AlreadyCounted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ChallengeBattleEvaluation {
    NotEligible(ChallengeNotEligibleReason),
    Eligible {
        metric_value: i64,
        contribution_value: Option<i64>,
        survived: Option<bool>,
        success: bool,
    },
}

fn is_useful(confidence: BaselineConfidence) -> bool {
    matches!(
        confidence,
        BaselineConfidence::Useful | BaselineConfidence::Reliable | BaselineConfidence::Stable
    )
}

fn rounded_floor(value: f64, factor: f64) -> Option<i64> {
    if !value.is_finite() || value <= 0.0 || !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let rounded = (value * factor + 0.5).floor();
    (rounded.is_finite() && rounded <= i64::MAX as f64).then_some((rounded as i64).max(1))
}

fn sample(
    battles: &[Battle],
    vehicle_id: i64,
    limit: usize,
    value: impl Fn(&Battle) -> Option<f64>,
) -> MetricSample {
    let values = battles
        .iter()
        .filter(|battle| battle.wg_tank_id == Some(vehicle_id) && stats_eligibility(battle))
        .filter_map(value)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .take(limit)
        .collect::<Vec<_>>();
    MetricSample {
        value: (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
        eligible: values.len(),
    }
}

fn checked_sum(values: &[Option<i64>]) -> Option<f64> {
    values
        .iter()
        .try_fold(0_i64, |total, value| total.checked_add((*value)?))
        .map(|value| value as f64)
}

pub(crate) fn build_vehicle_inputs(
    comparison: &SessionBaselineComparison,
    session_battles: &[Battle],
    baseline_battles: &[Battle],
    resolutions: &BTreeMap<i64, VehicleClassResolution>,
) -> Vec<ChallengeVehicleInput> {
    comparison
        .vehicles
        .iter()
        .filter_map(|vehicle| {
            let id = vehicle.vehicle_id;
            let resolution = resolutions.get(&id)?.clone();
            Some(ChallengeVehicleInput {
                comparison: vehicle.clone(),
                resolution,
                baseline_damage: sample(baseline_battles, id, 100, |battle| {
                    battle.damage.map(|value| value as f64)
                }),
                baseline_survival: sample(baseline_battles, id, 100, |battle| {
                    battle.survived.map(|value| if value { 100.0 } else { 0.0 })
                }),
                baseline_spotting: sample(baseline_battles, id, 100, |battle| {
                    battle.assist_spotting.map(|value| value as f64)
                }),
                baseline_tracking: sample(baseline_battles, id, 100, |battle| {
                    battle.assist_tracking.map(|value| value as f64)
                }),
                baseline_spotting_tracking: sample(baseline_battles, id, 100, |battle| {
                    checked_sum(&[battle.assist_spotting, battle.assist_tracking])
                }),
                baseline_stun: sample(baseline_battles, id, 100, |battle| {
                    battle.assist_stun.map(|value| value as f64)
                }),
                baseline_combined: sample(baseline_battles, id, 100, |battle| {
                    checked_sum(&[
                        battle.damage,
                        battle.assist_spotting,
                        battle.assist_tracking,
                    ])
                }),
                session_damage: sample(session_battles, id, usize::MAX, |battle| {
                    battle.damage.map(|value| value as f64)
                }),
                session_survival: sample(session_battles, id, usize::MAX, |battle| {
                    battle.survived.map(|value| if value { 100.0 } else { 0.0 })
                }),
                session_spotting_tracking: sample(session_battles, id, usize::MAX, |battle| {
                    checked_sum(&[battle.assist_spotting, battle.assist_tracking])
                }),
                session_stun: sample(session_battles, id, usize::MAX, |battle| {
                    battle.assist_stun.map(|value| value as f64)
                }),
            })
        })
        .collect()
}

fn insight_signal(
    comparison: &SessionBaselineComparison,
    vehicle_id: i64,
    metric: SessionInsightMetric,
) -> Option<(f64, f64)> {
    let summary = comparison
        .insights
        .summaries
        .iter()
        .find(|summary| summary.vehicle_id == Some(vehicle_id))?;
    if summary.state != SessionInsightState::Available {
        return None;
    }
    summary
        .supporting_signals
        .iter()
        .find(|signal| {
            signal.metric == metric && signal.direction == SessionInsightDirection::Negative
        })
        .map(|signal| (signal.delta_value, signal.normalized_strength))
}

fn raw_negative(current: MetricSample, baseline: MetricSample) -> Option<(f64, f64)> {
    if current.eligible < MIN_SESSION_BATTLES || baseline.eligible < MIN_BASELINE_BATTLES {
        return None;
    }
    let (Some(current), Some(baseline)) = (current.value, baseline.value) else {
        return None;
    };
    if !current.is_finite() || !baseline.is_finite() || current < 0.0 || baseline <= 0.0 {
        return None;
    }
    let delta = (current - baseline) / baseline * 100.0;
    (delta <= -ASSIST_SIGNAL_PERCENT)
        .then_some((delta, (delta.abs() / ASSIST_SIGNAL_PERCENT).min(3.0)))
}

pub(crate) fn allowed_challenge(vehicle_class: VehicleClass, kind: ChallengeType) -> bool {
    match vehicle_class {
        VehicleClass::LightTank => matches!(
            kind,
            ChallengeType::SpottingAssistConsistency
                | ChallengeType::EffectiveSurvival
                | ChallengeType::DamageConsistency
        ),
        VehicleClass::MediumTank => matches!(
            kind,
            ChallengeType::DamageConsistency
                | ChallengeType::EffectiveSurvival
                | ChallengeType::SpottingAssistConsistency
        ),
        VehicleClass::HeavyTank | VehicleClass::TankDestroyer => matches!(
            kind,
            ChallengeType::DamageConsistency | ChallengeType::EffectiveSurvival
        ),
        VehicleClass::Spg => matches!(
            kind,
            ChallengeType::StunAssistConsistency | ChallengeType::DamageConsistency
        ),
        VehicleClass::Unknown => false,
    }
}

pub(crate) fn valid_suggestion_contract(suggestion: &ChallengeSuggestion) -> bool {
    if !allowed_challenge(suggestion.vehicle_class, suggestion.challenge_type) {
        return false;
    }
    match suggestion.challenge_type {
        ChallengeType::DamageConsistency => {
            suggestion.required_successes == CONSISTENCY_SUCCESSES
                && suggestion.threshold_value.is_some_and(|value| value >= 1)
                && suggestion.contribution_floor.is_none()
                && suggestion.contribution_metric_kind == ChallengeContributionMetricKind::Damage
        }
        ChallengeType::SpottingAssistConsistency => {
            suggestion.required_successes == CONSISTENCY_SUCCESSES
                && suggestion.threshold_value.is_some_and(|value| value >= 1)
                && suggestion.contribution_floor.is_none()
                && suggestion.contribution_metric_kind
                    == ChallengeContributionMetricKind::SpottingTrackingAssist
        }
        ChallengeType::StunAssistConsistency => {
            suggestion.required_successes == CONSISTENCY_SUCCESSES
                && suggestion.threshold_value.is_some_and(|value| value >= 1)
                && suggestion.contribution_floor.is_none()
                && suggestion.contribution_metric_kind
                    == ChallengeContributionMetricKind::StunAssist
        }
        ChallengeType::EffectiveSurvival => {
            suggestion.threshold_value.is_none()
                && suggestion
                    .contribution_floor
                    .is_some_and(|value| value >= 1)
                && suggestion.contribution_metric_kind
                    == if suggestion.vehicle_class == VehicleClass::LightTank {
                        ChallengeContributionMetricKind::CombinedContribution
                    } else {
                        ChallengeContributionMetricKind::Damage
                    }
        }
    }
}

fn class_priority(vehicle_class: VehicleClass, kind: ChallengeType) -> u8 {
    match (vehicle_class, kind) {
        (VehicleClass::LightTank, ChallengeType::SpottingAssistConsistency)
        | (VehicleClass::MediumTank, ChallengeType::DamageConsistency)
        | (VehicleClass::HeavyTank, ChallengeType::DamageConsistency)
        | (VehicleClass::TankDestroyer, ChallengeType::DamageConsistency)
        | (VehicleClass::Spg, ChallengeType::StunAssistConsistency) => 0,
        (VehicleClass::LightTank, ChallengeType::EffectiveSurvival)
        | (VehicleClass::MediumTank, ChallengeType::EffectiveSurvival)
        | (VehicleClass::HeavyTank, ChallengeType::EffectiveSurvival)
        | (VehicleClass::TankDestroyer, ChallengeType::EffectiveSurvival)
        | (VehicleClass::Spg, ChallengeType::DamageConsistency) => 1,
        _ => 2,
    }
}

fn stable_type_order(kind: ChallengeType) -> u8 {
    match kind {
        ChallengeType::DamageConsistency => 0,
        ChallengeType::EffectiveSurvival => 1,
        ChallengeType::SpottingAssistConsistency => 2,
        ChallengeType::StunAssistConsistency => 3,
    }
}

fn suggestion_id(suggestion: &ChallengeSuggestion, session_id: &str) -> String {
    let canonical = format!(
        "{}|{}|{}|{:?}|{:?}|{}|{}|{:?}|{:?}|{}|{:?}|{}|{}|{}|{}|{}|{}|{}",
        suggestion.suggestion_version,
        session_id,
        suggestion.vehicle_id,
        suggestion.vehicle_class,
        suggestion.challenge_type,
        suggestion.target_battles,
        suggestion.required_successes,
        suggestion.threshold_value,
        suggestion.contribution_floor,
        suggestion.baseline_cutoff,
        suggestion.baseline_window,
        suggestion.baseline_version,
        suggestion.insight_version,
        suggestion.vehicle_class_resolver_version,
        suggestion.vehicle_class_catalog_schema_version,
        suggestion.expected_snapshot_version,
        suggestion.expected_snapshot_sha256,
        suggestion.frozen_baseline_value.to_bits(),
    );
    Sha256::digest(canonical.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct SuggestionParameters {
    challenge_type: ChallengeType,
    evidence_metric: ChallengeEvidenceMetric,
    evidence_delta: f64,
    evidence_strength: f64,
    baseline_value: f64,
    baseline_metric_eligible: usize,
    threshold_value: Option<i64>,
    contribution_kind: ChallengeContributionMetricKind,
    contribution_floor: Option<i64>,
    required_successes: usize,
}

fn base_suggestion(
    comparison: &SessionBaselineComparison,
    input: &ChallengeVehicleInput,
    parameters: SuggestionParameters,
) -> Option<ChallengeSuggestion> {
    let SuggestionParameters {
        challenge_type,
        evidence_metric,
        evidence_delta,
        evidence_strength,
        baseline_value,
        baseline_metric_eligible,
        threshold_value,
        contribution_kind,
        contribution_floor,
        required_successes,
    } = parameters;
    let baseline = &input.comparison.baseline;
    let expected_version = baseline.snapshot_source_version.clone()?;
    let expected_sha = baseline.snapshot_sha256.clone()?;
    let catalog_schema = input.resolution.catalog_schema_version?;
    if !baseline_value.is_finite()
        || baseline_value <= 0.0
        || !evidence_delta.is_finite()
        || !evidence_strength.is_finite()
    {
        return None;
    }
    let mut suggestion = ChallengeSuggestion {
        suggestion_id: String::new(),
        challenge_type,
        vehicle_id: input.comparison.vehicle_id,
        vehicle_name: input
            .comparison
            .tank_name
            .clone()
            .or_else(|| input.comparison.vehicle_key.clone())
            .unwrap_or_else(|| "Unknown vehicle".to_owned()),
        vehicle_class: input.resolution.vehicle_class,
        target_battles: TARGET_BATTLES,
        required_successes,
        frozen_baseline_value: baseline_value,
        threshold_value,
        contribution_metric_kind: contribution_kind,
        contribution_floor,
        evidence_metric,
        evidence_delta,
        evidence_normalized_strength: evidence_strength,
        baseline_confidence: baseline.confidence,
        baseline_eligible_battles: baseline.eligible_battles,
        baseline_metric_eligible_battles: baseline_metric_eligible,
        baseline_version: baseline.baseline_version.clone(),
        baseline_cutoff: comparison.cutoff,
        baseline_window: baseline.window,
        insight_version: SESSION_INSIGHT_VERSION.to_owned(),
        challenge_engine_version: CHALLENGE_ENGINE_VERSION.to_owned(),
        suggestion_version: CHALLENGE_SUGGESTION_VERSION.to_owned(),
        formula_version: baseline.formula_version.clone(),
        expected_snapshot_version: expected_version,
        expected_snapshot_sha256: expected_sha,
        vehicle_class_source: input.resolution.source,
        vehicle_class_resolution_state: input.resolution.state,
        vehicle_class_resolver_version: VEHICLE_CLASS_RESOLVER_VERSION.to_owned(),
        vehicle_class_catalog_schema_version: catalog_schema,
        compact_descriptor_protocol_source: input.resolution.protocol_source,
        frozen_damage_baseline: input.baseline_damage.value,
        frozen_survival_baseline: input.baseline_survival.value,
        frozen_combined_contribution_baseline: input.baseline_combined.value,
        frozen_spotting_assist_baseline: input.baseline_spotting.value,
        frozen_tracking_assist_baseline: input.baseline_tracking.value,
        frozen_stun_assist_baseline: input.baseline_stun.value,
    };
    suggestion.suggestion_id = suggestion_id(&suggestion, &comparison.session_id);
    Some(suggestion)
}

fn vehicle_suggestions(
    comparison: &SessionBaselineComparison,
    input: &ChallengeVehicleInput,
) -> Vec<ChallengeSuggestion> {
    if input.resolution.state != VehicleClassResolutionState::Resolved
        || input.resolution.vehicle_class == VehicleClass::Unknown
        || input.comparison.baseline.eligible_battles < MIN_BASELINE_BATTLES
        || !is_useful(input.comparison.baseline.confidence)
        || input.comparison.session_eligible_battles < MIN_SESSION_BATTLES
    {
        return Vec::new();
    }
    let class = input.resolution.vehicle_class;
    let mut candidates = Vec::new();
    if allowed_challenge(class, ChallengeType::DamageConsistency)
        && input.baseline_damage.eligible >= MIN_BASELINE_BATTLES
        && input.session_damage.eligible >= MIN_SESSION_BATTLES
        && let Some((delta, strength)) = insight_signal(
            comparison,
            input.comparison.vehicle_id,
            SessionInsightMetric::Damage,
        )
        && let Some(value) = input.baseline_damage.value
        && let Some(threshold) = rounded_floor(value, 0.70)
        && let Some(suggestion) = base_suggestion(
            comparison,
            input,
            SuggestionParameters {
                challenge_type: ChallengeType::DamageConsistency,
                evidence_metric: ChallengeEvidenceMetric::Damage,
                evidence_delta: delta,
                evidence_strength: strength,
                baseline_value: value,
                baseline_metric_eligible: input.baseline_damage.eligible,
                threshold_value: Some(threshold.min(value.ceil() as i64)),
                contribution_kind: ChallengeContributionMetricKind::Damage,
                contribution_floor: None,
                required_successes: CONSISTENCY_SUCCESSES,
            },
        )
    {
        candidates.push(suggestion);
    }
    if allowed_challenge(class, ChallengeType::EffectiveSurvival)
        && input.baseline_survival.eligible >= MIN_BASELINE_BATTLES
        && input.session_survival.eligible >= MIN_SESSION_BATTLES
        && let Some((delta, strength)) = insight_signal(
            comparison,
            input.comparison.vehicle_id,
            SessionInsightMetric::Survival,
        )
        && let Some(survival_percent) = input.baseline_survival.value
    {
        let contribution = if class == VehicleClass::LightTank {
            input.baseline_combined
        } else {
            input.baseline_damage
        };
        let kind = if class == VehicleClass::LightTank {
            ChallengeContributionMetricKind::CombinedContribution
        } else {
            ChallengeContributionMetricKind::Damage
        };
        if contribution.eligible >= MIN_BASELINE_BATTLES
            && let Some(contribution_value) = contribution.value
            && let Some(floor) = rounded_floor(contribution_value, 0.50)
        {
            let rate = (survival_percent / 100.0 + 0.10).clamp(0.40, 0.80);
            let successes = ((TARGET_BATTLES as f64 * rate).ceil() as usize).clamp(2, 4);
            if let Some(suggestion) = base_suggestion(
                comparison,
                input,
                SuggestionParameters {
                    challenge_type: ChallengeType::EffectiveSurvival,
                    evidence_metric: ChallengeEvidenceMetric::Survival,
                    evidence_delta: delta,
                    evidence_strength: strength,
                    baseline_value: survival_percent,
                    baseline_metric_eligible: input
                        .baseline_survival
                        .eligible
                        .min(contribution.eligible),
                    threshold_value: None,
                    contribution_kind: kind,
                    contribution_floor: Some(floor),
                    required_successes: successes,
                },
            ) {
                candidates.push(suggestion);
            }
        }
    }
    if allowed_challenge(class, ChallengeType::SpottingAssistConsistency)
        && input.baseline_spotting_tracking.value.is_some_and(|value| {
            value
                >= if class == VehicleClass::MediumTank {
                    250.0
                } else {
                    100.0
                }
        })
        && insight_signal(
            comparison,
            input.comparison.vehicle_id,
            SessionInsightMetric::Assist,
        )
        .is_some()
        && let Some((delta, strength)) = raw_negative(
            input.session_spotting_tracking,
            input.baseline_spotting_tracking,
        )
        && let Some(value) = input.baseline_spotting_tracking.value
        && let Some(threshold) = rounded_floor(value, 0.65)
        && let Some(suggestion) = base_suggestion(
            comparison,
            input,
            SuggestionParameters {
                challenge_type: ChallengeType::SpottingAssistConsistency,
                evidence_metric: ChallengeEvidenceMetric::SpottingTrackingAssist,
                evidence_delta: delta,
                evidence_strength: strength,
                baseline_value: value,
                baseline_metric_eligible: input.baseline_spotting_tracking.eligible,
                threshold_value: Some(threshold),
                contribution_kind: ChallengeContributionMetricKind::SpottingTrackingAssist,
                contribution_floor: None,
                required_successes: CONSISTENCY_SUCCESSES,
            },
        )
    {
        candidates.push(suggestion);
    }
    if allowed_challenge(class, ChallengeType::StunAssistConsistency)
        && input
            .baseline_stun
            .value
            .is_some_and(|value| value >= 100.0)
        && insight_signal(
            comparison,
            input.comparison.vehicle_id,
            SessionInsightMetric::Assist,
        )
        .is_some()
        && let Some((delta, strength)) = raw_negative(input.session_stun, input.baseline_stun)
        && let Some(value) = input.baseline_stun.value
        && let Some(threshold) = rounded_floor(value, 0.65)
        && let Some(suggestion) = base_suggestion(
            comparison,
            input,
            SuggestionParameters {
                challenge_type: ChallengeType::StunAssistConsistency,
                evidence_metric: ChallengeEvidenceMetric::StunAssist,
                evidence_delta: delta,
                evidence_strength: strength,
                baseline_value: value,
                baseline_metric_eligible: input.baseline_stun.eligible,
                threshold_value: Some(threshold),
                contribution_kind: ChallengeContributionMetricKind::StunAssist,
                contribution_floor: None,
                required_successes: CONSISTENCY_SUCCESSES,
            },
        )
    {
        candidates.push(suggestion);
    }
    candidates
}

pub(crate) fn generate_suggestions(
    comparison: &SessionBaselineComparison,
    inputs: &[ChallengeVehicleInput],
    active_exists: bool,
) -> ChallengeSuggestionResponse {
    if active_exists {
        return ChallengeSuggestionResponse {
            suggestions: Vec::new(),
            unavailable_reason: Some(ChallengeSuggestionUnavailableReason::ActiveChallengeExists),
        };
    }
    let resolved = inputs
        .iter()
        .any(|input| input.resolution.state == VehicleClassResolutionState::Resolved);
    let sufficiently_sampled = inputs.iter().any(|input| {
        input.comparison.baseline.eligible_battles >= MIN_BASELINE_BATTLES
            && input.comparison.session_eligible_battles >= MIN_SESSION_BATTLES
            && is_useful(input.comparison.baseline.confidence)
    });
    let mut suggestions = inputs
        .iter()
        .flat_map(|input| vehicle_suggestions(comparison, input))
        .collect::<Vec<_>>();
    suggestions.sort_by(|left, right| {
        right
            .evidence_normalized_strength
            .total_cmp(&left.evidence_normalized_strength)
            .then_with(|| {
                class_priority(left.vehicle_class, left.challenge_type)
                    .cmp(&class_priority(right.vehicle_class, right.challenge_type))
            })
            .then_with(|| {
                stable_type_order(left.challenge_type).cmp(&stable_type_order(right.challenge_type))
            })
            .then_with(|| left.vehicle_id.cmp(&right.vehicle_id))
    });
    let mut seen = BTreeSet::new();
    suggestions
        .retain(|suggestion| seen.insert((suggestion.vehicle_id, suggestion.challenge_type)));
    suggestions.truncate(2);
    let unavailable_reason = suggestions.is_empty().then_some(if !resolved {
        ChallengeSuggestionUnavailableReason::VehicleClassUnavailable
    } else if !sufficiently_sampled {
        ChallengeSuggestionUnavailableReason::InsufficientData
    } else {
        ChallengeSuggestionUnavailableReason::NoSupportedNegativeSignal
    });
    ChallengeSuggestionResponse {
        suggestions,
        unavailable_reason,
    }
}

pub(crate) fn evaluate_battle_for_challenge(
    challenge: &ChallengePresentation,
    battle: &Battle,
) -> ChallengeBattleEvaluation {
    if challenge.status != crate::models::ChallengeStatus::Active {
        return ChallengeBattleEvaluation::NotEligible(
            ChallengeNotEligibleReason::ChallengeNotActive,
        );
    }
    if battle.wg_tank_id != Some(challenge.vehicle_id) {
        return ChallengeBattleEvaluation::NotEligible(ChallengeNotEligibleReason::WrongVehicle);
    }
    if battle.battle_time < challenge.eligible_after
        || battle.battle_time_source.as_deref() != Some("metadata")
    {
        return ChallengeBattleEvaluation::NotEligible(
            ChallengeNotEligibleReason::BeforeEligibleAfter,
        );
    }
    if battle.data_quality != DataQuality::Full {
        return ChallengeBattleEvaluation::NotEligible(ChallengeNotEligibleReason::NotFull);
    }
    if !supports_core_feature_schema(battle.feature_schema_version) {
        return ChallengeBattleEvaluation::NotEligible(
            ChallengeNotEligibleReason::UnsupportedSchema,
        );
    }
    if !stats_eligibility(battle) {
        return ChallengeBattleEvaluation::NotEligible(ChallengeNotEligibleReason::NotRandomBattle);
    }
    match challenge.challenge_type {
        ChallengeType::DamageConsistency => {
            let (Some(value), Some(threshold)) = (battle.damage, challenge.threshold_value) else {
                return ChallengeBattleEvaluation::NotEligible(
                    ChallengeNotEligibleReason::MissingRequiredMetric,
                );
            };
            ChallengeBattleEvaluation::Eligible {
                metric_value: value,
                contribution_value: None,
                survived: battle.survived,
                success: value >= threshold,
            }
        }
        ChallengeType::SpottingAssistConsistency => {
            let Some(value) = battle.assist_spotting.and_then(|spotting| {
                battle
                    .assist_tracking
                    .and_then(|tracking| spotting.checked_add(tracking))
            }) else {
                return ChallengeBattleEvaluation::NotEligible(
                    ChallengeNotEligibleReason::MissingRequiredMetric,
                );
            };
            let Some(threshold) = challenge.threshold_value else {
                return ChallengeBattleEvaluation::NotEligible(
                    ChallengeNotEligibleReason::MissingRequiredMetric,
                );
            };
            ChallengeBattleEvaluation::Eligible {
                metric_value: value,
                contribution_value: None,
                survived: battle.survived,
                success: value >= threshold,
            }
        }
        ChallengeType::StunAssistConsistency => {
            let (Some(value), Some(threshold)) = (battle.assist_stun, challenge.threshold_value)
            else {
                return ChallengeBattleEvaluation::NotEligible(
                    ChallengeNotEligibleReason::MissingRequiredMetric,
                );
            };
            ChallengeBattleEvaluation::Eligible {
                metric_value: value,
                contribution_value: None,
                survived: battle.survived,
                success: value >= threshold,
            }
        }
        ChallengeType::EffectiveSurvival => {
            let Some(survived) = battle.survived else {
                return ChallengeBattleEvaluation::NotEligible(
                    ChallengeNotEligibleReason::MissingRequiredMetric,
                );
            };
            let contribution = match challenge.contribution_metric_kind {
                ChallengeContributionMetricKind::Damage => battle.damage,
                ChallengeContributionMetricKind::CombinedContribution => {
                    battle.damage.and_then(|damage| {
                        battle.assist_spotting.and_then(|spotting| {
                            battle.assist_tracking.and_then(|tracking| {
                                damage.checked_add(spotting)?.checked_add(tracking)
                            })
                        })
                    })
                }
                ChallengeContributionMetricKind::SpottingTrackingAssist
                | ChallengeContributionMetricKind::StunAssist => None,
            };
            let (Some(contribution), Some(floor)) = (contribution, challenge.contribution_floor)
            else {
                return ChallengeBattleEvaluation::NotEligible(
                    ChallengeNotEligibleReason::MissingRequiredMetric,
                );
            };
            ChallengeBattleEvaluation::Eligible {
                metric_value: i64::from(survived),
                contribution_value: Some(contribution),
                survived: Some(survived),
                success: survived && contribution >= floor,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        BaselineDelta, BaselineMetric, BaselineScope, BaselineWindowKind, BaselineWindowResult,
        BattleOutcome, CompactDescriptorProtocolSource, NormalizedBaselineMetrics, SessionInsight,
        SessionInsightDeltaUnit, SessionInsightEvidenceTextKey, SessionInsightPresentation,
        SessionInsightSummary, VehicleClassSource, VehicleKeyCrossCheck, Wn8ExclusionCounts,
        Wn8PresentationState,
    };

    #[test]
    fn rounded_floor_refuses_each_bad_input_and_values_beyond_i64() {
        assert_eq!(rounded_floor(10.0, 0.7), Some(7));
        assert_eq!(rounded_floor(0.1, 0.5), Some(1), "at least 1");
        for (value, factor) in [
            (f64::NAN, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (5.0, f64::NAN),
            (5.0, 0.0),
            (5.0, -1.0),
            (1e300, 1.0),
        ] {
            assert_eq!(rounded_floor(value, factor), None, "{value} × {factor}");
        }
    }

    #[test]
    fn sample_averages_finite_non_negative_values_of_that_tank() {
        let make = |id: &str, damage: Option<i64>, tank: i64| {
            let mut battle = battle_for_schema(id, 1, Some(3));
            battle.damage = damage;
            battle.wg_tank_id = Some(tank);
            battle
        };
        let battles = [
            make("a", Some(10), 101),
            make("b", Some(30), 101),
            make("c", Some(-5), 101),
            make("d", Some(1_000), 102),
            make("e", None, 101),
        ];
        let result = sample(&battles, 101, 100, |battle| {
            battle.damage.map(|value| value as f64)
        });
        assert_eq!(result.eligible, 2);
        assert_eq!(result.value, Some(20.0));
        let limited = sample(&battles, 101, 1, |battle| {
            battle.damage.map(|value| value as f64)
        });
        assert_eq!((limited.eligible, limited.value), (1, Some(10.0)));
        let none = sample(&battles, 999, 100, |battle| {
            battle.damage.map(|value| value as f64)
        });
        assert_eq!((none.eligible, none.value), (0, None));
    }

    #[test]
    fn checked_sum_needs_every_value_and_no_overflow() {
        assert_eq!(checked_sum(&[Some(2), Some(3)]), Some(5.0));
        assert_eq!(checked_sum(&[Some(2), None]), None);
        assert_eq!(checked_sum(&[Some(i64::MAX), Some(1)]), None);
        assert_eq!(checked_sum(&[]), Some(0.0));
    }

    #[test]
    fn raw_negative_needs_enough_battles_on_both_sides() {
        let low = metric(50.0, MIN_SESSION_BATTLES);
        let base = metric(100.0, MIN_BASELINE_BATTLES);
        assert!(raw_negative(low, base).is_some());
        assert_eq!(
            raw_negative(metric(50.0, MIN_SESSION_BATTLES - 1), base),
            None
        );
        assert_eq!(
            raw_negative(low, metric(100.0, MIN_BASELINE_BATTLES - 1)),
            None
        );
    }

    fn metric(value: f64, count: usize) -> MetricSample {
        MetricSample {
            value: Some(value),
            eligible: count,
        }
    }

    fn delta(current: f64, baseline: f64) -> BaselineDelta {
        BaselineDelta {
            current_value: Some(current),
            baseline_value: Some(baseline),
            absolute_delta: Some(current - baseline),
            relative_delta_percent: Some((current - baseline) / baseline * 100.0),
            comparison_state: crate::models::BaselineComparisonState::Below,
        }
    }

    fn baseline(class_id: i64) -> VehicleSessionBaselineComparison {
        let wn8 = crate::models::SessionWn8Presentation {
            state: Wn8PresentationState::Available,
            value: Some(1500.0),
            eligible_battles: 40,
            total_battles: 40,
            coverage_ratio: 1.0,
            exclusions: Wn8ExclusionCounts::default(),
            formula_version: "wn8-standard-v1".into(),
            snapshot_source_version: Some("test".into()),
            snapshot_sha256: Some("a".repeat(64)),
            unavailable_reason: None,
        };
        VehicleSessionBaselineComparison {
            vehicle_id: class_id,
            vehicle_key: Some("nation:tank".into()),
            tank_name: Some("Synthetic tank".into()),
            session_battles: 3,
            session_eligible_battles: 3,
            session_metric_battles: crate::models::SessionMetricBattles {
                wn8: 3,
                damage: 3,
                survival: 3,
                assist: 3,
            },
            baseline: BaselineWindowResult {
                baseline_version: "personal-baseline-v1".into(),
                scope: BaselineScope::Vehicle,
                window: BaselineWindowKind::Last100,
                cutoff: 1_000,
                vehicle_id: Some(class_id),
                candidate_battles: 40,
                eligible_battles: 40,
                coverage_ratio: 1.0,
                confidence: BaselineConfidence::Useful,
                first_battle_at: Some(1),
                last_battle_at: Some(2),
                wn8,
                win_rate: BaselineMetric {
                    value: Some(50.0),
                    eligible_battles: 40,
                    candidate_battles: 40,
                },
                survival_rate: BaselineMetric {
                    value: Some(40.0),
                    eligible_battles: 40,
                    candidate_battles: 40,
                },
                average_damage: BaselineMetric {
                    value: Some(1000.0),
                    eligible_battles: 40,
                    candidate_battles: 40,
                },
                average_assist: BaselineMetric {
                    value: Some(400.0),
                    eligible_battles: 40,
                    candidate_battles: 40,
                },
                average_frags: BaselineMetric {
                    value: Some(1.0),
                    eligible_battles: 40,
                    candidate_battles: 40,
                },
                average_xp: BaselineMetric {
                    value: Some(700.0),
                    eligible_battles: 40,
                    candidate_battles: 40,
                },
                normalized: NormalizedBaselineMetrics {
                    damage: Some(1.0),
                    frags: Some(1.0),
                    spots: Some(1.0),
                    defense: Some(1.0),
                    win: Some(1.0),
                },
                formula_version: "wn8-standard-v1".into(),
                snapshot_source_version: Some("test".into()),
                snapshot_sha256: Some("a".repeat(64)),
            },
            wn8: delta(1200.0, 1500.0),
            damage: delta(700.0, 1000.0),
            win_rate: delta(40.0, 50.0),
            survival_rate: delta(20.0, 40.0),
            assist: delta(200.0, 400.0),
        }
    }

    fn comparison(vehicle: VehicleSessionBaselineComparison) -> SessionBaselineComparison {
        let damage_signal = SessionInsight {
            metric: SessionInsightMetric::Damage,
            direction: SessionInsightDirection::Negative,
            current_value: 700.0,
            baseline_value: 1000.0,
            delta_value: -30.0,
            delta_unit: SessionInsightDeltaUnit::Percent,
            normalized_strength: 3.0,
            evidence_text_key: SessionInsightEvidenceTextKey::DamageBelowNorm,
        };
        let survival_signal = SessionInsight {
            metric: SessionInsightMetric::Survival,
            direction: SessionInsightDirection::Negative,
            current_value: 20.0,
            baseline_value: 40.0,
            delta_value: -20.0,
            delta_unit: SessionInsightDeltaUnit::PercentagePoints,
            normalized_strength: 2.0,
            evidence_text_key: SessionInsightEvidenceTextKey::SurvivalBelowNorm,
        };
        let assist_signal = SessionInsight {
            metric: SessionInsightMetric::Assist,
            direction: SessionInsightDirection::Negative,
            current_value: 200.0,
            baseline_value: 400.0,
            delta_value: -50.0,
            delta_unit: SessionInsightDeltaUnit::Percent,
            normalized_strength: 3.0,
            evidence_text_key: SessionInsightEvidenceTextKey::AssistBelowNorm,
        };
        let id = vehicle.vehicle_id;
        let baseline = vehicle.baseline.clone();
        SessionBaselineComparison {
            baseline_version: "personal-baseline-v1".into(),
            session_id: "session".into(),
            cutoff: 1_000,
            kind: crate::models::SessionBaselineKind::SingleVehicle,
            session_battles: 3,
            global_baseline: baseline,
            global_wn8: delta(1200.0, 1500.0),
            global_normalized: crate::models::NormalizedBaselineDeltas {
                damage: delta(0.7, 1.0),
                frags: delta(0.7, 1.0),
                spots: delta(0.7, 1.0),
                defense: delta(0.7, 1.0),
                win: delta(0.7, 1.0),
            },
            vehicles: vec![vehicle],
            insights: SessionInsightPresentation {
                version: SESSION_INSIGHT_VERSION.into(),
                state: SessionInsightState::Available,
                summaries: vec![SessionInsightSummary {
                    version: SESSION_INSIGHT_VERSION.into(),
                    state: SessionInsightState::Available,
                    confidence: BaselineConfidence::Useful,
                    vehicle_id: Some(id),
                    vehicle_key: None,
                    tank_name: Some("Synthetic tank".into()),
                    baseline_eligible_battles: 40,
                    session_eligible_battles: 3,
                    strongest_positive: None,
                    strongest_negative: Some(damage_signal.clone()),
                    supporting_signals: vec![damage_signal, survival_signal, assist_signal],
                    unavailable_reason: None,
                }],
                unavailable_reason: None,
            },
        }
    }

    fn input(class: VehicleClass) -> (SessionBaselineComparison, ChallengeVehicleInput) {
        let vehicle = baseline(101);
        let comparison = comparison(vehicle.clone());
        let input = ChallengeVehicleInput {
            comparison: vehicle,
            resolution: VehicleClassResolution {
                vehicle_id: 101,
                vehicle_class: class,
                state: VehicleClassResolutionState::Resolved,
                source: VehicleClassSource::WgClientPackedCatalog,
                catalog_schema_version: Some(1),
                catalog_source_version: Some("test".into()),
                catalog_fingerprint: Some("b".repeat(64)),
                protocol_source:
                    CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
                vehicle_key_cross_check: VehicleKeyCrossCheck::RawExact,
            },
            baseline_damage: metric(1000.0, 40),
            baseline_survival: metric(40.0, 40),
            baseline_spotting: metric(300.0, 40),
            baseline_tracking: metric(100.0, 40),
            baseline_spotting_tracking: metric(400.0, 40),
            baseline_stun: metric(500.0, 40),
            baseline_combined: metric(1400.0, 40),
            session_damage: metric(700.0, 3),
            session_survival: metric(20.0, 3),
            session_spotting_tracking: metric(200.0, 3),
            session_stun: metric(200.0, 3),
        };
        (comparison, input)
    }

    fn battle_for_schema(id: &str, battle_time: i64, schema: Option<i64>) -> Battle {
        Battle {
            arena_id: id.into(),
            arena_unique_id: None,
            battle_time,
            battle_time_source: Some("metadata".into()),
            vehicle_key: "tank".into(),
            wg_tank_id: Some(101),
            tank_name: Some("Synthetic tank".into()),
            map_name: None,
            map_key: None,
            battle_type: Some("random".into()),
            gameplay_id: Some("ctf".into()),
            source_kind: crate::models::SourceKind::Replay,
            data_quality: DataQuality::Full,
            parser_version: Some(if schema == Some(2) { 6 } else { 5 }),
            feature_schema_version: schema,
            game_version: None,
            data_completeness: Some("results".into()),
            replay_source: Some("replay_results".into()),
            parser_error_code: None,
            outcome: BattleOutcome::Victory,
            battle_pace: crate::models::strict_random_pace(),
            player_team: Some(1),
            winner_team_raw: Some(1),
            damage: Some(700),
            assist: Some(200),
            assist_spotting: Some(100),
            assist_tracking: Some(100),
            assist_stun: Some(200),
            blocked: None,
            damage_received: None,
            frags: Some(1),
            spots: Some(1),
            shots: Some(10),
            hits: Some(8),
            direct_enemy_hits: (schema == Some(2)).then_some(8),
            direct_team_hits: (schema == Some(2)).then_some(0),
            penetrations: Some(6),
            piercing_enemy_hits: (schema == Some(2)).then_some(6),
            explosion_hits: (schema == Some(2)).then_some(0),
            defense_points: Some(0),
            moving_avg_damage: None,
            damage_rating: None,
            survived: Some(false),
            xp: Some(500),
            original_xp: Some(500),
            duration_seconds: Some(300),
            session_id: None,
        }
    }

    #[test]
    fn challenge_suggestions_are_equivalent_for_core_feature_schemas_one_two_and_three() {
        for class in [
            VehicleClass::LightTank,
            VehicleClass::MediumTank,
            VehicleClass::HeavyTank,
            VehicleClass::TankDestroyer,
            VehicleClass::Spg,
        ] {
            let (comparison, template) = input(class);
            let resolutions = BTreeMap::from([(101, template.resolution.clone())]);
            let schema_one_baseline = (0..40)
                .map(|index| battle_for_schema(&format!("baseline-{index}"), index, Some(1)))
                .collect::<Vec<_>>();
            let schema_two_baseline = schema_one_baseline
                .iter()
                .cloned()
                .map(|mut battle| {
                    battle.feature_schema_version = Some(2);
                    battle.parser_version = Some(6);
                    battle.direct_enemy_hits = Some(8);
                    battle.direct_team_hits = Some(0);
                    battle.piercing_enemy_hits = Some(6);
                    battle.explosion_hits = Some(0);
                    battle
                })
                .collect::<Vec<_>>();
            let schema_one_session = (0..3)
                .map(|index| battle_for_schema(&format!("session-{index}"), 100 + index, Some(1)))
                .collect::<Vec<_>>();
            let schema_two_session = schema_one_session
                .iter()
                .cloned()
                .map(|mut battle| {
                    battle.feature_schema_version = Some(2);
                    battle.parser_version = Some(6);
                    battle.direct_enemy_hits = Some(8);
                    battle.direct_team_hits = Some(0);
                    battle.piercing_enemy_hits = Some(6);
                    battle.explosion_hits = Some(0);
                    battle
                })
                .collect::<Vec<_>>();
            let schema_three_baseline = schema_two_baseline
                .iter()
                .cloned()
                .map(|mut battle| {
                    battle.feature_schema_version = Some(3);
                    battle.parser_version = Some(7);
                    battle
                })
                .collect::<Vec<_>>();
            let schema_three_session = schema_two_session
                .iter()
                .cloned()
                .map(|mut battle| {
                    battle.feature_schema_version = Some(3);
                    battle.parser_version = Some(7);
                    battle
                })
                .collect::<Vec<_>>();
            let one = build_vehicle_inputs(
                &comparison,
                &schema_one_session,
                &schema_one_baseline,
                &resolutions,
            );
            let two = build_vehicle_inputs(
                &comparison,
                &schema_two_session,
                &schema_two_baseline,
                &resolutions,
            );
            let three = build_vehicle_inputs(
                &comparison,
                &schema_three_session,
                &schema_three_baseline,
                &resolutions,
            );
            assert_eq!(one.len(), 1);
            assert_eq!(two.len(), 1);
            assert_eq!(one[0].baseline_damage, two[0].baseline_damage);
            assert_eq!(one[0].baseline_survival, two[0].baseline_survival);
            assert_eq!(one[0].session_damage, two[0].session_damage);
            assert_eq!(
                generate_suggestions(&comparison, &one, false),
                generate_suggestions(&comparison, &two, false)
            );
            assert_eq!(
                generate_suggestions(&comparison, &two, false),
                generate_suggestions(&comparison, &three, false)
            );
        }

        for schema in [None, Some(-1), Some(0), Some(4)] {
            let (comparison, template) = input(VehicleClass::HeavyTank);
            let resolutions = BTreeMap::from([(101, template.resolution)]);
            let battles = vec![battle_for_schema("unsupported", 1, schema)];
            let inputs = build_vehicle_inputs(&comparison, &battles, &battles, &resolutions);
            assert_eq!(inputs[0].baseline_damage.eligible, 0);
            assert_eq!(inputs[0].session_damage.eligible, 0);
        }
    }

    #[test]
    fn resolved_matrix_and_formulas_are_fail_closed_and_deterministic() {
        for (class, allowed) in [
            (
                VehicleClass::LightTank,
                vec![
                    ChallengeType::SpottingAssistConsistency,
                    ChallengeType::EffectiveSurvival,
                    ChallengeType::DamageConsistency,
                ],
            ),
            (
                VehicleClass::MediumTank,
                vec![
                    ChallengeType::DamageConsistency,
                    ChallengeType::EffectiveSurvival,
                    ChallengeType::SpottingAssistConsistency,
                ],
            ),
            (
                VehicleClass::HeavyTank,
                vec![
                    ChallengeType::DamageConsistency,
                    ChallengeType::EffectiveSurvival,
                ],
            ),
            (
                VehicleClass::TankDestroyer,
                vec![
                    ChallengeType::DamageConsistency,
                    ChallengeType::EffectiveSurvival,
                ],
            ),
            (
                VehicleClass::Spg,
                vec![
                    ChallengeType::StunAssistConsistency,
                    ChallengeType::DamageConsistency,
                ],
            ),
        ] {
            for kind in [
                ChallengeType::DamageConsistency,
                ChallengeType::EffectiveSurvival,
                ChallengeType::SpottingAssistConsistency,
                ChallengeType::StunAssistConsistency,
            ] {
                assert_eq!(allowed_challenge(class, kind), allowed.contains(&kind));
            }
        }
        let (comparison, light) = input(VehicleClass::LightTank);
        let first = generate_suggestions(&comparison, std::slice::from_ref(&light), false);
        let second = generate_suggestions(&comparison, &[light], false);
        assert_eq!(first, second);
        assert_eq!(first.suggestions.len(), 2);
        assert_eq!(
            first.suggestions[0].challenge_type,
            ChallengeType::SpottingAssistConsistency
        );
        let damage = first
            .suggestions
            .iter()
            .find(|item| item.challenge_type == ChallengeType::DamageConsistency)
            .unwrap();
        assert_eq!(damage.threshold_value, Some(700));
        assert_eq!(damage.required_successes, 4);
        let (_, mut survival_input) = input(VehicleClass::LightTank);
        survival_input.session_damage.eligible = 2;
        survival_input.session_spotting_tracking = metric(400.0, 3);
        let survival = generate_suggestions(&comparison, &[survival_input], false)
            .suggestions
            .into_iter()
            .find(|item| item.challenge_type == ChallengeType::EffectiveSurvival)
            .unwrap();
        assert_eq!(survival.contribution_floor, Some(700));
        assert!((2..=4).contains(&survival.required_successes));
    }

    #[test]
    fn unresolved_small_samples_and_neutral_inputs_never_force_a_suggestion() {
        for state in [
            VehicleClassResolutionState::Unknown,
            VehicleClassResolutionState::Conflict,
            VehicleClassResolutionState::CatalogUnavailable,
        ] {
            let (comparison, mut input) = input(VehicleClass::HeavyTank);
            input.resolution.state = state;
            let result = generate_suggestions(&comparison, &[input], false);
            assert!(result.suggestions.is_empty());
            assert_eq!(
                result.unavailable_reason,
                Some(ChallengeSuggestionUnavailableReason::VehicleClassUnavailable)
            );
        }
        let (comparison, mut small_input) = input(VehicleClass::HeavyTank);
        small_input.comparison.baseline.eligible_battles = 29;
        assert!(
            generate_suggestions(&comparison, &[small_input], false)
                .suggestions
                .is_empty()
        );
        let (comparison, mut preliminary) = input(VehicleClass::HeavyTank);
        preliminary.comparison.baseline.confidence = BaselineConfidence::Preliminary;
        assert!(
            generate_suggestions(&comparison, &[preliminary], false)
                .suggestions
                .is_empty()
        );
        let (comparison, mut short_session) = input(VehicleClass::HeavyTank);
        short_session.comparison.session_eligible_battles = 2;
        assert!(
            generate_suggestions(&comparison, &[short_session], false)
                .suggestions
                .is_empty()
        );
        let (comparison, mut medium) = input(VehicleClass::MediumTank);
        medium.baseline_spotting_tracking = metric(249.0, 40);
        assert!(
            generate_suggestions(&comparison, &[medium], false)
                .suggestions
                .iter()
                .all(|item| item.challenge_type != ChallengeType::SpottingAssistConsistency)
        );
        let (comparison, active) = input(VehicleClass::HeavyTank);
        assert_eq!(
            generate_suggestions(&comparison, &[active], true).unavailable_reason,
            Some(ChallengeSuggestionUnavailableReason::ActiveChallengeExists)
        );

        let (mut comparison, wn8_only_input) = input(VehicleClass::HeavyTank);
        comparison.insights.summaries[0].supporting_signals.clear();
        comparison.insights.summaries[0].strongest_negative = Some(SessionInsight {
            metric: SessionInsightMetric::Wn8,
            direction: SessionInsightDirection::Negative,
            current_value: 1_000.0,
            baseline_value: 1_500.0,
            delta_value: -33.0,
            delta_unit: SessionInsightDeltaUnit::Percent,
            normalized_strength: 3.0,
            evidence_text_key: SessionInsightEvidenceTextKey::Wn8BelowNorm,
        });
        assert!(
            generate_suggestions(&comparison, &[wn8_only_input], false)
                .suggestions
                .is_empty()
        );
    }

    #[test]
    fn survival_targets_rounding_ids_and_order_are_deterministic() {
        assert_eq!(rounded_floor(5.0, 0.5), Some(3));
        assert_eq!(rounded_floor(1.0, 0.01), Some(1));
        assert_eq!(rounded_floor(0.0, 0.7), None);
        for (survival, expected) in [(10.0, 2), (50.0, 3), (90.0, 4)] {
            let (comparison, mut input) = input(VehicleClass::HeavyTank);
            input.baseline_survival = metric(survival, 40);
            let suggestion = vehicle_suggestions(&comparison, &input)
                .into_iter()
                .find(|item| item.challenge_type == ChallengeType::EffectiveSurvival)
                .unwrap();
            assert_eq!(suggestion.required_successes, expected);
            assert_eq!(suggestion.contribution_floor, Some(500));
        }

        let (comparison, primary_input) = input(VehicleClass::HeavyTank);
        let first = vehicle_suggestions(&comparison, &primary_input)
            .into_iter()
            .find(|item| item.challenge_type == ChallengeType::DamageConsistency)
            .unwrap();
        let mut changed = first.clone();
        changed.threshold_value = Some(first.threshold_value.unwrap() + 1);
        assert_ne!(
            suggestion_id(&first, &comparison.session_id),
            suggestion_id(&changed, &comparison.session_id)
        );
        assert!(valid_suggestion_contract(&first));
        changed.contribution_metric_kind = ChallengeContributionMetricKind::StunAssist;
        assert!(!valid_suggestion_contract(&changed));

        let (_, mut other) = input(VehicleClass::Spg);
        other.comparison.vehicle_id = 102;
        other.resolution.vehicle_id = 102;
        let forward =
            generate_suggestions(&comparison, &[primary_input.clone(), other.clone()], false);
        let reverse = generate_suggestions(&comparison, &[other, primary_input], false);
        assert_eq!(forward.suggestions, reverse.suggestions);
    }

    #[test]
    fn spotting_and_stun_components_are_independent() {
        let (_, light) = input(VehicleClass::LightTank);
        let mut battle = Battle {
            arena_id: "battle".into(),
            arena_unique_id: None,
            battle_time: 3_000,
            battle_time_source: Some("metadata".into()),
            vehicle_key: "tank".into(),
            wg_tank_id: Some(101),
            tank_name: None,
            map_name: None,
            map_key: None,
            battle_type: Some("random".into()),
            gameplay_id: None,
            source_kind: crate::models::SourceKind::Replay,
            data_quality: DataQuality::Full,
            parser_version: Some(2),
            feature_schema_version: Some(1),
            game_version: None,
            data_completeness: Some("results".into()),
            replay_source: Some("replay_results".into()),
            parser_error_code: None,
            outcome: BattleOutcome::Victory,
            battle_pace: crate::models::strict_random_pace(),
            player_team: Some(1),
            winner_team_raw: Some(1),
            damage: Some(500),
            assist: Some(999),
            assist_spotting: Some(100),
            assist_tracking: Some(100),
            assist_stun: Some(999),
            blocked: None,
            damage_received: None,
            frags: Some(0),
            spots: Some(0),
            shots: Some(0),
            hits: Some(0),
            direct_enemy_hits: None,
            direct_team_hits: None,
            penetrations: Some(0),
            piercing_enemy_hits: None,
            explosion_hits: None,
            defense_points: Some(0),
            moving_avg_damage: None,
            damage_rating: None,
            survived: Some(true),
            xp: Some(1),
            original_xp: Some(1),
            duration_seconds: None,
            session_id: None,
        };
        let (comparison, _) = input(VehicleClass::LightTank);
        let suggestion = generate_suggestions(&comparison, &[light], false)
            .suggestions
            .into_iter()
            .find(|item| item.challenge_type == ChallengeType::SpottingAssistConsistency)
            .unwrap();
        let challenge = ChallengePresentation {
            id: "id".into(),
            schema_version: 1,
            engine_version: CHALLENGE_ENGINE_VERSION.into(),
            suggestion_version: CHALLENGE_SUGGESTION_VERSION.into(),
            status: crate::models::ChallengeStatus::Active,
            challenge_type: suggestion.challenge_type,
            vehicle_id: 101,
            vehicle_name: "tank".into(),
            vehicle_class: VehicleClass::LightTank,
            vehicle_class_source: VehicleClassSource::WgClientPackedCatalog,
            vehicle_class_resolution_state: VehicleClassResolutionState::Resolved,
            vehicle_class_resolver_version: VEHICLE_CLASS_RESOLVER_VERSION.into(),
            vehicle_class_catalog_schema_version: 1,
            compact_descriptor_protocol_source:
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            created_at: 1_000,
            started_at: 1_000,
            eligible_after: 2_000,
            finished_at: None,
            target_battles: 5,
            required_successes: 4,
            frozen_baseline_value: 400.0,
            threshold_value: Some(260),
            contribution_metric_kind: ChallengeContributionMetricKind::SpottingTrackingAssist,
            contribution_floor: None,
            baseline_version: "v1".into(),
            baseline_cutoff: 1_000,
            baseline_window: BaselineWindowKind::Last100,
            baseline_confidence: BaselineConfidence::Useful,
            baseline_eligible_battles: 40,
            completed_battles: 0,
            successful_battles: 0,
            battles: Vec::new(),
        };
        assert_eq!(
            evaluate_battle_for_challenge(&challenge, &battle),
            ChallengeBattleEvaluation::Eligible {
                metric_value: 200,
                contribution_value: None,
                survived: Some(true),
                success: false
            }
        );
        battle.assist_tracking = None;
        assert_eq!(
            evaluate_battle_for_challenge(&challenge, &battle),
            ChallengeBattleEvaluation::NotEligible(
                ChallengeNotEligibleReason::MissingRequiredMetric
            )
        );

        let mut effective = challenge.clone();
        effective.challenge_type = ChallengeType::EffectiveSurvival;
        effective.threshold_value = None;
        effective.contribution_metric_kind = ChallengeContributionMetricKind::CombinedContribution;
        effective.contribution_floor = Some(700);
        battle.assist_tracking = Some(100);
        assert_eq!(
            evaluate_battle_for_challenge(&effective, &battle),
            ChallengeBattleEvaluation::Eligible {
                metric_value: 1,
                contribution_value: Some(700),
                survived: Some(true),
                success: true,
            }
        );
        battle.survived = Some(false);
        battle.damage = Some(5_000);
        assert!(matches!(
            evaluate_battle_for_challenge(&effective, &battle),
            ChallengeBattleEvaluation::Eligible { success: false, .. }
        ));
        battle.survived = Some(true);
        battle.assist_spotting = None;
        assert_eq!(
            evaluate_battle_for_challenge(&effective, &battle),
            ChallengeBattleEvaluation::NotEligible(
                ChallengeNotEligibleReason::MissingRequiredMetric
            )
        );

        let mut stun = challenge.clone();
        stun.challenge_type = ChallengeType::StunAssistConsistency;
        stun.vehicle_class = VehicleClass::Spg;
        stun.threshold_value = Some(300);
        stun.contribution_metric_kind = ChallengeContributionMetricKind::StunAssist;
        battle.assist_spotting = Some(10_000);
        battle.assist_tracking = Some(10_000);
        battle.assist_stun = Some(299);
        assert!(matches!(
            evaluate_battle_for_challenge(&stun, &battle),
            ChallengeBattleEvaluation::Eligible {
                success: false,
                metric_value: 299,
                ..
            }
        ));
    }

    #[test]
    fn evaluator_reports_each_fail_closed_eligibility_boundary() {
        let (comparison, input) = input(VehicleClass::HeavyTank);
        let suggestion = generate_suggestions(&comparison, &[input], false)
            .suggestions
            .into_iter()
            .find(|item| item.challenge_type == ChallengeType::DamageConsistency)
            .unwrap();
        let mut challenge = ChallengePresentation {
            id: "id".into(),
            schema_version: 1,
            engine_version: CHALLENGE_ENGINE_VERSION.into(),
            suggestion_version: CHALLENGE_SUGGESTION_VERSION.into(),
            status: crate::models::ChallengeStatus::Active,
            challenge_type: suggestion.challenge_type,
            vehicle_id: 101,
            vehicle_name: "tank".into(),
            vehicle_class: VehicleClass::HeavyTank,
            vehicle_class_source: VehicleClassSource::WgClientPackedCatalog,
            vehicle_class_resolution_state: VehicleClassResolutionState::Resolved,
            vehicle_class_resolver_version: VEHICLE_CLASS_RESOLVER_VERSION.into(),
            vehicle_class_catalog_schema_version: 1,
            compact_descriptor_protocol_source:
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            created_at: 1_000,
            started_at: 1_000,
            eligible_after: 2_000,
            finished_at: None,
            target_battles: 5,
            required_successes: 4,
            frozen_baseline_value: 1_000.0,
            threshold_value: Some(700),
            contribution_metric_kind: ChallengeContributionMetricKind::Damage,
            contribution_floor: None,
            baseline_version: "v1".into(),
            baseline_cutoff: 1_000,
            baseline_window: BaselineWindowKind::Last100,
            baseline_confidence: BaselineConfidence::Useful,
            baseline_eligible_battles: 40,
            completed_battles: 0,
            successful_battles: 0,
            battles: Vec::new(),
        };
        let mut battle = Battle {
            arena_id: "battle".into(),
            arena_unique_id: None,
            battle_time: 2_000,
            battle_time_source: Some("metadata".into()),
            vehicle_key: "tank".into(),
            wg_tank_id: Some(101),
            tank_name: None,
            map_name: None,
            map_key: None,
            battle_type: Some("random".into()),
            gameplay_id: None,
            source_kind: crate::models::SourceKind::Replay,
            data_quality: DataQuality::Full,
            parser_version: Some(2),
            feature_schema_version: Some(1),
            game_version: None,
            data_completeness: Some("results".into()),
            replay_source: Some("replay_results".into()),
            parser_error_code: None,
            outcome: BattleOutcome::Victory,
            battle_pace: crate::models::strict_random_pace(),
            player_team: Some(1),
            winner_team_raw: Some(1),
            damage: Some(700),
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
            survived: Some(true),
            xp: None,
            original_xp: None,
            duration_seconds: None,
            session_id: None,
        };
        battle.wg_tank_id = Some(102);
        assert_eq!(
            evaluate_battle_for_challenge(&challenge, &battle),
            ChallengeBattleEvaluation::NotEligible(ChallengeNotEligibleReason::WrongVehicle)
        );
        battle.wg_tank_id = Some(101);
        battle.battle_time = 1_999;
        assert_eq!(
            evaluate_battle_for_challenge(&challenge, &battle),
            ChallengeBattleEvaluation::NotEligible(ChallengeNotEligibleReason::BeforeEligibleAfter)
        );
        battle.battle_time = 2_000;
        battle.data_quality = DataQuality::Partial;
        assert_eq!(
            evaluate_battle_for_challenge(&challenge, &battle),
            ChallengeBattleEvaluation::NotEligible(ChallengeNotEligibleReason::NotFull)
        );
        battle.data_quality = DataQuality::Full;
        for schema in [None, Some(-1), Some(0), Some(4)] {
            battle.feature_schema_version = schema;
            assert_eq!(
                evaluate_battle_for_challenge(&challenge, &battle),
                ChallengeBattleEvaluation::NotEligible(
                    ChallengeNotEligibleReason::UnsupportedSchema
                )
            );
        }
        battle.feature_schema_version = Some(2);
        assert!(matches!(
            evaluate_battle_for_challenge(&challenge, &battle),
            ChallengeBattleEvaluation::Eligible { .. }
        ));
        battle.feature_schema_version = Some(1);
        battle.damage = None;
        assert_eq!(
            evaluate_battle_for_challenge(&challenge, &battle),
            ChallengeBattleEvaluation::NotEligible(
                ChallengeNotEligibleReason::MissingRequiredMetric
            )
        );
        battle.damage = Some(700);
        challenge.status = crate::models::ChallengeStatus::Abandoned;
        assert_eq!(
            evaluate_battle_for_challenge(&challenge, &battle),
            ChallengeBattleEvaluation::NotEligible(ChallengeNotEligibleReason::ChallengeNotActive)
        );
    }

    // ---- E0 characterization tests --------------------------------------------------------
    // They pin TODAY's behavior. Tests tagged CHARACTERIZATION(<id>) freeze a known defect from
    // the author's internal 2026-11 audit and must be inverted by the stage that fixes it.

    fn active_challenge(challenge_type: ChallengeType, threshold: i64) -> ChallengePresentation {
        ChallengePresentation {
            id: "characterization".into(),
            schema_version: 1,
            engine_version: CHALLENGE_ENGINE_VERSION.into(),
            suggestion_version: CHALLENGE_SUGGESTION_VERSION.into(),
            status: crate::models::ChallengeStatus::Active,
            challenge_type,
            vehicle_id: 101,
            vehicle_name: "Synthetic tank".into(),
            vehicle_class: VehicleClass::HeavyTank,
            vehicle_class_source: VehicleClassSource::WgClientPackedCatalog,
            vehicle_class_resolution_state: VehicleClassResolutionState::Resolved,
            vehicle_class_resolver_version: VEHICLE_CLASS_RESOLVER_VERSION.into(),
            vehicle_class_catalog_schema_version: 1,
            compact_descriptor_protocol_source:
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            created_at: 1_000,
            started_at: 1_000,
            eligible_after: 2_000,
            finished_at: None,
            target_battles: 5,
            required_successes: 4,
            frozen_baseline_value: 1_000.0,
            threshold_value: Some(threshold),
            contribution_metric_kind: ChallengeContributionMetricKind::Damage,
            contribution_floor: None,
            baseline_version: "v1".into(),
            baseline_cutoff: 1_000,
            baseline_window: BaselineWindowKind::Last100,
            baseline_confidence: BaselineConfidence::Useful,
            baseline_eligible_battles: 40,
            completed_battles: 0,
            successful_battles: 0,
            battles: Vec::new(),
        }
    }

    /// M2 (fixed in E1, was a characterization test): a non-random battle never counts toward an
    /// active challenge, whatever its text `battle_type` says.
    #[test]
    fn challenge_ignores_non_random_battles() {
        let challenge = active_challenge(ChallengeType::DamageConsistency, 700);
        for (battle_type_raw, bonus_type_raw) in
            [(Some(2), Some(2)), (Some(1), Some(7)), (None, None)]
        {
            let mut other = battle_for_schema("other-mode", 2_000, Some(1));
            other.battle_type = Some("random".into());
            other.battle_pace.battle_type_raw = battle_type_raw;
            other.battle_pace.bonus_type_raw = bonus_type_raw;
            assert_eq!(
                evaluate_battle_for_challenge(&challenge, &other),
                ChallengeBattleEvaluation::NotEligible(ChallengeNotEligibleReason::NotRandomBattle),
                "{battle_type_raw:?}/{bonus_type_raw:?}"
            );
        }
    }

    /// M2 (fixed in E1): the frozen challenge baseline samples random battles only.
    #[test]
    fn challenge_baseline_samples_only_random_battles() {
        let mut random = battle_for_schema("random", 1_000, Some(1));
        random.damage = Some(1_000);
        let mut other = battle_for_schema("other-mode", 1_001, Some(1));
        other.damage = Some(3_000);
        other.battle_pace.battle_type_raw = Some(2);
        other.battle_pace.bonus_type_raw = Some(2);
        let result = sample(&[random, other], 101, 100, |battle| {
            battle.damage.map(|value| value as f64)
        });
        assert_eq!(result.eligible, 1);
        assert_eq!(result.value, Some(1_000.0));
    }

    /// M4 (fixed in E1): challenges measure assist as spotting + tracking (stun is a separate
    /// challenge); the stored `assist` now uses the same definition.
    #[test]
    fn challenge_assist_is_spotting_plus_tracking() {
        let challenge = active_challenge(ChallengeType::SpottingAssistConsistency, 150);
        let mut value = battle_for_schema("assist", 2_000, Some(1));
        value.assist = Some(200);
        value.assist_spotting = Some(100);
        value.assist_tracking = Some(100);
        value.assist_stun = Some(150);
        assert_eq!(
            evaluate_battle_for_challenge(&challenge, &value),
            ChallengeBattleEvaluation::Eligible {
                metric_value: 200,
                contribution_value: None,
                survived: Some(false),
                success: true,
            }
        );
        assert_eq!(value.assist, Some(200));
    }

    /// Pins D12: no generated suggestion targets win rate.
    #[test]
    fn characterization_challenge_types_never_target_win_rate() {
        let (comparison, input) = input(VehicleClass::HeavyTank);
        let response = generate_suggestions(&comparison, &[input], false);
        assert!(!response.suggestions.is_empty());
        for suggestion in response.suggestions {
            assert!(matches!(
                suggestion.challenge_type,
                ChallengeType::DamageConsistency
                    | ChallengeType::SpottingAssistConsistency
                    | ChallengeType::StunAssistConsistency
                    | ChallengeType::EffectiveSurvival
            ));
        }
    }
}
