use crate::models::{
    BaselineConfidence, BaselineDelta, SessionBaselineComparison, SessionBaselineKind,
    SessionInsight, SessionInsightDeltaUnit, SessionInsightDirection,
    SessionInsightEvidenceTextKey, SessionInsightMetric, SessionInsightPresentation,
    SessionInsightState, SessionInsightSummary, SessionInsightUnavailableReason,
    VehicleSessionBaselineComparison,
};

pub const SESSION_INSIGHT_VERSION: &str = "session-deviation-v1";

// Presentation heuristics v1. They deliberately describe visible deviations only; they are not
// scientific thresholds and must never be treated as evidence about player decisions or causes.
const WN8_THRESHOLD_PERCENT: f64 = 10.0;
const DAMAGE_THRESHOLD_PERCENT: f64 = 10.0;
const ASSIST_THRESHOLD_PERCENT: f64 = 15.0;
const SURVIVAL_THRESHOLD_POINTS: f64 = 10.0;
const MIN_COMPARABLE_ASSIST_BASELINE: f64 = 100.0;
const MAX_NORMALIZED_STRENGTH: f64 = 3.0;
const MIN_BASELINE_BATTLES: usize = 30;
const MIN_SESSION_BATTLES: usize = 3;

enum MetricEvaluation {
    Unavailable,
    Neutral,
    Signal(SessionInsight),
}

pub(crate) fn unavailable_presentation(
    reason: SessionInsightUnavailableReason,
) -> SessionInsightPresentation {
    SessionInsightPresentation {
        version: SESSION_INSIGHT_VERSION.to_owned(),
        state: SessionInsightState::Unavailable,
        summaries: Vec::new(),
        unavailable_reason: Some(reason),
    }
}

fn metric_rank(metric: SessionInsightMetric) -> u8 {
    match metric {
        SessionInsightMetric::Damage => 0,
        SessionInsightMetric::Survival => 1,
        SessionInsightMetric::Wn8 => 2,
        SessionInsightMetric::Assist => 3,
    }
}

fn direction(delta: f64, threshold: f64) -> Option<SessionInsightDirection> {
    if delta >= threshold {
        Some(SessionInsightDirection::Positive)
    } else if delta <= -threshold {
        Some(SessionInsightDirection::Negative)
    } else {
        None
    }
}

fn evidence_key(
    metric: SessionInsightMetric,
    direction: SessionInsightDirection,
) -> SessionInsightEvidenceTextKey {
    match (metric, direction) {
        (SessionInsightMetric::Wn8, SessionInsightDirection::Positive) => {
            SessionInsightEvidenceTextKey::Wn8AboveNorm
        }
        (SessionInsightMetric::Wn8, SessionInsightDirection::Negative) => {
            SessionInsightEvidenceTextKey::Wn8BelowNorm
        }
        (SessionInsightMetric::Damage, SessionInsightDirection::Positive) => {
            SessionInsightEvidenceTextKey::DamageAboveNorm
        }
        (SessionInsightMetric::Damage, SessionInsightDirection::Negative) => {
            SessionInsightEvidenceTextKey::DamageBelowNorm
        }
        (SessionInsightMetric::Survival, SessionInsightDirection::Positive) => {
            SessionInsightEvidenceTextKey::SurvivalAboveNorm
        }
        (SessionInsightMetric::Survival, SessionInsightDirection::Negative) => {
            SessionInsightEvidenceTextKey::SurvivalBelowNorm
        }
        (SessionInsightMetric::Assist, SessionInsightDirection::Positive) => {
            SessionInsightEvidenceTextKey::AssistAboveNorm
        }
        (SessionInsightMetric::Assist, SessionInsightDirection::Negative) => {
            SessionInsightEvidenceTextKey::AssistBelowNorm
        }
        (_, SessionInsightDirection::Neutral) => unreachable!("neutral values are not signals"),
    }
}

