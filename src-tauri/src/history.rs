use crate::{
    battle_wn8_presentation,
    error::AppError,
    expected_values::LoadedExpectedSnapshot,
    models::{
        Battle, BattleHistoryCursor, BattleHistoryFilter, BattleHistoryItem, BattleHistoryPage,
        HistoryWn8Filter, VehicleHistoryOverview, VehicleHistoryReport, VehicleHistorySession,
        VehicleHistorySummary, VehicleMapSummary, Wn8PresentationState, stats_eligibility,
    },
    session_wn8_presentation,
    storage::{Database, HistoryBattleRow, metrics},
    vehicle_class_catalog::LoadedVehicleClassCatalog,
};
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_PAGE_SIZE: usize = 30;
const MAX_PAGE_SIZE: usize = 50;
const SCAN_CHUNK_SIZE: usize = 200;

pub(crate) fn safe_identity_value(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }
    let basename = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let lower = basename.to_ascii_lowercase();
    let replay_timestamp = basename.len() >= 14
        && basename.as_bytes()[..8].iter().all(u8::is_ascii_digit)
        && basename.as_bytes()[8] == b'_'
        && basename.as_bytes()[9..13].iter().all(u8::is_ascii_digit)
        && basename.as_bytes()[13] == b'_';
    if lower.ends_with(".wotreplay") || replay_timestamp {
        None
    } else {
        Some(basename.to_owned())
    }
}

fn history_item(
    row: HistoryBattleRow,
    expected: Option<&LoadedExpectedSnapshot>,
) -> BattleHistoryItem {
    let battle = row.battle;
    let wn8 = battle_wn8_presentation(&battle, expected);
    BattleHistoryItem {
        battle_id: battle.arena_id,
        session_id: battle.session_id,
        session_started_at: row.session_started_at,
        session_status: row.session_status,
        vehicle_id: battle.wg_tank_id,
        vehicle_key: safe_identity_value(Some(&battle.vehicle_key)),
        tank_name: safe_identity_value(battle.tank_name.as_deref()),
        map_name: safe_identity_value(battle.map_name.as_deref()),
        map_key: safe_identity_value(battle.map_key.as_deref()),
        battle_time: battle.battle_time,
        outcome: battle.outcome,
        battle_pace: battle.battle_pace,
        duration_seconds: battle.duration_seconds,
        damage: battle.damage,
        assist: battle.assist,
        frags: battle.frags,
        xp: battle.xp,
        survived: battle.survived,
        data_quality: battle.data_quality,
        wn8,
    }
}

fn matches_wn8(item: &BattleHistoryItem, filter: HistoryWn8Filter) -> bool {
    match filter {
        HistoryWn8Filter::All => true,
        HistoryWn8Filter::Available => item.wn8.state == Wn8PresentationState::Available,
        HistoryWn8Filter::Unavailable => item.wn8.state == Wn8PresentationState::Unavailable,
    }
}

