//! Facts of the template summaries (spec 30 §3, §4): which numbers a summary may say and in what
//! order. Rust keeps the data and the selection rules with their thresholds; the phrases, the
//! grammar and the number format live in TypeScript (`src/lib/recap/`). Nothing is stored: the
//! facts are computed when a screen asks, like the weekly report (spec 27). Only counted battles
//! (D4) enter any number.

use serde::Serialize;

use crate::models::{
    Battle, BattleOutcome, SessionBaselineComparison, VehicleClass, stats_eligibility,
};
use crate::session_story::{best_battle, damage_reference};
use crate::weekly::{
    GoalKind, GoalState, MIN_WEEK_BATTLES, WeekConfidence, WeeklyMetric, WeeklyReport,
};

/// Win and survival rates of a session are compared with the player's last 100 counted battles
/// before it (spec 30 §4.2).
pub(crate) const REFERENCE_BATTLES: usize = 100;
/// Fewer earlier battles than this give no reference (the D7 minimum of the insights).
pub(crate) const MIN_REFERENCE_BATTLES: usize = 30;
/// The survival rate is left out when it differs from the reference by less than 5 points.
pub(crate) const SURVIVAL_MIN_DIFFERENCE_POINTS: i64 = 5;
/// Session WN8 is compared with the WN8 of the 30 days before the session.
pub(crate) const WN8_REFERENCE_DAYS: i64 = 30;
const DAY_MS: i64 = 86_400_000;

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecapKind {
    Session,
    Battle,
    Week,
}

/// A rate compared with the same rate of the player's earlier battles, in whole percent.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateReference {
    /// The earlier rate, whole percent.
    pub percent: i64,
    /// Earlier battles the rate comes from (at most 100).
    pub battles: usize,
    /// Session percent minus the earlier percent, both already whole: the sentence can say
    /// "52%, 4 points more than 48%" without a rounding gap.
    pub difference: i64,
}

/// One fact of a session summary; the key is the one from spec 30 §4.2.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "key", rename_all_fields = "camelCase")]
pub(crate) enum SessionFactValue {
    /// Counted battles of the session.
    #[serde(rename = "liczba_bitew")]
    Battles { battles: usize },
    /// Won battles among those with a known outcome, against the last 100 battles before.
    #[serde(rename = "odsetek_wygranych")]
    WinRate {
        wins: usize,
        battles: usize,
        percent: i64,
        reference: Option<RateReference>,
    },
    /// Damage on the session's tanks against the player's average on them: the numbers of the
    /// headline and of the header note.
    #[serde(rename = "obrazenia")]
    Damage {
        current: i64,
        baseline: i64,
        /// Signed whole percent.
        percent: i64,
    },
    /// The highest damage battle with its map, one fact (spec 30 §4.2).
    #[serde(rename = "najlepsza")]
    Best {
        battle_id: String,
        damage: i64,
        map_key: Option<String>,
        map_name: Option<String>,
    },
    /// Session WN8 against the 30 days before the session.
    #[serde(rename = "wn8")]
    Wn8 {
        value: i64,
        reference: Option<i64>,
        /// Days of the reference window, so the sentence can name them.
        reference_days: i64,
    },
    /// Survived battles among those with a known survival, against the last 100 battles.
    #[serde(rename = "przetrwanie_odsetek")]
    SurvivalRate {
        survived: usize,
        battles: usize,
        percent: i64,
        reference: RateReference,
    },
    /// The most played tank of the session.
    #[serde(rename = "czolg")]
    Tank {
        vehicle_key: String,
        tank_name: Option<String>,
        battles: usize,
        /// Counted battles of the session, so the text can say "every battle".
        total: usize,
    },
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionFact {
    /// "F1", "F2", … in the order of the list; only kept facts get one.
    pub id: String,
    /// The text must say every important fact (spec 30 §7 point 8).
    pub important: bool,
    pub fact: SessionFactValue,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Recap {
    Session {
        /// Stable hash of the session id: the phrase variants follow it (spec 30 §6).
        seed: u32,
        facts: Vec<SessionFact>,
    },
    Battle {
        /// Stable hash of `arena_unique_id` (or the battle id when it is missing).
        seed: u32,
        facts: Vec<BattleFact>,
    },
    Week {
        /// Stable hash of the week's start (milliseconds, local Monday 00:00).
        seed: u32,
        facts: Vec<WeekFact>,
    },
}

/// WN8 of the session and of the 30 days before it, when WN8 data is on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Wn8Pair {
    pub session: f64,
    pub reference: Option<f64>,
}

pub(crate) struct SessionInput<'a> {
    pub session_id: &'a str,
    pub started_at: i64,
    /// The battles of the session.
    pub battles: &'a [Battle],
    /// Any stored battles; only those of other sessions before `started_at` are read.
    pub history: &'a [Battle],
    pub comparison: Option<&'a SessionBaselineComparison>,
    /// `None` when WN8 data is off (XVM disabled or no expected values).
    pub wn8: Option<Wn8Pair>,
}

/// FNV-1a, 32 bits: the same id gives the same seed on every machine and every run.
pub(crate) fn seed(id: &str) -> u32 {
    id.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

fn whole_percent(part: usize, total: usize) -> i64 {
    (part as f64 / total as f64 * 100.0).round() as i64
}

/// Counted battles of other sessions before `started_at`, newest first, at most `limit`.
fn earlier_counted<'a>(
    history: &'a [Battle],
    session_id: &str,
    started_at: i64,
    limit: usize,
) -> Vec<&'a Battle> {
    let mut earlier = history
        .iter()
        .filter(|battle| {
            stats_eligibility(battle)
                && battle.battle_time < started_at
                && battle.session_id.as_deref() != Some(session_id)
        })
        .collect::<Vec<_>>();
    earlier.sort_by(|left, right| {
        right
            .battle_time
            .cmp(&left.battle_time)
            .then_with(|| right.arena_id.cmp(&left.arena_id))
    });
    earlier.truncate(limit);
    earlier
}

/// Battles that enter the 30-day WN8 reference: other sessions, in the 30 days before the start.
pub(crate) fn wn8_reference_battles(
    history: &[Battle],
    session_id: &str,
    started_at: i64,
) -> Vec<Battle> {
    let from = started_at - WN8_REFERENCE_DAYS * DAY_MS;
    history
        .iter()
        .filter(|battle| {
            battle.battle_time >= from
                && battle.battle_time < started_at
                && battle.session_id.as_deref() != Some(session_id)
        })
        .cloned()
        .collect()
}

