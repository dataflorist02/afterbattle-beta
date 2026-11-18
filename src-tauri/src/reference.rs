//! A-1 reference tables (spec 32 §2): how the players in the author's battles did, per vehicle
//! tier and class, from the anonymous roster (`battle_roster`, D17). Counted at read time,
//! nothing derived is stored (D3). The answer holds aggregates only: counts and percentiles per
//! (tier, class), never a vehicle descriptor, a battle or a row.
//!
//! Definitions follow the author's A-1 research (private notes): population = the
//! roster rows of counted battles without the player, the player's platoon and rows without a
//! tier or class; linear percentiles (Hyndman-Fan type 7).

use serde::Serialize;

use crate::models::VehicleClass;

/// Population rows a cell needs for values without a mark (decision 2026-11-15).
pub(crate) const MIN_ROWS: usize = 200;
/// Population rows a cell needs for values marked "orientacyjnie"; below, only the count.
pub(crate) const MIN_ROWS_APPROXIMATE: usize = 50;
/// The player's battles in a cell needed for their median; below, the background only.
pub(crate) const MIN_OWN_BATTLES: usize = 10;
/// Lowest tier with values; lower tiers show the count only.
pub(crate) const MIN_TIER: u8 = 6;

/// One roster row as the tables need it (no vehicle descriptor, no slot, no battle id).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ReferenceRow {
    pub own_side: bool,
    pub is_own_player: bool,
    /// 0 = solo; otherwise the platoon number within the battle and side.
    pub platoon_group: u8,
    pub tier: Option<u8>,
    pub class: Option<VehicleClass>,
    pub damage: Option<i64>,
    pub assist_radio: Option<i64>,
    pub assist_track: Option<i64>,
    pub spotted: Option<i64>,
    pub kills: Option<i64>,
    pub blocked: Option<i64>,
    pub life_time: Option<i64>,
    pub shots: Option<i64>,
    pub direct_enemy_hits: Option<i64>,
    /// The player's own tank (`battles.wg_tank_id`), set only on the player's row: the best
    /// tanks of the Overview (decision 2026-11-18). Other rows never carry a vehicle.
    pub own_vehicle: Option<i64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceMetricKind {
    Damage,
    AssistRadioTrack,
    Spotted,
    Kills,
    Blocked,
    LifeTime,
    /// Background only (decision 2026-11-15): with few shots the share saturates at 100%.
    Accuracy,
}

impl ReferenceMetricKind {
    pub(crate) const ALL: [ReferenceMetricKind; 7] = [
        Self::Damage,
        Self::AssistRadioTrack,
        Self::Spotted,
        Self::Kills,
        Self::Blocked,
        Self::LifeTime,
        Self::Accuracy,
    ];

    fn value(self, row: &ReferenceRow) -> Option<f64> {
        let value = match self {
            Self::Damage => row.damage?,
            Self::AssistRadioTrack => row.assist_radio?.checked_add(row.assist_track?)?,
            Self::Spotted => row.spotted?,
            Self::Kills => row.kills?,
            Self::Blocked => row.blocked?,
            Self::LifeTime => row.life_time?,
            Self::Accuracy => {
                let shots = row.shots.filter(|shots| *shots > 0)?;
                return Some(row.direct_enemy_hits? as f64 * 100.0 / shots as f64);
            }
        };
        Some(value as f64)
    }
}