fn percent_metric(
    metric: SessionInsightMetric,
    delta: &BaselineDelta,
    threshold: f64,
    minimum_baseline: f64,
) -> MetricEvaluation {
    let (Some(current), Some(baseline)) = (delta.current_value, delta.baseline_value) else {
        return MetricEvaluation::Unavailable;
    };
    if !current.is_finite()
        || !baseline.is_finite()
        || current < 0.0
        || baseline < minimum_baseline
        || threshold <= 0.0
    {
        return MetricEvaluation::Unavailable;
    }
    let delta_value = (current - baseline) / baseline * 100.0;
    if !delta_value.is_finite() {
        return MetricEvaluation::Unavailable;
    }
    let Some(signal_direction) = direction(delta_value, threshold) else {
        return MetricEvaluation::Neutral;
    };
    MetricEvaluation::Signal(SessionInsight {
        metric,
        direction: signal_direction,
        current_value: current,
        baseline_value: baseline,
        delta_value,
        delta_unit: SessionInsightDeltaUnit::Percent,
        normalized_strength: (delta_value.abs() / threshold).min(MAX_NORMALIZED_STRENGTH),
        evidence_text_key: evidence_key(metric, signal_direction),
    })
}

fn survival_metric(delta: &BaselineDelta) -> MetricEvaluation {
    let (Some(current), Some(baseline)) = (delta.current_value, delta.baseline_value) else {
        return MetricEvaluation::Unavailable;
    };
    if !current.is_finite()
        || !baseline.is_finite()
        || !(0.0..=100.0).contains(&current)
        || !(0.0..=100.0).contains(&baseline)
    {
        return MetricEvaluation::Unavailable;
    }
    let delta_value = current - baseline;
    let Some(signal_direction) = direction(delta_value, SURVIVAL_THRESHOLD_POINTS) else {
        return MetricEvaluation::Neutral;
    };
    MetricEvaluation::Signal(SessionInsight {
        metric: SessionInsightMetric::Survival,
        direction: signal_direction,
        current_value: current,
        baseline_value: baseline,
        delta_value,
        delta_unit: SessionInsightDeltaUnit::PercentagePoints,
        normalized_strength: (delta_value.abs() / SURVIVAL_THRESHOLD_POINTS)
            .min(MAX_NORMALIZED_STRENGTH),
        evidence_text_key: evidence_key(SessionInsightMetric::Survival, signal_direction),
    })
}

/// D7: a metric is compared only when it has its own 30 baseline battles and 3 session battles
/// (WN8 counts only battles with expected values; the others do not need XVM).
fn gated(
    baseline_battles: usize,
    session_battles: usize,
    evaluate: impl FnOnce() -> MetricEvaluation,
) -> MetricEvaluation {
    if baseline_battles < MIN_BASELINE_BATTLES || session_battles < MIN_SESSION_BATTLES {
        MetricEvaluation::Unavailable
    } else {
        evaluate()
    }
}

fn sorted_signals(comparison: &VehicleSessionBaselineComparison) -> (usize, Vec<SessionInsight>) {
    let baseline = &comparison.baseline;
    let session = &comparison.session_metric_battles;
    let evaluations = [
        gated(
            baseline.average_damage.eligible_battles,
            session.damage,
            || {
                percent_metric(
                    SessionInsightMetric::Damage,
                    &comparison.damage,
                    DAMAGE_THRESHOLD_PERCENT,
                    f64::EPSILON,
                )
            },
        ),
        gated(
            baseline.survival_rate.eligible_battles,
            session.survival,
            || survival_metric(&comparison.survival_rate),
        ),
        gated(baseline.wn8.eligible_battles, session.wn8, || {
            percent_metric(
                SessionInsightMetric::Wn8,
                &comparison.wn8,
                WN8_THRESHOLD_PERCENT,
                f64::EPSILON,
            )
        }),
        gated(
            baseline.average_assist.eligible_battles,
            session.assist,
            || {
                percent_metric(
                    SessionInsightMetric::Assist,
                    &comparison.assist,
                    ASSIST_THRESHOLD_PERCENT,
                    MIN_COMPARABLE_ASSIST_BASELINE,
                )
            },
        ),
    ];
    let comparable = evaluations
        .iter()
        .filter(|evaluation| !matches!(evaluation, MetricEvaluation::Unavailable))
        .count();
    let mut signals = evaluations
        .into_iter()
        .filter_map(|evaluation| match evaluation {
            MetricEvaluation::Signal(signal) => Some(signal),
            MetricEvaluation::Unavailable | MetricEvaluation::Neutral => None,
        })
        .collect::<Vec<_>>();
    signals.sort_by(|left, right| {
        right
            .normalized_strength
            .total_cmp(&left.normalized_strength)
            .then_with(|| metric_rank(left.metric).cmp(&metric_rank(right.metric)))
    });
    (comparable, signals)
}

