//! Data foundations for the post-v2.0 vision (spec 20, E2, D17).
//!
//! Pure extraction from the replay result block, done while the parser already holds it in
//! memory:
//! - extra columns of the own player (`battles`, spec 20 E2);
//! - an anonymous 30-row battle roster (`battle_roster`, spec 20 E2, without
//!   `marks_on_gun`);
//! - per-side aggregates (`battle_team_stats`, spec 20 E2).
//!
//! No identifier leaves this module: nicknames, account ids, clans, raw `prebattleID` values and
//! in-battle vehicle ids are only used transiently to join rows, then replaced by local numbers
//! (`slot`, `killer_slot`, `platoon_group`) whose order never depends on an identifier.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Map, Value};

use crate::models::VehicleClass;

/// Version of the roster and team-aggregate facts.
pub(crate) const FOUNDATION_FACT_SCHEMA_VERSION: i64 = 1;
const TEAM_SIZE: usize = 15;
const ROSTER_SIZE: usize = 30;
const MAX_COUNTER: i64 = 1_000_000;
const MAX_LIFE_TIME: i64 = 7_200;

/// Vehicle facts from the client catalog at import time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CatalogVehicle {
    pub tier: Option<u8>,
    pub class: VehicleClass,
}

/// Extra columns of the own player (spec 20 E2). All nullable; missing is never zero.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct OwnPlayerFacts {
    pub life_time_seconds: Option<i64>,
    pub death_reason_raw: Option<i64>,
    pub max_health: Option<i64>,
    pub health_remaining: Option<i64>,
    pub direct_hits_received: Option<i64>,
    pub piercings_received: Option<i64>,
    pub no_damage_hits_received: Option<i64>,
    pub explosion_hits_received: Option<i64>,
    pub potential_damage_received: Option<i64>,
    pub damage_from_invisibles: Option<i64>,
    pub assist_smoke: Option<i64>,
    pub assist_inspire: Option<i64>,
    pub stun_duration: Option<f64>,
    pub stun_num: Option<i64>,
    pub capture_points: Option<i64>,
    pub mileage: Option<i64>,
    pub arena_type_id: Option<i64>,
    /// 1 = solo, 2-3 = platoon; only the count is stored.
    pub own_platoon_size: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Side {
    Own,
    Enemy,
}

impl Side {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::Enemy => "enemy",
        }
    }
}

/// One anonymous roster row (spec 20 E2 without `marks_on_gun`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RosterRow {
    pub slot: u8,
    pub side: Side,
    pub is_own_player: bool,
    pub type_comp_descr: i64,
    pub vehicle_tier: Option<u8>,
    pub vehicle_class: Option<VehicleClass>,
    /// 0 = solo; 1, 2, ... = platoons numbered in slot order within the battle.
    pub platoon_group: u8,
    pub damage_dealt: Option<i64>,
    pub kills: Option<i64>,
    pub spotted: Option<i64>,
    pub shots: Option<i64>,
    pub direct_enemy_hits: Option<i64>,
    pub piercing_enemy_hits: Option<i64>,
    pub explosion_hits: Option<i64>,
    pub assist_radio: Option<i64>,
    pub assist_track: Option<i64>,
    pub assist_stun: Option<i64>,
    pub damage_blocked: Option<i64>,
    pub damage_received: Option<i64>,
    pub potential_damage_received: Option<i64>,
    pub max_health: Option<i64>,
    pub life_time: Option<i64>,
    pub death_reason_raw: Option<i64>,
    pub killer_slot: Option<u8>,
    pub capture_points: Option<i64>,
    pub defense_points: Option<i64>,
    pub mileage: Option<i64>,
}

/// Aggregates of one side (spec 20 E2). Sums are `None` when any member lacks the value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TeamStats {
    pub side: Side,
    pub fact_schema_version: i64,
    pub players: i64,
    pub damage_total: Option<i64>,
    pub kills_total: Option<i64>,
    pub spotted_total: Option<i64>,
    pub shots_total: Option<i64>,
    pub direct_enemy_hits_total: Option<i64>,
    pub piercing_enemy_hits_total: Option<i64>,
    pub assist_radio_total: Option<i64>,
    pub assist_track_total: Option<i64>,
    pub assist_stun_total: Option<i64>,
    pub blocked_total: Option<i64>,
    pub damage_received_total: Option<i64>,
    pub max_health_total: Option<i64>,
    pub survivors: Option<i64>,
    pub life_time_total: Option<i64>,
    pub tier_min: Option<i64>,
    pub tier_max: Option<i64>,
    pub tier_sum: Option<i64>,
    pub class_light: i64,
    pub class_medium: i64,
    pub class_heavy: i64,
    pub class_td: i64,
    pub class_spg: i64,
    pub unresolved_vehicles: i64,
    pub platoons: i64,
    pub platooned_players: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BattleRoster {
    pub rows: Vec<RosterRow>,
    pub teams: [TeamStats; 2],
}