/// What a cell shows: values, values marked "orientacyjnie", or only how many rows it has.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceLevel {
    Values,
    Approximate,
    CountOnly,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceMetric {
    pub metric: ReferenceMetricKind,
    /// Population values of this metric (rows without the value are left out).
    pub count: usize,
    pub p25: f64,
    pub median: f64,
    pub p75: f64,
    pub p90: f64,
    /// The player's battles with this value in the cell.
    pub own_count: usize,
    /// The player's median, from `MIN_OWN_BATTLES` battles; never for accuracy.
    pub own_median: Option<f64>,
    /// Where the player's median sits in the population, 0–100 (share below plus half of the
    /// equal values).
    pub own_position: Option<f64>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceCell {
    pub tier: u8,
    pub vehicle_class: VehicleClass,
    pub level: ReferenceLevel,
    /// Population rows in the cell (the "masz 120" of "tło od 200 wyników").
    pub population: usize,
    pub own_battles: usize,
    /// Empty when the level is `CountOnly`.
    pub metrics: Vec<ReferenceMetric>,
}

/// One of the player's tanks against its tier × class cell (Overview, "Najlepsze czołgi",
/// decision 2026-11-18): the percentile of the tank's median damage among the other players'
/// rows of the cell. Only cells with values (≥ `MIN_ROWS`) and tanks with ≥ `MIN_OWN_BATTLES`.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TankPosition {
    /// The player's own tank, as in the vehicle history (never another player's).
    pub vehicle_id: i64,
    pub tier: u8,
    pub vehicle_class: VehicleClass,
    /// The player's counted battles on this tank in the cell (with damage).
    pub battles: usize,
    pub median_damage: f64,
    /// 0–100: share of the cell's damage values below plus half of the equal ones.
    pub position: f64,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceTables {
    /// Counted battles with a usable roster.
    pub battles: usize,
    pub min_rows: usize,
    pub min_rows_approximate: usize,
    pub min_own_battles: usize,
    pub cells: Vec<ReferenceCell>,
    /// The player's tanks by position, highest first (ties: more battles first).
    pub tanks: Vec<TankPosition>,
}

/// Linear percentile (Hyndman-Fan type 7) of an ascending slice; `p` in 0..=1.
pub(crate) fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    let last = sorted.len().checked_sub(1)?;
    let position = last as f64 * p.clamp(0.0, 1.0);
    let lower = position.floor() as usize;
    let upper = (lower + 1).min(last);
    let weight = position - lower as f64;
    Some(sorted[lower] + (sorted[upper] - sorted[lower]) * weight)
}

/// Share of an ascending slice below `value`, plus half of the values equal to it, in percent.
fn position(sorted: &[f64], value: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let below = sorted.partition_point(|item| *item < value);
    let up_to = sorted.partition_point(|item| *item <= value);
    Some((below as f64 + (up_to - below) as f64 / 2.0) * 100.0 / sorted.len() as f64)
}

/// A cell key: tier and a known class.
fn cell_of(row: &ReferenceRow) -> Option<(u8, VehicleClass)> {
    let class = row.class.filter(|class| *class != VehicleClass::Unknown)?;
    Some((row.tier?, class))
}

#[derive(Default)]
struct Collected {
    population: usize,
    own_battles: usize,
    values: [Vec<f64>; 7],
    own: [Vec<f64>; 7],
    /// The player's damage per own tank in this cell.
    tanks: std::collections::BTreeMap<i64, Vec<f64>>,
}

fn level(tier: u8, population: usize) -> ReferenceLevel {
    if tier < MIN_TIER || population < MIN_ROWS_APPROXIMATE {
        ReferenceLevel::CountOnly
    } else if population < MIN_ROWS {
        ReferenceLevel::Approximate
    } else {
        ReferenceLevel::Values
    }
}

fn sorted(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(f64::total_cmp);
    values
}

fn metric(kind: ReferenceMetricKind, values: Vec<f64>, own: Vec<f64>) -> Option<ReferenceMetric> {
    let values = sorted(values);
    let own = sorted(own);
    let at = |p| percentile(&values, p);
    let own_median = (kind != ReferenceMetricKind::Accuracy && own.len() >= MIN_OWN_BATTLES)
        .then(|| percentile(&own, 0.5))
        .flatten();
    Some(ReferenceMetric {
        metric: kind,
        count: values.len(),
        p25: at(0.25)?,
        median: at(0.5)?,
        p75: at(0.75)?,
        p90: at(0.9)?,
        own_count: own.len(),
        own_median,
        own_position: own_median.and_then(|median| position(&values, median)),
    })
}