fn summary(comparison: &VehicleSessionBaselineComparison) -> SessionInsightSummary {
    let baseline_eligible = comparison.baseline.eligible_battles;
    let session_eligible = comparison.session_eligible_battles;
    let mut result = SessionInsightSummary {
        version: SESSION_INSIGHT_VERSION.to_owned(),
        state: SessionInsightState::Unavailable,
        confidence: comparison.baseline.confidence,
        vehicle_id: Some(comparison.vehicle_id),
        vehicle_key: comparison.vehicle_key.clone(),
        tank_name: comparison.tank_name.clone(),
        baseline_eligible_battles: baseline_eligible,
        session_eligible_battles: session_eligible,
        strongest_positive: None,
        strongest_negative: None,
        supporting_signals: Vec::new(),
        unavailable_reason: None,
    };

    let baseline_confidence_is_useful = matches!(
        comparison.baseline.confidence,
        BaselineConfidence::Useful | BaselineConfidence::Reliable | BaselineConfidence::Stable
    );
    // M10: a baseline that is too small carries no signals at all.
    if baseline_eligible < MIN_BASELINE_BATTLES || !baseline_confidence_is_useful {
        result.state = SessionInsightState::InsufficientBaseline;
        result.unavailable_reason = Some(SessionInsightUnavailableReason::BaselineTooSmall);
        return result;
    }
    let (comparable, signals) = sorted_signals(comparison);
    if session_eligible < MIN_SESSION_BATTLES {
        result.state = SessionInsightState::InsufficientSession;
        result.unavailable_reason = Some(SessionInsightUnavailableReason::SessionTooSmall);
        return result;
    }
    if comparable == 0 {
        result.unavailable_reason = Some(SessionInsightUnavailableReason::NoComparableMetrics);
        return result;
    }

    result.state = SessionInsightState::Available;
    result.strongest_positive = signals
        .iter()
        .find(|signal| signal.direction == SessionInsightDirection::Positive)
        .cloned();
    result.strongest_negative = signals
        .iter()
        .find(|signal| signal.direction == SessionInsightDirection::Negative)
        .cloned();
    result.supporting_signals = signals;
    result
}