/// Everything the v10 import stores besides `Battle` and the team-relative facts (parser 9).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ImportExtras {
    /// Extra own-player columns; `None` for a battle without a result block.
    pub own: Option<OwnPlayerFacts>,
    /// Tier of the player's vehicle from the client catalog at import time.
    pub vehicle_tier: Option<u8>,
    /// Fingerprint of the catalog consulted for tiers and classes.
    pub catalog_fingerprint: Option<String>,
    /// Anonymous roster and team aggregates; `None` for a battle that does not qualify.
    pub roster: Option<BattleRoster>,
    pub packet_stream_bytes: Option<i64>,
}

/// Why a battle has no roster rows. Same eligibility as team-relative §6: full, strict random,
/// 15 vs 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RosterSkip {
    NotStrictRandom,
    MissingResults,
    MissingOwnPlayer,
    RosterSize,
    InvalidTeam,
}

fn counter(object: &Map<String, Value>, key: &str, maximum: i64) -> Option<i64> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .filter(|value| (0..=maximum).contains(value))
}

fn signed(object: &Map<String, Value>, key: &str, minimum: i64, maximum: i64) -> Option<i64> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .filter(|value| (minimum..=maximum).contains(value))
}

fn summary(results: &Value) -> Option<&Map<String, Value>> {
    results.as_array()?.first()?.as_object()
}

fn own_personal(summary: &Map<String, Value>) -> Option<&Map<String, Value>> {
    let personal = summary.get("personal")?.as_object()?;
    let mut entries = personal
        .iter()
        .filter(|(key, value)| key.as_str() != "avatar" && value.is_object());
    let (_, own) = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    own.as_object()
}

/// The `prebattleID` of each account, joined transiently; `None` when the field is missing.
fn prebattle_ids(summary: &Map<String, Value>) -> HashMap<i64, i64> {
    let mut ids = HashMap::new();
    if let Some(players) = summary.get("players").and_then(Value::as_object) {
        for (account, player) in players {
            if let (Ok(account), Some(prebattle)) = (
                account.parse::<i64>(),
                player.get("prebattleID").and_then(Value::as_i64),
            ) {
                ids.insert(account, prebattle);
            }
        }
    }
    ids
}

/// Extra own-player columns. Works for any full result block, not only strict random battles.
pub(crate) fn extract_own_player_facts(results: &Value) -> OwnPlayerFacts {
    let Some(summary) = summary(results) else {
        return OwnPlayerFacts::default();
    };
    let Some(own) = own_personal(summary) else {
        return OwnPlayerFacts::default();
    };
    let account = own.get("accountDBID").and_then(Value::as_i64);
    let team = own.get("team").and_then(Value::as_i64);
    let own_platoon_size = (|| {
        let account = account?;
        let ids = prebattle_ids(summary);
        let own_prebattle = *ids.get(&account)?;
        if own_prebattle == 0 {
            return Some(1);
        }
        let players = summary.get("players")?.as_object()?;
        let size = players
            .iter()
            .filter(|(key, player)| {
                key.parse::<i64>()
                    .ok()
                    .and_then(|id| ids.get(&id))
                    .is_some_and(|prebattle| *prebattle == own_prebattle)
                    && player.get("team").and_then(Value::as_i64) == team
            })
            .count();
        i64::try_from(size)
            .ok()
            .filter(|size| (1..=3).contains(size))
    })();
    OwnPlayerFacts {
        life_time_seconds: counter(own, "lifeTime", MAX_LIFE_TIME),
        death_reason_raw: signed(own, "deathReason", -1, 64),
        max_health: counter(own, "maxHealth", MAX_COUNTER),
        // `health` can be negative after an ammo-rack explosion; no lower bound check.
        health_remaining: signed(own, "health", -MAX_COUNTER, MAX_COUNTER),
        direct_hits_received: counter(own, "directHitsReceived", MAX_COUNTER),
        piercings_received: counter(own, "piercingsReceived", MAX_COUNTER),
        no_damage_hits_received: counter(own, "noDamageDirectHitsReceived", MAX_COUNTER),
        explosion_hits_received: counter(own, "explosionHitsReceived", MAX_COUNTER),
        potential_damage_received: counter(own, "potentialDamageReceived", MAX_COUNTER),
        damage_from_invisibles: counter(own, "damageReceivedFromInvisibles", MAX_COUNTER),
        assist_smoke: counter(own, "damageAssistedSmoke", MAX_COUNTER),
        assist_inspire: counter(own, "damageAssistedInspire", MAX_COUNTER),
        stun_duration: own
            .get("stunDuration")
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && (0.0..=MAX_COUNTER as f64).contains(value)),
        stun_num: counter(own, "stunNum", MAX_COUNTER),
        capture_points: counter(own, "capturePoints", MAX_COUNTER),
        mileage: counter(own, "mileage", MAX_COUNTER),
        arena_type_id: summary
            .get("common")
            .and_then(Value::as_object)
            .and_then(|common| counter(common, "arenaTypeID", i64::from(i32::MAX))),
        own_platoon_size,
    }
}

