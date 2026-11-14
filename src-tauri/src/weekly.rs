//! Weekly report (spec 20 E7, spec 27): this ISO week (Monday to Sunday, local time) against
//! the four weeks before it, measured against the player's own average on the same tanks, with
//! 2–3 weekly goals from a fixed rule catalog and an evaluation of last week's goals. Counted
//! battles only (D4). Win rate is context, never a goal (D12). Nothing is stored: every number is
//! computed from the battles, so the report is the same however often it is asked for.

use std::collections::{BTreeMap, HashMap};

use chrono::{Datelike, Duration, NaiveTime, TimeZone};
use serde::Serialize;

use crate::models::{Battle, BattleOutcome, stats_eligibility};

/// A week with fewer counted battles has no trend ("too few battles").
pub(crate) const MIN_WEEK_BATTLES: usize = 15;
/// From this many battles the week's confidence is "good" (below: "preliminary").
pub(crate) const GOOD_WEEK_BATTLES: usize = 30;
/// Earlier battles of a tank that make its own average (newest first, like D4).
pub(crate) const BASELINE_WINDOW: usize = 100;
/// A tank's own average counts from this many earlier battles.
pub(crate) const MIN_BASELINE_BATTLES: usize = 10;
pub(crate) const PREVIOUS_WEEKS: usize = 4;
/// Goals need this many battles in the four weeks before.
pub(crate) const MIN_GOAL_HISTORY: usize = 15;
/// Survival goal: weeks with enough battles before, at least this many.
pub(crate) const MIN_SURVIVAL_WEEKS: usize = 2;
pub(crate) const SURVIVAL_GOAL_POINTS: f64 = 5.0;
/// Damage plus assist and XP goals: the average plus 10%, rounded to a whole number.
pub(crate) const RAISE_GOAL_FACTOR: f64 = 1.10;
/// Tank goal: the most played tank of the four weeks with at least this many battles there.
pub(crate) const MIN_TANK_GOAL_HISTORY: usize = 15;
/// A goal is judged from this many battles in the week (the tank goal: on that tank).
pub(crate) const MIN_GOAL_BATTLES: usize = 10;
pub(crate) const MIN_TANK_GOAL_BATTLES: usize = 5;
/// "Close" from this share of the target.
pub(crate) const CLOSE_SHARE: f64 = 0.95;

const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const WEEK_HOURS: i64 = 7 * 24;

/// Start (Monday 00:00 local time) of the ISO week holding `at`, in epoch milliseconds.
pub(crate) fn week_start<Tz: TimeZone>(at: i64, zone: &Tz) -> i64 {
    let local = chrono::DateTime::from_timestamp_millis(at)
        .unwrap_or_default()
        .with_timezone(zone);
    let monday =
        local.date_naive() - Duration::days(i64::from(local.weekday().num_days_from_monday()));
    let midnight = monday.and_time(NaiveTime::MIN);
    // A clock change at midnight (even a skipped day): the first valid hour of that week.
    (0..WEEK_HOURS)
        .find_map(|hour| {
            zone.from_local_datetime(&(midnight + Duration::hours(hour)))
                .earliest()
        })
        .map_or(at, |value| value.timestamp_millis())
}

/// Start of the week `offset` weeks before the one starting at `start` (local Mondays).
fn earlier_week<Tz: TimeZone>(start: i64, offset: usize, zone: &Tz) -> i64 {
    let mut value = start;
    for _ in 0..offset {
        // The last moment of the previous week finds its Monday, also across DST changes.
        value = week_start(value - 1, zone);
    }
    value
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WeeklyMetric {
    DamageAssist,
    Survival,
    Xp,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WeekConfidence {
    /// Fewer than 15 battles: "too few battles for a trend".
    TooFew,
    Preliminary,
    Good,
}

pub(crate) fn confidence(battles: usize) -> WeekConfidence {
    if battles < MIN_WEEK_BATTLES {
        WeekConfidence::TooFew
    } else if battles < GOOD_WEEK_BATTLES {
        WeekConfidence::Preliminary
    } else {
        WeekConfidence::Good
    }
}

fn metric_value(battle: &Battle, metric: WeeklyMetric) -> Option<f64> {
    match metric {
        WeeklyMetric::DamageAssist => Some((battle.damage? + battle.assist?) as f64),
        WeeklyMetric::Survival => battle.survived.map(|value| if value { 1.0 } else { 0.0 }),
        WeeklyMetric::Xp => battle.original_xp.map(|value| value as f64),
    }
}

/// Counted battles sorted oldest first (ties by id).
fn counted(battles: &[Battle]) -> Vec<&Battle> {
    let mut list = battles
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .collect::<Vec<_>>();
    list.sort_by(|left, right| {
        left.battle_time
            .cmp(&right.battle_time)
            .then_with(|| left.arena_id.cmp(&right.arena_id))
    });
    list
}

fn in_range<'a>(battles: &[&'a Battle], from: i64, to: i64) -> Vec<&'a Battle> {
    battles
        .iter()
        .filter(|battle| battle.battle_time >= from && battle.battle_time < to)
        .copied()
        .collect()
}

