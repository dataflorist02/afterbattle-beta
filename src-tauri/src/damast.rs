//! Damast (spec 34): what the plate of an ended session is made of. Read only: the session's
//! counted battles with their tier, damage, WN8 and result, and the player's own averages from
//! the last 100 battles before the session. Nothing here is stored or changes a statistic; the
//! pattern itself is drawn by the window (`src/features/damast/`).

use crate::{
    baseline::damast_reference,
    error::AppError,
    expected_values::LoadedExpectedSnapshot,
    models::{
        Battle, BattleOutcome, DamastBattle, DamastSession, DamastSessionCursor, DamastSessionPage,
        Session, Wn8Calculation, stats_eligibility,
    },
    ratings::calculate_battle_wn8,
    session_wn8_presentation,
    storage::{Database, metrics},
};
use std::collections::HashMap;

/// Sessions per page of History → Sessions.
pub(crate) const DAMAST_PAGE_MAX: usize = 60;

fn battle_wn8(battle: &Battle, expected: Option<&LoadedExpectedSnapshot>) -> Option<f64> {
    match calculate_battle_wn8(battle, expected?) {
        Wn8Calculation::Available(result) => Some(result.value),
        Wn8Calculation::Unavailable(_) => None,
    }
}

fn plate_battle(
    battle: &Battle,
    tiers: &HashMap<i64, u8>,
    expected: Option<&LoadedExpectedSnapshot>,
) -> DamastBattle {
    DamastBattle {
        battle_id: battle.arena_id.clone(),
        battle_time: battle.battle_time,
        tier: battle.wg_tank_id.and_then(|id| tiers.get(&id).copied()),
        damage: battle.damage,
        wn8: battle_wn8(battle, expected),
        win: battle.outcome == BattleOutcome::Victory,
    }
}