/// Packet stream length after the JSON blocks (spec 20 E2): file size minus the header and
/// declared blocks. `None` when the declared blocks do not fit in the file.
pub(crate) fn packet_stream_bytes(file_size: u64, header_and_blocks: u64) -> Option<i64> {
    file_size
        .checked_sub(header_and_blocks)
        .and_then(|value| i64::try_from(value).ok())
}

#[derive(Debug, Clone)]
struct RawRow<'a> {
    vehicle_key: &'a str,
    team: i64,
    account: Option<i64>,
    killer_key: Option<String>,
    row: RosterRow,
}

/// Every stored field of a row except `slot` and `killer_slot`: the sort key that makes slot
/// numbering independent of identifiers and input order.
type SortKey = (
    Side,
    std::cmp::Reverse<Option<i64>>,
    i64,
    [Option<i64>; 18],
    (Option<u8>, Option<u8>, bool),
);

fn sort_key(row: &RosterRow) -> SortKey {
    (
        row.side,
        std::cmp::Reverse(row.damage_dealt),
        row.type_comp_descr,
        [
            row.kills,
            row.spotted,
            row.shots,
            row.direct_enemy_hits,
            row.piercing_enemy_hits,
            row.explosion_hits,
            row.assist_radio,
            row.assist_track,
            row.assist_stun,
            row.damage_blocked,
            row.damage_received,
            row.potential_damage_received,
            row.max_health,
            row.life_time,
            row.death_reason_raw,
            row.capture_points,
            row.defense_points,
            row.mileage,
        ],
        (
            row.vehicle_tier,
            row.vehicle_class.map(|class| class as u8),
            row.is_own_player,
        ),
    )
}