pub fn derive_session_insights(
    comparison: &SessionBaselineComparison,
) -> SessionInsightPresentation {
    if comparison.kind == SessionBaselineKind::NoComparableBattles || comparison.vehicles.is_empty()
    {
        return unavailable_presentation(SessionInsightUnavailableReason::NoComparableBattles);
    }

    let mut summaries = comparison.vehicles.iter().map(summary).collect::<Vec<_>>();
    summaries.sort_by_key(|item| item.vehicle_id);
    let state = if summaries
        .iter()
        .any(|item| item.state == SessionInsightState::Available)
    {
        SessionInsightState::Available
    } else if summaries
        .iter()
        .any(|item| item.state == SessionInsightState::InsufficientSession)
    {
        SessionInsightState::InsufficientSession
    } else if summaries
        .iter()
        .any(|item| item.state == SessionInsightState::InsufficientBaseline)
    {
        SessionInsightState::InsufficientBaseline
    } else {
        SessionInsightState::Unavailable
    };
    let unavailable_reason = (state != SessionInsightState::Available)
        .then(|| summaries.iter().find_map(|item| item.unavailable_reason))
        .flatten();
    SessionInsightPresentation {
        version: SESSION_INSIGHT_VERSION.to_owned(),
        state,
        summaries,
        unavailable_reason,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::models::{
        BaselineComparisonState, BaselineMetric, BaselineScope, BaselineWindowKind,
        BaselineWindowResult, NormalizedBaselineDeltas, NormalizedBaselineMetrics,
        SessionMetricBattles, SessionWn8Presentation, Wn8ExclusionCounts, Wn8PresentationState,
    };

    #[test]
    fn equal_strength_signals_keep_a_fixed_metric_order() {
        let order = [
            SessionInsightMetric::Damage,
            SessionInsightMetric::Survival,
            SessionInsightMetric::Wn8,
            SessionInsightMetric::Assist,
        ]
        .map(metric_rank);
        assert_eq!(order, [0, 1, 2, 3]);
    }

    #[test]
    fn percent_metric_refuses_non_finite_or_negative_inputs_and_accepts_the_edges() {
        let evaluate = |current: f64, baseline: f64, minimum: f64| {
            percent_metric(
                SessionInsightMetric::Damage,
                &delta(Some(current), Some(baseline)),
                10.0,
                minimum,
            )
        };
        let unavailable =
            |evaluation: MetricEvaluation| matches!(evaluation, MetricEvaluation::Unavailable);
        let signal =
            |evaluation: MetricEvaluation| matches!(evaluation, MetricEvaluation::Signal(_));
        assert!(unavailable(evaluate(f64::NAN, 1_000.0, 1.0)));
        assert!(unavailable(evaluate(f64::INFINITY, 1_000.0, 1.0)));
        assert!(unavailable(evaluate(1_000.0, f64::NAN, 1.0)));
        assert!(unavailable(evaluate(-1.0, 1_000.0, 1.0)));
        // Zero is a real session value (−100%), not a missing one.
        assert!(signal(evaluate(0.0, 1_000.0, 1.0)));
        // A baseline exactly at the minimum is comparable; just below it is not.
        assert!(signal(evaluate(150.0, 100.0, 100.0)));
        assert!(unavailable(evaluate(150.0, 99.0, 100.0)));
        assert!(unavailable(evaluate(
            150.0,
            100.0,
            100.0 + f64::EPSILON * 128.0
        )));
        assert!(unavailable(percent_metric(
            SessionInsightMetric::Damage,
            &delta(Some(150.0), Some(100.0)),
            0.0,
            1.0
        )));
    }

    #[test]
    fn survival_outside_zero_to_hundred_on_either_side_is_unavailable() {
        let unavailable = |current: f64, baseline: f64| {
            matches!(
                survival_metric(&delta(Some(current), Some(baseline))),
                MetricEvaluation::Unavailable
            )
        };
        assert!(unavailable(150.0, 50.0));
        assert!(unavailable(50.0, 150.0));
        assert!(unavailable(-1.0, 50.0));
        assert!(unavailable(50.0, -1.0));
        assert!(unavailable(f64::NAN, 50.0));
        assert!(unavailable(50.0, f64::INFINITY));
        assert!(!unavailable(100.0, 0.0));
    }

    pub(crate) fn delta(current: Option<f64>, baseline: Option<f64>) -> BaselineDelta {
        let absolute = current
            .zip(baseline)
            .map(|(current, baseline)| current - baseline);
        let relative = current.zip(baseline).and_then(|(current, baseline)| {
            (baseline != 0.0).then_some((current - baseline) / baseline * 100.0)
        });
        BaselineDelta {
            current_value: current,
            baseline_value: baseline,
            absolute_delta: absolute,
            relative_delta_percent: relative,
            comparison_state: BaselineComparisonState::Equal,
        }
    }

    fn empty_metric(value: Option<f64>, eligible: usize) -> BaselineMetric {
        BaselineMetric {
            value,
            eligible_battles: eligible,
            candidate_battles: eligible,
        }
    }

    fn baseline(eligible: usize) -> BaselineWindowResult {
        let confidence = match eligible {
            0..=9 => BaselineConfidence::Insufficient,
            10..=29 => BaselineConfidence::Preliminary,
            30..=49 => BaselineConfidence::Useful,
            50..=99 => BaselineConfidence::Reliable,
            _ => BaselineConfidence::Stable,
        };
        BaselineWindowResult {
            baseline_version: "personal-baseline-v1".to_owned(),
            scope: BaselineScope::Vehicle,
            window: BaselineWindowKind::Last100,
            cutoff: 1,
            vehicle_id: Some(1),
            candidate_battles: eligible,
            eligible_battles: eligible,
            coverage_ratio: 1.0,
            confidence,
            first_battle_at: None,
            last_battle_at: None,
            wn8: SessionWn8Presentation {
                state: Wn8PresentationState::Available,
                value: Some(1_000.0),
                eligible_battles: eligible,
                total_battles: eligible,
                coverage_ratio: 1.0,
                exclusions: Wn8ExclusionCounts::default(),
                formula_version: "wn8-standard-v1".to_owned(),
                snapshot_source_version: Some("test".to_owned()),
                snapshot_sha256: Some("synthetic".to_owned()),
                unavailable_reason: None,
            },
            win_rate: empty_metric(Some(50.0), eligible),
            survival_rate: empty_metric(Some(50.0), eligible),
            average_damage: empty_metric(Some(1_000.0), eligible),
            average_assist: empty_metric(Some(200.0), eligible),
            average_frags: empty_metric(None, eligible),
            average_xp: empty_metric(None, eligible),
            normalized: NormalizedBaselineMetrics {
                damage: None,
                frags: None,
                spots: None,
                defense: None,
                win: None,
            },
            formula_version: "wn8-standard-v1".to_owned(),
            snapshot_source_version: Some("test".to_owned()),
            snapshot_sha256: Some("synthetic".to_owned()),
        }
    }

    pub(crate) fn vehicle(
        vehicle_id: i64,
        eligible: usize,
        session_eligible: usize,
    ) -> VehicleSessionBaselineComparison {
        VehicleSessionBaselineComparison {
            vehicle_id,
            vehicle_key: Some(format!("vehicle-{vehicle_id}")),
            tank_name: Some(format!("Tank {vehicle_id}")),
            session_battles: session_eligible,
            session_eligible_battles: session_eligible,
            session_metric_battles: SessionMetricBattles {
                wn8: session_eligible,
                damage: session_eligible,
                survival: session_eligible,
                assist: session_eligible,
            },
            baseline: BaselineWindowResult {
                vehicle_id: Some(vehicle_id),
                ..baseline(eligible)
            },
            wn8: delta(Some(1_000.0), Some(1_000.0)),
            damage: delta(Some(1_000.0), Some(1_000.0)),
            win_rate: delta(Some(50.0), Some(50.0)),
            survival_rate: delta(Some(50.0), Some(50.0)),
            assist: delta(Some(200.0), Some(200.0)),
        }
    }

    pub(crate) fn comparison_with(
        vehicles: Vec<VehicleSessionBaselineComparison>,
    ) -> SessionBaselineComparison {
        SessionBaselineComparison {
            baseline_version: "personal-baseline-v1".to_owned(),
            session_id: "synthetic-session".to_owned(),
            cutoff: 1,
            kind: if vehicles.len() > 1 {
                SessionBaselineKind::MultiVehicle
            } else if vehicles.is_empty() {
                SessionBaselineKind::NoComparableBattles
            } else {
                SessionBaselineKind::SingleVehicle
            },
            session_battles: vehicles.iter().map(|item| item.session_battles).sum(),
            global_baseline: baseline(100),
            global_wn8: delta(Some(1_000.0), Some(1_000.0)),
            global_normalized: NormalizedBaselineDeltas {
                damage: delta(None, None),
                frags: delta(None, None),
                spots: delta(None, None),
                defense: delta(None, None),
                win: delta(None, None),
            },
            insights: unavailable_presentation(
                SessionInsightUnavailableReason::NoComparableBattles,
            ),
            vehicles,
        }
    }

    fn one(eligible: usize, session_eligible: usize) -> SessionBaselineComparison {
        comparison_with(vec![vehicle(1, eligible, session_eligible)])
    }

    fn only_summary(comparison: &SessionBaselineComparison) -> SessionInsightSummary {
        derive_session_insights(comparison).summaries.remove(0)
    }

    #[test]
    fn no_vehicles_or_no_comparable_kind_alone_make_the_session_unavailable() {
        let mut empty = comparison_with(Vec::new());
        empty.kind = SessionBaselineKind::SingleVehicle;
        let presentation = derive_session_insights(&empty);
        assert_eq!(presentation.state, SessionInsightState::Unavailable);
        assert_eq!(
            presentation.unavailable_reason,
            Some(SessionInsightUnavailableReason::NoComparableBattles)
        );
        let mut kind_only = one(100, 3);
        kind_only.kind = SessionBaselineKind::NoComparableBattles;
        let presentation = derive_session_insights(&kind_only);
        assert_eq!(presentation.state, SessionInsightState::Unavailable);
        assert!(presentation.summaries.is_empty());
    }

    #[test]
    fn the_session_state_is_the_best_vehicle_state_and_available_has_no_reason() {
        // Every metric missing: the vehicle (and the session) stays unavailable.
        let mut missing = vehicle(1, 100, 3);
        for metric in [
            &mut missing.damage,
            &mut missing.survival_rate,
            &mut missing.wn8,
            &mut missing.assist,
        ] {
            *metric = delta(None, None);
        }
        let presentation = derive_session_insights(&comparison_with(vec![missing]));
        assert_eq!(presentation.state, SessionInsightState::Unavailable);
        assert_eq!(
            presentation.unavailable_reason,
            Some(SessionInsightUnavailableReason::NoComparableMetrics)
        );
        // One vehicle available, another with a too small baseline: available, no reason.
        let presentation =
            derive_session_insights(&comparison_with(vec![vehicle(1, 100, 3), vehicle(2, 9, 3)]));
        assert_eq!(presentation.state, SessionInsightState::Available);
        assert_eq!(presentation.unavailable_reason, None);
    }

    #[test]
    fn baseline_below_ten_has_no_insights() {
        let summary = only_summary(&one(9, 3));
        assert_eq!(summary.state, SessionInsightState::InsufficientBaseline);
        assert!(summary.supporting_signals.is_empty());
        assert!(summary.strongest_positive.is_none());
        assert!(summary.strongest_negative.is_none());
    }

    /// M10 (was: signals were filled although the state said the baseline was too small).
    #[test]
    fn baseline_ten_to_twenty_nine_is_insufficient_and_carries_no_signals() {
        let mut comparison = one(29, 3);
        comparison.vehicles[0].damage = delta(Some(1_200.0), Some(1_000.0));
        let summary = only_summary(&comparison);
        assert_eq!(summary.state, SessionInsightState::InsufficientBaseline);
        assert!(summary.supporting_signals.is_empty());
        assert!(summary.strongest_positive.is_none());
    }

    /// D7: WN8 needs its own 30 baseline battles and 3 session battles with expected values;
    /// damage keeps its signal when WN8 does not qualify.
    #[test]
    fn wn8_without_enough_qualifying_battles_is_not_compared() {
        for (baseline_wn8, session_wn8) in [(29, 3), (30, 2), (0, 0)] {
            let mut comparison = one(30, 3);
            comparison.vehicles[0].baseline.wn8.eligible_battles = baseline_wn8;
            comparison.vehicles[0].session_metric_battles.wn8 = session_wn8;
            comparison.vehicles[0].wn8 = delta(Some(2_000.0), Some(1_000.0));
            comparison.vehicles[0].damage = delta(Some(1_100.0), Some(1_000.0));
            let summary = only_summary(&comparison);
            assert_eq!(summary.state, SessionInsightState::Available);
            let metrics = summary
                .supporting_signals
                .iter()
                .map(|signal| signal.metric)
                .collect::<Vec<_>>();
            assert_eq!(
                metrics,
                vec![SessionInsightMetric::Damage],
                "{baseline_wn8}/{session_wn8}"
            );
        }
        let mut qualified = one(30, 3);
        qualified.vehicles[0].wn8 = delta(Some(2_000.0), Some(1_000.0));
        assert_eq!(
            only_summary(&qualified).strongest_positive.unwrap().metric,
            SessionInsightMetric::Wn8
        );
    }

    /// D7: each metric is gated by its own counts.
    #[test]
    fn each_metric_is_gated_by_its_own_battle_counts() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0]
            .baseline
            .average_damage
            .eligible_battles = 29;
        comparison.vehicles[0].session_metric_battles.survival = 2;
        comparison.vehicles[0]
            .baseline
            .average_assist
            .eligible_battles = 10;
        comparison.vehicles[0].damage = delta(Some(1_500.0), Some(1_000.0));
        comparison.vehicles[0].survival_rate = delta(Some(80.0), Some(50.0));
        comparison.vehicles[0].assist = delta(Some(400.0), Some(200.0));
        comparison.vehicles[0].wn8 = delta(Some(1_100.0), Some(1_000.0));
        let summary = only_summary(&comparison);
        let metrics = summary
            .supporting_signals
            .iter()
            .map(|signal| signal.metric)
            .collect::<Vec<_>>();
        assert_eq!(metrics, vec![SessionInsightMetric::Wn8]);
    }

    #[test]
    fn no_metric_with_enough_battles_means_no_comparable_metrics() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].session_metric_battles = SessionMetricBattles::default();
        let summary = only_summary(&comparison);
        assert_eq!(summary.state, SessionInsightState::Unavailable);
        assert_eq!(
            summary.unavailable_reason,
            Some(SessionInsightUnavailableReason::NoComparableMetrics)
        );
    }

    #[test]
    fn useful_baseline_and_short_session_is_insufficient_session() {
        assert_eq!(
            only_summary(&one(30, 2)).state,
            SessionInsightState::InsufficientSession
        );
    }

    #[test]
    fn preliminary_confidence_never_produces_a_main_diagnosis() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].baseline.confidence = BaselineConfidence::Preliminary;
        comparison.vehicles[0].damage = delta(Some(1_200.0), Some(1_000.0));
        let summary = only_summary(&comparison);
        assert_eq!(summary.state, SessionInsightState::InsufficientBaseline);
        assert!(summary.strongest_positive.is_none());
        assert!(summary.supporting_signals.is_empty());
    }

    #[test]
    fn damage_plus_nine_percent_is_neutral() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(1_090.0), Some(1_000.0));
        assert!(only_summary(&comparison).strongest_positive.is_none());
    }

    #[test]
    fn damage_plus_ten_percent_is_positive() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(1_100.0), Some(1_000.0));
        assert_eq!(
            only_summary(&comparison).strongest_positive.unwrap().metric,
            SessionInsightMetric::Damage
        );
    }

    #[test]
    fn damage_minus_ten_percent_is_negative() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(900.0), Some(1_000.0));
        assert_eq!(
            only_summary(&comparison).strongest_negative.unwrap().metric,
            SessionInsightMetric::Damage
        );
    }

    #[test]
    fn survival_plus_ten_points_is_positive() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].survival_rate = delta(Some(50.0), Some(40.0));
        assert_eq!(
            only_summary(&comparison).strongest_positive.unwrap().metric,
            SessionInsightMetric::Survival
        );
    }

    #[test]
    fn survival_minus_ten_points_is_negative() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].survival_rate = delta(Some(40.0), Some(50.0));
        assert_eq!(
            only_summary(&comparison).strongest_negative.unwrap().metric,
            SessionInsightMetric::Survival
        );
    }

    #[test]
    fn assist_plus_fourteen_percent_is_neutral() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].assist = delta(Some(228.0), Some(200.0));
        assert!(only_summary(&comparison).strongest_positive.is_none());
    }

    #[test]
    fn assist_plus_fifteen_percent_is_positive() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].assist = delta(Some(230.0), Some(200.0));
        assert_eq!(
            only_summary(&comparison).strongest_positive.unwrap().metric,
            SessionInsightMetric::Assist
        );
    }

    #[test]
    fn strongest_positive_uses_normalized_strength() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(1_200.0), Some(1_000.0));
        comparison.vehicles[0].survival_rate = delta(Some(65.0), Some(50.0));
        assert_eq!(
            only_summary(&comparison).strongest_positive.unwrap().metric,
            SessionInsightMetric::Damage
        );
    }

    #[test]
    fn strongest_negative_uses_normalized_strength() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(850.0), Some(1_000.0));
        comparison.vehicles[0].survival_rate = delta(Some(30.0), Some(50.0));
        assert_eq!(
            only_summary(&comparison).strongest_negative.unwrap().metric,
            SessionInsightMetric::Survival
        );
    }

    #[test]
    fn stable_tie_breaker_prefers_damage_then_survival_then_wn8_then_assist() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(1_200.0), Some(1_000.0));
        comparison.vehicles[0].survival_rate = delta(Some(70.0), Some(50.0));
        comparison.vehicles[0].wn8 = delta(Some(1_200.0), Some(1_000.0));
        comparison.vehicles[0].assist = delta(Some(260.0), Some(200.0));
        assert_eq!(
            only_summary(&comparison).strongest_positive.unwrap().metric,
            SessionInsightMetric::Damage
        );
    }

    #[test]
    fn no_threshold_signals_means_session_close_to_norm() {
        let summary = only_summary(&one(30, 3));
        assert_eq!(summary.state, SessionInsightState::Available);
        assert!(summary.strongest_positive.is_none());
        assert!(summary.strongest_negative.is_none());
    }

    #[test]
    fn zero_baseline_is_safe_and_not_comparable() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(1_000.0), Some(0.0));
        let summary = only_summary(&comparison);
        assert!(
            summary
                .supporting_signals
                .iter()
                .all(|signal| signal.metric != SessionInsightMetric::Damage)
        );
    }

    #[test]
    fn nan_and_infinity_are_rejected() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(f64::NAN), Some(1_000.0));
        comparison.vehicles[0].wn8 = delta(Some(f64::INFINITY), Some(1_000.0));
        let summary = only_summary(&comparison);
        assert!(summary.strongest_positive.is_none());
        assert!(summary.strongest_negative.is_none());
    }

    #[test]
    fn normalized_strength_is_capped() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].damage = delta(Some(10_000.0), Some(1_000.0));
        assert_eq!(
            only_summary(&comparison)
                .strongest_positive
                .unwrap()
                .normalized_strength,
            MAX_NORMALIZED_STRENGTH
        );
    }

    #[test]
    fn vehicle_input_order_does_not_change_results() {
        let first = comparison_with(vec![vehicle(2, 30, 3), vehicle(1, 30, 3)]);
        let second = comparison_with(vec![vehicle(1, 30, 3), vehicle(2, 30, 3)]);
        assert_eq!(
            derive_session_insights(&first),
            derive_session_insights(&second)
        );
    }

    #[test]
    fn mixed_session_without_three_vehicle_battles_has_no_insights() {
        let presentation =
            derive_session_insights(&comparison_with(vec![vehicle(1, 30, 2), vehicle(2, 30, 1)]));
        assert_eq!(presentation.state, SessionInsightState::InsufficientSession);
        assert!(
            presentation
                .summaries
                .iter()
                .all(|summary| summary.strongest_positive.is_none())
        );
    }

    #[test]
    fn mixed_session_with_two_qualified_vehicles_returns_two_summaries() {
        let presentation =
            derive_session_insights(&comparison_with(vec![vehicle(1, 30, 3), vehicle(2, 50, 4)]));
        assert_eq!(presentation.state, SessionInsightState::Available);
        assert_eq!(presentation.summaries.len(), 2);
    }

    #[test]
    fn derivation_is_pure_and_does_not_mutate_input() {
        let comparison = one(30, 3);
        let before = comparison.clone();
        let _ = derive_session_insights(&comparison);
        assert_eq!(comparison, before);
    }

    #[test]
    fn presentation_returns_version() {
        let presentation = derive_session_insights(&one(30, 3));
        assert_eq!(presentation.version, SESSION_INSIGHT_VERSION);
        assert!(
            presentation
                .summaries
                .iter()
                .all(|summary| summary.version == SESSION_INSIGHT_VERSION)
        );
    }

    #[test]
    fn tiny_assist_baseline_cannot_win_with_exploding_percentage() {
        let mut comparison = one(30, 3);
        comparison.vehicles[0].assist = delta(Some(100.0), Some(1.0));
        comparison.vehicles[0].damage = delta(Some(1_100.0), Some(1_000.0));
        assert_eq!(
            only_summary(&comparison).strongest_positive.unwrap().metric,
            SessionInsightMetric::Damage
        );
    }
}