/// The plate of one session, whatever its status (the screen forges only ended ones): the
/// counted battles (D4) in the order they were played.
pub(crate) fn damast_session(
    database: &Database,
    session: Session,
    tiers: &HashMap<i64, u8>,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<DamastSession, AppError> {
    let battles = database.session_battles(&session.id)?;
    let summary = metrics(&battles);
    let mut counted = battles
        .iter()
        .filter(|battle| stats_eligibility(battle))
        .collect::<Vec<_>>();
    counted.sort_by(|left, right| {
        left.battle_time
            .cmp(&right.battle_time)
            .then_with(|| left.arena_id.cmp(&right.arena_id))
    });
    let plate = counted
        .into_iter()
        .map(|battle| plate_battle(battle, tiers, expected))
        .collect();
    let reference = damast_reference(database, &session, tiers, expected)?;
    Ok(DamastSession {
        wn8: session_wn8_presentation(&battles, expected).value,
        average_damage: summary.average_damage,
        session,
        battles: plate,
        reference,
    })
}

/// A page of ended sessions that have counted battles, newest first; `limit` is kept within
/// 1..=[`DAMAST_PAGE_MAX`].
pub(crate) fn damast_sessions_page(
    database: &Database,
    cursor: Option<&DamastSessionCursor>,
    limit: usize,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<DamastSessionPage, AppError> {
    let limit = limit.clamp(1, DAMAST_PAGE_MAX);
    let before = cursor.map(|cursor| (cursor.started_at, cursor.session_id.as_str()));
    let mut sessions = database.ended_sessions_page(before, limit + 1)?;
    let more = sessions.len() > limit;
    sessions.truncate(limit);
    let next_cursor = more
        .then(|| sessions.last())
        .flatten()
        .map(|session| DamastSessionCursor {
            started_at: session.started_at,
            session_id: session.id.clone(),
        });
    let tiers = database.vehicle_tiers()?;
    let items = sessions
        .into_iter()
        .map(|session| damast_session(database, session, &tiers, expected))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DamastSessionPage { items, next_cursor })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        baseline::player_baseline,
        expected_values::validate_snapshot_bytes,
        models::{BaselineWindowKind, DataQuality, SourceKind},
        storage::Database,
    };
    use serde_json::json;
    use tempfile::tempdir;

    /// Minutes of the default inactivity limit, in milliseconds (a gap this long starts a session).
    const HOUR: i64 = 60 * 60_000;

    fn snapshot() -> LoadedExpectedSnapshot {
        let rows = (1..=500)
            .map(|id| {
                json!({
                    "IDNum": id,
                    "expDef": 0.5,
                    "expFrag": 0.8,
                    "expSpot": 1.1,
                    "expDamage": 900.0 + id as f64 * 50.0,
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

    fn battle(id: &str, time: i64, vehicle_id: i64, damage: i64, win: bool) -> Battle {
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
            outcome: if win {
                BattleOutcome::Victory
            } else {
                BattleOutcome::Defeat
            },
            battle_pace: crate::models::strict_random_pace(),
            player_team: Some(1),
            winner_team_raw: Some(if win { 1 } else { 2 }),
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
            survived: Some(true),
            xp: Some(900),
            original_xp: Some(900),
            duration_seconds: None,
            session_id: None,
        }
    }

    /// Tanks 1–3 are tier 10, 9 and 8; tank 7 has no tier.
    fn set_tiers(path: &std::path::Path) {
        let connection = rusqlite::Connection::open(path).unwrap();
        for (tank, tier) in [(1, 10), (2, 9), (3, 8)] {
            connection
                .execute(
                    "UPDATE battles SET vehicle_tier=?2 WHERE wg_tank_id=?1",
                    rusqlite::params![tank, tier],
                )
                .unwrap();
        }
    }

    /// 130 history battles before the session, one per minute: tank 1 (tier X) for the first
    /// 60, then tank 2 (tier IX) and every 13th battle tank 3 (tier VIII); then a session of
    /// five battles an hour later, with one training battle in between (not counted).
    fn history_and_session() -> (tempfile::TempDir, Database, String) {
        let temp = tempdir().unwrap();
        let path = temp.path().join("damast.sqlite3");
        let database = Database::initialize(path.clone()).unwrap();
        for index in 0..130_i64 {
            let tank = if index % 13 == 0 {
                3
            } else if index < 60 {
                1
            } else {
                2
            };
            let mut value = battle(
                &format!("history-{index:03}"),
                index * 60_000,
                tank,
                1_000 + index * 10,
                index % 2 == 0,
            );
            database.insert_battle(&mut value, false).unwrap();
        }
        let start = 130 * 60_000 + HOUR;
        let session = [
            ("s1", 0, 1, 2_400, true),
            ("s2", 1, 2, 900, false),
            ("s3", 2, 7, 1_500, true),
            ("s5", 4, 3, 3_000, false),
            ("s4", 3, 1, 1_800, true),
        ];
        let mut session_id = None;
        for (id, minute, tank, damage, win) in session {
            let mut value = battle(id, start + minute * 60_000, tank, damage, win);
            database.insert_battle(&mut value, true).unwrap();
            session_id = value.session_id.clone();
        }
        let mut training = battle("s-training", start + 150_000, 1, 9_000, true);
        training.battle_type = Some("training".to_owned());
        training.battle_pace.battle_type_raw = Some(2);
        training.battle_pace.bonus_type_raw = Some(2);
        database.insert_battle(&mut training, true).unwrap();
        set_tiers(&path);
        (temp, database, session_id.unwrap())
    }

    #[test]
    fn the_plate_lists_the_counted_battles_in_order_with_tier_wn8_and_result() {
        let (_temp, database, session_id) = history_and_session();
        let expected = snapshot();
        let tiers = database.vehicle_tiers().unwrap();
        let session = database.session(&session_id).unwrap();
        let plate = damast_session(&database, session, &tiers, Some(&expected)).unwrap();

        let ids = plate
            .battles
            .iter()
            .map(|battle| battle.battle_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["s1", "s2", "s3", "s4", "s5"], "no training battle");
        let tiers_seen = plate.battles.iter().map(|b| b.tier).collect::<Vec<_>>();
        assert_eq!(tiers_seen, [Some(10), Some(9), None, Some(10), Some(8)]);
        let wins = plate.battles.iter().map(|b| b.win).collect::<Vec<_>>();
        assert_eq!(wins, [true, false, true, true, false]);
        let damage = plate.battles.iter().map(|b| b.damage).collect::<Vec<_>>();
        assert_eq!(
            damage,
            [
                Some(2_400),
                Some(900),
                Some(1_500),
                Some(1_800),
                Some(3_000)
            ]
        );
        // Every battle's WN8 is the WN8 of that battle alone (tank 7 is in the snapshot too).
        let all = database.session_battles(&session_id).unwrap();
        for item in &plate.battles {
            let source = all.iter().find(|b| b.arena_id == item.battle_id).unwrap();
            let Wn8Calculation::Available(alone) = calculate_battle_wn8(source, &expected) else {
                panic!("WN8 of {}", item.battle_id);
            };
            assert_eq!(item.wn8, Some(alone.value), "{}", item.battle_id);
        }
        // The session's numbers are the ones of the Session screen.
        assert_eq!(
            plate.wn8,
            session_wn8_presentation(&all, Some(&expected)).value
        );
        assert_eq!(plate.average_damage, metrics(&all).average_damage);
        assert_eq!(plate.average_damage, Some(1_920.0));
    }

    #[test]
    fn without_expected_values_the_plate_keeps_damage_and_has_no_wn8() {
        let (_temp, database, session_id) = history_and_session();
        let tiers = database.vehicle_tiers().unwrap();
        let session = database.session(&session_id).unwrap();
        let plate = damast_session(&database, session, &tiers, None).unwrap();
        assert_eq!(plate.battles.len(), 5);
        assert!(plate.battles.iter().all(|battle| battle.wn8.is_none()));
        assert!(plate.battles.iter().all(|battle| battle.damage.is_some()));
        assert_eq!(plate.wn8, None);
        assert_eq!(plate.reference.wn8, None);
        assert_eq!(plate.reference.damage_battles, 100);
    }

    #[test]
    fn the_reference_is_the_last_100_battles_before_the_session_by_tier() {
        let (_temp, database, session_id) = history_and_session();
        let expected = snapshot();
        let tiers = database.vehicle_tiers().unwrap();
        let session = database.session(&session_id).unwrap();
        let started_at = session.started_at;
        let plate = damast_session(&database, session, &tiers, Some(&expected)).unwrap();
        let reference = &plate.reference;

        // The same numbers as the player's last-100 baseline at the session's start.
        let baseline =
            player_baseline(&database, started_at, Some(&session_id), Some(&expected)).unwrap();
        let last100 = baseline
            .windows
            .iter()
            .find(|window| window.window == BaselineWindowKind::Last100)
            .unwrap();
        assert_eq!(reference.cutoff, started_at);
        assert_eq!(reference.wn8, last100.wn8.value);
        assert!(reference.wn8.is_some());
        assert_eq!(reference.wn8_battles, last100.wn8.eligible_battles);
        assert_eq!(reference.average_damage, last100.average_damage.value);
        assert_eq!(reference.damage_battles, 100);

        // Battles 30..=129 by hand: tank 3 every 13th (tier VIII), tank 1 until 59 (X), else IX.
        let mut sums = std::collections::BTreeMap::<u8, (usize, i64)>::new();
        for index in 30..130_i64 {
            let tier = if index % 13 == 0 {
                8
            } else if index < 60 {
                10
            } else {
                9
            };
            let entry = sums.entry(tier).or_default();
            entry.0 += 1;
            entry.1 += 1_000 + index * 10;
        }
        let by_hand = sums
            .into_iter()
            .map(|(tier, (count, sum))| (tier, count, sum as f64 / count as f64))
            .collect::<Vec<_>>();
        let got = reference
            .tiers
            .iter()
            .map(|tier| (tier.tier, tier.battles, tier.average_damage))
            .collect::<Vec<_>>();
        assert_eq!(got, by_hand);
        // Tier VIII has 7 battles in the window, fewer than 10: the screen falls back (spec 34 §2).
        assert_eq!(got[0], (8, 7, got[0].2));
        assert_eq!(got.iter().map(|tier| tier.1).sum::<usize>(), 100);
    }

    #[test]
    fn the_reference_skips_battles_whose_tank_has_no_tier() {
        let (_temp, database, session_id) = history_and_session();
        let session = database.session(&session_id).unwrap();
        // Without any tier known every battle still counts for the all-tier average.
        let plate = damast_session(&database, session, &HashMap::new(), None).unwrap();
        assert!(plate.reference.tiers.is_empty());
        assert_eq!(plate.reference.damage_battles, 100);
        assert!(plate.battles.iter().all(|battle| battle.tier.is_none()));
    }

    fn sessions_database() -> (tempfile::TempDir, Database) {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join("pages.sqlite3")).unwrap();
        // Four sessions two hours apart; the last one is still running.
        for session in 0..4_i64 {
            for battle_index in 0..2_i64 {
                let mut value = battle(
                    &format!("p{session}-{battle_index}"),
                    session * 2 * HOUR + battle_index * 60_000,
                    1,
                    1_000,
                    true,
                );
                database.insert_battle(&mut value, true).unwrap();
            }
        }
        // A fifth session with only a training battle: nothing counted, no plate.
        (temp, database)
    }

    #[test]
    fn pages_hold_ended_sessions_newest_first_and_never_the_running_one() {
        let (_temp, database) = sessions_database();
        let first = damast_sessions_page(&database, None, 2, None).unwrap();
        let started = |page: &DamastSessionPage| {
            page.items
                .iter()
                .map(|item| item.session.started_at)
                .collect::<Vec<_>>()
        };
        assert_eq!(started(&first), [4 * HOUR, 2 * HOUR]);
        assert!(
            first
                .items
                .iter()
                .all(|item| item.session.status != "active")
        );
        assert!(first.items.iter().all(|item| item.battles.len() == 2));
        let cursor = first.next_cursor.clone().unwrap();
        assert_eq!(cursor.started_at, 2 * HOUR);
        let second = damast_sessions_page(&database, Some(&cursor), 2, None).unwrap();
        assert_eq!(started(&second), [0]);
        assert_eq!(second.next_cursor, None);
    }

    /// rc.3 mutation (`sessions.len() > limit` → `>=`): a page that holds exactly the last
    /// sessions has no next page.
    #[test]
    fn a_page_of_exactly_the_remaining_sessions_has_no_next_cursor() {
        let (_temp, database) = sessions_database();
        let page = damast_sessions_page(&database, None, 3, None).unwrap();
        assert_eq!(page.items.len(), 3);
        assert_eq!(page.next_cursor, None);
    }

    #[test]
    fn a_page_is_never_larger_than_the_maximum() {
        let (_temp, database) = sessions_database();
        let page = damast_sessions_page(&database, None, 10_000, None).unwrap();
        assert_eq!(page.items.len(), 3);
        let empty = damast_sessions_page(&database, None, 0, None).unwrap();
        assert_eq!(empty.items.len(), 1, "a page holds at least one session");
    }
}