pub(crate) fn battle_history_page(
    database: &Database,
    filter: &BattleHistoryFilter,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<BattleHistoryPage, AppError> {
    if filter.vehicle_id.is_some_and(|id| id <= 0)
        || matches!((filter.date_from, filter.date_to), (Some(from), Some(to)) if from > to)
    {
        return Err(AppError::InvalidApiResponse);
    }
    let page_size = if filter.page_size == 0 {
        DEFAULT_PAGE_SIZE
    } else {
        filter.page_size.min(MAX_PAGE_SIZE)
    };
    let mut scan_cursor = filter.cursor.clone();
    let mut items = Vec::with_capacity(page_size + 1);

    loop {
        let rows = database.battle_history_rows(filter, scan_cursor.as_ref(), SCAN_CHUNK_SIZE)?;
        if rows.is_empty() {
            break;
        }
        let row_count = rows.len();
        for row in rows {
            let next_scan_cursor = BattleHistoryCursor {
                battle_time: row.battle.battle_time,
                battle_id: row.battle.arena_id.clone(),
            };
            let item = history_item(row, expected);
            scan_cursor = Some(next_scan_cursor);
            if matches_wn8(&item, filter.wn8_availability) {
                items.push(item);
                if items.len() > page_size {
                    break;
                }
            }
        }
        if items.len() > page_size || row_count < SCAN_CHUNK_SIZE {
            break;
        }
    }

    let has_more = items.len() > page_size;
    items.truncate(page_size);
    let next_cursor = has_more.then(|| {
        let last = items.last().expect("a full page has a last item");
        BattleHistoryCursor {
            battle_time: last.battle_time,
            battle_id: last.battle_id.clone(),
        }
    });
    Ok(BattleHistoryPage { items, next_cursor })
}

fn vehicle_summary(
    vehicle_id: i64,
    battles: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
    vehicle_classes: Option<&LoadedVehicleClassCatalog>,
    tier: Option<u8>,
) -> VehicleHistorySummary {
    let aggregate = metrics(battles);
    let newest = battles
        .iter()
        .max_by_key(|battle| (battle.battle_time, &battle.arena_id))
        .expect("vehicle groups are never empty");
    let sessions = battles
        .iter()
        .filter_map(|battle| battle.session_id.as_ref())
        .collect::<BTreeSet<_>>();
    VehicleHistorySummary {
        vehicle_id,
        vehicle_key: safe_identity_value(Some(&newest.vehicle_key)),
        tank_name: safe_identity_value(newest.tank_name.as_deref()),
        vehicle_class: vehicle_classes.map_or_else(
            || crate::vehicle_class_catalog::unavailable_resolution(vehicle_id),
            |catalog| catalog.resolve(vehicle_id, Some(&newest.vehicle_key)),
        ),
        tier,
        total_battles: battles.len(),
        session_count: sessions.len(),
        first_battle_at: battles
            .iter()
            .map(|battle| battle.battle_time)
            .min()
            .unwrap(),
        last_battle_at: battles
            .iter()
            .map(|battle| battle.battle_time)
            .max()
            .unwrap(),
        win_rate: aggregate.win_rate,
        outcome_sample_size: aggregate.outcome_sample_size,
        average_damage: aggregate.average_damage,
        average_assist: aggregate.average_assist,
        average_frags: aggregate.average_frags,
        average_xp: aggregate.average_xp,
        survival_rate: aggregate.survival_rate,
        wn8: session_wn8_presentation(battles, expected),
    }
}

pub(crate) fn vehicle_history_overview(
    database: &Database,
    expected: Option<&LoadedExpectedSnapshot>,
    vehicle_classes: Option<&LoadedVehicleClassCatalog>,
) -> Result<VehicleHistoryOverview, AppError> {
    let mut groups = BTreeMap::<i64, Vec<Battle>>::new();
    let mut unidentified_battle_count = 0;
    for battle in database.all_history_battles()? {
        match battle.wg_tank_id {
            Some(vehicle_id) => groups.entry(vehicle_id).or_default().push(battle),
            None => unidentified_battle_count += 1,
        }
    }
    let tiers = database.vehicle_tiers()?;
    let mut vehicles = groups
        .into_iter()
        .map(|(vehicle_id, battles)| {
            vehicle_summary(
                vehicle_id,
                &battles,
                expected,
                vehicle_classes,
                tiers.get(&vehicle_id).copied(),
            )
        })
        .collect::<Vec<_>>();
    vehicles.sort_by(|left, right| {
        right
            .last_battle_at
            .cmp(&left.last_battle_at)
            .then_with(|| left.vehicle_id.cmp(&right.vehicle_id))
    });
    Ok(VehicleHistoryOverview {
        vehicles,
        unidentified_battle_count,
    })
}

pub(crate) fn vehicle_history_report(
    database: &Database,
    vehicle_id: i64,
    expected: Option<&LoadedExpectedSnapshot>,
    vehicle_classes: Option<&LoadedVehicleClassCatalog>,
) -> Result<VehicleHistoryReport, AppError> {
    if vehicle_id <= 0 {
        return Err(AppError::InvalidApiResponse);
    }
    let rows = database.vehicle_history_rows(vehicle_id)?;
    if rows.is_empty() {
        return Err(AppError::InvalidApiResponse);
    }
    let battles = rows
        .iter()
        .map(|row| row.battle.clone())
        .collect::<Vec<_>>();
    let tier = database.vehicle_tiers()?.get(&vehicle_id).copied();
    let summary = vehicle_summary(vehicle_id, &battles, expected, vehicle_classes, tier);
    let maps = map_summaries(&battles);
    let mut grouped = BTreeMap::<String, (i64, String, Vec<Battle>)>::new();
    for row in rows {
        if let (Some(session_id), Some(started_at), Some(status)) = (
            row.battle.session_id.clone(),
            row.session_started_at,
            row.session_status,
        ) {
            grouped
                .entry(session_id)
                .or_insert_with(|| (started_at, status, Vec::new()))
                .2
                .push(row.battle);
        }
    }
    let mut sessions = grouped
        .into_iter()
        .map(
            |(session_id, (session_started_at, session_status, battles))| {
                let aggregate = metrics(&battles);
                VehicleHistorySession {
                    session_id,
                    session_started_at,
                    session_status,
                    battle_count: battles.len(),
                    win_rate: aggregate.win_rate,
                    outcome_sample_size: aggregate.outcome_sample_size,
                    average_damage: aggregate.average_damage,
                    average_assist: aggregate.average_assist,
                    wn8: session_wn8_presentation(&battles, expected),
                }
            },
        )
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| {
        right
            .session_started_at
            .cmp(&left.session_started_at)
            .then_with(|| right.session_id.cmp(&left.session_id))
    });
    Ok(VehicleHistoryReport {
        summary,
        sessions,
        maps,
    })
}