/// The tank's own average of `metric` over its last 100 counted battles before `before`, or
/// `None` below 10 such battles.
fn own_average(
    battles: &[&Battle],
    vehicle: i64,
    metric: WeeklyMetric,
    before: i64,
) -> Option<f64> {
    let values = battles
        .iter()
        .rev()
        .filter(|battle| battle.battle_time < before && battle.wg_tank_id == Some(vehicle))
        .filter_map(|battle| metric_value(battle, metric))
        .take(BASELINE_WINDOW)
        .collect::<Vec<_>>();
    (values.len() >= MIN_BASELINE_BATTLES).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// The week's result against the own average on the same tanks, in percent (100 = as usual):
/// the sum of the week's values over the sum of each battle's tank average. `None` with fewer
/// than 15 counted battles or without any comparable battle.
pub(crate) fn week_index(
    all: &[&Battle],
    week: &[&Battle],
    week_from: i64,
    metric: WeeklyMetric,
) -> Option<f64> {
    if week.len() < MIN_WEEK_BATTLES {
        return None;
    }
    let mut averages = HashMap::<i64, Option<f64>>::new();
    let (mut actual, mut expected) = (0.0, 0.0);
    for battle in week {
        let (Some(vehicle), Some(value)) = (battle.wg_tank_id, metric_value(battle, metric)) else {
            continue;
        };
        let average = *averages
            .entry(vehicle)
            .or_insert_with(|| own_average(all, vehicle, metric, week_from));
        if let Some(average) = average {
            actual += value;
            expected += average;
        }
    }
    (expected > 0.0).then(|| actual / expected * 100.0)
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeeklyIndicator {
    pub metric: WeeklyMetric,
    /// This week, percent of the own average; `None`: too few battles.
    pub current: Option<f64>,
    /// The four weeks before, newest first.
    pub previous: Vec<Option<f64>>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GoalKind {
    /// Survive at least the target share of battles.
    Survival,
    /// On one tank, average damage plus assist of at least the target.
    TankDamageAssist,
    /// Average XP of at least the target.
    Xp,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GoalState {
    /// Fewer battles than needed to judge (10 in the week, 5 on the tank).
    TooFew,
    Achieved,
    /// At least 95% of the target.
    Close,
    Below,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeeklyGoal {
    pub kind: GoalKind,
    pub target: f64,
    /// Where the target comes from: the median (survival) or average of the four weeks before.
    pub basis: f64,
    pub vehicle_id: Option<i64>,
    pub vehicle_key: Option<String>,
    pub tank_name: Option<String>,
    /// The week's value so far (survival in percent).
    pub current: Option<f64>,
    /// Battles that count for this goal in the week (on the tank for the tank goal).
    pub battles: usize,
    /// Survival only: survived battles in the week.
    pub survived: Option<usize>,
    pub state: GoalState,
}

fn on_vehicle<'a>(battles: &[&'a Battle], vehicle: i64) -> Vec<&'a Battle> {
    battles
        .iter()
        .filter(|battle| battle.wg_tank_id == Some(vehicle))
        .copied()
        .collect()
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    })
}

fn judge(current: Option<f64>, target: f64, battles: usize, needed: usize) -> GoalState {
    match current {
        _ if battles < needed => GoalState::TooFew,
        Some(value) if value >= target => GoalState::Achieved,
        Some(value) if value >= target * CLOSE_SHARE => GoalState::Close,
        _ => GoalState::Below,
    }
}

fn values(battles: &[&Battle], metric: WeeklyMetric) -> Vec<f64> {
    battles
        .iter()
        .filter_map(|battle| metric_value(battle, metric))
        .collect()
}

