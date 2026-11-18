use crate::{
    error::AppError,
    expected_values::LoadedExpectedSnapshot,
    history::safe_identity_value,
    models::{
        BaselineComparisonState, BaselineConfidence, BaselineDelta, BaselineMetric, BaselineScope,
        BaselineWindowKind, BaselineWindowResult, Battle, BattleHistoryCursor,
        NormalizedBaselineDeltas, NormalizedBaselineMetrics, PlayerBaselinePresentation,
        SessionBaselineComparison, SessionBaselineKind, SessionInsightUnavailableReason,
        SessionMetricBattles, VehicleBaselinePresentation, VehicleSessionBaselineComparison,
        Wn8Calculation, stats_eligibility,
    },
    ratings::{WN8_FORMULA_VERSION, calculate_battle_wn8, calculate_wn8},
    session_insights::{derive_session_insights, unavailable_presentation},
    session_wn8_presentation,
    storage::Database,
};
use std::collections::{BTreeMap, BTreeSet};

pub const PLAYER_BASELINE_VERSION: &str = "personal-baseline-v1";
const CANDIDATE_PAGE_SIZE: usize = 200;
const CANDIDATE_SAFETY_LIMIT: usize = 5_000;
const MAX_WINDOW: usize = 100;

pub(crate) struct SessionBaselineContext {
    pub comparison: SessionBaselineComparison,
    pub session_battles: Vec<Battle>,
    pub vehicle_baseline_battles: Vec<Battle>,
}

impl BaselineWindowKind {
    fn limit(self) -> usize {
        match self {
            Self::Last10 => 10,
            Self::Last30 => 30,
            Self::Last100 => 100,
        }
    }
}

pub(crate) fn confidence(eligible_battles: usize) -> BaselineConfidence {
    match eligible_battles {
        0..=9 => BaselineConfidence::Insufficient,
        10..=29 => BaselineConfidence::Preliminary,
        30..=49 => BaselineConfidence::Useful,
        50..=99 => BaselineConfidence::Reliable,
        _ => BaselineConfidence::Stable,
    }
}

/// D7: a reference battle is any battle that counts for statistics (D4). Whether it also has
/// expected values only matters for WN8, which counts its own battles.
fn is_reference_battle(battle: &Battle) -> bool {
    stats_eligibility(battle)
}

fn load_candidates(
    database: &Database,
    cutoff: i64,
    vehicle_id: Option<i64>,
    excluded_session_id: Option<&str>,
    required_vehicle_ids: &BTreeSet<i64>,
) -> Result<Vec<Battle>, AppError> {
    let mut candidates = Vec::new();
    let mut cursor = None;
    let mut global_reference_count = 0_usize;
    let mut vehicle_reference_counts = BTreeMap::<i64, usize>::new();
    loop {
        let remaining = CANDIDATE_SAFETY_LIMIT.saturating_sub(candidates.len());
        if remaining == 0 {
            break;
        }
        let rows = database.baseline_candidate_rows(
            cutoff,
            vehicle_id,
            excluded_session_id,
            cursor.as_ref(),
            CANDIDATE_PAGE_SIZE.min(remaining),
        )?;
        if rows.is_empty() {
            break;
        }
        let row_count = rows.len();
        for battle in rows {
            cursor = Some(BattleHistoryCursor {
                battle_time: battle.battle_time,
                battle_id: battle.arena_id.clone(),
            });
            if is_reference_battle(&battle) {
                global_reference_count += 1;
                if let Some(id) = battle.wg_tank_id
                    && required_vehicle_ids.contains(&id)
                {
                    *vehicle_reference_counts.entry(id).or_default() += 1;
                }
            }
            candidates.push(battle);
        }
        let global_ready = global_reference_count >= MAX_WINDOW;
        let vehicles_ready = required_vehicle_ids.iter().all(|id| {
            vehicle_reference_counts
                .get(id)
                .copied()
                .unwrap_or_default()
                >= MAX_WINDOW
        });
        if (global_ready && vehicles_ready) || row_count < CANDIDATE_PAGE_SIZE {
            break;
        }
    }
    Ok(candidates)
}

fn is_wn8_battle(battle: &Battle, expected: Option<&LoadedExpectedSnapshot>) -> bool {
    expected.is_some_and(|snapshot| {
        matches!(
            calculate_battle_wn8(battle, snapshot),
            Wn8Calculation::Available(_)
        )
    })
}

/// Which metrics a D4 battle can feed (D7). WN8 additionally needs expected values.
fn metric_flags(battle: &Battle, expected: Option<&LoadedExpectedSnapshot>) -> [bool; 8] {
    [
        true,
        battle.damage.is_some(),
        battle.assist.is_some(),
        battle.frags.is_some(),
        battle.original_xp.is_some(),
        battle.survived.is_some(),
        battle.outcome.win_contribution().is_some(),
        expected.is_none() || is_wn8_battle(battle, expected),
    ]
}