/// Minimum counted battles on one map before it is shown (E5-flare §3: 4; spec 25 had 5).
pub(crate) const MIN_MAP_BATTLES: usize = 4;

/// Average damage and survival per map over counted battles (D4), only maps with enough
/// battles, highest average damage first. Existing metrics only, grouped by map.
pub(crate) fn map_summaries(battles: &[Battle]) -> Vec<VehicleMapSummary> {
    let mut grouped = BTreeMap::<String, Vec<&Battle>>::new();
    for battle in battles.iter().filter(|battle| stats_eligibility(battle)) {
        if let Some(map_key) = battle.map_key.as_deref() {
            grouped.entry(map_key.to_owned()).or_default().push(battle);
        }
    }
    let mut maps = grouped
        .into_iter()
        .filter_map(|(map_key, battles)| {
            let damage = battles
                .iter()
                .filter_map(|battle| battle.damage)
                .collect::<Vec<_>>();
            if damage.len() < MIN_MAP_BATTLES {
                return None;
            }
            let survival = battles
                .iter()
                .filter_map(|battle| battle.survived)
                .collect::<Vec<_>>();
            Some(VehicleMapSummary {
                map_name: battles
                    .iter()
                    .find_map(|battle| safe_identity_value(battle.map_name.as_deref())),
                map_key,
                battles: battles.len(),
                average_damage: damage.iter().sum::<i64>() as f64 / damage.len() as f64,
                survival_rate: (!survival.is_empty()).then(|| {
                    survival.iter().filter(|value| **value).count() as f64 / survival.len() as f64
                        * 100.0
                }),
            })
        })
        .collect::<Vec<_>>();
    maps.sort_by(|left, right| {
        right
            .average_damage
            .total_cmp(&left.average_damage)
            .then_with(|| left.map_key.cmp(&right.map_key))
    });
    maps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expected_values::validate_snapshot_bytes,
        models::{BattleOutcome, DataQuality, HistoryDataQualityFilter, SourceKind},
    };
    use serde_json::json;
    use tempfile::tempdir;

    fn snapshot() -> LoadedExpectedSnapshot {
        let data = (1..=500)
            .map(|id| json!({"IDNum":id,"expDef":0.5,"expFrag":0.8,"expSpot":1.1,"expDamage":900.0,"expWinRate":49.5}))
            .collect::<Vec<_>>();
        validate_snapshot_bytes(
            &serde_json::to_vec(
                &json!({"header":{"source":"XVM","version":"2026-07-10"},"data":data}),
            )
            .unwrap(),
            None,
        )
        .unwrap()
    }

    fn battle(id: &str, time: i64, vehicle_id: Option<i64>, quality: DataQuality) -> Battle {
        let full = quality == DataQuality::Full;
        Battle {
            arena_id: id.to_owned(),
            arena_unique_id: None,
            battle_time: time,
            battle_time_source: Some("metadata".to_owned()),
            vehicle_key: format!("vehicle-{}", vehicle_id.unwrap_or_default()),
            wg_tank_id: vehicle_id,
            tank_name: vehicle_id.map(|id| format!("Tank {id}")),
            map_name: Some("Map".to_owned()),
            map_key: Some("01_map".to_owned()),
            battle_type: Some("random".to_owned()),
            gameplay_id: None,
            source_kind: SourceKind::Replay,
            data_quality: quality,
            parser_version: Some(2),
            feature_schema_version: full.then_some(1),
            game_version: None,
            data_completeness: full.then(|| "results".to_owned()),
            replay_source: Some("replay_results".to_owned()),
            parser_error_code: None,
            outcome: if full {
                BattleOutcome::Victory
            } else {
                BattleOutcome::Unknown
            },
            battle_pace: crate::models::strict_random_pace(),
            player_team: None,
            winner_team_raw: None,
            damage: full.then_some(1_800),
            assist: full.then_some(300),
            assist_spotting: None,
            assist_tracking: None,
            assist_stun: None,
            blocked: None,
            damage_received: None,
            frags: full.then_some(2),
            spots: full.then_some(2),
            shots: None,
            hits: None,
            direct_enemy_hits: None,
            direct_team_hits: None,
            penetrations: None,
            piercing_enemy_hits: None,
            explosion_hits: None,
            defense_points: full.then_some(1),
            moving_avg_damage: None,
            damage_rating: None,
            survived: full.then_some(true),
            xp: full.then_some(1_000),
            original_xp: None,
            duration_seconds: None,
            session_id: None,
        }
    }

    fn filter(page_size: usize) -> BattleHistoryFilter {
        BattleHistoryFilter {
            vehicle_id: None,
            data_quality: HistoryDataQualityFilter::All,
            wn8_availability: HistoryWn8Filter::All,
            date_from: None,
            date_to: None,
            map_key: None,
            cursor: None,
            page_size,
        }
    }

    /// Spec 25 §5.2, E5-flare §3: per-map averages of existing metrics, counted battles only,
    /// at least 4 battles per map, best map first.
    #[test]
    fn map_summaries_group_counted_battles_and_skip_small_maps() {
        let mut battles = Vec::new();
        for index in 0..6 {
            let mut good = battle(&format!("g{index}"), index, Some(1), DataQuality::Full);
            good.map_key = Some("01_good".to_owned());
            good.damage = Some(3_000);
            good.survived = Some(index % 2 == 0);
            battles.push(good);
            let mut weak = battle(&format!("w{index}"), index, Some(1), DataQuality::Full);
            weak.map_key = Some("02_weak".to_owned());
            weak.damage = Some(1_000 + index);
            weak.survived = Some(false);
            battles.push(weak);
        }
        for index in 0..3 {
            let mut small = battle(&format!("s{index}"), index, Some(1), DataQuality::Full);
            small.map_key = Some("03_small".to_owned());
            battles.push(small);
        }
        for index in 0..4 {
            let mut four = battle(&format!("f{index}"), index, Some(1), DataQuality::Full);
            four.map_key = Some("05_four".to_owned());
            four.damage = Some(2_000);
            battles.push(four);
        }
        for index in 0..5 {
            let mut partial = battle(&format!("p{index}"), index, Some(1), DataQuality::Partial);
            partial.map_key = Some("04_partial".to_owned());
            battles.push(partial);
        }
        let maps = map_summaries(&battles);
        assert_eq!(
            maps.iter()
                .map(|map| map.map_key.as_str())
                .collect::<Vec<_>>(),
            ["01_good", "05_four", "02_weak"]
        );
        assert_eq!(maps[0].battles, 6);
        assert_eq!(maps[0].average_damage, 3_000.0);
        assert_eq!(maps[0].survival_rate, Some(50.0));
        assert_eq!(maps[1].battles, 4);
        assert_eq!(maps[2].average_damage, 1_002.5);
        assert_eq!(maps[2].survival_rate, Some(0.0));
        assert_eq!(maps[0].map_name.as_deref(), Some("Map"));
        assert!(map_summaries(&[]).is_empty());
    }

    /// Spec 25 §5.5: the History screen filters by vehicle and by map.
    #[test]
    fn battle_history_filters_by_map_and_vehicle_together() {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join("history-map.sqlite3")).unwrap();
        for (id, time, vehicle, map) in [
            ("m1", 1_000, 1, "01_map"),
            ("m2", 2_000, 1, "02_other"),
            ("m3", 3_000, 2, "02_other"),
        ] {
            let mut value = battle(id, time, Some(vehicle), DataQuality::Full);
            value.map_key = Some(map.to_owned());
            database.insert_battle(&mut value, false).unwrap();
        }
        let ids = |filter: &BattleHistoryFilter| {
            battle_history_page(&database, filter, None)
                .unwrap()
                .items
                .into_iter()
                .map(|item| item.battle_id)
                .collect::<Vec<_>>()
        };
        let mut by_map = filter(10);
        by_map.map_key = Some("02_other".to_owned());
        assert_eq!(ids(&by_map), ["m3", "m2"]);
        by_map.vehicle_id = Some(1);
        assert_eq!(ids(&by_map), ["m2"]);
        by_map.map_key = Some("99_none".to_owned());
        assert!(ids(&by_map).is_empty());
        assert_eq!(ids(&filter(10)).len(), 3);
    }

    #[test]
    fn battle_history_is_stable_filtered_paginated_and_safe() {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join("history.sqlite3")).unwrap();
        let mut first = battle("b", 2_000, Some(1), DataQuality::Full);
        first.tank_name = Some(r"C:\private\20200101_0000_player_tank_map.wotreplay".to_owned());
        first.vehicle_key = r"C:\private\secret.wotreplay".to_owned();
        database.insert_battle(&mut first, false).unwrap();
        for mut value in [
            battle("a", 2_000, Some(2), DataQuality::Partial),
            battle("c", 1_500, Some(1), DataQuality::Full),
            battle("d", 1_000, None, DataQuality::Partial),
        ] {
            if value.arena_id == "c" {
                value.outcome = BattleOutcome::Draw;
                value.winner_team_raw = Some(0);
            }
            database.insert_battle(&mut value, false).unwrap();
        }
        let expected = snapshot();
        let first_page = battle_history_page(&database, &filter(2), Some(&expected)).unwrap();
        assert_eq!(
            first_page
                .items
                .iter()
                .map(|item| item.battle_id.as_str())
                .collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert!(first_page.items[0].tank_name.is_none());
        assert!(first_page.items[0].vehicle_key.is_none());
        assert!(first_page.items[0].session_id.is_none());
        let mut next_filter = filter(2);
        next_filter.cursor = first_page.next_cursor;
        let second_page = battle_history_page(&database, &next_filter, Some(&expected)).unwrap();
        assert_eq!(
            second_page
                .items
                .iter()
                .map(|item| item.battle_id.as_str())
                .collect::<Vec<_>>(),
            ["c", "d"]
        );
        assert_eq!(second_page.items[0].outcome, BattleOutcome::Draw);
        assert_eq!(second_page.items[1].outcome, BattleOutcome::Unknown);
        let mut vehicle_filter = filter(30);
        vehicle_filter.vehicle_id = Some(1);
        vehicle_filter.data_quality = HistoryDataQualityFilter::Full;
        vehicle_filter.date_from = Some(1_400);
        vehicle_filter.date_to = Some(2_000);
        assert_eq!(
            battle_history_page(&database, &vehicle_filter, Some(&expected))
                .unwrap()
                .items
                .len(),
            2
        );
        vehicle_filter.wn8_availability = HistoryWn8Filter::Available;
        assert_eq!(
            battle_history_page(&database, &vehicle_filter, Some(&expected))
                .unwrap()
                .items
                .len(),
            2
        );
        vehicle_filter.wn8_availability = HistoryWn8Filter::Unavailable;
        assert!(
            battle_history_page(&database, &vehicle_filter, Some(&expected))
                .unwrap()
                .items
                .is_empty()
        );
        let without_snapshot = battle_history_page(&database, &filter(30), None).unwrap();
        assert!(
            without_snapshot
                .items
                .iter()
                .all(|item| item.wn8.state == Wn8PresentationState::Unavailable)
        );
    }

    #[test]
    fn vehicle_overview_groups_by_id_and_preserves_partial_coverage() {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join("vehicles.sqlite3")).unwrap();
        let mut values = [
            battle("one", 1_000, Some(1), DataQuality::Full),
            battle("two", 2_000, Some(1), DataQuality::Partial),
            battle("three", 3_000, Some(2), DataQuality::Full),
            battle("unknown", 4_000, None, DataQuality::Partial),
        ];
        values[0].tank_name = Some("Old name".to_owned());
        values[1].tank_name = Some("New name".to_owned());
        values[2].tank_name = Some("New name".to_owned());
        for value in &mut values {
            database.insert_battle(value, true).unwrap();
        }
        let overview = vehicle_history_overview(&database, Some(&snapshot()), None).unwrap();
        assert_eq!(overview.vehicles.len(), 2);
        assert_eq!(overview.unidentified_battle_count, 1);
        let vehicle = overview
            .vehicles
            .iter()
            .find(|item| item.vehicle_id == 1)
            .unwrap();
        assert_eq!(vehicle.total_battles, 2);
        assert_eq!(vehicle.last_battle_at, 2_000);
        assert_eq!(vehicle.wn8.eligible_battles, 1);
        assert_eq!(vehicle.wn8.total_battles, 2);
        assert_eq!(vehicle.average_damage, Some(1_800.0));
        assert!(vehicle.session_count > 0);
    }

    /// Spec 25 §5.2: the tank header shows the tier stored with the battles (E2), taken from
    /// the newest battle that has one; a vehicle without a stored tier has none.
    #[test]
    fn vehicle_summary_carries_the_tier_of_the_newest_battle_that_has_one() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("vehicle-tier.sqlite3");
        let database = Database::initialize(path.clone()).unwrap();
        for (id, time, vehicle) in [
            ("t1", 1_000, 1),
            ("t2", 2_000, 1),
            ("t3", 3_000, 1),
            ("t4", 4_000, 2),
        ] {
            let mut value = battle(id, time, Some(vehicle), DataQuality::Full);
            database.insert_battle(&mut value, false).unwrap();
        }
        let connection = rusqlite::Connection::open(&path).unwrap();
        for (id, tier) in [("t1", 8), ("t2", 9)] {
            connection
                .execute(
                    "UPDATE battles SET vehicle_tier=?2 WHERE arena_id=?1",
                    rusqlite::params![id, tier],
                )
                .unwrap();
        }
        drop(connection);

        let overview = vehicle_history_overview(&database, None, None).unwrap();
        let tier = |vehicle_id: i64| {
            overview
                .vehicles
                .iter()
                .find(|item| item.vehicle_id == vehicle_id)
                .unwrap()
                .tier
        };
        assert_eq!(tier(1), Some(9));
        assert_eq!(tier(2), None);
        assert_eq!(
            vehicle_history_report(&database, 1, None, None)
                .unwrap()
                .summary
                .tier,
            Some(9)
        );
        assert_eq!(
            vehicle_history_report(&database, 2, None, None)
                .unwrap()
                .summary
                .tier,
            None
        );
    }

    #[test]
    fn vehicle_history_win_rate_counts_draw_and_exposes_unknown_coverage() {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join("vehicle-outcomes.sqlite3")).unwrap();
        for (index, outcome) in [
            BattleOutcome::Victory,
            BattleOutcome::Draw,
            BattleOutcome::Unknown,
        ]
        .into_iter()
        .enumerate()
        {
            let mut value = battle(
                &format!("outcome-{index}"),
                index as i64 + 1,
                Some(1),
                DataQuality::Full,
            );
            value.outcome = outcome;
            value.winner_team_raw = match outcome {
                BattleOutcome::Victory => Some(1),
                BattleOutcome::Draw => Some(0),
                BattleOutcome::Defeat => Some(2),
                BattleOutcome::Unknown => None,
            };
            database.insert_battle(&mut value, true).unwrap();
        }

        let overview = vehicle_history_overview(&database, Some(&snapshot()), None).unwrap();
        let summary = &overview.vehicles[0];
        assert_eq!(summary.total_battles, 3);
        assert_eq!(summary.win_rate, Some(50.0));
        assert_eq!(summary.outcome_sample_size, 2);
        assert_eq!(summary.wn8.eligible_battles, 2);
        assert_eq!(summary.wn8.total_battles, 3);

        let report = vehicle_history_report(&database, 1, Some(&snapshot()), None).unwrap();
        assert_eq!(report.sessions.len(), 1);
        assert_eq!(report.sessions[0].battle_count, 3);
        assert_eq!(report.sessions[0].win_rate, Some(50.0));
        assert_eq!(report.sessions[0].outcome_sample_size, 2);
    }

    #[test]
    fn vehicle_report_uses_only_selected_vehicle_in_mixed_session() {
        let temp = tempdir().unwrap();
        let database = Database::initialize(temp.path().join("report.sqlite3")).unwrap();
        let mut selected = battle("selected", 1_000, Some(1), DataQuality::Full);
        selected.damage = Some(2_000);
        database.insert_battle(&mut selected, true).unwrap();
        let mut other = battle("other", 1_100, Some(2), DataQuality::Full);
        other.damage = Some(9_000);
        database.insert_battle(&mut other, true).unwrap();
        let report = vehicle_history_report(&database, 1, Some(&snapshot()), None).unwrap();
        assert_eq!(report.summary.vehicle_id, 1);
        assert_eq!(report.summary.total_battles, 1);
        assert_eq!(report.summary.average_damage, Some(2_000.0));
        assert_eq!(report.sessions.len(), 1);
        assert_eq!(report.sessions[0].battle_count, 1);
        assert_eq!(report.sessions[0].average_damage, Some(2_000.0));
        let without_snapshot = vehicle_history_report(&database, 1, None, None).unwrap();
        assert_eq!(
            without_snapshot.summary.wn8.state,
            Wn8PresentationState::Unavailable
        );
    }

    #[test]
    fn history_reads_do_not_change_database_counts() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("read-only.sqlite3");
        let database = Database::initialize(path).unwrap();
        let mut value = battle("one", 1_000, Some(1), DataQuality::Full);
        database.insert_battle(&mut value, false).unwrap();
        let before = database.diagnostics_read_only().unwrap();
        let _ = battle_history_page(&database, &filter(30), Some(&snapshot())).unwrap();
        let _ = vehicle_history_overview(&database, Some(&snapshot()), None).unwrap();
        let _ = vehicle_history_report(&database, 1, Some(&snapshot()), None).unwrap();
        let after = database.diagnostics_read_only().unwrap();
        assert_eq!(before, after);
    }
}