/// A rate of the earlier battles that have the value (`count` returns `None` for unknown).
fn rate_reference(
    earlier: &[&Battle],
    session_percent: i64,
    value: impl Fn(&Battle) -> Option<bool>,
) -> Option<RateReference> {
    let known = earlier
        .iter()
        .filter_map(|battle| value(battle))
        .collect::<Vec<_>>();
    if known.len() < MIN_REFERENCE_BATTLES {
        return None;
    }
    let percent = whole_percent(known.iter().filter(|hit| **hit).count(), known.len());
    Some(RateReference {
        percent,
        battles: known.len(),
        difference: session_percent - percent,
    })
}

fn won(battle: &Battle) -> Option<bool> {
    battle
        .outcome
        .is_known()
        .then_some(battle.outcome == BattleOutcome::Victory)
}

/// The tank of more than half of the counted battles (author decision 2026-11-16), with the
/// last known name in battle order; `None` when no tank has more than half.
fn most_played(counted: &[&Battle]) -> Option<(String, Option<String>, usize)> {
    let mut tanks: Vec<(&str, usize, Option<&str>)> = Vec::new();
    let mut ordered = counted.to_vec();
    ordered.sort_by(|left, right| {
        left.battle_time
            .cmp(&right.battle_time)
            .then_with(|| left.arena_id.cmp(&right.arena_id))
    });
    for battle in ordered {
        match tanks
            .iter_mut()
            .find(|(key, ..)| *key == battle.vehicle_key)
        {
            Some(entry) => {
                entry.1 += 1;
                if battle.tank_name.is_some() {
                    entry.2 = battle.tank_name.as_deref();
                }
            }
            None => tanks.push((&battle.vehicle_key, 1, battle.tank_name.as_deref())),
        }
    }
    tanks
        .iter()
        .find(|(_, battles, ..)| battles * 2 > counted.len())
        .map(|(key, battles, name)| ((*key).to_owned(), name.map(str::to_owned), *battles))
}

/// The session facts with the rules of spec 30 §4.2, in text order, numbered F1, F2, …
pub(crate) fn session_recap(input: &SessionInput) -> Recap {
    let counted = input
        .battles
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .collect::<Vec<_>>();
    let mut facts: Vec<(bool, SessionFactValue)> = Vec::new();
    if !counted.is_empty() {
        let earlier = earlier_counted(
            input.history,
            input.session_id,
            input.started_at,
            REFERENCE_BATTLES,
        );
        facts.push((
            true,
            SessionFactValue::Battles {
                battles: counted.len(),
            },
        ));
        let outcomes = counted
            .iter()
            .filter_map(|battle| won(battle))
            .collect::<Vec<_>>();
        if !outcomes.is_empty() {
            let wins = outcomes.iter().filter(|hit| **hit).count();
            let percent = whole_percent(wins, outcomes.len());
            facts.push((
                true,
                SessionFactValue::WinRate {
                    wins,
                    battles: outcomes.len(),
                    percent,
                    reference: rate_reference(&earlier, percent, won),
                },
            ));
        }
        if let Some(reference) = input.comparison.and_then(damage_reference) {
            facts.push((
                true,
                SessionFactValue::Damage {
                    current: reference.current.round() as i64,
                    baseline: reference.baseline.round() as i64,
                    percent: reference.change.round() as i64,
                },
            ));
        }
        if let Some(best) = best_battle(input.battles) {
            facts.push((
                true,
                SessionFactValue::Best {
                    battle_id: best.battle_id,
                    damage: best.damage,
                    map_key: best.map_key,
                    map_name: best.map_name,
                },
            ));
        }
        if let Some(wn8) = input.wn8 {
            facts.push((
                true,
                SessionFactValue::Wn8 {
                    value: wn8.session.round() as i64,
                    reference: wn8.reference.map(|value| value.round() as i64),
                    reference_days: WN8_REFERENCE_DAYS,
                },
            ));
        }
        let survival = counted
            .iter()
            .filter_map(|battle| battle.survived)
            .collect::<Vec<_>>();
        if !survival.is_empty() {
            let survived = survival.iter().filter(|hit| **hit).count();
            let percent = whole_percent(survived, survival.len());
            if let Some(reference) = rate_reference(&earlier, percent, |battle| battle.survived)
                && reference.difference.abs() >= SURVIVAL_MIN_DIFFERENCE_POINTS
            {
                facts.push((
                    false,
                    SessionFactValue::SurvivalRate {
                        survived,
                        battles: survival.len(),
                        percent,
                        reference,
                    },
                ));
            }
        }
        if let Some((vehicle_key, tank_name, battles)) = most_played(&counted) {
            facts.push((
                false,
                SessionFactValue::Tank {
                    vehicle_key,
                    tank_name,
                    battles,
                    total: counted.len(),
                },
            ));
        }
    }
    Recap::Session {
        seed: seed(input.session_id),
        facts: facts
            .into_iter()
            .enumerate()
            .map(|(index, (important, fact))| SessionFact {
                id: format!("F{}", index + 1),
                important,
                fact,
            })
            .collect(),
    }
}

/// Battle rules from A-3 (spec 30 §4.1); thresholds unchanged until the author's review.
pub(crate) const ASSIST_MIN: i64 = 800;
pub(crate) const SPOTS_MIN: i64 = 3;
pub(crate) const BLOCKED_MIN_HEAVY: i64 = 1_000;
pub(crate) const BLOCKED_MIN_OTHER: i64 = 500;
pub(crate) const FRAGS_IMPORTANT: i64 = 2;
pub(crate) const PLACE_MAX: i64 = 3;
pub(crate) const SHARE_MIN_PERCENT: i64 = 20;
/// A battle summary says the result and the damage, then at most three more sentences
/// (destroyed vehicles and the place in the team share one), as in the A-3 template.
pub(crate) const BATTLE_REST_SENTENCES: usize = 3;

/// One fact of a battle summary; the key is the one from spec 30 §4.1.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "key", rename_all_fields = "camelCase")]
pub(crate) enum BattleFactValue {
    #[serde(rename = "wynik")]
    Outcome { outcome: BattleOutcome },
    #[serde(rename = "mapa")]
    Map { map_name: String },
    #[serde(rename = "czolg")]
    Tank {
        vehicle_key: String,
        tank_name: Option<String>,
        tier: Option<u8>,
    },
    #[serde(rename = "przetrwanie")]
    Survival { survived: bool },
    /// Damage against the player's average on this tank (last 100 battles before it).
    #[serde(rename = "obrazenia")]
    Damage { damage: i64, reference: Option<i64> },
    #[serde(rename = "zniszczone")]
    Destroyed { frags: i64 },
    #[serde(rename = "asysta")]
    Assist { assist: i64, reference: Option<i64> },
    #[serde(rename = "pancerz")]
    Blocked { blocked: i64 },
    #[serde(rename = "wykryte")]
    Spotted { spots: i64 },
    /// Place in the own team by damage (1 = most).
    #[serde(rename = "miejsce")]
    Place { place: i64 },
    /// Share of the own team's damage, whole percent (the number of the Battle header).
    #[serde(rename = "udzial")]
    Share { percent: i64 },
    #[serde(rename = "strzaly")]
    Shots {
        shots: i64,
        hits: i64,
        penetrations: i64,
    },
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BattleFact {
    pub id: String,
    pub important: bool,
    pub fact: BattleFactValue,
}

pub(crate) struct BattleInput<'a> {
    pub battle: &'a Battle,
    pub class: VehicleClass,
    pub tier: Option<u8>,
    /// The player's average damage and assist on this tank before the battle (30+ battles).
    pub damage_reference: Option<f64>,
    pub assist_reference: Option<f64>,
    /// Damage place in the own team (team facts), and the own team's total damage.
    pub place: Option<i64>,
    pub team_damage: Option<i64>,
}

