//! Deterministic session story for the map-led screens (E5-flare §2, §4, §5): the headline
//! sentence, the session moments, the best battle, the player's record on one map and the maps
//! of recent sessions. Plain rules with fixed thresholds (spec `docs/specs/26-e5-flare.md`); no
//! model, no randomness. Only counted battles (D4) enter any number.

use std::collections::HashMap;

use serde::Serialize;

use crate::models::{
    Battle, BattleOutcome, SessionBaselineComparison, SessionInsight, SessionInsightDirection,
    SessionInsightMetric, stats_eligibility,
};

/// Same thresholds as the session insights (D7, `session_insights.rs`).
pub(crate) const MIN_BASELINE_BATTLES: usize = 30;
pub(crate) const MIN_SESSION_BATTLES: usize = 3;
/// A session whose damage on its tanks differs by 10% or more is "strong" or "tough".
pub(crate) const HEADLINE_THRESHOLD_PERCENT: f64 = 10.0;
/// The damage trend needs 6 counted battles with damage; it compares the average of the first
/// third with the last third and shows when they differ by 20% or more.
pub(crate) const MIN_TREND_BATTLES: usize = 6;
pub(crate) const TREND_THRESHOLD_PERCENT: f64 = 20.0;
/// The best battle is named from 2 counted battles with damage.
pub(crate) const MIN_BEST_BATTLES: usize = 2;
/// Map thumbnails of one session card in History.
pub(crate) const MAX_SESSION_MAPS: usize = 4;
/// Sessions asked for at once.
pub(crate) const MAX_SESSION_IDS: usize = 50;

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HeadlineKind {
    /// Damage on the session's tanks 10% or more above the player's average.
    Strong,
    /// Within 10% of the average.
    Calm,
    /// 10% or more below the average.
    Tough,
    /// Too little data: "Session: 9 random battles".
    Neutral,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionHeadline {
    pub kind: HeadlineKind,
    /// Whole percent, absolute; present for strong and tough sessions.
    pub percent: Option<u32>,
    /// Counted battles of the session.
    pub battles: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BattleRef {
    pub battle_id: String,
    pub damage: i64,
    pub map_key: Option<String>,
    pub map_name: Option<String>,
    pub vehicle_key: String,
    pub tank_name: Option<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum SessionMoment {
    /// Damage of the last third against the first third of the session. `from`/`to` are the
    /// first and the last battle when `every_battle`, else the averages of the two thirds.
    DamageTrend {
        rising: bool,
        from: f64,
        to: f64,
        /// Battles in each third.
        battles_per_third: usize,
        /// Every battle had more (rising) or less (falling) damage than the one before.
        every_battle: bool,
    },
    /// The tank with the strongest negative insight of the session ("To improve"), with a
    /// battle of it whose map is the card's background.
    WeakestTank {
        vehicle_id: Option<i64>,
        vehicle_key: Option<String>,
        tank_name: Option<String>,
        metric: SessionInsightMetric,
        current: f64,
        baseline: f64,
        /// Survival only: survived and counted session battles on that tank.
        survived: Option<usize>,
        battles: usize,
        map_key: Option<String>,
        map_name: Option<String>,
    },
    BestBattle {
        battle: BattleRef,
    },
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionStory {
    pub headline: SessionHeadline,
    /// Highest damage counted battle; its map is the header background and its tile is framed.
    pub best_battle: Option<BattleRef>,
    pub moments: Vec<SessionMoment>,
}

/// Percent change rounded to a millionth, so thresholds such as exactly 10% or 20% are not
/// missed by floating-point error.
fn percent_change(current: f64, baseline: f64) -> f64 {
    ((current - baseline) / baseline * 100.0 * 1e6).round() / 1e6
}

/// Session damage on the compared tanks against the player's average on them (same weights).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DamageReference {
    pub current: f64,
    pub baseline: f64,
    /// Percent change, rounded to a millionth (see `percent_change`).
    pub change: f64,
}

/// Damage on the session's tanks with 30 or more baseline battles, weighted by session battles,
/// against their baselines with the same weights (the frontend `sessionDamageReference`).
pub(crate) fn damage_change(comparison: &SessionBaselineComparison) -> Option<f64> {
    damage_reference(comparison).map(|reference| reference.change)
}

/// The numbers behind `damage_change`: the headline and the session summary read the same ones.
pub(crate) fn damage_reference(comparison: &SessionBaselineComparison) -> Option<DamageReference> {
    let (mut current, mut baseline, mut weight) = (0.0, 0.0, 0usize);
    for vehicle in &comparison.vehicles {
        let reference = &vehicle.baseline.average_damage;
        let battles = vehicle.session_metric_battles.damage;
        let (Some(own), Some(average)) = (vehicle.damage.current_value, reference.value) else {
            continue;
        };
        if reference.eligible_battles < MIN_BASELINE_BATTLES || battles == 0 {
            continue;
        }
        current += own * battles as f64;
        baseline += average * battles as f64;
        weight += battles;
    }
    (weight >= MIN_SESSION_BATTLES && baseline > 0.0).then(|| DamageReference {
        current: current / weight as f64,
        baseline: baseline / weight as f64,
        change: percent_change(current, baseline),
    })
}

pub(crate) fn headline(
    comparison: Option<&SessionBaselineComparison>,
    counted_battles: usize,
) -> SessionHeadline {
    let change = comparison.and_then(damage_change);
    let (kind, percent) = match change {
        Some(change) if change >= HEADLINE_THRESHOLD_PERCENT => {
            (HeadlineKind::Strong, Some(change.round() as u32))
        }
        Some(change) if change <= -HEADLINE_THRESHOLD_PERCENT => {
            (HeadlineKind::Tough, Some((-change).round() as u32))
        }
        Some(_) => (HeadlineKind::Calm, None),
        None => (HeadlineKind::Neutral, None),
    };
    SessionHeadline {
        kind,
        percent,
        battles: counted_battles,
    }
}

fn battle_ref(battle: &Battle, damage: i64) -> BattleRef {
    BattleRef {
        battle_id: battle.arena_id.clone(),
        damage,
        map_key: battle.map_key.clone(),
        map_name: battle.map_name.clone(),
        vehicle_key: battle.vehicle_key.clone(),
        tank_name: battle.tank_name.clone(),
    }
}

/// Counted battles with damage, oldest first (ties by id, so the order is stable).
fn counted_with_damage(battles: &[Battle]) -> Vec<(&Battle, i64)> {
    let mut counted = battles
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .filter_map(|battle| battle.damage.map(|damage| (battle, damage)))
        .collect::<Vec<_>>();
    counted.sort_by(|(left, _), (right, _)| {
        left.battle_time
            .cmp(&right.battle_time)
            .then_with(|| left.arena_id.cmp(&right.arena_id))
    });
    counted
}

pub(crate) fn best_battle(battles: &[Battle]) -> Option<BattleRef> {
    let counted = counted_with_damage(battles);
    if counted.len() < MIN_BEST_BATTLES {
        return None;
    }
    // The first of equal maxima wins: `max_by_key` keeps the last, so search in reverse.
    counted
        .iter()
        .rev()
        .max_by_key(|(_, damage)| *damage)
        .map(|(battle, damage)| battle_ref(battle, *damage))
}

pub(crate) fn damage_trend(battles: &[Battle]) -> Option<SessionMoment> {
    let counted = counted_with_damage(battles);
    if counted.len() < MIN_TREND_BATTLES {
        return None;
    }
    let third = counted.len() / 3;
    let average = |part: &[(&Battle, i64)]| {
        part.iter().map(|(_, damage)| *damage as f64).sum::<f64>() / part.len() as f64
    };
    let from = average(&counted[..third]);
    let to = average(&counted[counted.len() - third..]);
    if from <= 0.0 {
        return None;
    }
    let change = percent_change(to, from);
    if change.abs() < TREND_THRESHOLD_PERCENT {
        return None;
    }
    let rising = change > 0.0;
    let every_battle = counted.windows(2).all(|pair| {
        if rising {
            pair[1].1 > pair[0].1
        } else {
            pair[1].1 < pair[0].1
        }
    });
    // E5-flare-fix: when every battle rose (fell), the card names the first and the last battle,
    // the same numbers as on the strip; otherwise the averages of the thirds.
    let (from, to) = if every_battle {
        (counted[0].1 as f64, counted[counted.len() - 1].1 as f64)
    } else {
        (from, to)
    };
    Some(SessionMoment::DamageTrend {
        rising,
        from,
        to,
        battles_per_third: third,
        every_battle,
    })
}

/// The strongest negative insight of the session (already gated by D7: 30 baseline and 3
/// session battles), with the battle on that tank that shows it best.
pub(crate) fn weakest_tank(
    comparison: &SessionBaselineComparison,
    battles: &[Battle],
) -> Option<SessionMoment> {
    let (summary, signal) = comparison
        .insights
        .summaries
        .iter()
        .filter_map(|summary| {
            let signal: &SessionInsight = summary.strongest_negative.as_ref()?;
            (signal.direction == SessionInsightDirection::Negative).then_some((summary, signal))
        })
        .max_by(|(left_summary, left), (right_summary, right)| {
            left.normalized_strength
                .total_cmp(&right.normalized_strength)
                .then_with(|| right_summary.vehicle_id.cmp(&left_summary.vehicle_id))
        })?;
    let own = battles
        .iter()
        .filter(|battle| stats_eligibility(battle) && battle.wg_tank_id == summary.vehicle_id)
        .collect::<Vec<_>>();
    if own.is_empty() {
        return None;
    }
    let shown = own
        .iter()
        .filter(|battle| {
            signal.metric != SessionInsightMetric::Survival || battle.survived == Some(false)
        })
        .min_by_key(|battle| (battle.damage.unwrap_or(i64::MAX), battle.battle_time))
        .or_else(|| own.first())?;
    let survived = (signal.metric == SessionInsightMetric::Survival).then(|| {
        own.iter()
            .filter(|battle| battle.survived == Some(true))
            .count()
    });
    Some(SessionMoment::WeakestTank {
        vehicle_id: summary.vehicle_id,
        vehicle_key: summary.vehicle_key.clone(),
        tank_name: summary.tank_name.clone(),
        metric: signal.metric,
        current: signal.current_value,
        baseline: signal.baseline_value,
        survived,
        battles: own.len(),
        map_key: shown.map_key.clone(),
        map_name: shown.map_name.clone(),
    })
}

pub(crate) fn session_story(
    battles: &[Battle],
    comparison: Option<&SessionBaselineComparison>,
) -> SessionStory {
    let counted = battles
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .count();
    let best = best_battle(battles);
    let mut moments = Vec::new();
    if let Some(trend) = damage_trend(battles) {
        moments.push(trend);
    }
    if let Some(weakest) = comparison.and_then(|comparison| weakest_tank(comparison, battles)) {
        moments.push(weakest);
    }
    if let Some(best) = &best {
        moments.push(SessionMoment::BestBattle {
            battle: best.clone(),
        });
    }
    SessionStory {
        headline: headline(comparison, counted),
        best_battle: best,
        moments,
    }
}

/// Every counted battle of the player on one map, any tank (Battle screen, "You on <map>").
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MapRecord {
    pub map_key: String,
    pub battles: usize,
    pub wins: usize,
    /// Battles with a known outcome (draws count as not won).
    pub outcome_battles: usize,
    pub win_rate: Option<f64>,
    pub average_damage: Option<f64>,
}

pub(crate) fn map_record(map_key: &str, battles: &[Battle]) -> MapRecord {
    let counted = battles
        .iter()
        .filter(|battle| stats_eligibility(battle) && battle.map_key.as_deref() == Some(map_key))
        .collect::<Vec<_>>();
    let outcomes = counted
        .iter()
        .filter(|battle| battle.outcome != BattleOutcome::Unknown)
        .collect::<Vec<_>>();
    let wins = outcomes
        .iter()
        .filter(|battle| battle.outcome == BattleOutcome::Victory)
        .count();
    let damage = counted
        .iter()
        .filter_map(|battle| battle.damage)
        .collect::<Vec<_>>();
    MapRecord {
        map_key: map_key.to_owned(),
        battles: counted.len(),
        wins,
        outcome_battles: outcomes.len(),
        win_rate: (!outcomes.is_empty()).then(|| wins as f64 / outcomes.len() as f64 * 100.0),
        average_damage: (!damage.is_empty())
            .then(|| damage.iter().sum::<i64>() as f64 / damage.len() as f64),
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionMaps {
    pub session_id: String,
    /// Up to 4 distinct map keys, newest battle first.
    pub map_keys: Vec<String>,
}

/// Map keys of each session's battles (History, session collage). Order follows `session_ids`.
pub(crate) fn session_maps(session_ids: &[String], battles: &[Battle]) -> Vec<SessionMaps> {
    let mut by_session = HashMap::<&str, Vec<&Battle>>::new();
    for battle in battles {
        if let Some(session) = battle.session_id.as_deref() {
            by_session.entry(session).or_default().push(battle);
        }
    }
    session_ids
        .iter()
        .map(|session_id| {
            let mut own = by_session.remove(session_id.as_str()).unwrap_or_default();
            own.sort_by(|left, right| {
                right
                    .battle_time
                    .cmp(&left.battle_time)
                    .then_with(|| right.arena_id.cmp(&left.arena_id))
            });
            let mut map_keys = Vec::new();
            for battle in own {
                let Some(key) = battle.map_key.as_deref() else {
                    continue;
                };
                if !map_keys.iter().any(|known| known == key) {
                    map_keys.push(key.to_owned());
                }
                if map_keys.len() == MAX_SESSION_MAPS {
                    break;
                }
            }
            SessionMaps {
                session_id: session_id.clone(),
                map_keys,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{SessionInsightDeltaUnit, SessionInsightEvidenceTextKey};
    use crate::session_insights::{
        derive_session_insights,
        tests::{comparison_with, delta, vehicle},
    };
    use crate::storage::tests::full_replay_battle;

    fn battle(index: i64, damage: i64, vehicle: i64, map: &str) -> Battle {
        let mut value = full_replay_battle(&format!("battle-{index:02}"));
        value.battle_time = 1_000 + index * 60_000;
        value.damage = Some(damage);
        value.wg_tank_id = Some(vehicle);
        value.vehicle_key = format!("synth-v{vehicle}");
        value.map_key = Some(map.to_owned());
        value.map_name = Some(format!("Map {map}"));
        value.session_id = Some("session-1".into());
        value.outcome = BattleOutcome::Victory;
        value.survived = Some(true);
        value
    }

    /// A comparison with (session damage, baseline, session battles, baseline battles) per
    /// vehicle and insights derived the same way as the app does.
    fn comparison(vehicles: &[(f64, f64, usize, usize)]) -> SessionBaselineComparison {
        let mut result = comparison_with(
            vehicles
                .iter()
                .enumerate()
                .map(|(index, (current, baseline, session, eligible))| {
                    let mut item = vehicle(index as i64 + 1, *eligible, *session);
                    item.damage = delta(Some(*current), Some(*baseline));
                    item.baseline.average_damage.value = Some(*baseline);
                    item
                })
                .collect(),
        );
        result.insights = derive_session_insights(&result);
        result
    }

    #[test]
    fn headline_names_strong_calm_tough_and_neutral_sessions() {
        let strong = headline(Some(&comparison(&[(3_240.0, 2_700.0, 9, 100)])), 9);
        assert_eq!(strong.kind, HeadlineKind::Strong);
        assert_eq!(strong.percent, Some(20));
        assert_eq!(strong.battles, 9);
        // E5-ui section 4 case: 1 528 against 3 244 over four tanks with 2 battles each.
        let spread = comparison(&[(1_528.0, 3_244.0, 2, 100); 4]);
        let tough = headline(Some(&spread), 8);
        assert_eq!((tough.kind, tough.percent), (HeadlineKind::Tough, Some(53)));
        let calm = headline(Some(&comparison(&[(2_800.0, 2_700.0, 9, 100)])), 9);
        assert_eq!((calm.kind, calm.percent), (HeadlineKind::Calm, None));
        for few in [
            comparison(&[(5_000.0, 2_700.0, 2, 100)]),
            comparison(&[(5_000.0, 2_700.0, 9, 29)]),
        ] {
            assert_eq!(headline(Some(&few), 9).kind, HeadlineKind::Neutral);
        }
        assert_eq!(headline(None, 9).kind, HeadlineKind::Neutral);
    }

    #[test]
    fn headline_thresholds_are_inclusive_at_ten_percent() {
        let kind = |current: f64| headline(Some(&comparison(&[(current, 1_000.0, 3, 30)])), 3).kind;
        assert_eq!(kind(1_100.0), HeadlineKind::Strong);
        assert_eq!(kind(900.0), HeadlineKind::Tough);
        assert_eq!(kind(1_099.0), HeadlineKind::Calm);
        assert_eq!(kind(901.0), HeadlineKind::Calm);
    }

    #[test]
    fn damage_change_weights_tanks_and_skips_small_baselines() {
        let mixed = comparison(&[
            (2_000.0, 2_500.0, 3, 100),
            (1_000.0, 1_500.0, 1, 100),
            (9_000.0, 1_000.0, 5, 10),
        ]);
        let change = damage_change(&mixed).unwrap();
        assert!((change - (1_750.0 / 2_250.0 - 1.0) * 100.0).abs() < 1e-6);
        let mut zero = comparison(&[(1_000.0, 1_000.0, 9, 100)]);
        zero.vehicles[0].baseline.average_damage.value = Some(0.0);
        assert_eq!(damage_change(&zero), None);
        let mut no_battles = comparison(&[(1_000.0, 1_000.0, 9, 100)]);
        no_battles.vehicles[0].session_metric_battles.damage = 0;
        assert_eq!(damage_change(&no_battles), None);
    }

    #[test]
    fn best_battle_needs_two_counted_battles_and_keeps_the_first_maximum() {
        let mut partial = battle(9, 9_000, 1, "99_partial");
        partial.data_quality = crate::models::DataQuality::Partial;
        assert_eq!(
            best_battle(&[battle(1, 4_000, 1, "01_a"), partial.clone()]),
            None
        );
        let battles = [
            battle(3, 4_480, 1, "03_c"),
            battle(1, 2_000, 1, "01_a"),
            battle(2, 4_480, 2, "02_b"),
            partial,
        ];
        let best = best_battle(&battles).unwrap();
        assert_eq!(best.battle_id, "battle-02");
        assert_eq!(best.damage, 4_480);
        assert_eq!(best.map_key.as_deref(), Some("02_b"));
        assert_eq!(best.map_name.as_deref(), Some("Map 02_b"));
        assert_eq!(best.vehicle_key, "synth-v2");
    }

    fn trend(battles: &[Battle]) -> (bool, f64, f64, usize, bool) {
        match damage_trend(battles) {
            Some(SessionMoment::DamageTrend {
                rising,
                from,
                to,
                battles_per_third,
                every_battle,
            }) => (rising, from, to, battles_per_third, every_battle),
            other => panic!("trend expected, got {other:?}"),
        }
    }

    #[test]
    fn damage_trend_compares_thirds_from_six_battles() {
        let rising = (0..9)
            .map(|index| battle(index, 2_000 + index * 310, 1, "01_a"))
            .collect::<Vec<_>>();
        // Every battle rising: the first and the last battle, as on the strip (E5-flare-fix).
        assert_eq!(trend(&rising), (true, 2_000.0, 4_480.0, 3, true));
        assert_eq!(damage_trend(&rising[..5]), None, "5 battles are too few");
        let mut uneven = rising.clone();
        uneven.swap(3, 4);
        uneven[3].battle_time = 1_000 + 3 * 60_000;
        uneven[4].battle_time = 1_000 + 4 * 60_000;
        // Otherwise the averages of the first and the last third.
        assert_eq!(trend(&uneven), (true, 2_310.0, 4_170.0, 3, false));
        let falling = rising
            .iter()
            .rev()
            .enumerate()
            .map(|(index, item)| {
                let mut value = item.clone();
                value.battle_time = 1_000 + index as i64 * 60_000;
                value
            })
            .collect::<Vec<_>>();
        assert_eq!(trend(&falling), (false, 4_480.0, 2_000.0, 3, true));
        let mut not_every_falling = falling.clone();
        not_every_falling[1].damage = Some(10_000);
        let (up, from, to, _, every) = trend(&not_every_falling);
        assert!(!up && !every);
        assert!((from - 18_340.0 / 3.0).abs() < 1e-9);
        assert_eq!(to, 2_310.0);
        let flat = (0..6)
            .map(|index| battle(index, 2_000 + index * 50, 1, "01_a"))
            .collect::<Vec<_>>();
        assert_eq!(damage_trend(&flat), None, "below 20%");
        let edge = [1_000, 1_000, 1_100, 1_100, 1_200, 1_200]
            .iter()
            .enumerate()
            .map(|(index, damage)| battle(index as i64, *damage, 1, "01_a"))
            .collect::<Vec<_>>();
        assert_eq!(trend(&edge).1, 1_000.0, "exactly 20% counts");
        let edge_down = [1_250, 1_250, 1_100, 1_100, 1_000, 1_000]
            .iter()
            .enumerate()
            .map(|(index, damage)| battle(index as i64, *damage, 1, "01_a"))
            .collect::<Vec<_>>();
        assert!(!trend(&edge_down).0, "exactly -20% counts");
        let mut plateau = rising.clone();
        plateau[4].damage = plateau[3].damage;
        assert!(
            !trend(&plateau).4,
            "an equal neighbour is not every battle rising"
        );
        let mut flat_fall = falling.clone();
        flat_fall[4].damage = flat_fall[3].damage;
        assert!(
            !trend(&flat_fall).4,
            "an equal neighbour is not every battle falling"
        );
        let zero = (0..6)
            .map(|index| battle(index, if index < 2 { 0 } else { 900 }, 1, "01_a"))
            .collect::<Vec<_>>();
        assert_eq!(damage_trend(&zero), None);
    }

    fn survival_signal(comparison: &mut SessionBaselineComparison, index: usize, strength: f64) {
        comparison.insights.summaries[index].strongest_negative = Some(SessionInsight {
            metric: SessionInsightMetric::Survival,
            direction: SessionInsightDirection::Negative,
            current_value: 0.0,
            baseline_value: 33.0,
            delta_value: -33.0,
            delta_unit: SessionInsightDeltaUnit::PercentagePoints,
            normalized_strength: strength,
            evidence_text_key: SessionInsightEvidenceTextKey::SurvivalBelowNorm,
        });
    }

    fn weakest(
        moment: Option<SessionMoment>,
    ) -> (
        Option<i64>,
        SessionInsightMetric,
        Option<usize>,
        usize,
        Option<String>,
        f64,
        f64,
    ) {
        match moment {
            Some(SessionMoment::WeakestTank {
                vehicle_id,
                metric,
                survived,
                battles,
                map_key,
                current,
                baseline,
                ..
            }) => (
                vehicle_id, metric, survived, battles, map_key, current, baseline,
            ),
            other => panic!("weakest tank expected, got {other:?}"),
        }
    }

    #[test]
    fn weakest_tank_takes_the_strongest_negative_insight_and_a_destroyed_battle() {
        let mut data = comparison(&[(1_000.0, 1_000.0, 3, 100), (1_000.0, 1_000.0, 3, 100)]);
        assert_eq!(weakest_tank(&data, &[]), None, "no negative insight");
        survival_signal(&mut data, 0, 1.5);
        survival_signal(&mut data, 1, 2.5);
        let mut battles = vec![
            battle(1, 3_000, 2, "01_a"),
            battle(2, 2_500, 2, "02_nordskar"),
            battle(3, 1_000, 2, "03_c"),
            battle(4, 500, 1, "04_d"),
        ];
        battles[1].survived = Some(false);
        let (vehicle_id, metric, survived, count, map_key, current, baseline) =
            weakest(weakest_tank(&data, &battles));
        assert_eq!(vehicle_id, Some(2));
        assert_eq!(metric, SessionInsightMetric::Survival);
        assert_eq!((survived, count), (Some(2), 3));
        assert_eq!(map_key.as_deref(), Some("02_nordskar"));
        assert_eq!((current, baseline), (0.0, 33.0));
        let mut tie = comparison(&[(1_000.0, 1_000.0, 3, 100), (1_000.0, 1_000.0, 3, 100)]);
        survival_signal(&mut tie, 0, 2.0);
        survival_signal(&mut tie, 1, 2.0);
        assert_eq!(
            weakest(weakest_tank(&tie, &battles)).0,
            Some(1),
            "tie: lower vehicle id"
        );
        let mut positive = comparison(&[(1_000.0, 1_000.0, 3, 100)]);
        survival_signal(&mut positive, 0, 2.0);
        positive.insights.summaries[0]
            .strongest_negative
            .as_mut()
            .unwrap()
            .direction = SessionInsightDirection::Positive;
        assert_eq!(weakest_tank(&positive, &battles), None);
    }

    #[test]
    fn weakest_tank_by_damage_shows_its_lowest_battle_and_needs_its_battles() {
        let data = comparison(&[(500.0, 1_000.0, 3, 100)]);
        let battles = vec![
            battle(1, 900, 1, "01_a"),
            battle(2, 300, 1, "02_low"),
            battle(3, 600, 1, "03_c"),
        ];
        let (_, metric, survived, count, map_key, current, _) =
            weakest(weakest_tank(&data, &battles));
        assert_eq!(metric, SessionInsightMetric::Damage);
        assert_eq!((survived, count), (None, 3));
        assert_eq!(map_key.as_deref(), Some("02_low"));
        assert_eq!(current, 500.0);
        assert_eq!(weakest_tank(&data, &[battle(1, 900, 7, "01_a")]), None);
        let mut survival = comparison(&[(1_000.0, 1_000.0, 3, 100)]);
        survival_signal(&mut survival, 0, 2.0);
        assert_eq!(
            weakest(weakest_tank(&survival, &battles)).4.as_deref(),
            Some("01_a"),
            "every battle survived: the first one stands in"
        );
    }

    #[test]
    fn story_combines_headline_best_battle_and_moments_in_order() {
        let battles = (0..9)
            .map(|index| battle(index, 2_000 + index * 310, 1, &format!("{index:02}_m")))
            .collect::<Vec<_>>();
        let data = comparison(&[(3_240.0, 2_700.0, 9, 100)]);
        let story = session_story(&battles, Some(&data));
        assert_eq!(story.headline.kind, HeadlineKind::Strong);
        assert_eq!(story.headline.battles, 9);
        assert_eq!(story.best_battle.as_ref().unwrap().battle_id, "battle-08");
        let kinds = story
            .moments
            .iter()
            .map(|moment| match moment {
                SessionMoment::DamageTrend { .. } => "trend",
                SessionMoment::WeakestTank { .. } => "weakest",
                SessionMoment::BestBattle { .. } => "best",
            })
            .collect::<Vec<_>>();
        assert_eq!(kinds, ["trend", "best"]);
        let mut weak = comparison(&[(3_240.0, 2_700.0, 9, 100)]);
        survival_signal(&mut weak, 0, 2.0);
        assert_eq!(session_story(&battles, Some(&weak)).moments.len(), 3);
        let empty = session_story(&[], None);
        assert_eq!(empty.headline.kind, HeadlineKind::Neutral);
        assert_eq!(empty.headline.battles, 0);
        assert!(empty.moments.is_empty() && empty.best_battle.is_none());
        let json = serde_json::to_value(&story.moments[0]).unwrap();
        assert_eq!(json["kind"], "damage_trend");
        assert_eq!(json["everyBattle"], true);
        assert_eq!(json["battlesPerThird"], 3);
    }

    #[test]
    fn map_record_counts_only_counted_battles_on_that_map() {
        let mut battles = vec![
            battle(1, 3_000, 1, "01_a"),
            battle(2, 2_000, 2, "01_a"),
            battle(3, 1_000, 1, "01_a"),
            battle(4, 9_000, 1, "02_b"),
        ];
        battles[1].outcome = BattleOutcome::Defeat;
        battles[2].outcome = BattleOutcome::Unknown;
        battles[2].damage = None;
        let mut second_win = battle(6, 2_500, 1, "01_a");
        second_win.outcome = BattleOutcome::Victory;
        battles.push(second_win);
        let mut partial = battle(5, 9_000, 1, "01_a");
        partial.data_quality = crate::models::DataQuality::Partial;
        battles.push(partial);
        let record = map_record("01_a", &battles);
        assert_eq!(record.map_key, "01_a");
        assert_eq!(
            (record.battles, record.wins, record.outcome_battles),
            (4, 2, 3)
        );
        assert!((record.win_rate.unwrap() - 200.0 / 3.0).abs() < 1e-9);
        assert_eq!(record.average_damage, Some(2_500.0));
        let none = map_record("99_none", &battles);
        assert_eq!(
            (none.battles, none.win_rate, none.average_damage),
            (0, None, None)
        );
    }

    #[test]
    fn session_maps_are_distinct_newest_first_and_at_most_four() {
        let mut battles = (0..7)
            .map(|index| battle(index, 1_000, 1, &format!("{:02}_m", index % 6)))
            .collect::<Vec<_>>();
        battles[6].map_key = None;
        let mut other = battle(20, 1_000, 1, "50_other");
        other.session_id = Some("session-2".into());
        battles.push(other);
        let mut loose = battle(21, 1_000, 1, "60_loose");
        loose.session_id = None;
        battles.push(loose);
        let maps = session_maps(
            &["session-1".into(), "session-2".into(), "session-3".into()],
            &battles,
        );
        assert_eq!(maps[0].map_keys, ["05_m", "04_m", "03_m", "02_m"]);
        assert_eq!(maps[1].map_keys, ["50_other"]);
        assert!(maps[2].map_keys.is_empty());
        assert_eq!(maps[2].session_id, "session-3");
        let mut same_time = vec![battle(1, 1_000, 1, "01_a"), battle(2, 1_000, 1, "02_b")];
        same_time[1].battle_time = same_time[0].battle_time;
        assert_eq!(
            session_maps(&["session-1".into()], &same_time)[0].map_keys,
            ["02_b", "01_a"]
        );
    }
}
