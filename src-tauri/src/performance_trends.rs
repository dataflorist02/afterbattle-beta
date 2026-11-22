use crate::{
    error::AppError,
    expected_values::LoadedExpectedSnapshot,
    models::{
        Battle, PerformanceMetricTrend, PerformanceTrendMetric, PerformanceTrendPoint,
        PerformanceTrendRange, PerformanceTrendReport, PerformanceTrendSignal, TrendDirection,
        TrendScope, Wn8Calculation,
    },
    ratings::calculate_wn8,
    storage::Database,
};

pub const PERFORMANCE_TREND_VERSION: &str = "performance-trend-v1";
const MIN_DELTA_SAMPLES: usize = 5;
const MIN_SIGNAL_SAMPLES: usize = 10;
const MIN_POINT_SAMPLES: usize = 3;

impl PerformanceTrendRange {
    fn limit(self) -> usize {
        match self {
            Self::Last10 => 10,
            Self::Last30 => 30,
            Self::Last100 => 100,
        }
    }

    fn rolling_limit(self) -> usize {
        match self {
            Self::Last10 => 5,
            Self::Last30 | Self::Last100 => 10,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct WindowValue {
    value: Option<f64>,
    sample_size: usize,
    coverage: f64,
}

fn coverage(sample_size: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        sample_size as f64 / total as f64
    }
}

fn ordinary_window(battles: &[Battle], value: impl Fn(&Battle) -> Option<f64>) -> WindowValue {
    let values = battles
        .iter()
        .filter_map(value)
        .filter(|value| value.is_finite() && *value >= 0.0)
        .collect::<Vec<_>>();
    WindowValue {
        value: (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64),
        sample_size: values.len(),
        coverage: coverage(values.len(), battles.len()),
    }
}

fn metric_window(
    metric: PerformanceTrendMetric,
    battles: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
) -> WindowValue {
    match metric {
        PerformanceTrendMetric::Wn8 => {
            let Some(expected) = expected else {
                return WindowValue {
                    value: None,
                    sample_size: 0,
                    coverage: 0.0,
                };
            };
            match calculate_wn8(battles, expected) {
                Wn8Calculation::Available(result) => WindowValue {
                    value: Some(result.value),
                    sample_size: result.eligible_battles,
                    coverage: result.coverage_ratio,
                },
                Wn8Calculation::Unavailable(result) => WindowValue {
                    value: None,
                    sample_size: result.eligible_battles,
                    coverage: result.coverage_ratio,
                },
            }
        }
        PerformanceTrendMetric::Damage => {
            ordinary_window(battles, |battle| battle.damage.map(|value| value as f64))
        }
        PerformanceTrendMetric::Survival => ordinary_window(battles, |battle| {
            battle.survived.map(|value| if value { 100.0 } else { 0.0 })
        }),
        PerformanceTrendMetric::WinRate => ordinary_window(battles, |battle| {
            battle.outcome.win_contribution().map(|value| value * 100.0)
        }),
    }
}

fn threshold(metric: PerformanceTrendMetric) -> f64 {
    match metric {
        PerformanceTrendMetric::Wn8 | PerformanceTrendMetric::Damage => 10.0,
        PerformanceTrendMetric::Survival | PerformanceTrendMetric::WinRate => 10.0,
    }
}

fn points(
    metric: PerformanceTrendMetric,
    battles: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
    rolling_limit: usize,
    vehicle_id: Option<i64>,
) -> Vec<PerformanceTrendPoint> {
    (0..battles.len())
        .map(|index| {
            let start = (index + 1).saturating_sub(rolling_limit);
            let slice = &battles[start..=index];
            let window = metric_window(metric, slice, expected);
            PerformanceTrendPoint {
                ordinal: index + 1,
                battle_time: battles[index].battle_time,
                value: (window.sample_size >= MIN_POINT_SAMPLES)
                    .then_some(window.value)
                    .flatten(),
                sample_size: window.sample_size,
                coverage: window.coverage,
                vehicle_id,
                damage: battles[index].damage,
                map_name: battles[index].map_name.clone(),
                vehicle_key: battles[index].vehicle_key.clone(),
                tank_name: battles[index].tank_name.clone(),
            }
        })
        .collect()
}

fn trend(
    metric: PerformanceTrendMetric,
    current: &[Battle],
    previous: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
    rolling_limit: usize,
    vehicle_id: Option<i64>,
) -> PerformanceMetricTrend {
    let current_value = metric_window(metric, current, expected);
    let previous_value = metric_window(metric, previous, expected);
    let comparable = current_value.sample_size >= MIN_DELTA_SAMPLES
        && previous_value.sample_size >= MIN_DELTA_SAMPLES;
    let absolute_delta = comparable
        .then(|| Some(current_value.value? - previous_value.value?))
        .flatten();
    let relative_delta = if matches!(
        metric,
        PerformanceTrendMetric::Wn8 | PerformanceTrendMetric::Damage
    ) && comparable
    {
        previous_value
            .value
            .filter(|value| *value > 0.0)
            .and_then(|previous| Some((current_value.value? - previous) / previous * 100.0))
    } else {
        None
    };
    let percentage_point_delta = if matches!(
        metric,
        PerformanceTrendMetric::Survival | PerformanceTrendMetric::WinRate
    ) && comparable
    {
        absolute_delta
    } else {
        None
    };
    let comparison_delta = match metric {
        PerformanceTrendMetric::Wn8 | PerformanceTrendMetric::Damage => relative_delta,
        PerformanceTrendMetric::Survival | PerformanceTrendMetric::WinRate => {
            percentage_point_delta
        }
    };
    let direction = match comparison_delta {
        None => TrendDirection::Unavailable,
        Some(delta) if delta >= threshold(metric) => TrendDirection::Up,
        Some(delta) if delta <= -threshold(metric) => TrendDirection::Down,
        Some(_) => TrendDirection::Neutral,
    };
    let meaningful = matches!(direction, TrendDirection::Up | TrendDirection::Down)
        && current_value.sample_size >= MIN_SIGNAL_SAMPLES
        && previous_value.sample_size >= MIN_SIGNAL_SAMPLES;
    PerformanceMetricTrend {
        metric,
        current_value: current_value.value,
        previous_value: previous_value.value,
        absolute_delta,
        relative_delta,
        percentage_point_delta,
        direction,
        current_sample_size: current_value.sample_size,
        previous_sample_size: previous_value.sample_size,
        current_coverage: current_value.coverage,
        previous_coverage: previous_value.coverage,
        meaningful,
        points: points(metric, current, expected, rolling_limit, vehicle_id),
    }
}

fn signal(metric: &PerformanceMetricTrend) -> Option<PerformanceTrendSignal> {
    if !metric.meaningful || !matches!(metric.direction, TrendDirection::Up | TrendDirection::Down)
    {
        return None;
    }
    let delta = match metric.metric {
        PerformanceTrendMetric::Wn8 | PerformanceTrendMetric::Damage => metric.relative_delta?,
        PerformanceTrendMetric::Survival | PerformanceTrendMetric::WinRate => {
            metric.percentage_point_delta?
        }
    };
    Some(PerformanceTrendSignal {
        metric: metric.metric,
        direction: metric.direction,
        current_value: metric.current_value?,
        previous_value: metric.previous_value?,
        delta,
        normalized_strength: (delta / threshold(metric.metric)).abs(),
    })
}

fn strongest(
    metrics: &[PerformanceMetricTrend],
    direction: TrendDirection,
) -> Option<PerformanceTrendSignal> {
    let mut strongest = None::<PerformanceTrendSignal>;
    for metric_kind in [
        PerformanceTrendMetric::Wn8,
        PerformanceTrendMetric::Damage,
        PerformanceTrendMetric::Survival,
    ] {
        let candidate = metrics
            .iter()
            .find(|metric| metric.metric == metric_kind)
            .and_then(signal)
            .filter(|candidate| candidate.direction == direction);
        if let Some(candidate) = candidate
            && strongest
                .as_ref()
                .is_none_or(|current| candidate.normalized_strength > current.normalized_strength)
        {
            strongest = Some(candidate);
        }
    }
    strongest
}

pub(crate) fn performance_trends(
    database: &Database,
    range: PerformanceTrendRange,
    vehicle_id: Option<i64>,
    expected: Option<&LoadedExpectedSnapshot>,
    generated_at: i64,
) -> Result<PerformanceTrendReport, AppError> {
    if vehicle_id.is_some_and(|id| id <= 0) {
        return Err(AppError::InvalidApiResponse);
    }
    let limit = range.limit();
    let rows = database.performance_trend_battles(vehicle_id, limit * 2)?;
    let current_count = rows.len().min(limit);
    let mut current = rows[..current_count].to_vec();
    let mut previous = rows[current_count..].to_vec();
    current.reverse();
    previous.reverse();
    let metrics = [
        PerformanceTrendMetric::Wn8,
        PerformanceTrendMetric::Damage,
        PerformanceTrendMetric::Survival,
        PerformanceTrendMetric::WinRate,
    ]
    .into_iter()
    .map(|metric| {
        trend(
            metric,
            &current,
            &previous,
            expected,
            range.rolling_limit(),
            vehicle_id,
        )
    })
    .collect::<Vec<_>>();
    Ok(PerformanceTrendReport {
        version: PERFORMANCE_TREND_VERSION.to_owned(),
        range,
        scope: vehicle_id.map_or(TrendScope::Global, TrendScope::Vehicle),
        current_battle_count: current.len(),
        previous_battle_count: previous.len(),
        highlight: strongest(&metrics, TrendDirection::Up),
        lowlight: strongest(&metrics, TrendDirection::Down),
        metrics,
        generated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expected_values::{
            CACHE_SCHEMA_VERSION, ExpectedSnapshotManifest, ExpectedValuesRow,
            ExpectedValuesSource, SnapshotValidationStatus, VALIDATOR_VERSION,
            XVM_WARGAMING_SOURCE_URL,
        },
        models::{BattleOutcome, DataQuality, SourceKind},
    };
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn expected() -> LoadedExpectedSnapshot {
        let rows = [
            ExpectedValuesRow {
                vehicle_id: 1,
                exp_damage: 1_000.0,
                exp_frags: 1.0,
                exp_spots: 1.0,
                exp_defense: 1.0,
                exp_win_rate: 50.0,
            },
            ExpectedValuesRow {
                vehicle_id: 2,
                exp_damage: 2_000.0,
                exp_frags: 1.5,
                exp_spots: 1.2,
                exp_defense: 0.8,
                exp_win_rate: 51.0,
            },
        ];
        LoadedExpectedSnapshot {
            manifest: ExpectedSnapshotManifest {
                cache_schema_version: CACHE_SCHEMA_VERSION,
                validator_version: VALIDATOR_VERSION,
                source: ExpectedValuesSource::XvmWargaming,
                source_url: XVM_WARGAMING_SOURCE_URL.to_owned(),
                downloaded_at: 1,
                source_version: "test".to_owned(),
                source_date: "2026-01-01".to_owned(),
                sha256: "a".repeat(64),
                byte_length: 1,
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

    fn battle(id: &str, time: i64, vehicle_id: i64, damage: i64) -> Battle {
        Battle {
            arena_id: id.to_owned(),
            arena_unique_id: None,
            battle_time: time,
            battle_time_source: Some("metadata".to_owned()),
            vehicle_key: format!("vehicle-{vehicle_id}"),
            wg_tank_id: Some(vehicle_id),
            tank_name: None,
            map_name: None,
            map_key: None,
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
            assist: None,
            assist_spotting: Some(0),
            assist_tracking: Some(0),
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
            survived: Some(time % 2 == 0),
            xp: None,
            original_xp: None,
            duration_seconds: None,
            session_id: None,
        }
    }

    fn database_with(count: usize) -> (tempfile::TempDir, Database) {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("trends.sqlite3")).unwrap();
        for index in 1..=count {
            let mut value = battle(
                &format!("battle-{index:03}"),
                index as i64,
                if index % 2 == 0 { 1 } else { 2 },
                index as i64 * 10,
            );
            database.insert_battle(&mut value, false).unwrap();
        }
        (directory, database)
    }

    fn metric(
        report: &PerformanceTrendReport,
        kind: PerformanceTrendMetric,
    ) -> &PerformanceMetricTrend {
        report
            .metrics
            .iter()
            .find(|metric| metric.metric == kind)
            .unwrap()
    }

    #[test]
    fn ranges_select_two_non_overlapping_stable_windows() {
        let (_directory, database) = database_with(220);
        for (range, expected_count) in [
            (PerformanceTrendRange::Last10, 10),
            (PerformanceTrendRange::Last30, 30),
            (PerformanceTrendRange::Last100, 100),
        ] {
            let report = performance_trends(&database, range, None, Some(&expected()), 7).unwrap();
            assert_eq!(report.current_battle_count, expected_count);
            assert_eq!(report.previous_battle_count, expected_count);
            let damage = metric(&report, PerformanceTrendMetric::Damage);
            assert_eq!(damage.points.len(), expected_count);
            assert!(damage.points.windows(2).all(|pair| {
                pair[0].battle_time < pair[1].battle_time && pair[0].ordinal + 1 == pair[1].ordinal
            }));
            assert_eq!(damage.current_sample_size, expected_count);
            assert_eq!(damage.previous_sample_size, expected_count);
        }
        let report = performance_trends(
            &database,
            PerformanceTrendRange::Last10,
            None,
            Some(&expected()),
            7,
        )
        .unwrap();
        assert_eq!(
            metric(&report, PerformanceTrendMetric::Damage).current_value,
            Some(2_155.0)
        );
        assert_eq!(
            metric(&report, PerformanceTrendMetric::Damage).previous_value,
            Some(2_055.0)
        );
    }

    #[test]
    fn quality_time_and_vehicle_filters_are_fail_closed() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("filters.sqlite3")).unwrap();
        for index in 1..=8 {
            let mut value = battle(&format!("valid-{index}"), index, 1, 1_000);
            database.insert_battle(&mut value, false).unwrap();
        }
        let mut partial = battle("partial", 20, 1, 99_000);
        partial.data_quality = DataQuality::Partial;
        database.insert_battle(&mut partial, false).unwrap();
        let mut ambiguous = battle("ambiguous", 21, 1, 99_000);
        ambiguous.data_quality = DataQuality::Ambiguous;
        database.insert_battle(&mut ambiguous, false).unwrap();
        let mut uncertain = battle("uncertain", 22, 1, 99_000);
        uncertain.battle_time_source = Some("file_mtime".to_owned());
        database.insert_battle(&mut uncertain, false).unwrap();
        let mut other = battle("other", 23, 2, 2_000);
        database.insert_battle(&mut other, false).unwrap();

        let global = performance_trends(
            &database,
            PerformanceTrendRange::Last10,
            None,
            Some(&expected()),
            1,
        )
        .unwrap();
        assert_eq!(global.current_battle_count, 9);
        let scoped = performance_trends(
            &database,
            PerformanceTrendRange::Last10,
            Some(1),
            Some(&expected()),
            1,
        )
        .unwrap();
        assert_eq!(scoped.current_battle_count, 8);
        assert_eq!(scoped.scope, TrendScope::Vehicle(1));
        assert!(
            metric(&scoped, PerformanceTrendMetric::Damage)
                .points
                .iter()
                .all(|point| point.vehicle_id == Some(1))
        );
        assert!(
            performance_trends(&database, PerformanceTrendRange::Last10, Some(0), None, 1).is_err()
        );
    }

    #[test]
    fn missing_values_remain_missing_and_never_become_zero() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("missing.sqlite3")).unwrap();
        for index in 1..=10 {
            let mut value = battle(&format!("battle-{index}"), index, 1, 1_000);
            if index > 5 {
                value.damage = None;
                value.survived = None;
                value.outcome = BattleOutcome::Unknown;
            }
            database.insert_battle(&mut value, false).unwrap();
        }
        let report =
            performance_trends(&database, PerformanceTrendRange::Last10, None, None, 1).unwrap();
        let damage = metric(&report, PerformanceTrendMetric::Damage);
        assert_eq!(damage.current_sample_size, 5);
        assert_eq!(damage.current_value, Some(1_000.0));
        assert_eq!(damage.current_coverage, 0.5);
        assert_eq!(damage.points.last().unwrap().value, None);
        assert_eq!(
            metric(&report, PerformanceTrendMetric::Survival).current_sample_size,
            5
        );
        assert_eq!(
            metric(&report, PerformanceTrendMetric::WinRate).current_sample_size,
            5
        );
        assert_eq!(
            metric(&report, PerformanceTrendMetric::Wn8).current_value,
            None
        );
    }

    #[test]
    fn wn8_summary_and_rolling_points_reuse_aggregate_engine() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("wn8.sqlite3")).unwrap();
        let mut rows = Vec::new();
        for index in 1..=10 {
            let mut value = battle(
                &format!("battle-{index}"),
                index,
                if index % 2 == 0 { 1 } else { 2 },
                if index % 2 == 0 { 2_000 } else { 1_000 },
            );
            database.insert_battle(&mut value, false).unwrap();
            rows.push(value);
        }
        let snapshot = expected();
        let report = performance_trends(
            &database,
            PerformanceTrendRange::Last10,
            None,
            Some(&snapshot),
            1,
        )
        .unwrap();
        let wn8 = metric(&report, PerformanceTrendMetric::Wn8);
        let Wn8Calculation::Available(expected_total) = calculate_wn8(&rows, &snapshot) else {
            panic!("synthetic rows must qualify");
        };
        assert_eq!(wn8.current_value, Some(expected_total.value));
        assert_eq!(wn8.current_sample_size, 10);
        assert_eq!(wn8.points[0].value, None);
        assert_eq!(wn8.points[1].value, None);
        let Wn8Calculation::Available(first_three) = calculate_wn8(&rows[..3], &snapshot) else {
            panic!("first rolling window must qualify");
        };
        assert_eq!(wn8.points[2].value, Some(first_three.value));
    }

    #[test]
    fn rolling_damage_survival_and_win_rate_start_at_three_samples() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("rolling.sqlite3")).unwrap();
        for index in 1..=6 {
            let mut value = battle(&format!("battle-{index}"), index, 1, index * 100);
            value.outcome = if index <= 3 {
                BattleOutcome::Victory
            } else {
                BattleOutcome::Defeat
            };
            value.survived = Some(index % 2 == 0);
            database.insert_battle(&mut value, false).unwrap();
        }
        let report = performance_trends(
            &database,
            PerformanceTrendRange::Last10,
            None,
            Some(&expected()),
            1,
        )
        .unwrap();
        let damage = metric(&report, PerformanceTrendMetric::Damage);
        assert_eq!(damage.points[0].value, None);
        assert_eq!(damage.points[1].value, None);
        assert_eq!(damage.points[2].value, Some(200.0));
        assert_eq!(damage.points[5].value, Some(400.0));
        assert_eq!(
            metric(&report, PerformanceTrendMetric::Survival).points[2].value,
            Some(100.0 / 3.0)
        );
        assert_eq!(
            metric(&report, PerformanceTrendMetric::WinRate).points[2].value,
            Some(100.0)
        );
    }

    #[test]
    fn points_carry_their_own_battle_for_the_chart_reading() {
        // E14 polish 2: the chart's reading names the battle at each point (its damage, map
        // and tank), next to the rolling value the line draws.
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("reading.sqlite3")).unwrap();
        for index in 1..=4 {
            let mut value = battle(&format!("battle-{index}"), index, 7, index * 100);
            value.map_name = Some(format!("Synthetic Map {index}"));
            value.tank_name = (index != 2).then(|| "Synthetic Tank".to_owned());
            if index == 3 {
                value.damage = None;
            }
            database.insert_battle(&mut value, false).unwrap();
        }
        let report = performance_trends(
            &database,
            PerformanceTrendRange::Last10,
            Some(7),
            Some(&expected()),
            1,
        )
        .unwrap();
        for kind in [
            PerformanceTrendMetric::Damage,
            PerformanceTrendMetric::Wn8,
            PerformanceTrendMetric::Survival,
        ] {
            let points = &metric(&report, kind).points;
            assert_eq!(
                points.iter().map(|point| point.damage).collect::<Vec<_>>(),
                vec![Some(100), Some(200), None, Some(400)]
            );
            assert_eq!(points[0].map_name.as_deref(), Some("Synthetic Map 1"));
            assert_eq!(points[3].map_name.as_deref(), Some("Synthetic Map 4"));
            assert_eq!(points[0].tank_name.as_deref(), Some("Synthetic Tank"));
            assert_eq!(points[1].tank_name, None);
            assert!(points.iter().all(|point| point.vehicle_key == "vehicle-7"));
        }
        // The line's own value stays the rolling one.
        let damage = metric(&report, PerformanceTrendMetric::Damage);
        assert_eq!(damage.points[3].value, Some((100.0 + 200.0 + 400.0) / 3.0));
    }

    #[test]
    fn draw_counts_in_win_rate_and_wn8_trends_while_unknown_reduces_coverage() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("draw-trends.sqlite3")).unwrap();
        for (index, outcome) in [
            BattleOutcome::Victory,
            BattleOutcome::Draw,
            BattleOutcome::Defeat,
            BattleOutcome::Unknown,
        ]
        .into_iter()
        .enumerate()
        {
            let mut value = battle(&format!("battle-{index}"), index as i64 + 1, 1, 1_000);
            value.outcome = outcome;
            database.insert_battle(&mut value, false).unwrap();
        }

        let report = performance_trends(
            &database,
            PerformanceTrendRange::Last10,
            None,
            Some(&expected()),
            1,
        )
        .unwrap();
        let win_rate = metric(&report, PerformanceTrendMetric::WinRate);
        assert!((win_rate.current_value.unwrap() - 100.0 / 3.0).abs() < 1e-12);
        assert_eq!(win_rate.current_sample_size, 3);
        assert_eq!(win_rate.current_coverage, 0.75);
        assert!((win_rate.points[2].value.unwrap() - 100.0 / 3.0).abs() < 1e-12);
        assert_eq!(win_rate.points[3].sample_size, 3);
        assert_eq!(win_rate.points[3].coverage, 0.75);

        let wn8 = metric(&report, PerformanceTrendMetric::Wn8);
        assert_eq!(wn8.current_sample_size, 3);
        assert_eq!(wn8.current_coverage, 0.75);
        assert!(wn8.current_value.is_some());
    }

    #[test]
    fn current_vs_previous_win_rate_treats_draw_as_known_zero() {
        let outcomes = |prefix: &str, values: &[BattleOutcome]| {
            values
                .iter()
                .enumerate()
                .map(|(index, outcome)| {
                    let mut value = battle(&format!("{prefix}-{index}"), index as i64, 1, 1_000);
                    value.outcome = *outcome;
                    value
                })
                .collect::<Vec<_>>()
        };
        let current = outcomes(
            "current",
            &[
                BattleOutcome::Draw,
                BattleOutcome::Defeat,
                BattleOutcome::Draw,
                BattleOutcome::Defeat,
                BattleOutcome::Draw,
            ],
        );
        let previous = outcomes(
            "previous",
            &[
                BattleOutcome::Victory,
                BattleOutcome::Draw,
                BattleOutcome::Victory,
                BattleOutcome::Draw,
                BattleOutcome::Victory,
            ],
        );

        let result = trend(
            PerformanceTrendMetric::WinRate,
            &current,
            &previous,
            None,
            5,
            None,
        );
        assert_eq!(result.current_value, Some(0.0));
        assert_eq!(result.previous_value, Some(60.0));
        assert_eq!(result.percentage_point_delta, Some(-60.0));
        assert_eq!(result.current_sample_size, 5);
        assert_eq!(result.previous_sample_size, 5);
        assert_eq!(result.current_coverage, 1.0);
        assert_eq!(result.previous_coverage, 1.0);
    }

    fn synthetic_trend(
        metric: PerformanceTrendMetric,
        current: f64,
        previous: f64,
        samples: usize,
    ) -> PerformanceMetricTrend {
        let is_rate = matches!(
            metric,
            PerformanceTrendMetric::Survival | PerformanceTrendMetric::WinRate
        );
        let absolute = current - previous;
        let relative = (!is_rate).then_some(absolute / previous * 100.0);
        let percentage_points = is_rate.then_some(absolute);
        let delta = relative.or(percentage_points).unwrap();
        PerformanceMetricTrend {
            metric,
            current_value: Some(current),
            previous_value: Some(previous),
            absolute_delta: Some(absolute),
            relative_delta: relative,
            percentage_point_delta: percentage_points,
            direction: if delta >= 10.0 {
                TrendDirection::Up
            } else {
                TrendDirection::Down
            },
            current_sample_size: samples,
            previous_sample_size: samples,
            current_coverage: 1.0,
            previous_coverage: 1.0,
            meaningful: samples >= 10,
            points: Vec::new(),
        }
    }

    #[test]
    fn highlights_require_ten_samples_ignore_win_rate_and_use_stable_ties() {
        let insufficient = synthetic_trend(PerformanceTrendMetric::Wn8, 1_200.0, 1_000.0, 9);
        assert!(signal(&insufficient).is_none());
        let wn8 = synthetic_trend(PerformanceTrendMetric::Wn8, 1_200.0, 1_000.0, 10);
        let damage = synthetic_trend(PerformanceTrendMetric::Damage, 1_200.0, 1_000.0, 10);
        let survival = synthetic_trend(PerformanceTrendMetric::Survival, 40.0, 50.0, 10);
        let win = synthetic_trend(PerformanceTrendMetric::WinRate, 90.0, 50.0, 10);
        assert_eq!(
            strongest(&[damage, wn8, win], TrendDirection::Up)
                .unwrap()
                .metric,
            PerformanceTrendMetric::Wn8
        );
        assert_eq!(
            strongest(&[survival], TrendDirection::Down).unwrap().metric,
            PerformanceTrendMetric::Survival
        );
    }

    #[test]
    fn empty_report_and_private_dto_are_safe() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("empty.sqlite3")).unwrap();
        let report = performance_trends(
            &database,
            PerformanceTrendRange::Last30,
            None,
            Some(&expected()),
            123,
        )
        .unwrap();
        assert_eq!(
            (report.current_battle_count, report.previous_battle_count),
            (0, 0)
        );
        assert!(report.highlight.is_none() && report.lowlight.is_none());
        assert!(
            report
                .metrics
                .iter()
                .all(|metric| metric.current_value.is_none())
        );
        let json = serde_json::to_string(&report).unwrap();
        for private in [
            "arenaId",
            "battleId",
            "replay",
            "accountId",
            "suggestionId",
            "sha256",
        ] {
            assert!(!json.contains(private));
        }
    }

    #[test]
    fn storage_read_is_hard_bounded_to_two_hundred_rows() {
        let (_directory, database) = database_with(250);
        assert_eq!(
            database
                .performance_trend_battles(None, 10_000)
                .unwrap()
                .len(),
            200
        );
    }

    // ---- E0 characterization tests --------------------------------------------------------
    // They pin TODAY's behavior. Tests tagged CHARACTERIZATION(<id>) freeze a known defect from
    // the author's internal 2026-11 audit and must be inverted by the stage that fixes it.

    /// M2 (fixed in E1, was a characterization test): trend windows and points contain random
    /// battles only.
    #[test]
    fn trends_ignore_non_random_battles() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("m2.sqlite3")).unwrap();
        for index in 1..=5 {
            let mut random = battle(&format!("random-{index}"), index, 1, 1_000);
            database.insert_battle(&mut random, false).unwrap();
        }
        for index in 6..=10 {
            let mut other = battle(&format!("other-{index}"), index, 1, 3_000);
            other.battle_type = Some("training".to_owned());
            other.battle_pace.battle_type_raw = Some(2);
            other.battle_pace.bonus_type_raw = Some(2);
            database.insert_battle(&mut other, false).unwrap();
        }
        let report = performance_trends(
            &database,
            PerformanceTrendRange::Last10,
            None,
            Some(&expected()),
            1,
        )
        .unwrap();
        assert_eq!(report.current_battle_count, 5);
        let damage = metric(&report, PerformanceTrendMetric::Damage);
        assert_eq!(damage.current_sample_size, 5);
        assert_eq!(damage.current_value, Some(1_000.0));
        assert_eq!(damage.points.len(), 5);
    }

    /// Pins that win rate is a reported metric with draws counted as known non-wins, and that
    /// it stays out of highlights (D12).
    #[test]
    fn characterization_trend_win_rate_is_reported_but_never_a_highlight() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("wr.sqlite3")).unwrap();
        for index in 1..=20 {
            let mut value = battle(&format!("wr-{index:02}"), index, 1, 1_000);
            value.outcome = if index > 10 {
                BattleOutcome::Victory
            } else {
                BattleOutcome::Defeat
            };
            database.insert_battle(&mut value, false).unwrap();
        }
        let report =
            performance_trends(&database, PerformanceTrendRange::Last10, None, None, 1).unwrap();
        let win_rate = metric(&report, PerformanceTrendMetric::WinRate);
        assert_eq!(win_rate.current_value, Some(100.0));
        assert_eq!(win_rate.previous_value, Some(0.0));
        assert!(
            report
                .highlight
                .iter()
                .chain(report.lowlight.iter())
                .all(|signal| signal.metric != PerformanceTrendMetric::WinRate)
        );
    }
}