/// Facts of one counted battle with the rules of spec 30 §4.1, numbered in text order.
pub(crate) fn battle_recap(input: &BattleInput) -> Recap {
    let battle = input.battle;
    let seed_id = battle
        .arena_unique_id
        .map_or_else(|| battle.arena_id.clone(), |id| id.to_string());
    let mut facts: Vec<(bool, BattleFactValue)> = Vec::new();
    if stats_eligibility(battle) {
        let light = input.class == VehicleClass::LightTank;
        let heavy = input.class == VehicleClass::HeavyTank;
        if battle.outcome.is_known() {
            facts.push((
                true,
                BattleFactValue::Outcome {
                    outcome: battle.outcome,
                },
            ));
        }
        if let Some(map_name) = &battle.map_name {
            facts.push((
                false,
                BattleFactValue::Map {
                    map_name: map_name.clone(),
                },
            ));
        }
        facts.push((
            false,
            BattleFactValue::Tank {
                vehicle_key: battle.vehicle_key.clone(),
                tank_name: battle.tank_name.clone(),
                tier: input.tier,
            },
        ));
        if let Some(survived) = battle.survived {
            facts.push((true, BattleFactValue::Survival { survived }));
        }
        if let Some(damage) = battle.damage {
            facts.push((
                true,
                BattleFactValue::Damage {
                    damage,
                    reference: input.damage_reference.map(|value| value.round() as i64),
                },
            ));
        }
        facts.extend(within_budget(battle_rest(input, light, heavy)));
    }
    Recap::Battle {
        seed: seed(&seed_id),
        facts: facts
            .into_iter()
            .enumerate()
            .map(|(index, (important, fact))| BattleFact {
                id: format!("F{}", index + 1),
                important,
                fact,
            })
            .collect(),
    }
}

/// The facts after damage with their rules, in the A-3 order, important ones first.
fn battle_rest(input: &BattleInput, light: bool, heavy: bool) -> Vec<(bool, BattleFactValue)> {
    let battle = input.battle;
    let mut rest: Vec<(bool, BattleFactValue)> = Vec::new();
    if let Some(frags) = battle.frags.filter(|frags| *frags > 0) {
        rest.push((
            frags >= FRAGS_IMPORTANT,
            BattleFactValue::Destroyed { frags },
        ));
    }
    if let Some(assist) = battle
        .assist
        .filter(|assist| light || *assist >= ASSIST_MIN)
    {
        rest.push((
            light,
            BattleFactValue::Assist {
                assist,
                reference: input.assist_reference.map(|value| value.round() as i64),
            },
        ));
    }
    let blocked_min = if heavy {
        BLOCKED_MIN_HEAVY
    } else {
        BLOCKED_MIN_OTHER
    };
    if let Some(blocked) = battle.blocked.filter(|blocked| *blocked >= blocked_min) {
        rest.push((
            heavy && blocked > BLOCKED_MIN_HEAVY,
            BattleFactValue::Blocked { blocked },
        ));
    }
    if let Some(spots) = battle.spots.filter(|spots| light || *spots >= SPOTS_MIN) {
        rest.push((false, BattleFactValue::Spotted { spots }));
    }
    let place = input.place.filter(|place| *place <= PLACE_MAX);
    if let Some(place) = place {
        rest.push((true, BattleFactValue::Place { place }));
    }
    let share = match (battle.damage, input.team_damage) {
        (Some(damage), Some(team)) if team > 0 => {
            Some((damage as f64 / team as f64 * 100.0).round() as i64)
        }
        _ => None,
    };
    if let Some(percent) = share.filter(|percent| *percent >= SHARE_MIN_PERCENT)
        && place.is_none()
    {
        rest.push((false, BattleFactValue::Share { percent }));
    }
    if let Some(shots) = battle.shots {
        rest.push((
            false,
            BattleFactValue::Shots {
                shots,
                hits: battle.direct_enemy_hits.or(battle.hits).unwrap_or(0),
                penetrations: battle
                    .piercing_enemy_hits
                    .or(battle.penetrations)
                    .unwrap_or(0),
            },
        ));
    }
    rest.sort_by_key(|(important, _)| !important);
    rest
}

/// Up to `BATTLE_REST_SENTENCES` sentences of the remaining facts; destroyed vehicles take the
/// place in the team along into their sentence, as in the A-3 template.
fn within_budget(mut rest: Vec<(bool, BattleFactValue)>) -> Vec<(bool, BattleFactValue)> {
    let mut kept = Vec::new();
    let mut sentences = 0;
    while !rest.is_empty() && sentences < BATTLE_REST_SENTENCES {
        let fact = rest.remove(0);
        let destroyed = matches!(fact.1, BattleFactValue::Destroyed { .. });
        kept.push(fact);
        if destroyed
            && let Some(index) = rest
                .iter()
                .position(|(_, value)| matches!(value, BattleFactValue::Place { .. }))
        {
            kept.push(rest.remove(index));
        }
        sentences += 1;
    }
    kept
}

/// An indicator of the week is left out within 5 points of its usual 100% (spec 30 §4.3).
pub(crate) const INDICATOR_MIN_POINTS: i64 = 5;
/// "100% = as usual" (spec 27 §2): the indicators are percent of the player's own average.
pub(crate) const INDICATOR_USUAL: i64 = 100;
/// Battles of one challenge (the challenge screen's "5 battles").
pub(crate) const CHALLENGE_BATTLES: i64 = crate::challenges::TARGET_BATTLES as i64;