/// Anonymous roster and team aggregates for a strict random 15 vs 15 battle.
pub(crate) fn extract_battle_roster(
    metadata: &Value,
    results: &Value,
    catalog: &dyn Fn(i64) -> Option<CatalogVehicle>,
) -> Result<BattleRoster, RosterSkip> {
    let summary = summary(results).ok_or(RosterSkip::MissingResults)?;
    let bonus_type = summary
        .get("common")
        .and_then(Value::as_object)
        .and_then(|common| common.get("bonusType"))
        .and_then(Value::as_i64);
    if metadata.get("battleType").and_then(Value::as_i64) != Some(1) || bonus_type != Some(1) {
        return Err(RosterSkip::NotStrictRandom);
    }
    let own = own_personal(summary).ok_or(RosterSkip::MissingOwnPlayer)?;
    let own_account = own
        .get("accountDBID")
        .and_then(Value::as_i64)
        .ok_or(RosterSkip::MissingOwnPlayer)?;
    let vehicles = summary
        .get("vehicles")
        .and_then(Value::as_object)
        .ok_or(RosterSkip::MissingResults)?;
    if vehicles.len() != ROSTER_SIZE {
        return Err(RosterSkip::RosterSize);
    }
    let prebattles = prebattle_ids(summary);

    let mut raw = Vec::with_capacity(ROSTER_SIZE);
    for (vehicle_key, entries) in vehicles {
        let entry = entries
            .as_array()
            .and_then(|entries| entries.first())
            .and_then(Value::as_object)
            .ok_or(RosterSkip::MissingResults)?;
        let team = entry
            .get("team")
            .and_then(Value::as_i64)
            .filter(|team| matches!(team, 1 | 2))
            .ok_or(RosterSkip::InvalidTeam)?;
        let type_comp_descr = entry
            .get("typeCompDescr")
            .and_then(Value::as_i64)
            .filter(|value| *value > 0)
            .ok_or(RosterSkip::MissingResults)?;
        let account = entry.get("accountDBID").and_then(Value::as_i64);
        let catalog_vehicle = catalog(type_comp_descr);
        let killer_key = entry
            .get("killerID")
            .and_then(Value::as_i64)
            .filter(|id| *id > 0)
            .map(|id| id.to_string());
        raw.push(RawRow {
            vehicle_key: vehicle_key.as_str(),
            team,
            account,
            killer_key,
            row: RosterRow {
                slot: 0,
                side: Side::Enemy,
                is_own_player: account == Some(own_account),
                type_comp_descr,
                vehicle_tier: catalog_vehicle.and_then(|vehicle| vehicle.tier),
                vehicle_class: catalog_vehicle
                    .map(|vehicle| vehicle.class)
                    .filter(|class| class.is_known()),
                platoon_group: 0,
                damage_dealt: counter(entry, "damageDealt", MAX_COUNTER),
                kills: counter(entry, "kills", 30),
                spotted: counter(entry, "spotted", 30),
                shots: counter(entry, "shots", MAX_COUNTER),
                direct_enemy_hits: counter(entry, "directEnemyHits", MAX_COUNTER),
                piercing_enemy_hits: counter(entry, "piercingEnemyHits", MAX_COUNTER),
                explosion_hits: counter(entry, "explosionHits", MAX_COUNTER),
                assist_radio: counter(entry, "damageAssistedRadio", MAX_COUNTER),
                assist_track: counter(entry, "damageAssistedTrack", MAX_COUNTER),
                assist_stun: counter(entry, "damageAssistedStun", MAX_COUNTER),
                damage_blocked: counter(entry, "damageBlockedByArmor", MAX_COUNTER),
                damage_received: counter(entry, "damageReceived", MAX_COUNTER),
                potential_damage_received: counter(entry, "potentialDamageReceived", MAX_COUNTER),
                max_health: counter(entry, "maxHealth", MAX_COUNTER),
                life_time: counter(entry, "lifeTime", MAX_LIFE_TIME),
                death_reason_raw: signed(entry, "deathReason", -1, 64),
                killer_slot: None,
                capture_points: counter(entry, "capturePoints", MAX_COUNTER),
                defense_points: counter(entry, "droppedCapturePoints", MAX_COUNTER),
                mileage: counter(entry, "mileage", MAX_COUNTER),
            },
        });
    }
    let own_rows = raw.iter().filter(|row| row.row.is_own_player).count();
    if own_rows != 1 {
        return Err(RosterSkip::MissingOwnPlayer);
    }
    let own_team = raw
        .iter()
        .find(|row| row.row.is_own_player)
        .map(|row| row.team)
        .ok_or(RosterSkip::MissingOwnPlayer)?;
    for row in &mut raw {
        row.row.side = if row.team == own_team {
            Side::Own
        } else {
            Side::Enemy
        };
    }
    if raw.iter().filter(|row| row.row.side == Side::Own).count() != TEAM_SIZE {
        return Err(RosterSkip::RosterSize);
    }

    raw.sort_by_key(|row| sort_key(&row.row));
    // Rows with an identical sort key are identical in every stored field; a reference to any of
    // them resolves to the lowest slot, so the output never depends on input order.
    let mut first_slot_of_key = BTreeMap::<SortKey, u8>::new();
    let mut slot_of_vehicle = HashMap::<&str, u8>::new();
    for (index, row) in raw.iter_mut().enumerate() {
        let slot = u8::try_from(index).map_err(|_| RosterSkip::RosterSize)?;
        row.row.slot = slot;
        let first = *first_slot_of_key.entry(sort_key(&row.row)).or_insert(slot);
        slot_of_vehicle.insert(row.vehicle_key, first);
    }
    let mut groups = HashMap::<(i64, i64), u8>::new();
    let mut next_group = 1_u8;
    let mut rows = Vec::with_capacity(ROSTER_SIZE);
    for raw_row in &raw {
        let mut row = raw_row.row.clone();
        row.killer_slot = raw_row
            .killer_key
            .as_deref()
            .and_then(|key| slot_of_vehicle.get(key).copied());
        row.platoon_group = match raw_row
            .account
            .and_then(|account| prebattles.get(&account))
            .copied()
            .filter(|prebattle| *prebattle != 0)
        {
            Some(prebattle) => *groups.entry((raw_row.team, prebattle)).or_insert_with(|| {
                let group = next_group;
                next_group = next_group.saturating_add(1);
                group
            }),
            None => 0,
        };
        rows.push(row);
    }
    let teams = [team_stats(&rows, Side::Own), team_stats(&rows, Side::Enemy)];
    Ok(BattleRoster { rows, teams })
}

fn checked_sum(mut values: impl Iterator<Item = Option<i64>>) -> Option<i64> {
    values.try_fold(0_i64, |total, value| total.checked_add(value?))
}