/// Goals for the week `[from, to)`, set from the four weeks before it and judged on that week.
/// The catalog has three rules; win rate is never a goal (D12).
pub(crate) fn goals<Tz: TimeZone>(
    all: &[&Battle],
    from: i64,
    to: i64,
    zone: &Tz,
) -> Vec<WeeklyGoal> {
    let history_from = earlier_week(from, PREVIOUS_WEEKS, zone);
    let history = in_range(all, history_from, from);
    if history.len() < MIN_GOAL_HISTORY {
        return Vec::new();
    }
    let week = in_range(all, from, to);
    let mut result = Vec::new();

    // 1. Survival: the median of the weekly survival rates of weeks with 15 battles, plus 5 points.
    let mut rates = (1..=PREVIOUS_WEEKS)
        .filter_map(|offset| {
            let start = earlier_week(from, offset, zone);
            let end = earlier_week(from, offset - 1, zone);
            let survival = values(&in_range(all, start, end), WeeklyMetric::Survival);
            (survival.len() >= MIN_WEEK_BATTLES).then(|| mean(&survival).unwrap_or(0.0) * 100.0)
        })
        .collect::<Vec<_>>();
    if rates.len() >= MIN_SURVIVAL_WEEKS
        && let Some(basis) = median(&mut rates)
    {
        let target = (basis + SURVIVAL_GOAL_POINTS).min(100.0);
        let survival = values(&week, WeeklyMetric::Survival);
        let current = mean(&survival).map(|value| value * 100.0);
        result.push(WeeklyGoal {
            kind: GoalKind::Survival,
            target,
            basis,
            vehicle_id: None,
            vehicle_key: None,
            tank_name: None,
            current,
            battles: survival.len(),
            survived: Some(survival.iter().filter(|value| **value > 0.0).count()),
            state: judge(current, target, survival.len(), MIN_GOAL_BATTLES),
        });
    }

    // 2. Damage plus assist on the most played tank of the four weeks (15 battles there), +10%.
    let mut counts = BTreeMap::<i64, usize>::new();
    for battle in &history {
        if let Some(vehicle) = battle.wg_tank_id {
            *counts.entry(vehicle).or_default() += 1;
        }
    }
    // Most battles; equal counts: the lower vehicle id (BTreeMap order, first maximum kept).
    let tank = counts
        .iter()
        .fold(None::<(i64, usize)>, |best, (vehicle, count)| match best {
            Some((_, most)) if most >= *count => best,
            _ => Some((*vehicle, *count)),
        })
        .filter(|(_, count)| *count >= MIN_TANK_GOAL_HISTORY);
    if let Some((vehicle, _)) = tank
        && let Some(basis) = mean(&values(
            &on_vehicle(&history, vehicle),
            WeeklyMetric::DamageAssist,
        ))
    {
        let target = (basis * RAISE_GOAL_FACTOR).round();
        let week_values = values(&on_vehicle(&week, vehicle), WeeklyMetric::DamageAssist);
        let current = mean(&week_values);
        let newest = history
            .iter()
            .rev()
            .find(|battle| battle.wg_tank_id == Some(vehicle));
        result.push(WeeklyGoal {
            kind: GoalKind::TankDamageAssist,
            target,
            basis,
            vehicle_id: Some(vehicle),
            vehicle_key: newest.map(|battle| battle.vehicle_key.clone()),
            tank_name: newest.and_then(|battle| battle.tank_name.clone()),
            current,
            battles: week_values.len(),
            survived: None,
            state: judge(current, target, week_values.len(), MIN_TANK_GOAL_BATTLES),
        });
    }

    // 3. XP: the average of the four weeks plus 10%.
    let xp = values(&history, WeeklyMetric::Xp);
    if xp.len() >= MIN_GOAL_HISTORY
        && let Some(basis) = mean(&xp)
    {
        let target = (basis * RAISE_GOAL_FACTOR).round();
        let week_values = values(&week, WeeklyMetric::Xp);
        let current = mean(&week_values);
        result.push(WeeklyGoal {
            kind: GoalKind::Xp,
            target,
            basis,
            vehicle_id: None,
            vehicle_key: None,
            tank_name: None,
            current,
            battles: week_values.len(),
            survived: None,
            state: judge(current, target, week_values.len(), MIN_GOAL_BATTLES),
        });
    }
    result
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeekChallenges {
    pub completed: usize,
    pub failed: usize,
    pub abandoned: usize,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviousWeek {
    pub week_start: i64,
    pub battles: usize,
    pub goals: Vec<WeeklyGoal>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeeklyReport {
    pub week_start: i64,
    /// Exclusive end: the next Monday 00:00 local time.
    pub week_end: i64,
    pub battles: usize,
    pub confidence: WeekConfidence,
    pub indicators: Vec<WeeklyIndicator>,
    /// Context only (D12): this week's win rate from battles with a known outcome.
    pub win_rate: Option<f64>,
    pub goals: Vec<WeeklyGoal>,
    pub previous_week: PreviousWeek,
    pub challenges: WeekChallenges,
    /// WN8 of this week and of the four weeks before (newest first), with XVM data only.
    pub wn8: Option<WeeklyWn8>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WeeklyWn8 {
    pub current: Option<f64>,
    pub previous: Vec<Option<f64>>,
}

/// Counted battles of this week and the four weeks before (newest first), for WN8 outside this
/// module (it needs the expected values).
pub(crate) fn week_battles<Tz: TimeZone>(
    battles: &[Battle],
    now: i64,
    zone: &Tz,
) -> Vec<Vec<Battle>> {
    let all = counted(battles);
    let from = week_start(now, zone);
    let mut weeks = vec![in_range(
        &all,
        from,
        week_start(from + WEEK_MS + WEEK_MS / 2, zone),
    )];
    for offset in 1..=PREVIOUS_WEEKS {
        weeks.push(in_range(
            &all,
            earlier_week(from, offset, zone),
            earlier_week(from, offset - 1, zone),
        ));
    }
    weeks
        .into_iter()
        .map(|week| week.into_iter().cloned().collect())
        .collect()
}

fn win_rate(week: &[&Battle]) -> Option<f64> {
    let known = week
        .iter()
        .filter(|battle| battle.outcome != BattleOutcome::Unknown)
        .collect::<Vec<_>>();
    (!known.is_empty()).then(|| {
        known
            .iter()
            .filter(|battle| battle.outcome == BattleOutcome::Victory)
            .count() as f64
            / known.len() as f64
            * 100.0
    })
}

/// Challenges finished in the week: (status, finished at) of finished challenges.
pub(crate) fn week_challenges(finished: &[(&str, i64)], from: i64, to: i64) -> WeekChallenges {
    let mut result = WeekChallenges {
        completed: 0,
        failed: 0,
        abandoned: 0,
    };
    for (status, at) in finished {
        if *at < from || *at >= to {
            continue;
        }
        match *status {
            "completed" => result.completed += 1,
            "failed" => result.failed += 1,
            "abandoned" => result.abandoned += 1,
            _ => {}
        }
    }
    result
}

pub(crate) fn weekly_report<Tz: TimeZone>(
    battles: &[Battle],
    finished_challenges: &[(&str, i64)],
    now: i64,
    zone: &Tz,
) -> WeeklyReport {
    let all = counted(battles);
    let from = week_start(now, zone);
    // Half a week after Monday plus a week is inside the next week, also across DST.
    let to = week_start(from + WEEK_MS + WEEK_MS / 2, zone);
    let week = in_range(&all, from, to);
    let indicators = [
        WeeklyMetric::DamageAssist,
        WeeklyMetric::Survival,
        WeeklyMetric::Xp,
    ]
    .into_iter()
    .map(|metric| WeeklyIndicator {
        metric,
        current: week_index(&all, &week, from, metric),
        previous: (1..=PREVIOUS_WEEKS)
            .map(|offset| {
                let start = earlier_week(from, offset, zone);
                let end = earlier_week(from, offset - 1, zone);
                week_index(&all, &in_range(&all, start, end), start, metric)
            })
            .collect(),
    })
    .collect();
    let last_from = earlier_week(from, 1, zone);
    WeeklyReport {
        week_start: from,
        week_end: to,
        battles: week.len(),
        confidence: confidence(week.len()),
        indicators,
        win_rate: win_rate(&week),
        goals: goals(&all, from, to, zone),
        previous_week: PreviousWeek {
            week_start: last_from,
            battles: in_range(&all, last_from, from).len(),
            goals: goals(&all, last_from, from, zone),
        },
        challenges: week_challenges(finished_challenges, from, to),
        wn8: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::tests::full_replay_battle;
    use chrono::{FixedOffset, NaiveDate, NaiveDateTime};

    const HOUR: i64 = 60 * 60 * 1000;
    const DAY: i64 = 24 * HOUR;

    fn zone() -> FixedOffset {
        FixedOffset::east_opt(3_600).unwrap()
    }

    /// Local time (UTC+1) of a day in November 2026 as epoch milliseconds.
    fn local(day: u32, hour: u32, minute: u32) -> i64 {
        zone()
            .from_local_datetime(
                &NaiveDate::from_ymd_opt(2026, 11, day)
                    .unwrap()
                    .and_hms_opt(hour, minute, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis()
    }

    /// Monday 9 November 2026 00:00 local: the week of the report in most tests.
    fn this_week() -> i64 {
        local(9, 0, 0)
    }

    fn battle(
        id: usize,
        at: i64,
        vehicle: i64,
        damage: i64,
        assist: i64,
        survived: bool,
        xp: i64,
    ) -> Battle {
        let mut value = full_replay_battle(&format!("w-{id:04}"));
        value.battle_time = at;
        value.wg_tank_id = Some(vehicle);
        value.vehicle_key = format!("synth-v{vehicle}");
        value.tank_name = Some(format!("Synth {vehicle}"));
        value.damage = Some(damage);
        value.assist = Some(assist);
        value.survived = Some(survived);
        value.original_xp = Some(xp);
        value.outcome = if id.is_multiple_of(2) {
            BattleOutcome::Victory
        } else {
            BattleOutcome::Defeat
        };
        value
    }

    /// `count` battles in the week starting `weeks_back` weeks before this week, one per hour
    /// from Tuesday noon.
    fn week_of(
        first_id: usize,
        weeks_back: i64,
        count: usize,
        vehicle: i64,
        damage: i64,
        survive_every: usize,
        xp: i64,
    ) -> Vec<Battle> {
        let start = this_week() - weeks_back * WEEK_MS + DAY + 12 * HOUR;
        (0..count)
            .map(|index| {
                battle(
                    first_id + index,
                    start + index as i64 * HOUR,
                    vehicle,
                    damage,
                    200,
                    survive_every > 0 && index % survive_every == 0,
                    xp,
                )
            })
            .collect()
    }

    #[test]
    fn iso_weeks_start_on_monday_midnight_local_time() {
        assert_eq!(week_start(local(11, 15, 0), &zone()), this_week());
        assert_eq!(week_start(local(9, 0, 10), &zone()), this_week());
        // Sunday 23:30 local is still the previous week, though it is 22:30 UTC.
        assert_eq!(week_start(local(8, 23, 30), &zone()), this_week() - WEEK_MS);
        assert_eq!(week_start(local(15, 23, 59), &zone()), this_week());
        assert_eq!(week_start(local(16, 0, 0), &zone()), this_week() + WEEK_MS);
        assert_eq!(
            earlier_week(this_week(), 4, &zone()),
            this_week() - 4 * WEEK_MS
        );
        assert_eq!(earlier_week(this_week(), 0, &zone()), this_week());
    }

    /// Spring forward on Sunday 29 March 2026, 02:00 → 03:00 local (UTC+1 → UTC+2).
    #[derive(Clone, Copy)]
    struct SpringZone;
    impl TimeZone for SpringZone {
        type Offset = FixedOffset;
        fn from_offset(_: &FixedOffset) -> Self {
            SpringZone
        }
        fn offset_from_local_date(&self, _: &NaiveDate) -> chrono::LocalResult<FixedOffset> {
            chrono::LocalResult::Single(FixedOffset::east_opt(3_600).unwrap())
        }
        fn offset_from_local_datetime(
            &self,
            local: &NaiveDateTime,
        ) -> chrono::LocalResult<FixedOffset> {
            let change = NaiveDate::from_ymd_opt(2026, 3, 29)
                .unwrap()
                .and_hms_opt(2, 0, 0)
                .unwrap();
            if *local < change {
                chrono::LocalResult::Single(FixedOffset::east_opt(3_600).unwrap())
            } else if *local < change + Duration::hours(1) {
                chrono::LocalResult::None
            } else {
                chrono::LocalResult::Single(FixedOffset::east_opt(7_200).unwrap())
            }
        }
        fn offset_from_utc_date(&self, _: &NaiveDate) -> FixedOffset {
            FixedOffset::east_opt(3_600).unwrap()
        }
        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            let change = NaiveDate::from_ymd_opt(2026, 3, 29)
                .unwrap()
                .and_hms_opt(1, 0, 0)
                .unwrap();
            FixedOffset::east_opt(if *utc < change { 3_600 } else { 7_200 }).unwrap()
        }
    }

    #[test]
    fn week_boundaries_follow_local_time_across_a_dst_change() {
        let utc = |text: &str| {
            NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M")
                .unwrap()
                .and_utc()
                .timestamp_millis()
        };
        // Tuesday 31 March 2026 noon (summer time): its Monday is 30 March 00:00 at UTC+2.
        assert_eq!(
            week_start(utc("2026-03-31 10:00"), &SpringZone),
            utc("2026-03-29 22:00")
        );
        // The week before started on Monday 23 March 00:00 at UTC+1: 167 hours earlier.
        assert_eq!(
            earlier_week(utc("2026-03-29 22:00"), 1, &SpringZone),
            utc("2026-03-22 23:00")
        );
        let report = weekly_report(&[], &[], utc("2026-03-27 12:00"), &SpringZone);
        assert_eq!(report.week_start, utc("2026-03-22 23:00"));
        assert_eq!(report.week_end, utc("2026-03-29 22:00"));
    }

    /// Clocks jump `HOURS` hours forward exactly at Monday 30 March 2026 00:00 local, from
    /// UTC−11 (like Samoa skipping a whole day with `HOURS` = 24).
    #[derive(Clone, Copy)]
    struct MidnightGapZone<const HOURS: i32>;
    impl<const HOURS: i32> MidnightGapZone<HOURS> {
        const BEFORE: i32 = -11 * 3_600;
        fn gap_start() -> NaiveDateTime {
            NaiveDate::from_ymd_opt(2026, 3, 30)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        }
        fn offset(seconds: i32) -> FixedOffset {
            FixedOffset::east_opt(seconds).unwrap()
        }
    }
    impl<const HOURS: i32> TimeZone for MidnightGapZone<HOURS> {
        type Offset = FixedOffset;
        fn from_offset(_: &FixedOffset) -> Self {
            MidnightGapZone
        }
        fn offset_from_local_date(&self, _: &NaiveDate) -> chrono::LocalResult<FixedOffset> {
            chrono::LocalResult::Single(Self::offset(Self::BEFORE))
        }
        fn offset_from_local_datetime(
            &self,
            local: &NaiveDateTime,
        ) -> chrono::LocalResult<FixedOffset> {
            let start = Self::gap_start();
            if *local < start {
                chrono::LocalResult::Single(Self::offset(Self::BEFORE))
            } else if *local < start + Duration::hours(i64::from(HOURS)) {
                chrono::LocalResult::None
            } else {
                chrono::LocalResult::Single(Self::offset(Self::BEFORE + HOURS * 3_600))
            }
        }
        fn offset_from_utc_date(&self, _: &NaiveDate) -> FixedOffset {
            Self::offset(Self::BEFORE)
        }
        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            let change = Self::gap_start() + Duration::hours(11);
            Self::offset(if *utc < change {
                Self::BEFORE
            } else {
                Self::BEFORE + HOURS * 3_600
            })
        }
    }

    #[test]
    fn a_monday_midnight_lost_to_a_clock_change_starts_the_week_at_its_first_valid_moment() {
        let utc = |text: &str| {
            NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M")
                .unwrap()
                .and_utc()
                .timestamp_millis()
        };
        // Wednesday 1 April 2026 noon local; the clock change happened at 11:00 UTC on Monday.
        let change = utc("2026-03-30 11:00");
        assert_eq!(
            week_start(utc("2026-04-01 12:00") + 10 * HOUR, &MidnightGapZone::<1>),
            change
        );
        assert_eq!(
            week_start(utc("2026-04-01 12:00") + 9 * HOUR, &MidnightGapZone::<2>),
            change
        );
        // The whole Monday is skipped: the week starts on Tuesday 00:00.
        assert_eq!(
            week_start(utc("2026-04-01 12:00") - 13 * HOUR, &MidnightGapZone::<24>),
            change
        );
        // The week before the change starts on Monday 23 March 00:00 at UTC−11.
        assert_eq!(
            earlier_week(change, 1, &MidnightGapZone::<24>),
            utc("2026-03-23 11:00")
        );
    }

    #[test]
    fn a_battle_at_monday_midnight_belongs_to_the_new_week_and_not_to_the_own_average() {
        let mut battles = week_of(0, 1, 15, 1, 1_800, 2, 1_000);
        // Exactly at the start of this week, and exactly at the start of the next one.
        battles.push(battle(500, this_week(), 1, 49_800, 200, true, 1_000));
        battles.push(battle(
            501,
            this_week() + WEEK_MS,
            1,
            49_800,
            200,
            true,
            1_000,
        ));
        let all = counted(&battles);
        let week = in_range(&all, this_week(), this_week() + WEEK_MS);
        assert_eq!(
            week.iter().map(|item| item.battle_time).collect::<Vec<_>>(),
            [this_week()]
        );
        assert_eq!(
            own_average(&all, 1, WeeklyMetric::DamageAssist, this_week()),
            Some(2_000.0)
        );
        let report = weekly_report(&battles, &[], local(12, 12, 0), &zone());
        assert_eq!(report.battles, 1);
        assert_eq!(report.previous_week.battles, 15);
    }

    /// Fall back on Sunday 25 October 2026, 03:00 → 02:00 local (UTC+2 → UTC+1): a 169-hour week.
    #[derive(Clone, Copy)]
    struct FallZone;
    impl FallZone {
        fn offset(summer: bool) -> FixedOffset {
            FixedOffset::east_opt(if summer { 7_200 } else { 3_600 }).unwrap()
        }
    }
    impl TimeZone for FallZone {
        type Offset = FixedOffset;
        fn from_offset(_: &FixedOffset) -> Self {
            FallZone
        }
        fn offset_from_local_date(&self, _: &NaiveDate) -> chrono::LocalResult<FixedOffset> {
            chrono::LocalResult::Single(Self::offset(true))
        }
        fn offset_from_local_datetime(
            &self,
            local: &NaiveDateTime,
        ) -> chrono::LocalResult<FixedOffset> {
            let change = NaiveDate::from_ymd_opt(2026, 10, 25)
                .unwrap()
                .and_hms_opt(2, 0, 0)
                .unwrap();
            if *local < change {
                chrono::LocalResult::Single(Self::offset(true))
            } else if *local < change + Duration::hours(1) {
                chrono::LocalResult::Ambiguous(Self::offset(true), Self::offset(false))
            } else {
                chrono::LocalResult::Single(Self::offset(false))
            }
        }
        fn offset_from_utc_date(&self, _: &NaiveDate) -> FixedOffset {
            Self::offset(true)
        }
        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            let change = NaiveDate::from_ymd_opt(2026, 10, 25)
                .unwrap()
                .and_hms_opt(1, 0, 0)
                .unwrap();
            Self::offset(*utc < change)
        }
    }

    #[test]
    fn a_week_with_an_extra_hour_ends_at_the_next_local_monday() {
        let utc = |text: &str| {
            NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M")
                .unwrap()
                .and_utc()
                .timestamp_millis()
        };
        let report = weekly_report(&[], &[], utc("2026-10-21 10:00"), &FallZone);
        assert_eq!(report.week_start, utc("2026-10-18 22:00"));
        assert_eq!(report.week_end, utc("2026-10-25 23:00"));
    }

    #[test]
    fn the_previous_weeks_of_each_indicator_are_measured_in_their_own_week() {
        // Two weeks back: the own average (2 000 with assist); last week: 3 000, i.e. 150%.
        let mut battles = week_of(0, 2, 15, 1, 1_800, 2, 1_000);
        battles.extend(week_of(100, 1, 15, 1, 2_800, 2, 1_000));
        let report = weekly_report(&battles, &[], local(12, 12, 0), &zone());
        let damage = &report.indicators[0];
        assert_eq!(damage.previous.len(), PREVIOUS_WEEKS);
        assert!((damage.previous[0].unwrap() - 150.0).abs() < 1e-9);
        assert_eq!(damage.previous[1], None, "no own average before that week");
        assert_eq!(&damage.previous[2..], [None, None]);
    }

    #[test]
    fn the_tank_goal_takes_the_most_played_tank_even_with_a_higher_id() {
        let mut battles = week_of(0, 1, 5, 1, 2_800, 2, 1_200);
        battles.extend(week_of(100, 2, 20, 2, 2_800, 2, 1_200));
        let report = weekly_report(&battles, &[], local(10, 12, 0), &zone());
        assert_eq!(
            goal(&report.goals, GoalKind::TankDamageAssist).vehicle_id,
            Some(2)
        );
    }

    #[test]
    fn the_survival_goal_shows_this_weeks_share_in_percent() {
        let mut battles = history();
        battles.extend(week_of(900, 0, 10, 1, 2_800, 2, 1_200));
        let report = weekly_report(&battles, &[], local(15, 12, 0), &zone());
        let survival = goal(&report.goals, GoalKind::Survival);
        assert_eq!((survival.current, survival.battles), (Some(50.0), 10));
        assert_eq!(survival.survived, Some(5));
    }

    #[test]
    fn confidence_follows_the_number_of_battles() {
        assert_eq!(confidence(0), WeekConfidence::TooFew);
        assert_eq!(confidence(14), WeekConfidence::TooFew);
        assert_eq!(confidence(15), WeekConfidence::Preliminary);
        assert_eq!(confidence(29), WeekConfidence::Preliminary);
        assert_eq!(confidence(30), WeekConfidence::Good);
    }

    #[test]
    fn a_week_under_fifteen_battles_has_no_trend() {
        let mut battles = week_of(0, 1, 20, 1, 2_000, 2, 1_000);
        battles.extend(week_of(100, 0, 14, 1, 3_000, 2, 1_000));
        let report = weekly_report(&battles, &[], local(12, 12, 0), &zone());
        assert_eq!(report.battles, 14);
        assert_eq!(report.confidence, WeekConfidence::TooFew);
        assert!(
            report
                .indicators
                .iter()
                .all(|indicator| indicator.current.is_none())
        );
        battles.push(battle(500, local(13, 12, 0), 1, 3_000, 200, true, 1_000));
        let report = weekly_report(&battles, &[], local(13, 13, 0), &zone());
        assert_eq!(report.confidence, WeekConfidence::Preliminary);
        let damage = &report.indicators[0];
        assert_eq!(damage.metric, WeeklyMetric::DamageAssist);
        // 3 200 against an own average of 2 200 on the same tank.
        assert!((damage.current.unwrap() - 3_200.0 / 2_200.0 * 100.0).abs() < 1e-9);
        assert_eq!(damage.previous.len(), 4);
    }

    #[test]
    fn the_index_compares_each_battle_with_its_own_tank_and_skips_tanks_without_ten_earlier_battles()
     {
        let mut battles = week_of(0, 1, 10, 1, 1_800, 2, 1_000); // tank 1 average 2 000 with assist
        battles.extend(week_of(50, 2, 10, 2, 800, 2, 1_000)); // tank 2 average 1 000
        battles.extend(week_of(100, 0, 8, 1, 2_800, 2, 1_000)); // 3 000 = 150%
        battles.extend(week_of(200, 0, 8, 2, 300, 2, 1_000)); // 500 = 50%
        let mut fresh = week_of(300, 0, 4, 3, 9_800, 2, 1_000); // tank 3 has no own average
        for item in &mut fresh {
            item.battle_time += 20 * HOUR;
        }
        battles.extend(fresh);
        let all = counted(&battles);
        let week = in_range(&all, this_week(), this_week() + WEEK_MS);
        assert_eq!(week.len(), 20);
        let index = week_index(&all, &week, this_week(), WeeklyMetric::DamageAssist).unwrap();
        assert!(
            (index - (8.0 * 3_000.0 + 8.0 * 500.0) / (8.0 * 2_000.0 + 8.0 * 1_000.0) * 100.0).abs()
                < 1e-9
        );
        // Survival: half of the battles survived before, all now on tank 1.
        let mut survived = battles.clone();
        for item in survived
            .iter_mut()
            .filter(|item| item.battle_time >= this_week())
        {
            item.survived = Some(true);
        }
        let all = counted(&survived);
        let week = in_range(&all, this_week(), this_week() + WEEK_MS);
        assert!(
            (week_index(&all, &week, this_week(), WeeklyMetric::Survival).unwrap() - 200.0).abs()
                < 1e-9
        );
        assert_eq!(
            week_index(&all, &week[..14], this_week(), WeeklyMetric::Xp),
            None
        );
        let none = week_of(400, 0, 15, 9, 1_000, 2, 1_000);
        let all = counted(&none);
        assert_eq!(
            week_index(&all, &all, this_week(), WeeklyMetric::Xp),
            None,
            "no own average at all"
        );
    }

    #[test]
    fn own_average_uses_the_last_hundred_earlier_battles_of_that_tank() {
        let mut battles = (0..120)
            .map(|index| {
                battle(
                    index,
                    this_week() - (200 - index as i64) * HOUR,
                    1,
                    if index < 20 { 9_000 } else { 1_000 },
                    0,
                    true,
                    0,
                )
            })
            .collect::<Vec<_>>();
        battles.push(battle(999, this_week() + HOUR, 1, 50_000, 0, true, 0));
        let all = counted(&battles);
        assert_eq!(
            own_average(&all, 1, WeeklyMetric::DamageAssist, this_week()),
            Some(1_000.0)
        );
        assert_eq!(
            own_average(&all, 2, WeeklyMetric::DamageAssist, this_week()),
            None
        );
        let nine = counted(&battles[..9]);
        assert_eq!(
            own_average(&nine, 1, WeeklyMetric::DamageAssist, this_week()),
            None
        );
        let ten = counted(&battles[..10]);
        assert_eq!(
            own_average(&ten, 1, WeeklyMetric::DamageAssist, this_week()),
            Some(9_000.0)
        );
    }

    fn history() -> Vec<Battle> {
        // Four earlier weeks: survival 20%, 40%, 30%, 50% (median 35%), tank 1 the most played.
        let mut battles = Vec::new();
        for (offset, survive_every) in [(1, 5), (2, 0), (3, 0), (4, 2)] {
            battles.extend(week_of(
                offset as usize * 100,
                offset,
                20,
                1,
                2_800,
                survive_every,
                1_200,
            ));
        }
        // Weeks 2 and 3 survive 40% and 30% (8 and 6 of 20).
        for (offset, survivors) in [(2, 8), (3, 6)] {
            for (index, item) in battles
                .iter_mut()
                .filter(|item| item.arena_id.starts_with(&format!("w-0{offset}")))
                .enumerate()
            {
                item.survived = Some(index < survivors);
            }
        }
        battles
    }

    fn goal(goals: &[WeeklyGoal], kind: GoalKind) -> &WeeklyGoal {
        goals.iter().find(|goal| goal.kind == kind).expect("goal")
    }

    #[test]
    fn goals_come_from_the_four_weeks_before_and_never_from_win_rate() {
        let battles = history();
        let report = weekly_report(&battles, &[], local(10, 12, 0), &zone());
        assert_eq!(
            report
                .goals
                .iter()
                .map(|goal| goal.kind)
                .collect::<Vec<_>>(),
            [GoalKind::Survival, GoalKind::TankDamageAssist, GoalKind::Xp]
        );
        let survival = goal(&report.goals, GoalKind::Survival);
        assert_eq!((survival.basis, survival.target), (35.0, 40.0));
        let tank = goal(&report.goals, GoalKind::TankDamageAssist);
        assert_eq!(tank.vehicle_id, Some(1));
        assert_eq!(tank.vehicle_key.as_deref(), Some("synth-v1"));
        assert!((tank.basis - 3_000.0).abs() < 1e-9 && (tank.target - 3_300.0).abs() < 1e-9);
        let xp = goal(&report.goals, GoalKind::Xp);
        assert!((xp.target - 1_320.0).abs() < 1e-9);
        assert!(
            report
                .goals
                .iter()
                .all(|goal| goal.state == GoalState::TooFew)
        );
        // D12: the serialized kinds never name win rate.
        let json = serde_json::to_string(&report.goals).unwrap().to_lowercase();
        assert!(!json.contains("win"));
        assert!(report.win_rate.is_none());
    }

    #[test]
    fn goals_are_judged_on_the_week_with_minimum_battles() {
        let mut battles = history();
        // This week: 12 battles on tank 1, 3 300 + assist, 5 of 12 survived (41.7%), XP 1 300.
        let mut week = week_of(900, 0, 12, 1, 3_100, 0, 1_300);
        for (index, item) in week.iter_mut().enumerate() {
            item.survived = Some(index < 5);
        }
        battles.extend(week);
        let report = weekly_report(&battles, &[], local(14, 12, 0), &zone());
        let survival = goal(&report.goals, GoalKind::Survival);
        assert_eq!((survival.battles, survival.survived), (12, Some(5)));
        assert_eq!(survival.state, GoalState::Achieved);
        assert_eq!(
            goal(&report.goals, GoalKind::TankDamageAssist).state,
            GoalState::Achieved
        );
        let xp = goal(&report.goals, GoalKind::Xp);
        assert!((xp.current.unwrap() - 1_300.0).abs() < 1e-9);
        assert_eq!(xp.state, GoalState::Close, "1 300 of 1 320 is at least 95%");
        assert_eq!(judge(Some(1_000.0), 1_320.0, 12, 10), GoalState::Below);
        assert_eq!(judge(Some(1_254.0), 1_320.0, 12, 10), GoalState::Close);
        assert_eq!(judge(Some(1_253.0), 1_320.0, 12, 10), GoalState::Below);
        assert_eq!(judge(Some(1_320.0), 1_320.0, 10, 10), GoalState::Achieved);
        assert_eq!(judge(Some(9_999.0), 1_320.0, 9, 10), GoalState::TooFew);
        assert_eq!(judge(None, 1_320.0, 10, 10), GoalState::Below);
    }

    #[test]
    fn goals_need_enough_history_and_each_rule_its_own_minimum() {
        // 14 battles in the four weeks: no goals at all.
        let few = week_of(0, 1, 14, 1, 2_000, 2, 1_000);
        assert!(
            weekly_report(&few, &[], local(10, 12, 0), &zone())
                .goals
                .is_empty()
        );
        // 15 battles in one week only: survival needs two weeks, the tank goal 15 on one tank.
        let one = week_of(0, 1, 15, 1, 2_000, 2, 1_000);
        let kinds = weekly_report(&one, &[], local(10, 12, 0), &zone())
            .goals
            .iter()
            .map(|goal| goal.kind)
            .collect::<Vec<_>>();
        assert_eq!(kinds, [GoalKind::TankDamageAssist, GoalKind::Xp]);
        // Spread over tanks: no tank has 15 battles.
        let mut spread = week_of(0, 1, 14, 1, 2_000, 2, 1_000);
        spread.extend(week_of(50, 2, 14, 2, 2_000, 2, 1_000));
        let kinds = weekly_report(&spread, &[], local(10, 12, 0), &zone())
            .goals
            .iter()
            .map(|goal| goal.kind)
            .collect::<Vec<_>>();
        assert_eq!(kinds, [GoalKind::Xp]);
        // Equal counts: the lower vehicle id; the survival target is capped at 100%.
        let mut tie = week_of(0, 1, 15, 7, 2_000, 1, 1_000);
        tie.extend(week_of(50, 2, 15, 3, 2_000, 1, 1_000));
        let report = weekly_report(&tie, &[], local(10, 12, 0), &zone());
        assert_eq!(
            goal(&report.goals, GoalKind::TankDamageAssist).vehicle_id,
            Some(3)
        );
        assert_eq!(goal(&report.goals, GoalKind::Survival).target, 100.0);
        // Missing XP values keep the XP goal away.
        let mut no_xp = week_of(0, 1, 20, 1, 2_000, 2, 1_000);
        for item in &mut no_xp {
            item.original_xp = None;
        }
        assert!(
            weekly_report(&no_xp, &[], local(10, 12, 0), &zone())
                .goals
                .iter()
                .all(|goal| goal.kind != GoalKind::Xp)
        );
    }

    #[test]
    fn the_previous_week_is_judged_with_goals_from_the_four_weeks_before_it() {
        let mut battles = history();
        // Shift the history one week back so last week is judged against it.
        for item in &mut battles {
            item.battle_time -= WEEK_MS;
        }
        let mut last = week_of(800, 1, 16, 1, 3_200, 2, 1_400);
        for (index, item) in last.iter_mut().enumerate() {
            item.survived = Some(index < 8);
        }
        battles.extend(last);
        let report = weekly_report(&battles, &[], local(10, 12, 0), &zone());
        assert_eq!(report.previous_week.week_start, this_week() - WEEK_MS);
        assert_eq!(report.previous_week.battles, 16);
        let states = report
            .previous_week
            .goals
            .iter()
            .map(|goal| (goal.kind, goal.state))
            .collect::<Vec<_>>();
        assert_eq!(
            states,
            [
                (GoalKind::Survival, GoalState::Achieved),
                (GoalKind::TankDamageAssist, GoalState::Achieved),
                (GoalKind::Xp, GoalState::Achieved),
            ]
        );
    }

    #[test]
    fn win_rate_is_context_from_known_outcomes_and_challenges_are_counted_in_the_week() {
        let mut week = week_of(0, 0, 4, 1, 2_000, 2, 1_000);
        week[3].outcome = BattleOutcome::Unknown;
        let all = counted(&week);
        assert!((win_rate(&all).unwrap() - 200.0 / 3.0).abs() < 1e-9);
        assert_eq!(win_rate(&[]), None);
        let finished = [
            ("completed", this_week() + DAY),
            ("failed", this_week() + 2 * DAY),
            ("abandoned", this_week() + 3 * DAY),
            ("completed", this_week() - 1),
            ("completed", this_week() + WEEK_MS),
            ("active", this_week() + DAY),
            // Exactly at Monday 00:00: this week.
            ("failed", this_week()),
        ];
        assert_eq!(
            week_challenges(&finished, this_week(), this_week() + WEEK_MS),
            WeekChallenges {
                completed: 1,
                failed: 2,
                abandoned: 1
            }
        );
    }

    #[test]
    fn partial_and_non_random_battles_never_count() {
        let mut battles = week_of(0, 0, 20, 1, 2_000, 2, 1_000);
        battles[0].data_quality = crate::models::DataQuality::Partial;
        battles[1].battle_pace.bonus_type_raw = Some(2);
        let report = weekly_report(&battles, &[], local(12, 12, 0), &zone());
        assert_eq!(report.battles, 18);
    }

    #[test]
    fn week_battles_split_this_week_and_the_four_before_newest_first() {
        let mut battles = Vec::new();
        for offset in 0..6 {
            battles.extend(week_of(
                offset as usize * 100,
                offset,
                offset as usize + 1,
                1,
                1_000,
                2,
                1_000,
            ));
        }
        let weeks = week_battles(&battles, local(12, 12, 0), &zone());
        assert_eq!(
            weeks.iter().map(Vec::len).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn median_and_mean_handle_even_odd_and_empty() {
        assert_eq!(median(&mut []), None);
        assert_eq!(median(&mut [3.0]), Some(3.0));
        assert_eq!(median(&mut [4.0, 1.0, 3.0]), Some(3.0));
        assert_eq!(median(&mut [4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(mean(&[]), None);
        assert_eq!(mean(&[1.0, 2.0]), Some(1.5));
    }
}