/// One fact of a week summary; the key follows spec 30 §4.3.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "key", rename_all_fields = "camelCase")]
pub(crate) enum WeekFactValue {
    /// Counted battles of the week and its confidence; `minimum` is the trend threshold (15).
    #[serde(rename = "liczba_bitew")]
    Battles {
        battles: usize,
        confidence: WeekConfidence,
        minimum: usize,
    },
    /// Context only (D12), never a goal.
    #[serde(rename = "odsetek_wygranych")]
    WinRate { percent: i64 },
    /// Percent of the player's own average (100 = as usual).
    #[serde(rename = "wskaznik")]
    Indicator {
        metric: WeeklyMetric,
        percent: i64,
        usual: i64,
    },
    #[serde(rename = "cel")]
    Goal {
        kind: GoalKind,
        state: GoalState,
        vehicle_key: Option<String>,
        tank_name: Option<String>,
    },
    /// "K of M goals" of the week before, counted like the Goals screen (all its goals).
    #[serde(rename = "poprzedni_tydzien")]
    PreviousWeek { achieved: usize, goals: usize },
    /// Challenges of 5 battles finished this week.
    #[serde(rename = "wyzwania")]
    Challenges {
        completed: usize,
        failed: usize,
        abandoned: usize,
        battles: i64,
    },
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeekFact {
    pub id: String,
    pub important: bool,
    pub fact: WeekFactValue,
}

/// Facts of the weekly report with the rules of spec 30 §4.3; seed from the week's start.
pub(crate) fn week_recap(report: &WeeklyReport) -> Recap {
    let mut facts: Vec<(bool, WeekFactValue)> = vec![(
        true,
        WeekFactValue::Battles {
            battles: report.battles,
            confidence: report.confidence,
            minimum: MIN_WEEK_BATTLES,
        },
    )];
    if report.confidence != WeekConfidence::TooFew {
        if let Some(rate) = report.win_rate {
            facts.push((
                false,
                WeekFactValue::WinRate {
                    percent: rate.round() as i64,
                },
            ));
        }
        for indicator in &report.indicators {
            let Some(current) = indicator.current else {
                continue;
            };
            let percent = current.round() as i64;
            if (percent - INDICATOR_USUAL).abs() >= INDICATOR_MIN_POINTS {
                facts.push((
                    true,
                    WeekFactValue::Indicator {
                        metric: indicator.metric,
                        percent,
                        usual: INDICATOR_USUAL,
                    },
                ));
            }
        }
        for goal in report
            .goals
            .iter()
            .filter(|goal| goal.state != GoalState::TooFew)
        {
            facts.push((
                true,
                WeekFactValue::Goal {
                    kind: goal.kind,
                    state: goal.state,
                    vehicle_key: goal.vehicle_key.clone(),
                    tank_name: goal.tank_name.clone(),
                },
            ));
        }
        let before = &report.previous_week.goals;
        if !before.is_empty() {
            facts.push((
                false,
                WeekFactValue::PreviousWeek {
                    achieved: before
                        .iter()
                        .filter(|goal| goal.state == GoalState::Achieved)
                        .count(),
                    goals: before.len(),
                },
            ));
        }
        let challenges = &report.challenges;
        if challenges.completed + challenges.failed + challenges.abandoned > 0 {
            facts.push((
                false,
                WeekFactValue::Challenges {
                    completed: challenges.completed,
                    failed: challenges.failed,
                    abandoned: challenges.abandoned,
                    battles: CHALLENGE_BATTLES,
                },
            ));
        }
    }
    Recap::Week {
        seed: seed(&report.week_start.to_string()),
        facts: facts
            .into_iter()
            .enumerate()
            .map(|(index, (important, fact))| WeekFact {
                id: format!("F{}", index + 1),
                important,
                fact,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_insights::tests::{comparison_with, delta, vehicle};
    use crate::storage::tests::full_replay_battle;

    const START: i64 = 1_800_000_000_000;

    fn battle(id: &str, time: i64, session: &str) -> Battle {
        let mut value = full_replay_battle(id);
        value.battle_time = time;
        value.session_id = Some(session.to_owned());
        value.vehicle_key = "synth-a".into();
        value.tank_name = Some("Synth A".into());
        value.map_key = Some("01_synth".into());
        value.map_name = Some("Synth Map".into());
        value.outcome = BattleOutcome::Victory;
        value.survived = Some(true);
        value
    }

    /// `count` session battles one minute apart from START.
    fn session_battles(count: usize) -> Vec<Battle> {
        (0..count)
            .map(|index| {
                let mut value =
                    battle(&format!("s-{index:02}"), START + index as i64 * 60_000, "s");
                value.damage = Some(1_000 + index as i64 * 100);
                value
            })
            .collect()
    }

    /// `count` earlier battles before START; `won` and `survived` decide the first ones.
    fn earlier(count: usize, wins: usize, survived: usize) -> Vec<Battle> {
        (0..count)
            .map(|index| {
                let mut value = battle(
                    &format!("e-{index:03}"),
                    START - 1_000 - index as i64 * 60_000,
                    "old",
                );
                value.outcome = if index < wins {
                    BattleOutcome::Victory
                } else {
                    BattleOutcome::Defeat
                };
                value.survived = Some(index < survived);
                value
            })
            .collect()
    }

    fn recap(battles: &[Battle], history: &[Battle]) -> Vec<SessionFact> {
        recap_with(battles, history, None, None)
    }

    fn recap_with(
        battles: &[Battle],
        history: &[Battle],
        comparison: Option<&SessionBaselineComparison>,
        wn8: Option<Wn8Pair>,
    ) -> Vec<SessionFact> {
        let Recap::Session { facts, .. } = session_recap(&SessionInput {
            session_id: "s",
            started_at: START,
            battles,
            history,
            comparison,
            wn8,
        }) else {
            panic!("session recap");
        };
        facts
    }

    fn keys(facts: &[SessionFact]) -> Vec<&'static str> {
        facts
            .iter()
            .map(|fact| match fact.fact {
                SessionFactValue::Battles { .. } => "liczba_bitew",
                SessionFactValue::WinRate { .. } => "odsetek_wygranych",
                SessionFactValue::Damage { .. } => "obrazenia",
                SessionFactValue::Best { .. } => "najlepsza",
                SessionFactValue::Wn8 { .. } => "wn8",
                SessionFactValue::SurvivalRate { .. } => "przetrwanie_odsetek",
                SessionFactValue::Tank { .. } => "czolg",
            })
            .collect()
    }

    fn find<'a>(facts: &'a [SessionFact], key: &str) -> Option<&'a SessionFactValue> {
        keys(facts)
            .iter()
            .position(|known| *known == key)
            .map(|index| &facts[index].fact)
    }

    fn damage_comparison(current: f64, baseline: f64, battles: usize) -> SessionBaselineComparison {
        let mut item = vehicle(1, 100, battles);
        item.damage = delta(Some(current), Some(baseline));
        item.baseline.average_damage.value = Some(baseline);
        comparison_with(vec![item])
    }

    #[test]
    fn seed_is_fnv1a_and_stable() {
        assert_eq!(seed(""), 0x811c_9dc5);
        assert_eq!(seed("a"), 0xe40c_292c);
        assert_eq!(seed("session-1"), seed("session-1"));
        assert_ne!(seed("session-1"), seed("session-2"));
    }

    #[test]
    fn no_counted_battle_gives_no_facts() {
        let mut partial = session_battles(1);
        partial[0].data_quality = crate::models::DataQuality::Partial;
        assert!(recap(&partial, &earlier(50, 25, 25)).is_empty());
        assert!(recap(&[], &[]).is_empty());
    }

    #[test]
    fn full_session_keeps_every_fact_in_text_order_with_continuous_ids() {
        let mut battles = session_battles(4);
        battles[3].survived = Some(false);
        battles[2].vehicle_key = "synth-b".into();
        let comparison = damage_comparison(1_200.0, 1_000.0, 4);
        let facts = recap_with(
            &battles,
            &earlier(100, 50, 100),
            Some(&comparison),
            Some(Wn8Pair {
                session: 1_499.6,
                reference: Some(1_300.4),
            }),
        );
        assert_eq!(
            keys(&facts),
            [
                "liczba_bitew",
                "odsetek_wygranych",
                "obrazenia",
                "najlepsza",
                "wn8",
                "przetrwanie_odsetek",
                "czolg"
            ]
        );
        let ids = facts
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["F1", "F2", "F3", "F4", "F5", "F6", "F7"]);
        let important = facts.iter().map(|fact| fact.important).collect::<Vec<_>>();
        assert_eq!(important, [true, true, true, true, true, false, false]);
        assert_eq!(facts[0].fact, SessionFactValue::Battles { battles: 4 });
        assert_eq!(
            facts[4].fact,
            SessionFactValue::Wn8 {
                value: 1_500,
                reference: Some(1_300),
                reference_days: 30
            }
        );
    }