/// The tables from the rosters of counted battles, one slice of rows per battle. A battle needs
/// exactly one row of the player (as the parser builds it); otherwise it is left out.
pub(crate) fn reference_tables<'a>(
    battles: impl IntoIterator<Item = &'a [ReferenceRow]>,
) -> ReferenceTables {
    let mut cells = std::collections::BTreeMap::<(u8, VehicleClass), Collected>::new();
    let mut counted = 0;
    for rows in battles {
        let mut players = rows.iter().filter(|row| row.is_own_player);
        let (Some(player), None) = (players.next(), players.next()) else {
            continue;
        };
        counted += 1;
        for row in rows {
            let platoon_mate = !row.is_own_player
                && player.platoon_group != 0
                && row.own_side
                && row.platoon_group == player.platoon_group;
            let Some(key) = cell_of(row).filter(|_| !platoon_mate) else {
                continue;
            };
            let cell = cells.entry(key).or_default();
            let (target, count) = if row.is_own_player {
                (&mut cell.own, &mut cell.own_battles)
            } else {
                (&mut cell.values, &mut cell.population)
            };
            *count += 1;
            if let (true, Some(vehicle), Some(damage)) =
                (row.is_own_player, row.own_vehicle, row.damage)
            {
                cell.tanks.entry(vehicle).or_default().push(damage as f64);
            }
            for (values, kind) in target.iter_mut().zip(ReferenceMetricKind::ALL) {
                if let Some(value) = kind.value(row) {
                    values.push(value);
                }
            }
        }
    }
    let mut tanks = Vec::new();
    let cells = cells
        .into_iter()
        .map(|((tier, vehicle_class), collected)| {
            let level = level(tier, collected.population);
            if level == ReferenceLevel::Values {
                let damage = sorted(collected.values[0].clone());
                for (vehicle_id, own) in &collected.tanks {
                    if own.len() < MIN_OWN_BATTLES {
                        continue;
                    }
                    let own = sorted(own.clone());
                    let (Some(median_damage), false) = (percentile(&own, 0.5), damage.is_empty())
                    else {
                        continue;
                    };
                    tanks.push(TankPosition {
                        vehicle_id: *vehicle_id,
                        tier,
                        vehicle_class,
                        battles: own.len(),
                        median_damage,
                        position: position(&damage, median_damage).unwrap_or(0.0),
                    });
                }
            }
            let metrics = if level == ReferenceLevel::CountOnly {
                Vec::new()
            } else {
                ReferenceMetricKind::ALL
                    .into_iter()
                    .zip(collected.values.into_iter().zip(collected.own))
                    .filter_map(|(kind, (values, own))| metric(kind, values, own))
                    .collect()
            };
            ReferenceCell {
                tier,
                vehicle_class,
                level,
                population: collected.population,
                own_battles: collected.own_battles,
                metrics,
            }
        })
        .collect();
    ReferenceTables {
        battles: counted,
        min_rows: MIN_ROWS,
        min_rows_approximate: MIN_ROWS_APPROXIMATE,
        min_own_battles: MIN_OWN_BATTLES,
        cells,
        tanks: {
            tanks.sort_by(|a, b| {
                b.position
                    .total_cmp(&a.position)
                    .then(b.battles.cmp(&a.battles))
                    .then(a.vehicle_id.cmp(&b.vehicle_id))
            });
            tanks
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(tier: u8, class: VehicleClass, damage: i64) -> ReferenceRow {
        ReferenceRow {
            own_side: false,
            tier: Some(tier),
            class: Some(class),
            damage: Some(damage),
            assist_radio: Some(damage / 2),
            assist_track: Some(0),
            spotted: Some(1),
            kills: Some(0),
            blocked: Some(0),
            life_time: Some(300),
            shots: Some(10),
            direct_enemy_hits: Some(5),
            ..ReferenceRow::default()
        }
    }

    fn own(tier: u8, class: VehicleClass, damage: i64) -> ReferenceRow {
        ReferenceRow {
            own_side: true,
            is_own_player: true,
            ..row(tier, class, damage)
        }
    }

    /// Battles of one own row each plus the given population rows, spread over the battles.
    fn tables(own_rows: Vec<ReferenceRow>, population: Vec<ReferenceRow>) -> ReferenceTables {
        let mut battles: Vec<Vec<ReferenceRow>> =
            own_rows.into_iter().map(|row| vec![row]).collect();
        if battles.is_empty() {
            battles.push(vec![own(10, VehicleClass::Spg, 0)]);
        }
        let count = battles.len();
        for (index, row) in population.into_iter().enumerate() {
            battles[index % count].push(row);
        }
        reference_tables(battles.iter().map(Vec::as_slice))
    }

    fn cell(tables: &ReferenceTables, tier: u8, class: VehicleClass) -> &ReferenceCell {
        tables
            .cells
            .iter()
            .find(|cell| cell.tier == tier && cell.vehicle_class == class)
            .unwrap()
    }

    fn metric(cell: &ReferenceCell, kind: ReferenceMetricKind) -> &ReferenceMetric {
        cell.metrics
            .iter()
            .find(|metric| metric.metric == kind)
            .unwrap()
    }

    fn heavy(count: i64) -> Vec<ReferenceRow> {
        (1..=count)
            .map(|damage| row(8, VehicleClass::HeavyTank, damage))
            .collect()
    }

    #[test]
    fn percentile_is_linear_type_seven() {
        let values = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&values, 0.25), Some(1.75));
        assert_eq!(percentile(&values, 0.5), Some(2.5));
        assert_eq!(percentile(&values, 0.9), Some(3.7));
        assert_eq!(percentile(&[7.0], 0.75), Some(7.0));
        assert_eq!(percentile(&[], 0.5), None);
    }

    /// Every metric of a cell is read from its own column (STOP 2 preparation: the Tank screen
    /// looked as if every row had the damage numbers; it was the frontend fixture).
    #[test]
    fn every_metric_takes_its_own_column() {
        let distinct = |is_own: bool| ReferenceRow {
            own_side: is_own,
            is_own_player: is_own,
            tier: Some(9),
            class: Some(VehicleClass::LightTank),
            damage: Some(if is_own { 2_000 } else { 1_000 }),
            assist_radio: Some(200),
            assist_track: Some(100),
            spotted: Some(4),
            kills: Some(2),
            blocked: Some(700),
            life_time: Some(250),
            shots: Some(10),
            direct_enemy_hits: Some(6),
            ..ReferenceRow::default()
        };
        let tables = tables(
            (0..10).map(|_| distinct(true)).collect(),
            (0..200).map(|_| distinct(false)).collect(),
        );
        let cell = cell(&tables, 9, VehicleClass::LightTank);
        let medians: Vec<(ReferenceMetricKind, f64)> = cell
            .metrics
            .iter()
            .map(|metric| (metric.metric, metric.median))
            .collect();
        assert_eq!(
            medians,
            vec![
                (ReferenceMetricKind::Damage, 1_000.0),
                (ReferenceMetricKind::AssistRadioTrack, 300.0),
                (ReferenceMetricKind::Spotted, 4.0),
                (ReferenceMetricKind::Kills, 2.0),
                (ReferenceMetricKind::Blocked, 700.0),
                (ReferenceMetricKind::LifeTime, 250.0),
                (ReferenceMetricKind::Accuracy, 60.0),
            ]
        );
        assert_eq!(
            metric(cell, ReferenceMetricKind::Damage).own_median,
            Some(2_000.0)
        );
        assert_eq!(
            metric(cell, ReferenceMetricKind::Blocked).own_median,
            Some(700.0)
        );
    }

    fn own_on(tier: u8, class: VehicleClass, vehicle: i64, damage: i64) -> ReferenceRow {
        ReferenceRow {
            own_vehicle: Some(vehicle),
            ..own(tier, class, damage)
        }
    }

    #[test]
    fn a_tank_is_placed_by_its_median_damage_in_its_cell_from_10_battles() {
        // Population 1..=200 in heavy VIII; tank 7 with median 149.5 (74.5th percentile), tank 8
        // with median 50 (24.75th), tank 9 with only 9 battles.
        let mut own_rows: Vec<ReferenceRow> = (0..10)
            .map(|index| own_on(8, VehicleClass::HeavyTank, 7, 145 + index))
            .collect();
        own_rows.extend((0..11).map(|index| own_on(8, VehicleClass::HeavyTank, 8, 45 + index)));
        own_rows.extend((0..9).map(|_| own_on(8, VehicleClass::HeavyTank, 9, 199)));
        let tables = tables(own_rows, heavy(200));
        let tanks: Vec<(i64, usize, f64, f64)> = tables
            .tanks
            .iter()
            .map(|tank| {
                (
                    tank.vehicle_id,
                    tank.battles,
                    tank.median_damage,
                    tank.position,
                )
            })
            .collect();
        assert_eq!(tanks, vec![(7, 10, 149.5, 74.5), (8, 11, 50.0, 24.75)]);
        assert!(
            tables
                .tanks
                .iter()
                .all(|tank| tank.tier == 8 && tank.vehicle_class == VehicleClass::HeavyTank)
        );
    }

    #[test]
    fn a_tank_in_a_cell_below_200_rows_has_no_position() {
        let own_rows: Vec<ReferenceRow> = (0..12)
            .map(|_| own_on(8, VehicleClass::HeavyTank, 7, 100))
            .collect();
        assert!(tables(own_rows.clone(), heavy(199)).tanks.is_empty());
        assert_eq!(tables(own_rows, heavy(200)).tanks.len(), 1);
    }

    #[test]
    fn a_cell_of_known_values_gives_the_known_percentiles() {
        let tables = tables(Vec::new(), heavy(200));
        let cell = cell(&tables, 8, VehicleClass::HeavyTank);
        assert_eq!(cell.level, ReferenceLevel::Values);
        assert_eq!(cell.population, 200);
        let damage = metric(cell, ReferenceMetricKind::Damage);
        assert_eq!(damage.count, 200);
        assert!((damage.p25 - 50.75).abs() < 1e-9);
        assert!((damage.median - 100.5).abs() < 1e-9);
        assert!((damage.p75 - 150.25).abs() < 1e-9);
        assert!((damage.p90 - 180.1).abs() < 1e-9);
        assert_eq!(cell.metrics.len(), ReferenceMetricKind::ALL.len());
    }

    #[test]
    fn rows_decide_the_level_at_49_50_199_and_200() {
        for (rows, level) in [
            (49, ReferenceLevel::CountOnly),
            (50, ReferenceLevel::Approximate),
            (199, ReferenceLevel::Approximate),
            (200, ReferenceLevel::Values),
        ] {
            let tables = tables(Vec::new(), heavy(rows));
            let cell = cell(&tables, 8, VehicleClass::HeavyTank);
            assert_eq!((cell.population, cell.level), (rows as usize, level));
            assert_eq!(cell.metrics.is_empty(), level == ReferenceLevel::CountOnly);
        }
    }

    #[test]
    fn tiers_below_six_show_only_the_count() {
        let population = (1..=300)
            .map(|damage| row(5, VehicleClass::MediumTank, damage))
            .collect();
        let tables = tables(Vec::new(), population);
        let cell = cell(&tables, 5, VehicleClass::MediumTank);
        assert_eq!(
            (cell.population, cell.level),
            (300, ReferenceLevel::CountOnly)
        );
        assert!(cell.metrics.is_empty());
    }

    #[test]
    fn the_players_median_needs_ten_battles_and_sits_in_the_distribution() {
        for (battles, shown) in [(9, false), (10, true)] {
            let own_rows = (0..battles)
                .map(|_| own(8, VehicleClass::HeavyTank, 150))
                .collect();
            let tables = tables(own_rows, heavy(200));
            let cell = cell(&tables, 8, VehicleClass::HeavyTank);
            assert_eq!(cell.own_battles, battles);
            let damage = metric(cell, ReferenceMetricKind::Damage);
            assert_eq!(damage.own_count, battles);
            assert_eq!(damage.own_median.is_some(), shown);
            assert_eq!(damage.own_position.is_some(), shown);
            if shown {
                assert_eq!(damage.own_median, Some(150.0));
                // 149 values below 150, one equal: (149 + 0.5) / 200.
                assert!((damage.own_position.unwrap() - 74.75).abs() < 1e-9);
            }
            // The player's own rows never enter the population.
            assert_eq!(cell.population, 200);
        }
    }

    #[test]
    fn accuracy_is_background_only() {
        let own_rows = (0..12)
            .map(|_| own(8, VehicleClass::HeavyTank, 100))
            .collect();
        let tables = tables(own_rows, heavy(200));
        let accuracy = metric(
            cell(&tables, 8, VehicleClass::HeavyTank),
            ReferenceMetricKind::Accuracy,
        );
        assert_eq!(accuracy.count, 200);
        assert!((accuracy.median - 50.0).abs() < 1e-9);
        assert_eq!((accuracy.own_median, accuracy.own_position), (None, None));
    }

    #[test]
    fn the_players_platoon_is_left_out_but_not_the_enemy_group_of_the_same_number() {
        let player = ReferenceRow {
            platoon_group: 2,
            ..own(8, VehicleClass::HeavyTank, 100)
        };
        let mate = ReferenceRow {
            own_side: true,
            platoon_group: 2,
            ..row(8, VehicleClass::HeavyTank, 1)
        };
        let other_platoon = ReferenceRow {
            own_side: true,
            platoon_group: 1,
            ..row(8, VehicleClass::HeavyTank, 2)
        };
        let enemy_same_number = ReferenceRow {
            platoon_group: 2,
            ..row(8, VehicleClass::HeavyTank, 3)
        };
        let solo_team_mate = ReferenceRow {
            own_side: true,
            ..row(8, VehicleClass::HeavyTank, 4)
        };
        let battle = [
            player,
            mate,
            other_platoon,
            enemy_same_number,
            solo_team_mate,
        ];
        let tables = reference_tables([battle.as_slice()]);
        assert_eq!(cell(&tables, 8, VehicleClass::HeavyTank).population, 3);

        // A solo player has no platoon: own-side rows of group 0 stay.
        let solo = [
            own(8, VehicleClass::HeavyTank, 100),
            row(8, VehicleClass::HeavyTank, 1),
        ];
        let solo_mate = ReferenceRow {
            own_side: true,
            ..row(8, VehicleClass::HeavyTank, 2)
        };
        let battle = [solo[0].clone(), solo[1].clone(), solo_mate];
        let tables = reference_tables([battle.as_slice()]);
        assert_eq!(cell(&tables, 8, VehicleClass::HeavyTank).population, 2);
    }

    #[test]
    fn rows_without_tier_or_class_and_battles_without_one_player_row_are_left_out() {
        let no_tier = ReferenceRow {
            tier: None,
            ..row(8, VehicleClass::HeavyTank, 1)
        };
        let unknown_class = row(8, VehicleClass::Unknown, 1);
        let no_class = ReferenceRow {
            class: None,
            ..row(8, VehicleClass::HeavyTank, 1)
        };
        let good = row(8, VehicleClass::HeavyTank, 1);
        let battle = [
            own(8, VehicleClass::HeavyTank, 1),
            no_tier,
            unknown_class,
            no_class,
            good,
        ];
        let without_player = [row(8, VehicleClass::HeavyTank, 1)];
        let two_players = [
            own(8, VehicleClass::HeavyTank, 1),
            own(8, VehicleClass::HeavyTank, 1),
            row(8, VehicleClass::HeavyTank, 1),
        ];
        let tables = reference_tables([
            battle.as_slice(),
            without_player.as_slice(),
            two_players.as_slice(),
        ]);
        assert_eq!(tables.battles, 1);
        assert_eq!(tables.cells.len(), 1);
        assert_eq!(cell(&tables, 8, VehicleClass::HeavyTank).population, 1);
    }

    #[test]
    fn assist_needs_both_parts_and_a_metric_counts_only_rows_with_its_value() {
        let mut population = heavy(200);
        population[0].assist_track = None;
        population[1].damage = None;
        population[2].shots = Some(0);
        let tables = tables(Vec::new(), population);
        let cell = cell(&tables, 8, VehicleClass::HeavyTank);
        assert_eq!(cell.population, 200);
        assert_eq!(
            metric(cell, ReferenceMetricKind::AssistRadioTrack).count,
            199
        );
        assert_eq!(metric(cell, ReferenceMetricKind::Damage).count, 199);
        assert_eq!(metric(cell, ReferenceMetricKind::Accuracy).count, 199);
    }

    #[test]
    fn cells_come_in_tier_then_class_order_and_the_answer_holds_no_row_data() {
        let mut population = heavy(60);
        population.extend((1..=60).map(|damage| row(6, VehicleClass::LightTank, damage)));
        population.extend((1..=60).map(|damage| row(8, VehicleClass::LightTank, damage)));
        let tables = tables(Vec::new(), population);
        let order: Vec<_> = tables
            .cells
            .iter()
            .map(|cell| (cell.tier, cell.vehicle_class))
            .collect();
        assert_eq!(
            order,
            [
                (6, VehicleClass::LightTank),
                (8, VehicleClass::LightTank),
                (8, VehicleClass::HeavyTank),
                (10, VehicleClass::Spg),
            ]
        );
        let json = serde_json::to_string(&tables).unwrap();
        for forbidden in [
            "typeCompDescr",
            "type_comp_descr",
            "slot",
            "battleId",
            "arena",
        ] {
            assert!(!json.contains(forbidden), "{forbidden}");
        }
    }
}