/// The newest battles, extended until every metric has `limit` qualifying D4 battles of its own
/// (D7), or the candidates run out. Each metric then takes its newest `limit` values from this
/// prefix.
fn window_candidates(
    candidates: &[Battle],
    limit: usize,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Vec<Battle> {
    let mut ordered = candidates.to_vec();
    ordered.sort_by(|left, right| {
        right
            .battle_time
            .cmp(&left.battle_time)
            .then_with(|| right.arena_id.cmp(&left.arena_id))
    });
    let mut selected = Vec::new();
    let mut counts = [0_usize; 8];
    for battle in ordered {
        if is_reference_battle(&battle) {
            for (count, qualifies) in counts.iter_mut().zip(metric_flags(&battle, expected)) {
                if qualifies {
                    *count += 1;
                }
            }
        }
        selected.push(battle);
        if counts.iter().all(|count| *count >= limit) {
            break;
        }
    }
    selected
}

/// The newest `limit` D4 battles of the prefix, with the battles skipped in between, so WN8
/// reports its own coverage and never uses more than `limit` battles.
fn wn8_window(
    selected: &[Battle],
    limit: usize,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Vec<Battle> {
    let mut used = 0_usize;
    let mut window = Vec::new();
    for battle in selected {
        if used == limit {
            break;
        }
        if is_reference_battle(battle) && is_wn8_battle(battle, expected) {
            used += 1;
        }
        window.push(battle.clone());
    }
    window
}

fn numeric_metric(
    candidates: &[Battle],
    limit: usize,
    value: impl Fn(&Battle) -> Option<i64>,
) -> BaselineMetric {
    let values = candidates
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .filter_map(value)
        .take(limit)
        .map(|value| value as f64)
        .collect::<Vec<_>>();
    BaselineMetric {
        value: (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
        eligible_battles: values.len(),
        candidate_battles: candidates.len(),
    }
}

fn boolean_rate(
    candidates: &[Battle],
    limit: usize,
    value: impl Fn(&Battle) -> Option<bool>,
) -> BaselineMetric {
    let values = candidates
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .filter_map(value)
        .take(limit)
        .collect::<Vec<_>>();
    BaselineMetric {
        value: (!values.is_empty()).then(|| {
            values.iter().filter(|value| **value).count() as f64 / values.len() as f64 * 100.0
        }),
        eligible_battles: values.len(),
        candidate_battles: candidates.len(),
    }
}

fn outcome_rate(candidates: &[Battle], limit: usize) -> BaselineMetric {
    let values = candidates
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .filter_map(|battle| battle.outcome.win_contribution())
        .take(limit)
        .collect::<Vec<_>>();
    BaselineMetric {
        value: (!values.is_empty())
            .then(|| values.iter().sum::<f64>() / values.len() as f64 * 100.0),
        eligible_battles: values.len(),
        candidate_battles: candidates.len(),
    }
}

fn normalized_metrics(
    battles: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
) -> NormalizedBaselineMetrics {
    let Some(expected) = expected else {
        return NormalizedBaselineMetrics {
            damage: None,
            frags: None,
            spots: None,
            defense: None,
            win: None,
        };
    };
    let Wn8Calculation::Available(result) = calculate_wn8(battles, expected) else {
        return NormalizedBaselineMetrics {
            damage: None,
            frags: None,
            spots: None,
            defense: None,
            win: None,
        };
    };
    NormalizedBaselineMetrics {
        damage: Some(result.actual_totals.damage / result.expected_totals.damage),
        frags: Some(result.actual_totals.frags / result.expected_totals.frags),
        spots: Some(result.actual_totals.spots / result.expected_totals.spots),
        defense: Some(result.actual_totals.defense / result.expected_totals.defense),
        win: Some(result.actual_totals.win_points / result.expected_totals.win_points),
    }
}

fn baseline_window(
    candidates: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
    cutoff: i64,
    scope: BaselineScope,
    vehicle_id: Option<i64>,
    window: BaselineWindowKind,
) -> BaselineWindowResult {
    let limit = window.limit();
    let selected = window_candidates(candidates, limit, expected);
    let wn8_battles = wn8_window(&selected, limit, expected);
    let wn8 = session_wn8_presentation(&wn8_battles, expected);
    let reference_times = selected
        .iter()
        .filter(|battle| is_reference_battle(battle))
        .take(limit)
        .map(|battle| battle.battle_time)
        .collect::<Vec<_>>();
    let normalized = normalized_metrics(&wn8_battles, expected);
    BaselineWindowResult {
        baseline_version: PLAYER_BASELINE_VERSION.to_owned(),
        scope,
        window,
        cutoff,
        vehicle_id,
        candidate_battles: selected.len(),
        // D7: qualification and confidence come from D4 battles, not from the XVM snapshot.
        eligible_battles: reference_times.len(),
        coverage_ratio: if selected.is_empty() {
            0.0
        } else {
            reference_times.len() as f64 / selected.len() as f64
        },
        confidence: confidence(reference_times.len()),
        first_battle_at: reference_times.iter().min().copied(),
        last_battle_at: reference_times.iter().max().copied(),
        win_rate: outcome_rate(&selected, limit),
        survival_rate: boolean_rate(&selected, limit, |battle| battle.survived),
        average_damage: numeric_metric(&selected, limit, |battle| battle.damage),
        average_assist: numeric_metric(&selected, limit, |battle| battle.assist),
        average_frags: numeric_metric(&selected, limit, |battle| battle.frags),
        // D8 / audit M11: averages use XP without premium.
        average_xp: numeric_metric(&selected, limit, |battle| battle.original_xp),
        formula_version: WN8_FORMULA_VERSION.to_owned(),
        snapshot_source_version: expected.map(|snapshot| snapshot.manifest.source_version.clone()),
        snapshot_sha256: expected.map(|snapshot| snapshot.manifest.sha256.clone()),
        wn8,
        normalized,
    }
}

fn windows(
    candidates: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
    cutoff: i64,
    scope: BaselineScope,
    vehicle_id: Option<i64>,
) -> Vec<BaselineWindowResult> {
    [
        BaselineWindowKind::Last10,
        BaselineWindowKind::Last30,
        BaselineWindowKind::Last100,
    ]
    .into_iter()
    .map(|window| baseline_window(candidates, expected, cutoff, scope, vehicle_id, window))
    .collect()
}

pub(crate) fn player_baseline(
    database: &Database,
    cutoff: i64,
    excluded_session_id: Option<&str>,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<PlayerBaselinePresentation, AppError> {
    let candidates = load_candidates(
        database,
        cutoff,
        None,
        excluded_session_id,
        &BTreeSet::new(),
    )?;
    Ok(PlayerBaselinePresentation {
        baseline_version: PLAYER_BASELINE_VERSION.to_owned(),
        cutoff,
        windows: windows(&candidates, expected, cutoff, BaselineScope::Player, None),
    })
}

fn vehicle_presentation_from_candidates(
    candidates: &[Battle],
    cutoff: i64,
    vehicle_id: i64,
    expected: Option<&LoadedExpectedSnapshot>,
) -> VehicleBaselinePresentation {
    let newest = candidates.first();
    VehicleBaselinePresentation {
        baseline_version: PLAYER_BASELINE_VERSION.to_owned(),
        cutoff,
        vehicle_id,
        vehicle_key: newest.and_then(|battle| safe_identity_value(Some(&battle.vehicle_key))),
        tank_name: newest.and_then(|battle| safe_identity_value(battle.tank_name.as_deref())),
        windows: windows(
            candidates,
            expected,
            cutoff,
            BaselineScope::Vehicle,
            Some(vehicle_id),
        ),
    }
}

pub(crate) fn vehicle_baseline(
    database: &Database,
    vehicle_id: i64,
    cutoff: i64,
    excluded_session_id: Option<&str>,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<VehicleBaselinePresentation, AppError> {
    if vehicle_id <= 0 {
        return Err(AppError::InvalidApiResponse);
    }
    let candidates = load_candidates(
        database,
        cutoff,
        Some(vehicle_id),
        excluded_session_id,
        &BTreeSet::new(),
    )?;
    Ok(vehicle_presentation_from_candidates(
        &candidates,
        cutoff,
        vehicle_id,
        expected,
    ))
}

pub(crate) fn delta(current: Option<f64>, baseline: Option<f64>) -> BaselineDelta {
    let (Some(current), Some(baseline)) = (current, baseline) else {
        return BaselineDelta {
            current_value: current,
            baseline_value: baseline,
            absolute_delta: None,
            relative_delta_percent: None,
            comparison_state: BaselineComparisonState::Unavailable,
        };
    };
    let absolute = current - baseline;
    BaselineDelta {
        current_value: Some(current),
        baseline_value: Some(baseline),
        absolute_delta: Some(absolute),
        relative_delta_percent: (baseline != 0.0).then_some(absolute / baseline * 100.0),
        comparison_state: if absolute.abs() < f64::EPSILON {
            BaselineComparisonState::Equal
        } else if absolute > 0.0 {
            BaselineComparisonState::Above
        } else {
            BaselineComparisonState::Below
        },
    }
}

fn normalized_deltas(
    current: &NormalizedBaselineMetrics,
    baseline: &NormalizedBaselineMetrics,
) -> NormalizedBaselineDeltas {
    NormalizedBaselineDeltas {
        damage: delta(current.damage, baseline.damage),
        frags: delta(current.frags, baseline.frags),
        spots: delta(current.spots, baseline.spots),
        defense: delta(current.defense, baseline.defense),
        win: delta(current.win, baseline.win),
    }
}

fn vehicle_comparison(
    current: &[Battle],
    candidates: &[Battle],
    cutoff: i64,
    vehicle_id: i64,
    expected: Option<&LoadedExpectedSnapshot>,
) -> VehicleSessionBaselineComparison {
    let baseline = baseline_window(
        candidates,
        expected,
        cutoff,
        BaselineScope::Vehicle,
        Some(vehicle_id),
        BaselineWindowKind::Last100,
    );
    let current_wn8 = session_wn8_presentation(current, expected);
    let current_damage = numeric_metric(current, current.len(), |battle| battle.damage);
    let current_assist = numeric_metric(current, current.len(), |battle| battle.assist);
    let current_win_rate = outcome_rate(current, current.len());
    let current_survival = boolean_rate(current, current.len(), |battle| battle.survived);
    let newest = current
        .iter()
        .max_by_key(|battle| (battle.battle_time, &battle.arena_id));
    VehicleSessionBaselineComparison {
        vehicle_id,
        vehicle_key: newest.and_then(|battle| safe_identity_value(Some(&battle.vehicle_key))),
        tank_name: newest.and_then(|battle| safe_identity_value(battle.tank_name.as_deref())),
        session_battles: current.len(),
        session_eligible_battles: current
            .iter()
            .filter(|battle| is_reference_battle(battle))
            .count(),
        session_metric_battles: SessionMetricBattles {
            wn8: current_wn8.eligible_battles,
            damage: current_damage.eligible_battles,
            survival: current_survival.eligible_battles,
            assist: current_assist.eligible_battles,
        },
        wn8: delta(current_wn8.value, baseline.wn8.value),
        damage: delta(current_damage.value, baseline.average_damage.value),
        win_rate: delta(current_win_rate.value, baseline.win_rate.value),
        survival_rate: delta(current_survival.value, baseline.survival_rate.value),
        assist: delta(current_assist.value, baseline.average_assist.value),
        baseline,
    }
}

pub(crate) fn session_baseline_context(
    database: &Database,
    session_id: &str,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<SessionBaselineContext, AppError> {
    let session = database.session(session_id)?;
    let session_battles = database.session_battles(session_id)?;
    let vehicle_ids = session_battles
        .iter()
        .filter(|battle| is_reference_battle(battle))
        .filter_map(|battle| battle.wg_tank_id)
        .collect::<BTreeSet<_>>();
    let candidates = load_candidates(
        database,
        session.started_at,
        None,
        Some(session_id),
        &vehicle_ids,
    )?;
    let global_baseline = baseline_window(
        &candidates,
        expected,
        session.started_at,
        BaselineScope::Player,
        None,
        BaselineWindowKind::Last100,
    );
    let current_wn8 = session_wn8_presentation(&session_battles, expected);
    let current_normalized = normalized_metrics(&session_battles, expected);
    let mut vehicle_baseline_battles = Vec::new();
    let mut vehicles = vehicle_ids
        .iter()
        .map(|vehicle_id| {
            let current = session_battles
                .iter()
                .filter(|battle| battle.wg_tank_id == Some(*vehicle_id))
                .cloned()
                .collect::<Vec<_>>();
            let historical = candidates
                .iter()
                .filter(|battle| battle.wg_tank_id == Some(*vehicle_id))
                .cloned()
                .collect::<Vec<_>>();
            vehicle_baseline_battles.extend(window_candidates(
                &historical,
                BaselineWindowKind::Last100.limit(),
                expected,
            ));
            vehicle_comparison(
                &current,
                &historical,
                session.started_at,
                *vehicle_id,
                expected,
            )
        })
        .collect::<Vec<_>>();
    vehicles.sort_by_key(|comparison| comparison.vehicle_id);
    let kind = match vehicle_ids.len() {
        0 => SessionBaselineKind::NoComparableBattles,
        1 => SessionBaselineKind::SingleVehicle,
        _ => SessionBaselineKind::MultiVehicle,
    };
    let mut comparison = SessionBaselineComparison {
        baseline_version: PLAYER_BASELINE_VERSION.to_owned(),
        session_id: session_id.to_owned(),
        cutoff: session.started_at,
        kind,
        session_battles: session_battles.len(),
        global_wn8: delta(current_wn8.value, global_baseline.wn8.value),
        global_normalized: normalized_deltas(&current_normalized, &global_baseline.normalized),
        global_baseline,
        vehicles,
        insights: unavailable_presentation(SessionInsightUnavailableReason::NoComparableBattles),
    };
    comparison.insights = derive_session_insights(&comparison);
    Ok(SessionBaselineContext {
        comparison,
        session_battles,
        vehicle_baseline_battles,
    })
}

pub(crate) fn session_baseline_comparison(
    database: &Database,
    session_id: &str,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<SessionBaselineComparison, AppError> {
    Ok(session_baseline_context(database, session_id, expected)?.comparison)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DataQuality;
    use crate::{
        expected_values::validate_snapshot_bytes,
        models::{BattleOutcome, SourceKind, Wn8PresentationState},
    };
    use rusqlite::{Connection, OpenFlags};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    fn snapshot() -> LoadedExpectedSnapshot {
        let rows = (1..=500)
            .map(|id| {
                json!({
                    "IDNum": id,
                    "expDef": 0.5,
                    "expFrag": if id == 2 { 1.2 } else { 0.8 },
                    "expSpot": 1.1,
                    "expDamage": if id == 2 { 1500.0 } else { 900.0 },
                    "expWinRate": 49.5
                })
            })
            .collect::<Vec<_>>();
        validate_snapshot_bytes(
            &serde_json::to_vec(
                &json!({"header":{"source":"XVM","version":"2026-07-10"},"data":rows}),
            )
            .unwrap(),
            None,
        )
        .unwrap()
    }

    fn battle(id: &str, time: i64, vehicle_id: i64, damage: i64) -> Battle {
        Battle {
            arena_id: id.to_owned(),
            arena_unique_id: None,
            battle_time: time,
            battle_time_source: Some("metadata".to_owned()),
            vehicle_key: format!("vehicle-{vehicle_id}-{id}"),
            wg_tank_id: Some(vehicle_id),
            tank_name: Some(format!("Tank {vehicle_id}")),
            map_name: Some("Map".to_owned()),
            map_key: Some("01_map".to_owned()),
            battle_type: Some("random".to_owned()),
            gameplay_id: None,
            source_kind: SourceKind::Replay,
            data_quality: DataQuality::Full,
            parser_version: Some(2),
            feature_schema_version: Some(1),
            game_version: None,
            data_completeness: Some("results".to_owned()),
            replay_source: Some("replay_results".to_owned()),
            parser_error_code: None,
            outcome: if time % 2 == 0 {
                BattleOutcome::Victory
            } else {
                BattleOutcome::Defeat
            },
            battle_pace: crate::models::strict_random_pace(),
            player_team: Some(1),
            winner_team_raw: Some(if time % 2 == 0 { 1 } else { 2 }),
            damage: Some(damage),
            assist: Some(200),
            assist_spotting: Some(100),
            assist_tracking: Some(100),
            assist_stun: Some(0),
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
            survived: Some(time % 3 == 0),
            xp: Some(900),
            original_xp: Some(900),
            duration_seconds: None,
            session_id: None,
        }
    }

    #[test]
    fn candidates_stop_after_the_page_that_fills_every_required_window() {
        // 500 counted battles, tanks 1 and 2 alternating, pages of 200.
        let (_temp, database) = database_with_history(500);
        let load = |required: &[i64]| {
            load_candidates(
                &database,
                1_000,
                None,
                None,
                &required.iter().copied().collect::<BTreeSet<_>>(),
            )
            .unwrap()
            .len()
        };
        // 200 battles: 100 global and 100 for tank 1 are already there.
        assert_eq!(load(&[]), CANDIDATE_PAGE_SIZE);
        assert_eq!(load(&[1]), CANDIDATE_PAGE_SIZE);
        // Tank 3 never reaches 100: every page is read, up to the short last one.
        assert_eq!(load(&[3]), 500);
        assert_eq!(load(&[1, 3]), 500);
    }

    #[test]
    fn the_wn8_window_holds_only_its_own_number_of_battles() {
        let (_temp, database) = database_with_history(30);
        let expected = snapshot();
        let result = player_baseline(&database, 1_000, None, Some(&expected)).unwrap();
        let last10 = window(&result, BaselineWindowKind::Last10);
        assert_eq!(last10.wn8.eligible_battles, 10);
        assert_eq!(last10.candidate_battles, 10);
    }

    /// Spec 32 §1: the two survivors of 2026-11-15 in `wn8_window`. Only D4 battles with
    /// expected values count towards the limit (a battle of a tank missing from the snapshot
    /// does not), and the window stops once the limit is reached.
    #[test]
    fn the_wn8_window_counts_only_battles_with_expected_values_and_stops_at_the_limit() {
        let expected = snapshot();
        let with_wn8 = |id: &str, time: i64| battle(id, time, 1, 1_000);
        let selected = vec![
            with_wn8("a", 6),
            battle("no-expected-values", 5, 999, 1_000),
            with_wn8("b", 4),
            with_wn8("c", 3),
            with_wn8("d", 2),
        ];
        assert!(is_reference_battle(&selected[1]));
        assert!(!is_wn8_battle(&selected[1], Some(&expected)));
        let ids = |window: Vec<Battle>| {
            window
                .into_iter()
                .map(|battle| battle.arena_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(wn8_window(&selected, 2, Some(&expected))),
            ["a", "no-expected-values", "b"]
        );
        assert_eq!(
            ids(wn8_window(&selected, 0, Some(&expected))),
            Vec::<String>::new()
        );
    }

    fn database_with_history(count: usize) -> (tempfile::TempDir, Database) {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join("baseline.sqlite3")).unwrap();
        for index in 1..=count {
            let vehicle_id = if index % 2 == 0 { 2 } else { 1 };
            let mut value = battle(
                &format!("history-{index:03}"),
                index as i64,
                vehicle_id,
                800 + index as i64 * 10,
            );
            database.insert_battle(&mut value, false).unwrap();
        }
        (temp, database)
    }

    fn logical_digest(path: &std::path::Path) -> String {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        connection.execute_batch("PRAGMA query_only=ON").unwrap();
        let mut digest = Sha256::new();
        for query in [
            "SELECT arena_id||'|'||battle_time||'|'||COALESCE(session_id,'')||'|'||data_quality FROM battles ORDER BY arena_id",
            "SELECT id||'|'||started_at||'|'||status||'|'||COALESCE(last_battle_at,'') FROM sessions ORDER BY id",
            "SELECT key||'|'||value FROM settings ORDER BY key",
            "SELECT source_path_key||'|'||content_fingerprint FROM replay_files ORDER BY source_path_key",
        ] {
            let mut statement = connection.prepare(query).unwrap();
            for row in statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
            {
                digest.update(row.unwrap().as_bytes());
                digest.update(b"\n");
            }
        }
        crate::hashing::hex_lower(digest.finalize())
    }

    #[test]
    fn confidence_thresholds_are_central_and_exact() {
        for (count, expected) in [
            (0, BaselineConfidence::Insufficient),
            (9, BaselineConfidence::Insufficient),
            (10, BaselineConfidence::Preliminary),
            (29, BaselineConfidence::Preliminary),
            (30, BaselineConfidence::Useful),
            (49, BaselineConfidence::Useful),
            (50, BaselineConfidence::Reliable),
            (99, BaselineConfidence::Reliable),
            (100, BaselineConfidence::Stable),
        ] {
            assert_eq!(confidence(count), expected);
        }
    }

    #[test]
    fn windows_are_bounded_newest_first_stable_and_order_independent() {
        let expected = snapshot();
        let candidates = (1..=120)
            .map(|index| battle(&format!("id-{index:03}"), index, 1, index))
            .collect::<Vec<_>>();
        let results = windows(
            &candidates,
            Some(&expected),
            1_000,
            BaselineScope::Player,
            None,
        );
        assert_eq!(results[0].eligible_battles, 10);
        assert_eq!(results[1].eligible_battles, 30);
        assert_eq!(results[2].eligible_battles, 100);
        assert_eq!(results[0].first_battle_at, Some(111));
        assert_eq!(results[0].last_battle_at, Some(120));
        assert_eq!(results[0].average_damage.value, Some(115.5));
        let mut reversed = candidates.clone();
        reversed.reverse();
        assert_eq!(
            baseline_window(
                &candidates,
                Some(&expected),
                1_000,
                BaselineScope::Player,
                None,
                BaselineWindowKind::Last100,
            ),
            baseline_window(
                &reversed,
                Some(&expected),
                1_000,
                BaselineScope::Player,
                None,
                BaselineWindowKind::Last100,
            )
        );
        let tied = [battle("a", 500, 1, 100), battle("b", 500, 1, 200)];
        let tied_result = baseline_window(
            &tied,
            Some(&expected),
            1_000,
            BaselineScope::Player,
            None,
            BaselineWindowKind::Last10,
        );
        assert_eq!(tied_result.average_damage.value, Some(150.0));
        assert_eq!(
            window_candidates(&tied, 1, Some(&expected))[0].arena_id,
            "b"
        );
    }

    #[test]
    fn global_baseline_uses_wn8_totals_normalized_ratios_and_partial_coverage() {
        let (_temp, database) = database_with_history(10);
        let mut partial = battle("partial", 11, 1, 99_999);
        partial.data_quality = DataQuality::Partial;
        partial.feature_schema_version = None;
        partial.damage = None;
        database.insert_battle(&mut partial, false).unwrap();
        let expected = snapshot();
        let result = player_baseline(&database, 100, None, Some(&expected)).unwrap();
        let last10 = &result.windows[0];
        assert_eq!(last10.eligible_battles, 10);
        assert_eq!(last10.candidate_battles, 11);
        assert_eq!(last10.coverage_ratio, 10.0 / 11.0);
        let candidates = database
            .baseline_candidate_rows(100, None, None, None, 20)
            .unwrap();
        let engine = calculate_wn8(&candidates, &expected);
        let Wn8Calculation::Available(engine) = engine else {
            panic!("expected an aggregate WN8 result")
        };
        assert_eq!(last10.wn8.value, Some(engine.value));
        assert_eq!(
            last10.normalized.damage,
            Some(engine.actual_totals.damage / engine.expected_totals.damage)
        );
        let per_battle_mean = candidates
            .iter()
            .filter_map(|battle| match calculate_battle_wn8(battle, &expected) {
                Wn8Calculation::Available(result) => Some(result.value),
                Wn8Calculation::Unavailable(_) => None,
            })
            .sum::<f64>()
            / 10.0;
        assert_ne!(last10.wn8.value.unwrap(), per_battle_mean);
    }

    #[test]
    fn missing_snapshot_and_fields_preserve_available_metrics_without_zeroes() {
        let (_temp, database) = database_with_history(12);
        let result = player_baseline(&database, 100, None, None).unwrap();
        let window = &result.windows[0];
        assert_eq!(window.wn8.state, Wn8PresentationState::Unavailable);
        // D7: qualification does not depend on the snapshot.
        assert_eq!(window.eligible_battles, 10);
        assert_eq!(window.wn8.eligible_battles, 0);
        assert!(window.average_damage.value.is_some());
        assert!(window.win_rate.value.is_some());
        let mut candidates = database
            .baseline_candidate_rows(100, None, None, None, 20)
            .unwrap();
        for battle in &mut candidates {
            battle.assist = None;
        }
        let missing = baseline_window(
            &candidates,
            None,
            100,
            BaselineScope::Player,
            None,
            BaselineWindowKind::Last10,
        );
        assert_eq!(missing.average_assist.value, None);
        assert_eq!(missing.average_assist.eligible_battles, 0);
    }

    #[test]
    fn baseline_win_rate_counts_draws_and_excludes_unknown_outcomes() {
        let mut victory = battle("victory", 1, 1, 1_000);
        victory.outcome = BattleOutcome::Victory;
        let mut draw = battle("draw", 2, 1, 1_000);
        draw.outcome = BattleOutcome::Draw;
        let mut defeat = battle("defeat", 3, 1, 1_000);
        defeat.outcome = BattleOutcome::Defeat;
        let mut unknown = battle("unknown", 4, 1, 1_000);
        unknown.outcome = BattleOutcome::Unknown;

        let result = outcome_rate(&[victory, draw, defeat, unknown], 10);
        assert!((result.value.unwrap() - 100.0 / 3.0).abs() < 1e-12);
        assert_eq!(result.eligible_battles, 3);
        assert_eq!(result.candidate_battles, 4);
    }

    #[test]
    fn point_in_time_baseline_keeps_draw_before_cutoff_and_excludes_future_results() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("draw-cutoff.db")).unwrap();
        for (id, time, outcome) in [
            ("old-victory", 10, BattleOutcome::Victory),
            ("old-draw", 20, BattleOutcome::Draw),
            ("future-draw", 200, BattleOutcome::Draw),
        ] {
            let mut value = battle(id, time, 1, 1_000);
            value.outcome = outcome;
            database.insert_battle(&mut value, false).unwrap();
        }

        let result = player_baseline(&database, 100, None, None).unwrap();
        let last10 = &result.windows[0];
        assert_eq!(last10.win_rate.value, Some(50.0));
        assert_eq!(last10.win_rate.eligible_battles, 2);
        assert_eq!(last10.candidate_battles, 2);
        assert_eq!(last10.cutoff, 100);
    }

    #[test]
    fn vehicle_baseline_uses_only_id_and_keeps_latest_safe_name() {
        let (_temp, database) = database_with_history(40);
        let expected = snapshot();
        let result = vehicle_baseline(&database, 1, 100, None, Some(&expected)).unwrap();
        assert_eq!(result.vehicle_id, 1);
        assert!(
            result
                .windows
                .iter()
                .all(|window| window.vehicle_id == Some(1))
        );
        assert_eq!(result.windows[1].eligible_battles, 20);
        assert_eq!(result.windows[1].average_damage.eligible_battles, 20);
        assert_eq!(result.baseline_version, PLAYER_BASELINE_VERSION);
        assert_eq!(result.windows[0].formula_version, WN8_FORMULA_VERSION);
        assert_eq!(
            result.windows[0].snapshot_source_version.as_deref(),
            Some("2026-07-10")
        );
    }

    #[test]
    fn session_comparison_is_point_in_time_excludes_self_and_returns_mixed_breakdown() {
        let (temp, database) = database_with_history(120);
        let expected = snapshot();
        let mut first = battle("session-one", 1_000, 1, 2_000);
        database.insert_battle(&mut first, true).unwrap();
        let session_id = first.session_id.clone().unwrap();
        let mut second = battle("session-two", 1_001, 2, 3_000);
        database.insert_battle(&mut second, true).unwrap();
        let before_context =
            session_baseline_context(&database, &session_id, Some(&expected)).unwrap();
        let before_future = before_context.comparison.clone();
        assert_eq!(before_future.cutoff, 1_000);
        assert_eq!(before_future.kind, SessionBaselineKind::MultiVehicle);
        assert_eq!(before_future.vehicles.len(), 2);
        assert_eq!(before_future.global_baseline.eligible_battles, 100);
        assert!(
            before_future
                .vehicles
                .iter()
                .all(|vehicle| vehicle.session_battles == 1)
        );
        let digest_before = logical_digest(&temp.path().join("baseline.sqlite3"));
        let _ = player_baseline(&database, 1_000, Some(&session_id), Some(&expected)).unwrap();
        let _ = vehicle_baseline(&database, 1, 1_000, Some(&session_id), Some(&expected)).unwrap();
        let _ = session_baseline_comparison(&database, &session_id, Some(&expected)).unwrap();
        assert_eq!(
            digest_before,
            logical_digest(&temp.path().join("baseline.sqlite3"))
        );
        let mut future = battle("future", 2_000, 1, 99_999);
        database.insert_battle(&mut future, false).unwrap();
        let after_context =
            session_baseline_context(&database, &session_id, Some(&expected)).unwrap();
        let after_future = after_context.comparison.clone();
        assert_eq!(before_future, after_future);
        assert_eq!(
            before_context.vehicle_baseline_battles,
            after_context.vehicle_baseline_battles
        );
        assert!(
            after_future
                .global_baseline
                .first_battle_at
                .is_none_or(|time| time < 1_000)
        );
    }

    #[test]
    fn session_context_keeps_the_full_accepted_window_for_per_metric_coverage() {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join("context.sqlite3")).unwrap();
        for index in 1..=100 {
            let mut value = battle(&format!("eligible-{index:03}"), index, 1, 1_000);
            database.insert_battle(&mut value, false).unwrap();
        }
        for index in 101..=200 {
            let mut value = battle(&format!("missing-{index:03}"), index, 1, 1_000);
            value.damage = None;
            database.insert_battle(&mut value, false).unwrap();
        }
        let mut current = battle("session-one", 1_000, 1, 100);
        database.insert_battle(&mut current, true).unwrap();
        let session_id = current.session_id.clone().unwrap();
        for index in 1..3 {
            let mut next = battle(&format!("session-{}", index + 1), 1_000 + index, 1, 100);
            database.insert_battle(&mut next, true).unwrap();
        }

        let context = session_baseline_context(&database, &session_id, Some(&snapshot())).unwrap();
        let vehicle = &context.comparison.vehicles[0];
        assert_eq!(vehicle.baseline.average_damage.value, Some(1_000.0));
        assert_eq!(vehicle.baseline.average_damage.eligible_battles, 100);
        assert_eq!(vehicle.baseline.candidate_battles, 200);
        assert_eq!(context.vehicle_baseline_battles.len(), 200);
        assert_eq!(
            context
                .vehicle_baseline_battles
                .iter()
                .filter(|battle| battle.damage.is_some())
                .count(),
            100
        );
        assert!(
            context
                .vehicle_baseline_battles
                .iter()
                .all(|battle| battle.battle_time < context.comparison.cutoff)
        );
    }

    #[test]
    fn single_vehicle_session_uses_that_vehicle_and_deltas_are_neutral_math() {
        let (_temp, database) = database_with_history(40);
        let expected = snapshot();
        let mut current = battle("single-session", 1_000, 1, 2_000);
        database.insert_battle(&mut current, true).unwrap();
        let comparison = session_baseline_comparison(
            &database,
            current.session_id.as_deref().unwrap(),
            Some(&expected),
        )
        .unwrap();
        assert_eq!(comparison.kind, SessionBaselineKind::SingleVehicle);
        assert_eq!(comparison.vehicles.len(), 1);
        assert_eq!(comparison.vehicles[0].vehicle_id, 1);
        let percent = delta(Some(120.0), Some(100.0));
        assert_eq!(percent.absolute_delta, Some(20.0));
        assert_eq!(percent.relative_delta_percent, Some(20.0));
        assert_eq!(percent.comparison_state, BaselineComparisonState::Above);
        let points = delta(Some(60.0), Some(50.0));
        assert_eq!(points.absolute_delta, Some(10.0));
        let zero = delta(Some(1.0), Some(0.0));
        assert_eq!(zero.relative_delta_percent, None);
        assert_eq!(
            delta(None, Some(1.0)).comparison_state,
            BaselineComparisonState::Unavailable
        );
    }

    // ---- E0 characterization tests --------------------------------------------------------
    // They pin TODAY's behavior. Tests tagged CHARACTERIZATION(<id>) freeze a known defect from
    // the author's internal 2026-11 audit and must be inverted by the stage that fixes it.

    fn window(
        presentation: &PlayerBaselinePresentation,
        kind: BaselineWindowKind,
    ) -> &BaselineWindowResult {
        presentation
            .windows
            .iter()
            .find(|window| window.window == kind)
            .unwrap()
    }

    fn empty_database(name: &str) -> (tempfile::TempDir, Database) {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join(name)).unwrap();
        (temp, database)
    }

    /// M2 (fixed in E1, was a characterization test): non-random battles (training, raw type 2)
    /// never enter the baseline windows or averages.
    #[test]
    fn baseline_ignores_non_random_battles() {
        let (_temp, database) = empty_database("m2.sqlite3");
        for index in 1..=10 {
            let mut random = battle(&format!("random-{index:02}"), index, 1, 1_000);
            database.insert_battle(&mut random, false).unwrap();
        }
        for index in 11..=20 {
            let mut other = battle(&format!("other-{index:02}"), index, 1, 3_000);
            other.battle_type = Some("training".to_owned());
            other.battle_pace.battle_type_raw = Some(2);
            other.battle_pace.bonus_type_raw = Some(2);
            database.insert_battle(&mut other, false).unwrap();
        }
        let expected = snapshot();
        let result = player_baseline(&database, 1_000, None, Some(&expected)).unwrap();

        for kind in [BaselineWindowKind::Last10, BaselineWindowKind::Last30] {
            let value = window(&result, kind);
            assert_eq!(value.average_damage.value, Some(1_000.0), "{kind:?}");
            assert_eq!(value.average_damage.eligible_battles, 10, "{kind:?}");
            assert_eq!(value.eligible_battles, 10, "{kind:?}");
        }
    }

    /// M4 (fixed in E1): the baseline averages the stored `assist`, which the parser now
    /// writes as spotting + tracking, the same definition the challenges use.
    #[test]
    fn baseline_assist_matches_the_challenge_definition() {
        let (_temp, database) = empty_database("m4.sqlite3");
        for index in 1..=10 {
            let mut value = battle(&format!("assist-{index:02}"), index, 1, 1_000);
            value.assist = Some(200);
            value.assist_spotting = Some(100);
            value.assist_tracking = Some(100);
            value.assist_stun = Some(50);
            database.insert_battle(&mut value, false).unwrap();
        }
        let result = player_baseline(&database, 1_000, None, Some(&snapshot())).unwrap();
        let last10 = window(&result, BaselineWindowKind::Last10);
        assert_eq!(last10.average_assist.value, Some(200.0));
    }

    /// M11 (fixed in E1, was a characterization test): average XP uses `original_xp`, never
    /// the premium-inflated `xp`; a battle without `original_xp` is not counted for XP.
    #[test]
    fn baseline_xp_uses_original_xp() {
        let (_temp, database) = empty_database("m11.sqlite3");
        for index in 1..=10 {
            let mut value = battle(&format!("xp-{index:02}"), index, 1, 1_000);
            value.xp = Some(1_000);
            value.original_xp = (index != 1).then_some(800);
            database.insert_battle(&mut value, false).unwrap();
        }
        let result = player_baseline(&database, 1_000, None, Some(&snapshot())).unwrap();
        let last10 = window(&result, BaselineWindowKind::Last10);
        assert_eq!(last10.average_xp.value, Some(800.0));
        assert_eq!(last10.average_xp.eligible_battles, 9);
    }

    /// M6 (fixed in E1, was a characterization test): WN8 and the plain averages of one window
    /// use the same battle set; a full row without a supported feature schema enters neither.
    #[test]
    fn window_averages_and_wn8_use_the_same_battle_set() {
        let (_temp, database) = empty_database("m6.sqlite3");
        for index in 1..=10 {
            let mut value = battle(&format!("schema-{index:02}"), index, 1, 1_000);
            database.insert_battle(&mut value, false).unwrap();
        }
        for index in 11..=12 {
            let mut legacy = battle(&format!("legacy-{index:02}"), index, 1, 5_000);
            legacy.feature_schema_version = None;
            legacy.source_kind = SourceKind::LocalHybrid;
            database.insert_battle(&mut legacy, false).unwrap();
        }
        let result = player_baseline(&database, 1_000, None, Some(&snapshot())).unwrap();
        let last30 = window(&result, BaselineWindowKind::Last30);
        assert_eq!(last30.eligible_battles, 10);
        assert_eq!(last30.average_damage.eligible_battles, 10);
        assert_eq!(last30.average_damage.value, Some(1_000.0));
    }

    fn session_of_three(database: &Database, vehicle_id: i64, damage: i64) -> String {
        let mut first = battle("session-one", 1_000, vehicle_id, damage);
        database.insert_battle(&mut first, true).unwrap();
        let session_id = first.session_id.clone().unwrap();
        for (id, time) in [("session-two", 1_001), ("session-three", 1_002)] {
            let mut next = battle(id, time, vehicle_id, damage);
            database.insert_battle(&mut next, true).unwrap();
        }
        session_id
    }

    fn signal_metrics(
        comparison: &SessionBaselineComparison,
    ) -> Vec<crate::models::SessionInsightMetric> {
        comparison.insights.summaries[0]
            .supporting_signals
            .iter()
            .map(|signal| signal.metric)
            .collect()
    }

    /// M5 / D7 (was CHARACTERIZATION(M5)): without an XVM snapshot the per-vehicle baseline
    /// still qualifies on damage, survival and assist, so session insights work; only WN8 is
    /// unavailable.
    #[test]
    fn session_insights_without_a_snapshot_use_the_metrics_that_need_none() {
        let (_temp, database) = database_with_history(120);
        let session_id = session_of_three(&database, 1, 2_000);

        let without = session_baseline_comparison(&database, &session_id, None).unwrap();
        let vehicle = without.vehicles.first().unwrap();
        assert_eq!(vehicle.baseline.eligible_battles, 60);
        assert_eq!(vehicle.baseline.confidence, BaselineConfidence::Reliable);
        assert_eq!(vehicle.session_eligible_battles, 3);
        assert_eq!(vehicle.session_metric_battles.wn8, 0);
        assert_eq!(vehicle.session_metric_battles.damage, 3);
        assert_eq!(
            without.insights.state,
            crate::models::SessionInsightState::Available
        );
        let metrics = signal_metrics(&without);
        assert!(metrics.contains(&crate::models::SessionInsightMetric::Damage));
        assert!(!metrics.contains(&crate::models::SessionInsightMetric::Wn8));

        let expected = snapshot();
        let with = session_baseline_comparison(&database, &session_id, Some(&expected)).unwrap();
        assert_eq!(with.vehicles.first().unwrap().baseline.eligible_battles, 60);
        assert_eq!(with.vehicles[0].session_metric_battles.wn8, 3);
        assert!(signal_metrics(&with).contains(&crate::models::SessionInsightMetric::Wn8));
    }

    /// M5 / D7: a vehicle without expected values in the snapshot keeps its damage, survival
    /// and assist comparison while a snapshot is active.
    #[test]
    fn vehicle_without_expected_values_keeps_insights_while_a_snapshot_is_active() {
        let (_temp, database) = empty_database("m5-missing-row.sqlite3");
        for index in 1..=40 {
            let mut value = battle(&format!("history-{index:03}"), index, 9_999, 1_000);
            database.insert_battle(&mut value, false).unwrap();
        }
        let session_id = session_of_three(&database, 9_999, 2_000);
        let expected = snapshot();
        let comparison =
            session_baseline_comparison(&database, &session_id, Some(&expected)).unwrap();
        assert_eq!(comparison.kind, SessionBaselineKind::SingleVehicle);
        let vehicle = &comparison.vehicles[0];
        assert_eq!(vehicle.baseline.eligible_battles, 40);
        assert_eq!(vehicle.baseline.wn8.eligible_battles, 0);
        assert_eq!(
            comparison.insights.state,
            crate::models::SessionInsightState::Available
        );
        assert_eq!(
            comparison.insights.summaries[0]
                .strongest_positive
                .as_ref()
                .unwrap()
                .metric,
            crate::models::SessionInsightMetric::Damage
        );
    }

    /// D7: qualification counts the newest D4 battles; WN8 looks further back for battles with
    /// expected values (here all 12 are scanned and only 8 have them), the plain averages use
    /// the newest 10.
    #[test]
    fn window_is_d4_battles_and_wn8_counts_only_battles_with_expected_values() {
        let (_temp, database) = empty_database("d7-window.sqlite3");
        for index in 1..=12 {
            let vehicle_id = if index <= 4 { 9_999 } else { 1 };
            let mut value = battle(&format!("mixed-{index:02}"), index, vehicle_id, 1_000);
            database.insert_battle(&mut value, false).unwrap();
        }
        let expected = snapshot();
        let result = player_baseline(&database, 1_000, None, Some(&expected)).unwrap();
        let last10 = window(&result, BaselineWindowKind::Last10);
        assert_eq!(last10.candidate_battles, 12);
        assert_eq!(last10.eligible_battles, 10);
        assert_eq!(last10.first_battle_at, Some(3));
        assert_eq!(last10.average_damage.eligible_battles, 10);
        assert_eq!(last10.wn8.eligible_battles, 8);
        let without = player_baseline(&database, 1_000, None, None).unwrap();
        assert_eq!(
            window(&without, BaselineWindowKind::Last10).candidate_battles,
            10
        );
    }
}