fn team_stats(rows: &[RosterRow], side: Side) -> TeamStats {
    let members = rows
        .iter()
        .filter(|row| row.side == side)
        .collect::<Vec<_>>();
    let sum =
        |field: fn(&RosterRow) -> Option<i64>| checked_sum(members.iter().map(|row| field(row)));
    let tiers = members
        .iter()
        .filter_map(|row| row.vehicle_tier.map(i64::from))
        .collect::<Vec<_>>();
    let all_tiers = tiers.len() == members.len() && !members.is_empty();
    let class_count = |class: VehicleClass| {
        members
            .iter()
            .filter(|row| row.vehicle_class == Some(class))
            .count() as i64
    };
    let mut platoon_sizes = BTreeMap::<u8, i64>::new();
    for row in &members {
        if row.platoon_group != 0 {
            *platoon_sizes.entry(row.platoon_group).or_default() += 1;
        }
    }
    TeamStats {
        side,
        fact_schema_version: FOUNDATION_FACT_SCHEMA_VERSION,
        players: members.len() as i64,
        damage_total: sum(|row| row.damage_dealt),
        kills_total: sum(|row| row.kills),
        spotted_total: sum(|row| row.spotted),
        shots_total: sum(|row| row.shots),
        direct_enemy_hits_total: sum(|row| row.direct_enemy_hits),
        piercing_enemy_hits_total: sum(|row| row.piercing_enemy_hits),
        assist_radio_total: sum(|row| row.assist_radio),
        assist_track_total: sum(|row| row.assist_track),
        assist_stun_total: sum(|row| row.assist_stun),
        blocked_total: sum(|row| row.damage_blocked),
        damage_received_total: sum(|row| row.damage_received),
        max_health_total: sum(|row| row.max_health),
        survivors: members
            .iter()
            .map(|row| row.death_reason_raw.map(|reason| i64::from(reason == -1)))
            .try_fold(0_i64, |total, value| total.checked_add(value?)),
        life_time_total: sum(|row| row.life_time),
        tier_min: all_tiers.then(|| tiers.iter().copied().min()).flatten(),
        tier_max: all_tiers.then(|| tiers.iter().copied().max()).flatten(),
        tier_sum: all_tiers.then(|| tiers.iter().sum()),
        class_light: class_count(VehicleClass::LightTank),
        class_medium: class_count(VehicleClass::MediumTank),
        class_heavy: class_count(VehicleClass::HeavyTank),
        class_td: class_count(VehicleClass::TankDestroyer),
        class_spg: class_count(VehicleClass::Spg),
        unresolved_vehicles: members
            .iter()
            .filter(|row| row.vehicle_class.is_none() || row.vehicle_tier.is_none())
            .count() as i64,
        platoons: platoon_sizes.len() as i64,
        platooned_players: platoon_sizes.values().sum(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) const OWN_ACCOUNT: i64 = 900_000_100;

    /// A synthetic strict random 15 vs 15 battle. Account ids, nicknames, clans, prebattle ids
    /// and vehicle keys are recognizable markers so tests can prove none of them is stored.
    pub(crate) fn synthetic_battle() -> (Value, Value) {
        let mut vehicles = Map::new();
        let mut players = Map::new();
        for index in 0..30_i64 {
            let team = if index < 15 { 1 } else { 2 };
            let account = OWN_ACCOUNT + index;
            let vehicle_key = (777_000 + index).to_string();
            let killer = if index % 4 == 1 && index < 29 {
                json!(777_000 + (29 - index))
            } else {
                json!(0)
            };
            vehicles.insert(
                vehicle_key,
                json!([{
                    "accountDBID": account,
                    "team": team,
                    "typeCompDescr": 1_000 + (index % 7) * 16 + 1,
                    "damageDealt": 100 * ((index * 7) % 13),
                    "kills": index % 3,
                    "spotted": index % 4,
                    "shots": 5 + index % 6,
                    "directEnemyHits": 3,
                    "piercingEnemyHits": 2,
                    "explosionHits": 0,
                    "damageAssistedRadio": 10 * index,
                    "damageAssistedTrack": 5,
                    "damageAssistedStun": 0,
                    "damageBlockedByArmor": 50,
                    "damageReceived": 300,
                    "potentialDamageReceived": 900,
                    "maxHealth": 1_500,
                    "lifeTime": 120 + index,
                    "deathReason": if index % 2 == 0 { -1 } else { 0 },
                    "killerID": killer,
                    "capturePoints": 0,
                    "droppedCapturePoints": index % 2,
                    "mileage": 1_000 + index,
                    "marksOnGun": 3,
                    "xp": 999,
                    "credits": 12345,
                    "index": 0
                }]),
            );
            let prebattle = match index {
                0 | 1 => 555_001,
                16..=18 => 555_002,
                20 | 21 => 555_003,
                _ => 0,
            };
            players.insert(
                account.to_string(),
                json!({
                    "name": format!("SyntheticNick{index}"),
                    "realName": format!("SyntheticReal{index}"),
                    "clanAbbrev": "SYNTH",
                    "clanDBID": 444_000 + index,
                    "team": team,
                    "prebattleID": prebattle
                }),
            );
        }
        let own_key = "777000".to_owned();
        let own = vehicles[&own_key][0].clone();
        let mut personal = own.as_object().unwrap().clone();
        personal.insert("health".into(), json!(-12));
        personal.insert("directHitsReceived".into(), json!(6));
        personal.insert("piercingsReceived".into(), json!(4));
        personal.insert("noDamageDirectHitsReceived".into(), json!(2));
        personal.insert("explosionHitsReceived".into(), json!(1));
        personal.insert("damageReceivedFromInvisibles".into(), json!(80));
        personal.insert("damageAssistedSmoke".into(), json!(7));
        personal.insert("damageAssistedInspire".into(), json!(3));
        personal.insert("stunDuration".into(), json!(12.5));
        personal.insert("stunNum".into(), json!(2));
        personal.insert("capturePoints".into(), json!(15));
        let results = json!([{
            "arenaUniqueID": 12345678901_i64,
            "personal": {"avatar": {}, "1001": personal},
            "vehicles": vehicles,
            "players": players,
            "common": {"bonusType": 1, "arenaTypeID": 393_281, "winnerTeam": 1}
        }]);
        let metadata = json!({"battleType": 1, "gameplayID": "ctf"});
        (metadata, results)
    }

    fn catalog(id: i64) -> Option<CatalogVehicle> {
        let classes = [
            VehicleClass::LightTank,
            VehicleClass::MediumTank,
            VehicleClass::HeavyTank,
            VehicleClass::TankDestroyer,
            VehicleClass::Spg,
        ];
        let index = ((id - 1_001) / 16) as usize;
        (index < 7).then(|| CatalogVehicle {
            tier: Some(8 + (index % 3) as u8),
            class: classes[index % classes.len()],
        })
    }

    fn roster() -> BattleRoster {
        let (metadata, results) = synthetic_battle();
        extract_battle_roster(&metadata, &results, &catalog).unwrap()
    }

    #[test]
    fn own_player_facts_read_every_column_and_keep_missing_values_missing() {
        let (_, results) = synthetic_battle();
        let facts = extract_own_player_facts(&results);
        assert_eq!(facts.life_time_seconds, Some(120));
        assert_eq!(facts.death_reason_raw, Some(-1));
        assert_eq!(facts.max_health, Some(1_500));
        assert_eq!(facts.health_remaining, Some(-12));
        assert_eq!(facts.direct_hits_received, Some(6));
        assert_eq!(facts.piercings_received, Some(4));
        assert_eq!(facts.no_damage_hits_received, Some(2));
        assert_eq!(facts.explosion_hits_received, Some(1));
        assert_eq!(facts.potential_damage_received, Some(900));
        assert_eq!(facts.damage_from_invisibles, Some(80));
        assert_eq!(facts.assist_smoke, Some(7));
        assert_eq!(facts.assist_inspire, Some(3));
        assert_eq!(facts.stun_duration, Some(12.5));
        assert_eq!(facts.stun_num, Some(2));
        assert_eq!(facts.capture_points, Some(15));
        assert_eq!(facts.mileage, Some(1_000));
        assert_eq!(facts.arena_type_id, Some(393_281));
        assert_eq!(facts.own_platoon_size, Some(2));

        assert_eq!(
            extract_own_player_facts(&json!([])),
            OwnPlayerFacts::default()
        );
        let mut missing = results.clone();
        let own = missing[0]["personal"]["1001"].as_object_mut().unwrap();
        own.remove("lifeTime");
        own.insert("mileage".into(), json!("1000"));
        own.insert("stunDuration".into(), json!(f64::MAX));
        let facts = extract_own_player_facts(&missing);
        assert_eq!(facts.life_time_seconds, None);
        assert_eq!(facts.mileage, None);
        assert_eq!(facts.stun_duration, None);
    }

    #[test]
    fn solo_player_has_platoon_size_one_and_missing_prebattle_is_unknown() {
        let (_, mut results) = synthetic_battle();
        results[0]["players"][OWN_ACCOUNT.to_string()]["prebattleID"] = json!(0);
        assert_eq!(extract_own_player_facts(&results).own_platoon_size, Some(1));
        results[0]["players"][OWN_ACCOUNT.to_string()]
            .as_object_mut()
            .unwrap()
            .remove("prebattleID");
        assert_eq!(extract_own_player_facts(&results).own_platoon_size, None);
    }

    #[test]
    fn roster_has_thirty_slots_one_own_player_and_fifteen_per_side() {
        let roster = roster();
        assert_eq!(roster.rows.len(), 30);
        assert_eq!(
            roster.rows.iter().map(|row| row.slot).collect::<Vec<_>>(),
            (0..30).collect::<Vec<_>>()
        );
        assert_eq!(
            roster.rows.iter().filter(|row| row.is_own_player).count(),
            1
        );
        assert_eq!(
            roster
                .rows
                .iter()
                .filter(|row| row.side == Side::Own)
                .count(),
            15
        );
        assert!(roster.rows[..15].iter().all(|row| row.side == Side::Own));
        for pair in roster.rows[..15].windows(2) {
            assert!(pair[0].damage_dealt >= pair[1].damage_dealt);
        }
    }

    // E2 test: slot numbering is deterministic and independent of the input order of the
    // `vehicles` and `players` objects.
    #[test]
    fn permuted_input_produces_identical_rows() {
        let (metadata, results) = synthetic_battle();
        let baseline = extract_battle_roster(&metadata, &results, &catalog).unwrap();
        for rotation in [1_usize, 7, 13, 29] {
            let mut permuted = results.clone();
            for key in ["vehicles", "players"] {
                let object = permuted[0][key].as_object().unwrap();
                let mut entries = object
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<Vec<_>>();
                let shift = rotation % entries.len();
                entries.rotate_left(shift);
                entries.reverse();
                permuted[0][key] = Value::Object(entries.into_iter().collect());
            }
            assert_eq!(
                extract_battle_roster(&metadata, &permuted, &catalog).unwrap(),
                baseline,
                "rotation {rotation}"
            );
        }
    }

    /// Relabels every in-battle vehicle id and account id in reverse order, so the key order of
    /// the JSON objects is reversed too; the stored rows must not change.
    fn relabeled(results: &Value) -> Value {
        let vehicle = |key: &str| (888_000 + (777_029 - key.parse::<i64>().unwrap())).to_string();
        let account = |id: i64| 600_000_000 + (OWN_ACCOUNT + 29 - id);
        let mut out = results.clone();
        let mut vehicles = Map::new();
        for (key, entries) in results[0]["vehicles"].as_object().unwrap() {
            let mut entry = entries[0].clone();
            entry["accountDBID"] = json!(account(entry["accountDBID"].as_i64().unwrap()));
            let killer = entry["killerID"].as_i64().unwrap();
            if killer > 0 {
                entry["killerID"] = json!(vehicle(&killer.to_string()).parse::<i64>().unwrap());
            }
            vehicles.insert(vehicle(key), json!([entry]));
        }
        out[0]["vehicles"] = Value::Object(vehicles);
        let mut players = Map::new();
        for (key, player) in results[0]["players"].as_object().unwrap() {
            players.insert(account(key.parse().unwrap()).to_string(), player.clone());
        }
        out[0]["players"] = Value::Object(players);
        let own = account(OWN_ACCOUNT);
        out[0]["personal"]["1001"]["accountDBID"] = json!(own);
        out
    }

    #[test]
    fn relabeled_identifiers_produce_identical_rows() {
        let (metadata, results) = synthetic_battle();
        let baseline = extract_battle_roster(&metadata, &results, &catalog).unwrap();
        let relabeled = extract_battle_roster(&metadata, &relabeled(&results), &catalog).unwrap();
        assert_eq!(relabeled, baseline);
        assert_eq!(
            extract_own_player_facts(&relabeled_results_for_facts()),
            extract_own_player_facts(&results)
        );
    }

    fn relabeled_results_for_facts() -> Value {
        relabeled(&synthetic_battle().1)
    }

    #[test]
    fn identical_rows_resolve_killer_to_the_lowest_slot() {
        let (metadata, mut results) = synthetic_battle();
        // Two enemies with identical stats and vehicle; one of them kills the own player.
        let template = results[0]["vehicles"]["777020"][0].clone();
        results[0]["vehicles"]["777022"][0] = template;
        results[0]["vehicles"]["777000"][0]["killerID"] = json!(777_022);
        results[0]["personal"]["1001"]["killerID"] = json!(777_022);
        let roster = extract_battle_roster(&metadata, &results, &catalog).unwrap();
        let own = roster.rows.iter().find(|row| row.is_own_player).unwrap();
        let killer = own.killer_slot.unwrap();
        let twins = roster
            .rows
            .iter()
            .filter(|row| sort_key(row) == sort_key(&roster.rows[usize::from(killer)]))
            .map(|row| row.slot)
            .collect::<Vec<_>>();
        assert_eq!(twins.len(), 2);
        assert_eq!(killer, twins[0]);
    }

    #[test]
    fn killer_slot_points_inside_the_roster_or_is_null() {
        let roster = roster();
        let with_killer = roster
            .rows
            .iter()
            .filter(|row| row.killer_slot.is_some())
            .count();
        assert!(with_killer > 0);
        for row in &roster.rows {
            if let Some(killer) = row.killer_slot {
                assert!(usize::from(killer) < roster.rows.len());
                assert_ne!(killer, row.slot);
            }
        }
        let (metadata, mut results) = synthetic_battle();
        results[0]["vehicles"]["777001"][0]["killerID"] = json!(123_456_789);
        let roster = extract_battle_roster(&metadata, &results, &catalog).unwrap();
        let foreign = roster
            .rows
            .iter()
            .filter(|row| row.killer_slot.is_some())
            .count();
        assert_eq!(
            foreign,
            with_killer - 1,
            "unknown killer id resolves to NULL"
        );
    }

    #[test]
    fn platoon_groups_are_local_numbers_and_solo_is_zero() {
        let roster = roster();
        let mut groups = roster
            .rows
            .iter()
            .map(|row| row.platoon_group)
            .collect::<Vec<_>>();
        groups.sort_unstable();
        groups.dedup();
        assert_eq!(groups, vec![0, 1, 2, 3]);
        let sizes = |group: u8| {
            roster
                .rows
                .iter()
                .filter(|row| row.platoon_group == group)
                .count()
        };
        let mut nonzero = [sizes(1), sizes(2), sizes(3)];
        nonzero.sort_unstable();
        assert_eq!(nonzero, [2, 2, 3]);
        assert_eq!(sizes(0), 23);
        let first_group_slot = roster
            .rows
            .iter()
            .find(|row| row.platoon_group != 0)
            .unwrap();
        assert_eq!(first_group_slot.platoon_group, 1, "numbered in slot order");
        // Each side counts its own platoons.
        assert_eq!(roster.teams[0].platoons, 1);
        assert_eq!(roster.teams[0].platooned_players, 2);
        assert_eq!(roster.teams[1].platoons, 2);
        assert_eq!(roster.teams[1].platooned_players, 5);
    }

    #[test]
    fn team_stats_are_sums_of_the_roster_rows() {
        let roster = roster();
        for team in &roster.teams {
            let members = roster
                .rows
                .iter()
                .filter(|row| row.side == team.side)
                .collect::<Vec<_>>();
            assert_eq!(team.players, 15);
            assert_eq!(
                team.damage_total,
                Some(members.iter().map(|row| row.damage_dealt.unwrap()).sum())
            );
            assert_eq!(
                team.assist_radio_total,
                Some(members.iter().map(|row| row.assist_radio.unwrap()).sum())
            );
            assert_eq!(
                team.survivors,
                Some(
                    members
                        .iter()
                        .filter(|row| row.death_reason_raw == Some(-1))
                        .count() as i64
                )
            );
            assert_eq!(team.max_health_total, Some(15 * 1_500));
            assert_eq!(
                team.class_light
                    + team.class_medium
                    + team.class_heavy
                    + team.class_td
                    + team.class_spg
                    + team.unresolved_vehicles,
                15
            );
        }
    }

    #[test]
    fn missing_member_value_makes_the_total_null_and_unresolved_tiers_are_counted() {
        let (metadata, mut results) = synthetic_battle();
        results[0]["vehicles"]["777003"][0]
            .as_object_mut()
            .unwrap()
            .remove("damageDealt");
        // typeCompDescr outside the synthetic catalog: no class, no tier.
        results[0]["vehicles"]["777004"][0]["typeCompDescr"] = json!(9_999);
        let roster = extract_battle_roster(&metadata, &results, &catalog).unwrap();
        let own = &roster.teams[0];
        assert_eq!(own.damage_total, None);
        assert!(own.kills_total.is_some());
        assert_eq!(own.unresolved_vehicles, 1);
        assert_eq!(
            (own.tier_min, own.tier_max, own.tier_sum),
            (None, None, None)
        );
        let enemy = &roster.teams[1];
        assert!(enemy.tier_min.is_some() && enemy.tier_sum.is_some());
    }

    #[test]
    fn ineligible_battles_have_no_roster() {
        let (metadata, results) = synthetic_battle();
        let mut training = metadata.clone();
        training["battleType"] = json!(2);
        assert_eq!(
            extract_battle_roster(&training, &results, &catalog),
            Err(RosterSkip::NotStrictRandom)
        );
        let mut ranked = results.clone();
        ranked[0]["common"]["bonusType"] = json!(22);
        assert_eq!(
            extract_battle_roster(&metadata, &ranked, &catalog),
            Err(RosterSkip::NotStrictRandom)
        );
        let mut small = results.clone();
        small[0]["vehicles"]
            .as_object_mut()
            .unwrap()
            .remove("777029");
        assert_eq!(
            extract_battle_roster(&metadata, &small, &catalog),
            Err(RosterSkip::RosterSize)
        );
        let mut uneven = results.clone();
        uneven[0]["vehicles"]["777020"][0]["team"] = json!(1);
        assert_eq!(
            extract_battle_roster(&metadata, &uneven, &catalog),
            Err(RosterSkip::RosterSize)
        );
        let mut bad_team = results.clone();
        bad_team[0]["vehicles"]["777020"][0]["team"] = json!(3);
        assert_eq!(
            extract_battle_roster(&metadata, &bad_team, &catalog),
            Err(RosterSkip::InvalidTeam)
        );
        let mut no_own = results.clone();
        no_own[0]["vehicles"]["777000"][0]["accountDBID"] = json!(1);
        assert_eq!(
            extract_battle_roster(&metadata, &no_own, &catalog),
            Err(RosterSkip::MissingOwnPlayer)
        );
        assert_eq!(
            extract_battle_roster(&metadata, &json!([]), &catalog),
            Err(RosterSkip::MissingResults)
        );
    }

    #[test]
    fn packet_stream_length_is_the_tail_after_the_json_blocks() {
        assert_eq!(packet_stream_bytes(1_000, 200), Some(800));
        assert_eq!(packet_stream_bytes(200, 200), Some(0));
        assert_eq!(packet_stream_bytes(100, 200), None);
    }
}