    #[test]
    fn ids_stay_continuous_when_facts_are_left_out() {
        let facts = recap(&session_battles(1), &[]);
        assert_eq!(keys(&facts), ["liczba_bitew", "odsetek_wygranych", "czolg"]);
        let ids = facts
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["F1", "F2", "F3"]);
    }

    #[test]
    fn win_rate_counts_known_outcomes_against_the_last_hundred_earlier_battles() {
        let mut battles = session_battles(4);
        battles[1].outcome = BattleOutcome::Defeat;
        battles[2].outcome = BattleOutcome::Unknown;
        // 120 earlier battles: the newest 100 have 40 wins; the 20 oldest are all wins.
        let mut history = earlier(120, 40, 0);
        for old in &mut history[100..] {
            old.outcome = BattleOutcome::Victory;
        }
        // Battles of this session and later ones never enter the reference.
        history.extend(session_battles(4));
        let mut later = battle("later", START + 999_999, "other");
        later.outcome = BattleOutcome::Defeat;
        history.push(later);
        let facts = recap(&battles, &history);
        assert_eq!(
            find(&facts, "odsetek_wygranych"),
            Some(&SessionFactValue::WinRate {
                wins: 2,
                battles: 3,
                percent: 67,
                reference: Some(RateReference {
                    percent: 40,
                    battles: 100,
                    difference: 27
                })
            })
        );
    }

    #[test]
    fn rate_reference_needs_thirty_earlier_battles() {
        let at_least = recap(&session_battles(3), &earlier(30, 15, 15));
        let Some(SessionFactValue::WinRate { reference, .. }) =
            find(&at_least, "odsetek_wygranych")
        else {
            panic!("win rate");
        };
        assert_eq!(reference.map(|value| value.battles), Some(30));
        let fewer = recap(&session_battles(3), &earlier(29, 15, 15));
        let Some(SessionFactValue::WinRate { reference, .. }) = find(&fewer, "odsetek_wygranych")
        else {
            panic!("win rate");
        };
        assert_eq!(*reference, None);
    }

    #[test]
    fn a_battle_of_another_session_at_the_very_start_is_not_earlier() {
        // 29 earlier battles give no reference; a 30th exactly at the session start must not
        // complete it (the reference is strictly before the start).
        let mut history = earlier(29, 0, 0);
        history.push(battle("at-start", START, "other"));
        let facts = recap(&session_battles(3), &history);
        let Some(SessionFactValue::WinRate { reference, .. }) = find(&facts, "odsetek_wygranych")
        else {
            panic!("win rate");
        };
        assert_eq!(*reference, None);
        history.last_mut().unwrap().battle_time = START - 1;
        let facts = recap(&session_battles(3), &history);
        let Some(SessionFactValue::WinRate { reference, .. }) = find(&facts, "odsetek_wygranych")
        else {
            panic!("win rate");
        };
        assert_eq!(reference.map(|value| value.battles), Some(30));
    }

    #[test]
    fn earlier_battles_that_are_not_counted_do_not_enter_the_reference() {
        let mut history = earlier(40, 40, 40);
        for old in &mut history[..20] {
            old.data_quality = crate::models::DataQuality::Partial;
        }
        let facts = recap(&session_battles(3), &history);
        let Some(SessionFactValue::WinRate { reference, .. }) = find(&facts, "odsetek_wygranych")
        else {
            panic!("win rate");
        };
        assert_eq!(*reference, None);
    }

    #[test]
    fn win_rate_is_left_out_without_known_outcomes() {
        let mut battles = session_battles(2);
        for battle in &mut battles {
            battle.outcome = BattleOutcome::Unknown;
        }
        assert_eq!(find(&recap(&battles, &[]), "odsetek_wygranych"), None);
    }

    #[test]
    fn damage_needs_the_headline_reference_and_keeps_its_numbers() {
        let comparison = damage_comparison(1_234.4, 1_000.0, 3);
        let facts = recap_with(&session_battles(3), &[], Some(&comparison), None);
        assert_eq!(
            find(&facts, "obrazenia"),
            Some(&SessionFactValue::Damage {
                current: 1_234,
                baseline: 1_000,
                percent: 23
            })
        );
        let lower = damage_comparison(880.0, 1_000.0, 3);
        let facts = recap_with(&session_battles(3), &[], Some(&lower), None);
        assert!(matches!(
            find(&facts, "obrazenia"),
            Some(SessionFactValue::Damage { percent: -12, .. })
        ));
        let too_few = damage_comparison(1_234.0, 1_000.0, 2);
        assert_eq!(
            find(
                &recap_with(&session_battles(3), &[], Some(&too_few), None),
                "obrazenia"
            ),
            None
        );
        assert_eq!(find(&recap(&session_battles(3), &[]), "obrazenia"), None);
    }

    #[test]
    fn best_battle_needs_two_battles_with_damage() {
        let one = recap(&session_battles(1), &[]);
        assert_eq!(find(&one, "najlepsza"), None);
        let two = recap(&session_battles(2), &[]);
        assert_eq!(
            find(&two, "najlepsza"),
            Some(&SessionFactValue::Best {
                battle_id: "s-01".into(),
                damage: 1_100,
                map_key: Some("01_synth".into()),
                map_name: Some("Synth Map".into())
            })
        );
    }

    #[test]
    fn wn8_only_when_wn8_data_is_on() {
        assert_eq!(find(&recap(&session_battles(3), &[]), "wn8"), None);
        let facts = recap_with(
            &session_battles(3),
            &[],
            None,
            Some(Wn8Pair {
                session: 900.5,
                reference: None,
            }),
        );
        assert_eq!(
            find(&facts, "wn8"),
            Some(&SessionFactValue::Wn8 {
                value: 901,
                reference: None,
                reference_days: 30
            })
        );
    }

    #[test]
    fn survival_rate_is_kept_from_five_points_of_difference() {
        // Session: 2 of 4 survived = 50%.
        let mut battles = session_battles(4);
        battles[0].survived = Some(false);
        battles[1].survived = Some(false);
        let at = |survived: usize| {
            find(
                &recap(&battles, &earlier(100, 0, survived)),
                "przetrwanie_odsetek",
            )
            .cloned()
        };
        assert_eq!(
            at(45),
            Some(SessionFactValue::SurvivalRate {
                survived: 2,
                battles: 4,
                percent: 50,
                reference: RateReference {
                    percent: 45,
                    battles: 100,
                    difference: 5
                }
            })
        );
        assert!(at(55).is_some());
        assert_eq!(at(46), None);
        assert_eq!(at(54), None);
        // No reference: no survival fact.
        assert_eq!(
            find(&recap(&battles, &earlier(29, 0, 0)), "przetrwanie_odsetek"),
            None
        );
    }

    #[test]
    fn survival_counts_only_battles_with_a_known_survival() {
        let mut battles = session_battles(3);
        battles[0].survived = None;
        battles[1].survived = Some(false);
        let facts = recap(&battles, &earlier(40, 0, 40));
        assert!(matches!(
            find(&facts, "przetrwanie_odsetek"),
            Some(SessionFactValue::SurvivalRate {
                survived: 1,
                battles: 2,
                percent: 50,
                ..
            })
        ));
        for battle in &mut battles {
            battle.survived = None;
        }
        assert_eq!(
            find(&recap(&battles, &earlier(40, 0, 40)), "przetrwanie_odsetek"),
            None
        );
    }

    #[test]
    fn most_played_tank_needs_more_than_half_of_the_battles() {
        // Author decision 2026-11-16: a tie of 1 in 2 (or 2 in 4) is no "most played" tank.
        let mut battles = session_battles(2);
        battles[0].vehicle_key = "synth-b".into();
        assert_eq!(find(&recap(&battles, &[]), "czolg"), None);
        let mut battles = session_battles(4);
        battles[0].vehicle_key = "synth-b".into();
        battles[0].tank_name = None;
        battles[1].vehicle_key = "synth-b".into();
        battles[1].tank_name = Some("Synth B".into());
        assert_eq!(find(&recap(&battles, &[]), "czolg"), None);
        battles[2].vehicle_key = "synth-b".into();
        battles[2].tank_name = None;
        assert_eq!(
            find(&recap(&battles, &[]), "czolg"),
            Some(&SessionFactValue::Tank {
                vehicle_key: "synth-b".into(),
                tank_name: Some("Synth B".into()),
                battles: 3,
                total: 4
            })
        );
        battles[3].vehicle_key = "synth-b".into();
        assert!(matches!(
            find(&recap(&battles, &[]), "czolg"),
            Some(SessionFactValue::Tank {
                battles: 4,
                total: 4,
                ..
            })
        ));
    }

    #[test]
    fn wn8_reference_battles_are_the_thirty_days_before_the_start() {
        let mut history = vec![
            battle("in", START - 30 * DAY_MS, "old"),
            battle("out", START - 30 * DAY_MS - 1, "old"),
            battle("now", START, "other"),
            battle("own", START - 1, "s"),
            battle("last", START - 1, "old"),
        ];
        history.rotate_left(1);
        let ids = wn8_reference_battles(&history, "s", START)
            .into_iter()
            .map(|battle| battle.arena_id)
            .collect::<Vec<_>>();
        assert_eq!(ids, ["last", "in"]);
    }

    #[test]
    fn json_uses_the_spec_keys() {
        let comparison = damage_comparison(1_200.0, 1_000.0, 3);
        let value = serde_json::to_value(session_recap(&SessionInput {
            session_id: "s",
            started_at: START,
            battles: &session_battles(3),
            history: &earlier(40, 20, 0),
            comparison: Some(&comparison),
            wn8: None,
        }))
        .unwrap();
        assert_eq!(value["kind"], "session");
        assert_eq!(value["seed"], seed("s"));
        assert_eq!(value["facts"][0]["id"], "F1");
        assert_eq!(value["facts"][0]["fact"]["key"], "liczba_bitew");
        assert_eq!(value["facts"][1]["fact"]["key"], "odsetek_wygranych");
        assert_eq!(value["facts"][1]["fact"]["reference"]["difference"], 50);
        assert_eq!(value["facts"][2]["fact"]["key"], "obrazenia");
        assert_eq!(value["facts"][3]["fact"]["mapName"], "Synth Map");
        assert_eq!(value["facts"][4]["fact"]["key"], "przetrwanie_odsetek");
        assert_eq!(value["facts"][5]["fact"]["vehicleKey"], "synth-a");
    }

    // --- Battle (spec 30 §4.1) ---

    fn counted_battle() -> Battle {
        let mut value = battle("b-1", START, "s");
        value.arena_unique_id = Some(4_242);
        value.damage = Some(2_028);
        value.frags = Some(0);
        value.assist = Some(0);
        value.blocked = Some(0);
        value.spots = Some(0);
        value.shots = None;
        value
    }

    fn battle_input(battle: &Battle) -> BattleInput<'_> {
        BattleInput {
            battle,
            class: VehicleClass::MediumTank,
            tier: Some(8),
            damage_reference: Some(1_959.4),
            assist_reference: None,
            place: None,
            team_damage: None,
        }
    }

    fn battle_facts(input: &BattleInput) -> Vec<BattleFact> {
        let Recap::Battle { facts, .. } = battle_recap(input) else {
            panic!("battle recap");
        };
        facts
    }

    fn battle_keys(facts: &[BattleFact]) -> Vec<String> {
        facts
            .iter()
            .map(|fact| {
                serde_json::to_value(&fact.fact).unwrap()["key"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    }

    fn rest_with(change: impl Fn(&mut Battle), class: VehicleClass) -> Vec<(String, bool)> {
        let mut battle = counted_battle();
        change(&mut battle);
        let mut input = battle_input(&battle);
        input.class = class;
        let facts = battle_facts(&input);
        facts[5..]
            .iter()
            .zip(battle_keys(&facts)[5..].iter())
            .map(|(fact, key)| (key.clone(), fact.important))
            .collect()
    }

    fn kept(key: &str, important: bool) -> Vec<(String, bool)> {
        vec![(key.to_owned(), important)]
    }

    #[test]
    fn battle_opens_with_result_map_tank_survival_and_damage() {
        let battle = counted_battle();
        let facts = battle_facts(&battle_input(&battle));
        assert_eq!(
            battle_keys(&facts),
            ["wynik", "mapa", "czolg", "przetrwanie", "obrazenia"]
        );
        let important = facts.iter().map(|fact| fact.important).collect::<Vec<_>>();
        assert_eq!(important, [true, false, false, true, true]);
        assert_eq!(
            facts[4].fact,
            BattleFactValue::Damage {
                damage: 2_028,
                reference: Some(1_959)
            }
        );
        assert_eq!(
            facts[2].fact,
            BattleFactValue::Tank {
                vehicle_key: "synth-a".into(),
                tank_name: Some("Synth A".into()),
                tier: Some(8)
            }
        );
        let ids = facts
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["F1", "F2", "F3", "F4", "F5"]);
    }

    #[test]
    fn battle_seed_follows_the_arena_id_and_falls_back_to_the_battle_id() {
        let mut battle = counted_battle();
        let Recap::Battle {
            seed: with_arena, ..
        } = battle_recap(&battle_input(&battle))
        else {
            panic!("battle");
        };
        assert_eq!(with_arena, seed("4242"));
        battle.arena_unique_id = None;
        let Recap::Battle { seed: fallback, .. } = battle_recap(&battle_input(&battle)) else {
            panic!("battle");
        };
        assert_eq!(fallback, seed("b-1"));
    }

    #[test]
    fn a_battle_that_does_not_count_has_no_facts() {
        let mut battle = counted_battle();
        battle.battle_pace.bonus_type_raw = Some(2);
        assert!(battle_facts(&battle_input(&battle)).is_empty());
    }

    #[test]
    fn unknown_outcome_map_survival_or_damage_leave_their_fact_out() {
        let mut battle = counted_battle();
        battle.outcome = BattleOutcome::Unknown;
        battle.map_name = None;
        battle.survived = None;
        battle.damage = None;
        assert_eq!(
            battle_keys(&battle_facts(&battle_input(&battle))),
            ["czolg"]
        );
        let mut input_battle = counted_battle();
        input_battle.outcome = BattleOutcome::Draw;
        let mut input = battle_input(&input_battle);
        input.damage_reference = None;
        let facts = battle_facts(&input);
        assert_eq!(
            facts[0].fact,
            BattleFactValue::Outcome {
                outcome: BattleOutcome::Draw
            }
        );
        assert_eq!(
            facts[4].fact,
            BattleFactValue::Damage {
                damage: 2_028,
                reference: None
            }
        );
    }

    #[test]
    fn destroyed_vehicles_from_one_and_important_from_two() {
        let medium = VehicleClass::MediumTank;
        assert!(rest_with(|b| b.frags = Some(0), medium).is_empty());
        assert_eq!(
            rest_with(|b| b.frags = Some(1), medium),
            kept("zniszczone", false)
        );
        assert_eq!(
            rest_with(|b| b.frags = Some(2), medium),
            kept("zniszczone", true)
        );
    }

    #[test]
    fn assist_from_800_unless_light_where_it_is_always_kept_and_important() {
        let medium = VehicleClass::MediumTank;
        assert!(rest_with(|b| b.assist = Some(799), medium).is_empty());
        assert_eq!(
            rest_with(|b| b.assist = Some(800), medium),
            kept("asysta", false)
        );
        let light = rest_with(|b| b.assist = Some(10), VehicleClass::LightTank);
        assert_eq!(light[0], ("asysta".to_owned(), true));
    }

    #[test]
    fn spotted_from_three_unless_light() {
        let medium = VehicleClass::MediumTank;
        assert!(rest_with(|b| b.spots = Some(2), medium).is_empty());
        assert_eq!(
            rest_with(|b| b.spots = Some(3), medium),
            kept("wykryte", false)
        );
        let light = rest_with(|b| b.assist = Some(900), VehicleClass::LightTank);
        assert!(light.contains(&("wykryte".to_owned(), false)));
    }

    #[test]
    fn blocked_damage_from_500_or_1000_on_heavy_tanks_important_above_1000() {
        let medium = VehicleClass::MediumTank;
        let heavy = VehicleClass::HeavyTank;
        assert!(rest_with(|b| b.blocked = Some(499), medium).is_empty());
        assert_eq!(
            rest_with(|b| b.blocked = Some(500), medium),
            kept("pancerz", false)
        );
        assert_eq!(
            rest_with(|b| b.blocked = Some(5_000), medium),
            kept("pancerz", false)
        );
        assert!(rest_with(|b| b.blocked = Some(999), heavy).is_empty());
        assert_eq!(
            rest_with(|b| b.blocked = Some(1_000), heavy),
            kept("pancerz", false)
        );
        assert_eq!(
            rest_with(|b| b.blocked = Some(1_001), heavy),
            kept("pancerz", true)
        );
    }

    #[test]
    fn place_up_to_third_and_share_from_twenty_percent_only_without_a_place() {
        let battle = counted_battle();
        let rest = |place: Option<i64>, team: Option<i64>| {
            let mut input = battle_input(&battle);
            input.place = place;
            input.team_damage = team;
            let facts = battle_facts(&input);
            facts[5..]
                .iter()
                .map(|fact| fact.fact.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            rest(Some(3), Some(5_000)),
            [BattleFactValue::Place { place: 3 }]
        );
        // 2 028 of 10 140 is exactly 20%; of 10 141 it rounds to 20%, of 10 700 to 19%.
        assert_eq!(
            rest(Some(4), Some(10_140)),
            [BattleFactValue::Share { percent: 20 }]
        );
        assert!(rest(Some(4), Some(10_700)).is_empty());
        assert!(rest(None, Some(0)).is_empty());
        assert!(rest(None, None).is_empty());
        let mut input = battle_input(&battle);
        input.place = Some(1);
        assert!(battle_facts(&input)[5].important);
    }

    #[test]
    fn shots_use_hits_and_penetrations_on_enemies_like_the_screen() {
        let facts = rest_with(
            |b| {
                b.shots = Some(18);
                b.hits = Some(14);
                b.direct_enemy_hits = Some(12);
                b.penetrations = Some(9);
                b.piercing_enemy_hits = None;
            },
            VehicleClass::MediumTank,
        );
        assert_eq!(facts, kept("strzaly", false));
        let mut battle = counted_battle();
        battle.shots = Some(18);
        battle.hits = None;
        battle.direct_enemy_hits = Some(12);
        battle.penetrations = Some(9);
        let all = battle_facts(&battle_input(&battle));
        assert_eq!(
            all[5].fact,
            BattleFactValue::Shots {
                shots: 18,
                hits: 12,
                penetrations: 9
            }
        );
    }

    #[test]
    fn rest_keeps_important_first_within_three_sentences_with_place_beside_destroyed() {
        let mut battle = counted_battle();
        battle.frags = Some(2);
        battle.assist = Some(900);
        battle.blocked = Some(600);
        battle.spots = Some(4);
        battle.shots = Some(10);
        let mut input = battle_input(&battle);
        input.place = Some(2);
        let facts = battle_facts(&input);
        assert_eq!(
            battle_keys(&facts)[5..],
            ["zniszczone", "miejsce", "asysta", "pancerz"]
        );
        // Without destroyed vehicles the place is a sentence of its own.
        battle.frags = Some(0);
        let mut input = battle_input(&battle);
        input.place = Some(2);
        assert_eq!(
            battle_keys(&battle_facts(&input))[5..],
            ["miejsce", "asysta", "pancerz"]
        );
    }

    #[test]
    fn battle_json_uses_the_spec_keys() {
        let mut battle = counted_battle();
        battle.frags = Some(2);
        let value = serde_json::to_value(battle_recap(&battle_input(&battle))).unwrap();
        assert_eq!(value["kind"], "battle");
        assert_eq!(value["facts"][0]["fact"]["outcome"], "victory");
        assert_eq!(value["facts"][1]["fact"]["mapName"], "Synth Map");
        assert_eq!(value["facts"][3]["fact"]["survived"], true);
        assert_eq!(value["facts"][5]["fact"]["key"], "zniszczone");
    }

    // --- Week (spec 30 §4.3) ---

    fn goal(kind: GoalKind, state: GoalState) -> crate::weekly::WeeklyGoal {
        crate::weekly::WeeklyGoal {
            kind,
            target: 35.0,
            basis: 32.0,
            vehicle_id: None,
            vehicle_key: None,
            tank_name: None,
            current: Some(40.0),
            battles: 20,
            survived: None,
            state,
        }
    }

    fn report(battles: usize) -> WeeklyReport {
        WeeklyReport {
            week_start: 1_800_000_000_000,
            week_end: 1_800_604_800_000,
            battles,
            confidence: crate::weekly::confidence(battles),
            indicators: vec![
                crate::weekly::WeeklyIndicator {
                    metric: WeeklyMetric::DamageAssist,
                    current: Some(104.6),
                    previous: vec![],
                },
                crate::weekly::WeeklyIndicator {
                    metric: WeeklyMetric::Survival,
                    current: Some(95.4),
                    previous: vec![],
                },
                crate::weekly::WeeklyIndicator {
                    metric: WeeklyMetric::Xp,
                    current: None,
                    previous: vec![],
                },
            ],
            win_rate: Some(55.4),
            goals: vec![
                goal(GoalKind::Survival, GoalState::Achieved),
                goal(GoalKind::Xp, GoalState::TooFew),
            ],
            previous_week: crate::weekly::PreviousWeek {
                week_start: 1_799_395_200_000,
                battles: 30,
                goals: vec![
                    goal(GoalKind::Survival, GoalState::Achieved),
                    goal(GoalKind::Xp, GoalState::Below),
                    goal(GoalKind::TankDamageAssist, GoalState::TooFew),
                ],
            },
            challenges: crate::weekly::WeekChallenges {
                completed: 1,
                failed: 0,
                abandoned: 0,
            },
            wn8: None,
        }
    }

    fn week_facts(report: &WeeklyReport) -> Vec<WeekFact> {
        let Recap::Week { facts, .. } = week_recap(report) else {
            panic!("week recap");
        };
        facts
    }

    #[test]
    fn a_week_under_fifteen_battles_says_only_the_battles() {
        let facts = week_facts(&report(14));
        assert_eq!(
            facts,
            [WeekFact {
                id: "F1".into(),
                important: true,
                fact: WeekFactValue::Battles {
                    battles: 14,
                    confidence: WeekConfidence::TooFew,
                    minimum: 15
                }
            }]
        );
    }

    #[test]
    fn a_week_keeps_indicators_from_five_points_judged_goals_previous_week_and_challenges() {
        let facts = week_facts(&report(15));
        let values = facts
            .iter()
            .map(|fact| fact.fact.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            [
                WeekFactValue::Battles {
                    battles: 15,
                    confidence: WeekConfidence::Preliminary,
                    minimum: 15
                },
                WeekFactValue::WinRate { percent: 55 },
                WeekFactValue::Indicator {
                    metric: WeeklyMetric::DamageAssist,
                    percent: 105,
                    usual: 100
                },
                WeekFactValue::Indicator {
                    metric: WeeklyMetric::Survival,
                    percent: 95,
                    usual: 100
                },
                WeekFactValue::Goal {
                    kind: GoalKind::Survival,
                    state: GoalState::Achieved,
                    vehicle_key: None,
                    tank_name: None
                },
                WeekFactValue::PreviousWeek {
                    achieved: 1,
                    goals: 3
                },
                WeekFactValue::Challenges {
                    completed: 1,
                    failed: 0,
                    abandoned: 0,
                    battles: 5
                },
            ]
        );
        let ids = facts
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["F1", "F2", "F3", "F4", "F5", "F6", "F7"]);
        let important = facts.iter().map(|fact| fact.important).collect::<Vec<_>>();
        assert_eq!(important, [true, false, true, true, true, false, false]);
    }

    #[test]
    fn indicators_within_five_points_are_left_out() {
        let at = |current: f64| {
            let mut week = report(30);
            week.indicators.truncate(1);
            week.indicators[0].current = Some(current);
            week_facts(&week)
                .iter()
                .any(|fact| matches!(fact.fact, WeekFactValue::Indicator { .. }))
        };
        assert!(at(105.0));
        assert!(at(95.0));
        assert!(!at(104.4));
        assert!(!at(95.6));
    }

    #[test]
    fn empty_challenges_previous_week_and_win_rate_are_left_out() {
        let mut week = report(30);
        week.challenges = crate::weekly::WeekChallenges {
            completed: 0,
            failed: 0,
            abandoned: 0,
        };
        week.previous_week.goals.clear();
        week.win_rate = None;
        week.goals.clear();
        week.indicators.clear();
        assert_eq!(week_facts(&week).len(), 1);
        let Recap::Week {
            seed: week_seed, ..
        } = week_recap(&week)
        else {
            panic!("week");
        };
        assert_eq!(week_seed, seed("1800000000000"));
    }
}
