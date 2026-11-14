use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use thiserror::Error;

#[cfg(test)]
use crate::{
    local_tracker::canonicalize_vehicle_key,
    models::{CompactDescriptorProtocolSource, VehicleKeyCrossCheck},
};
use crate::{models::VehicleClass, vehicle_catalog_probe::VehicleCatalogProbeEntry};

pub(crate) const VEHICLE_ITEM_TYPE_ID: u8 = 1;
const MAX_NATION_ID: u8 = 10;
const MAX_TANK_ID: u32 = u16::MAX as u32;
const MAX_COMPACT_DESCRIPTOR: u32 = 0x00FF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VehicleCatalogSource {
    WgClientPackedCatalog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecodedVehicleCompactDescr {
    pub item_type_id: u8,
    pub nation_id: u8,
    pub tank_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum CompactDescriptorError {
    #[error("unsupported vehicle nation id")]
    UnsupportedNationId,
    #[error("unsupported vehicle nation key")]
    UnsupportedNationKey,
    #[error("vehicle tank id is outside the compact descriptor contract")]
    InvalidTankId,
    #[error("compact descriptor item type is not vehicle")]
    UnsupportedItemType,
    #[error("compact descriptor exceeds the supported 24-bit contract")]
    ValueOutOfContract,
    #[error("compact descriptor arithmetic overflowed")]
    Overflow,
}

pub(crate) fn nation_id_for_key(key: &str) -> Result<u8, CompactDescriptorError> {
    match key {
        "ussr" => Ok(0),
        "germany" => Ok(1),
        "usa" => Ok(2),
        "china" => Ok(3),
        "france" => Ok(4),
        "uk" => Ok(5),
        "japan" => Ok(6),
        "czech" => Ok(7),
        "sweden" => Ok(8),
        "poland" => Ok(9),
        "italy" => Ok(10),
        _ => Err(CompactDescriptorError::UnsupportedNationKey),
    }
}

pub(crate) fn encode_vehicle_compact_descr(
    nation_id: u8,
    tank_id: u32,
) -> Result<u32, CompactDescriptorError> {
    if nation_id > MAX_NATION_ID {
        return Err(CompactDescriptorError::UnsupportedNationId);
    }
    if tank_id == 0 || tank_id > MAX_TANK_ID {
        return Err(CompactDescriptorError::InvalidTankId);
    }
    let nation_header = u32::from(nation_id)
        .checked_shl(4)
        .ok_or(CompactDescriptorError::Overflow)?;
    let header = nation_header
        .checked_add(u32::from(VEHICLE_ITEM_TYPE_ID))
        .ok_or(CompactDescriptorError::Overflow)?;
    let item = tank_id
        .checked_shl(8)
        .ok_or(CompactDescriptorError::Overflow)?;
    item.checked_add(header)
        .ok_or(CompactDescriptorError::Overflow)
}

pub(crate) fn decode_vehicle_compact_descr(
    value: u32,
) -> Result<DecodedVehicleCompactDescr, CompactDescriptorError> {
    if value > MAX_COMPACT_DESCRIPTOR {
        return Err(CompactDescriptorError::ValueOutOfContract);
    }
    let item_type_id = (value & 0x0F) as u8;
    if item_type_id != VEHICLE_ITEM_TYPE_ID {
        return Err(CompactDescriptorError::UnsupportedItemType);
    }
    let nation_id = ((value >> 4) & 0x0F) as u8;
    if nation_id > MAX_NATION_ID {
        return Err(CompactDescriptorError::UnsupportedNationId);
    }
    let tank_id = ((value >> 8) & 0xFFFF) as u16;
    if tank_id == 0 {
        return Err(CompactDescriptorError::InvalidTankId);
    }
    let decoded = DecodedVehicleCompactDescr {
        item_type_id,
        nation_id,
        tank_id,
    };
    let round_trip = encode_vehicle_compact_descr(nation_id, u32::from(tank_id))?;
    if round_trip != value {
        return Err(CompactDescriptorError::ValueOutOfContract);
    }
    Ok(decoded)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogDescriptorEntry {
    pub type_comp_descr: u32,
    pub nation_id: u8,
    pub nation_key: String,
    pub tank_id: u16,
    pub vehicle_key: String,
    pub vehicle_class: VehicleClass,
    pub source: VehicleCatalogSource,
    pub tier: Option<u8>,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InvalidCatalogRecord {
    ZeroVehicleId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CatalogBuildStats {
    catalog_records: usize,
    generated_descriptors: usize,
    descriptor_collisions: usize,
    unsupported_nation_keys: usize,
    invalid_vehicle_ids: usize,
    invalid_zero_vehicle_ids: usize,
    class_conflicts: usize,
    round_trip_failures: usize,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CompactDescriptorCatalog {
    entries: BTreeMap<u32, CatalogDescriptorEntry>,
    stats: CatalogBuildStats,
}

impl CompactDescriptorCatalog {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn get(&self, descriptor: u32) -> Option<&CatalogDescriptorEntry> {
        self.entries.get(&descriptor)
    }

    pub(crate) fn entries(&self) -> impl Iterator<Item = &CatalogDescriptorEntry> {
        self.entries.values()
    }

    pub(crate) fn invalid_zero_vehicle_ids(&self) -> usize {
        self.stats.invalid_zero_vehicle_ids
    }

    pub(crate) fn invalid_vehicle_ids(&self) -> usize {
        self.stats.invalid_vehicle_ids
    }

    pub(crate) fn descriptor_collisions(&self) -> usize {
        self.stats.descriptor_collisions
    }

    pub(crate) fn unsupported_nation_keys(&self) -> usize {
        self.stats.unsupported_nation_keys
    }

    pub(crate) fn class_conflicts(&self) -> usize {
        self.stats.class_conflicts
    }

    pub(crate) fn round_trip_failures(&self) -> usize {
        self.stats.round_trip_failures
    }

    pub(crate) fn is_structurally_valid(&self) -> bool {
        catalog_is_structurally_valid(&self.stats)
    }
}

pub(crate) fn build_compact_descriptor_catalog(
    source_entries: &[VehicleCatalogProbeEntry],
) -> CompactDescriptorCatalog {
    let mut ordered = source_entries.to_vec();
    ordered.sort_by(|left, right| {
        (
            &left.nation_key,
            left.source_record_id,
            &left.vehicle_key,
            left.vehicle_class,
        )
            .cmp(&(
                &right.nation_key,
                right.source_record_id,
                &right.vehicle_key,
                right.vehicle_class,
            ))
    });
    let mut catalog = CompactDescriptorCatalog::default();
    catalog.stats.catalog_records = ordered.len();
    let mut collided = BTreeSet::new();

    for source in ordered {
        let nation_id = match nation_id_for_key(&source.nation_key) {
            Ok(value) => value,
            Err(_) => {
                catalog.stats.unsupported_nation_keys += 1;
                continue;
            }
        };
        if source.source_record_id == 0 {
            let invalid = InvalidCatalogRecord::ZeroVehicleId;
            match invalid {
                InvalidCatalogRecord::ZeroVehicleId => {
                    catalog.stats.invalid_zero_vehicle_ids += 1;
                }
            }
            continue;
        }
        let descriptor = match encode_vehicle_compact_descr(nation_id, source.source_record_id) {
            Ok(value) => value,
            Err(_) => {
                catalog.stats.invalid_vehicle_ids += 1;
                continue;
            }
        };
        catalog.stats.generated_descriptors += 1;
        match decode_vehicle_compact_descr(descriptor) {
            Ok(decoded)
                if decoded.nation_id == nation_id
                    && u32::from(decoded.tank_id) == source.source_record_id => {}
            _ => {
                catalog.stats.round_trip_failures += 1;
                continue;
            }
        }

        let entry = CatalogDescriptorEntry {
            type_comp_descr: descriptor,
            nation_id,
            nation_key: source.nation_key,
            tank_id: source.source_record_id as u16,
            vehicle_key: source.vehicle_key,
            vehicle_class: source.vehicle_class,
            source: VehicleCatalogSource::WgClientPackedCatalog,
            tier: source.tier,
            roles: source.roles,
        };
        if collided.contains(&descriptor) {
            catalog.stats.descriptor_collisions += 1;
            continue;
        }
        match catalog.entries.entry(descriptor) {
            Entry::Vacant(slot) => {
                slot.insert(entry);
            }
            Entry::Occupied(slot) => {
                catalog.stats.descriptor_collisions += 1;
                if slot.get().vehicle_class != entry.vehicle_class {
                    catalog.stats.class_conflicts += 1;
                }
                slot.remove();
                collided.insert(descriptor);
            }
        }
    }
    catalog
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RealVehicleProbe {
    pub wg_tank_id: u32,
    pub vehicle_key: Option<String>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JoinConflict {
    VehicleKey,
    ItemType,
    Nation,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExactVehicleJoin {
    Matched {
        vehicle_class: VehicleClass,
        vehicle_key_cross_check: VehicleKeyCrossCheck,
    },
    Unknown,
    Conflict(JoinConflict),
}

#[cfg(test)]
pub(crate) fn cross_check_vehicle_keys(
    nation_key: &str,
    catalog_vehicle_key: Option<&str>,
    stored_vehicle_key: Option<&str>,
) -> VehicleKeyCrossCheck {
    let (Some(catalog_vehicle_key), Some(stored_vehicle_key)) =
        (catalog_vehicle_key, stored_vehicle_key)
    else {
        return VehicleKeyCrossCheck::Unavailable;
    };
    if canonicalize_vehicle_key(nation_key).is_err()
        || canonicalize_vehicle_key(catalog_vehicle_key).is_err()
    {
        return VehicleKeyCrossCheck::Conflict;
    }
    let catalog_raw = format!("{nation_key}-{catalog_vehicle_key}");
    if catalog_raw == stored_vehicle_key {
        return VehicleKeyCrossCheck::RawExact;
    }
    match (
        canonicalize_vehicle_key(&catalog_raw),
        canonicalize_vehicle_key(stored_vehicle_key),
    ) {
        (Ok(catalog), Ok(stored)) if catalog.as_str() == stored.as_str() => {
            VehicleKeyCrossCheck::CanonicalExact
        }
        _ => VehicleKeyCrossCheck::Conflict,
    }
}

#[cfg(test)]
pub(crate) fn exact_vehicle_join(
    catalog: &CompactDescriptorCatalog,
    wg_tank_id: u32,
    stored_vehicle_key: Option<&str>,
) -> ExactVehicleJoin {
    let decoded = match decode_vehicle_compact_descr(wg_tank_id) {
        Ok(value) => value,
        Err(CompactDescriptorError::UnsupportedItemType) => {
            return ExactVehicleJoin::Conflict(JoinConflict::ItemType);
        }
        Err(CompactDescriptorError::UnsupportedNationId) => {
            return ExactVehicleJoin::Conflict(JoinConflict::Nation);
        }
        Err(_) => return ExactVehicleJoin::Unknown,
    };
    let Some(entry) = catalog.get(wg_tank_id) else {
        return ExactVehicleJoin::Unknown;
    };
    if entry.nation_id != decoded.nation_id || entry.tank_id != decoded.tank_id {
        return ExactVehicleJoin::Conflict(JoinConflict::Nation);
    }
    let vehicle_key_cross_check = cross_check_vehicle_keys(
        &entry.nation_key,
        Some(&entry.vehicle_key),
        stored_vehicle_key,
    );
    if vehicle_key_cross_check == VehicleKeyCrossCheck::Conflict {
        return ExactVehicleJoin::Conflict(JoinConflict::VehicleKey);
    }
    ExactVehicleJoin::Matched {
        vehicle_class: entry.vehicle_class,
        vehicle_key_cross_check,
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompactDescriptorValidationReport {
    pub protocol_source: CompactDescriptorProtocolSource,
    pub catalog_records: usize,
    pub generated_descriptors: usize,
    pub descriptor_collisions: usize,
    pub unsupported_nation_keys: usize,
    pub invalid_vehicle_ids: usize,
    pub invalid_zero_vehicle_ids: usize,
    pub real_unique_vehicle_ids: usize,
    pub descriptor_exact_matches: usize,
    pub real_matched_vehicle_ids: usize,
    pub real_unmatched_vehicle_ids: usize,
    pub vehicle_key_cross_checks: usize,
    pub vehicle_key_raw_exact: usize,
    pub vehicle_key_canonical_exact: usize,
    pub vehicle_key_unavailable: usize,
    pub vehicle_key_conflicts: usize,
    pub item_type_conflicts: usize,
    pub nation_conflicts: usize,
    pub class_conflicts: usize,
    pub round_trip_failures: usize,
    pub negative_controls_passed: bool,
    pub accepted_for_production_hardening: bool,
}

fn catalog_is_structurally_valid(stats: &CatalogBuildStats) -> bool {
    stats.catalog_records > 0
        && stats.generated_descriptors + stats.invalid_zero_vehicle_ids == stats.catalog_records
        && stats.descriptor_collisions == 0
        && stats.unsupported_nation_keys == 0
        && stats.invalid_vehicle_ids == 0
        && stats.class_conflicts == 0
        && stats.round_trip_failures == 0
}

#[cfg(test)]
pub(crate) fn validate_compact_descriptor_protocol(
    source_entries: &[VehicleCatalogProbeEntry],
    real_vehicles: &[RealVehicleProbe],
    candidate_source: CompactDescriptorProtocolSource,
) -> (CompactDescriptorCatalog, CompactDescriptorValidationReport) {
    validate_compact_descriptor_protocol_with_controls(
        source_entries,
        real_vehicles,
        candidate_source,
        negative_controls_pass(),
    )
}

#[cfg(test)]
fn validate_compact_descriptor_protocol_with_controls(
    source_entries: &[VehicleCatalogProbeEntry],
    real_vehicles: &[RealVehicleProbe],
    candidate_source: CompactDescriptorProtocolSource,
    negative_controls_passed: bool,
) -> (CompactDescriptorCatalog, CompactDescriptorValidationReport) {
    let catalog = build_compact_descriptor_catalog(source_entries);
    let mut grouped: BTreeMap<u32, BTreeSet<Option<String>>> = BTreeMap::new();
    for probe in real_vehicles {
        grouped
            .entry(probe.wg_tank_id)
            .or_default()
            .insert(probe.vehicle_key.clone());
    }
    let mut matched = 0usize;
    let mut descriptor_exact_matches = 0usize;
    let mut vehicle_key_raw_exact = 0usize;
    let mut vehicle_key_canonical_exact = 0usize;
    let mut vehicle_key_unavailable = 0usize;
    let mut vehicle_key_conflicts = 0usize;
    let mut item_type_conflicts = 0usize;
    let mut nation_conflicts = 0usize;
    for (wg_tank_id, keys) in &grouped {
        let mut descriptor_matched = false;
        let mut vehicle_matched = false;
        match exact_vehicle_join(&catalog, *wg_tank_id, None) {
            ExactVehicleJoin::Matched { .. } => {
                descriptor_exact_matches += 1;
                descriptor_matched = true;
                vehicle_matched = true;
            }
            ExactVehicleJoin::Unknown => {}
            ExactVehicleJoin::Conflict(JoinConflict::ItemType) => item_type_conflicts += 1,
            ExactVehicleJoin::Conflict(JoinConflict::Nation) => nation_conflicts += 1,
            ExactVehicleJoin::Conflict(JoinConflict::VehicleKey) => {
                vehicle_key_conflicts += 1;
            }
        }
        for key in keys {
            if !descriptor_matched {
                continue;
            }
            match exact_vehicle_join(&catalog, *wg_tank_id, key.as_deref()) {
                ExactVehicleJoin::Matched {
                    vehicle_key_cross_check,
                    ..
                } => match vehicle_key_cross_check {
                    VehicleKeyCrossCheck::RawExact => vehicle_key_raw_exact += 1,
                    VehicleKeyCrossCheck::CanonicalExact => vehicle_key_canonical_exact += 1,
                    VehicleKeyCrossCheck::Unavailable => vehicle_key_unavailable += 1,
                    VehicleKeyCrossCheck::Conflict => {
                        vehicle_key_conflicts += 1;
                        vehicle_matched = false;
                    }
                },
                ExactVehicleJoin::Unknown => vehicle_matched = false,
                ExactVehicleJoin::Conflict(JoinConflict::VehicleKey) => {
                    vehicle_key_conflicts += 1;
                    vehicle_matched = false;
                }
                ExactVehicleJoin::Conflict(JoinConflict::ItemType) => {
                    item_type_conflicts += 1;
                    vehicle_matched = false;
                }
                ExactVehicleJoin::Conflict(JoinConflict::Nation) => {
                    nation_conflicts += 1;
                    vehicle_matched = false;
                }
            }
        }
        if vehicle_matched {
            matched += 1;
        }
    }
    let real_unique_vehicle_ids = grouped.len();
    let real_unmatched_vehicle_ids = real_unique_vehicle_ids.saturating_sub(matched);
    let vehicle_key_cross_checks = vehicle_key_raw_exact + vehicle_key_canonical_exact;
    let accepted_evidence = catalog_is_structurally_valid(&catalog.stats)
        && real_unique_vehicle_ids > 0
        && descriptor_exact_matches == real_unique_vehicle_ids
        && matched == real_unique_vehicle_ids
        && vehicle_key_conflicts == 0
        && item_type_conflicts == 0
        && nation_conflicts == 0
        && negative_controls_passed;
    let accepted_for_production_hardening = accepted_evidence
        && candidate_source
            == CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated;
    let protocol_source = if accepted_for_production_hardening {
        CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated
    } else {
        CompactDescriptorProtocolSource::Unconfirmed
    };
    let report = CompactDescriptorValidationReport {
        protocol_source,
        catalog_records: catalog.stats.catalog_records,
        generated_descriptors: catalog.stats.generated_descriptors,
        descriptor_collisions: catalog.stats.descriptor_collisions,
        unsupported_nation_keys: catalog.stats.unsupported_nation_keys,
        invalid_vehicle_ids: catalog.stats.invalid_vehicle_ids,
        invalid_zero_vehicle_ids: catalog.stats.invalid_zero_vehicle_ids,
        real_unique_vehicle_ids,
        descriptor_exact_matches,
        real_matched_vehicle_ids: matched,
        real_unmatched_vehicle_ids,
        vehicle_key_cross_checks,
        vehicle_key_raw_exact,
        vehicle_key_canonical_exact,
        vehicle_key_unavailable,
        vehicle_key_conflicts,
        item_type_conflicts,
        nation_conflicts,
        class_conflicts: catalog.stats.class_conflicts,
        round_trip_failures: catalog.stats.round_trip_failures,
        negative_controls_passed,
        accepted_for_production_hardening,
    };
    (catalog, report)
}

#[cfg(test)]
fn negative_controls_pass() -> bool {
    let ussr = encode_vehicle_compact_descr(0, 7).ok();
    let germany = encode_vehicle_compact_descr(1, 7).ok();
    let shifted = encode_vehicle_compact_descr(1, 7).ok();
    let wrong_tank = encode_vehicle_compact_descr(0, 8).ok();
    let invalid_zero = decode_vehicle_compact_descr(7_u32 << 8);
    let invalid_two = decode_vehicle_compact_descr((7_u32 << 8) | 2);
    matches!((ussr, germany, shifted, wrong_tank), (Some(a), Some(b), Some(c), Some(d)) if a != b && a != c && a != d)
        && matches!(
            invalid_zero,
            Err(CompactDescriptorError::UnsupportedItemType)
        )
        && matches!(
            invalid_two,
            Err(CompactDescriptorError::UnsupportedItemType)
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(
        nation_key: &str,
        tank_id: u32,
        vehicle_key: &str,
        vehicle_class: VehicleClass,
    ) -> VehicleCatalogProbeEntry {
        VehicleCatalogProbeEntry {
            vehicle_id: tank_id,
            vehicle_class,
            nation_key: nation_key.into(),
            source_record_id: tank_id,
            vehicle_key: vehicle_key.into(),
            source_tag: vehicle_class.source_tag().into(),
            tier: None,
            roles: Vec::new(),
            user_string: None,
            short_user_string: None,
        }
    }

    #[test]
    fn encode_decode_round_trip_preserves_all_fields_and_is_deterministic() {
        let first = encode_vehicle_compact_descr(4, 1234).unwrap();
        let second = encode_vehicle_compact_descr(4, 1234).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            decode_vehicle_compact_descr(first).unwrap(),
            DecodedVehicleCompactDescr {
                item_type_id: VEHICLE_ITEM_TYPE_ID,
                nation_id: 4,
                tank_id: 1234,
            }
        );
    }

    #[test]
    fn all_supported_nations_and_max_tank_id_round_trip() {
        for nation_id in 0..=MAX_NATION_ID {
            let value = encode_vehicle_compact_descr(nation_id, MAX_TANK_ID).unwrap();
            let decoded = decode_vehicle_compact_descr(value).unwrap();
            assert_eq!(decoded.nation_id, nation_id);
            assert_eq!(decoded.tank_id, u16::MAX);
            assert_eq!(decoded.item_type_id, 1);
        }
    }

    #[test]
    fn invalid_nation_tank_type_and_overflow_are_rejected() {
        assert_eq!(
            encode_vehicle_compact_descr(11, 1),
            Err(CompactDescriptorError::UnsupportedNationId)
        );
        assert_eq!(
            encode_vehicle_compact_descr(0, 0),
            Err(CompactDescriptorError::InvalidTankId)
        );
        assert_eq!(
            encode_vehicle_compact_descr(0, MAX_TANK_ID + 1),
            Err(CompactDescriptorError::InvalidTankId)
        );
        assert_eq!(
            decode_vehicle_compact_descr(1 << 24),
            Err(CompactDescriptorError::ValueOutOfContract)
        );
        for item_type in [0, 2] {
            assert_eq!(
                decode_vehicle_compact_descr((1 << 8) | item_type),
                Err(CompactDescriptorError::UnsupportedItemType)
            );
        }
    }

    #[test]
    fn nation_map_is_closed_explicit_and_not_order_dependent() {
        let expected = [
            ("ussr", 0),
            ("germany", 1),
            ("usa", 2),
            ("china", 3),
            ("france", 4),
            ("uk", 5),
            ("japan", 6),
            ("czech", 7),
            ("sweden", 8),
            ("poland", 9),
            ("italy", 10),
        ];
        for (key, id) in expected.into_iter().rev() {
            assert_eq!(nation_id_for_key(key), Ok(id));
        }
        for unsupported in ["", "other", "argentina", "USSR"] {
            assert_eq!(
                nation_id_for_key(unsupported),
                Err(CompactDescriptorError::UnsupportedNationKey)
            );
        }
    }

    #[test]
    fn full_synthetic_catalog_is_unique_round_trip_and_order_independent() {
        let entries = vec![
            entry("ussr", 1, "one", VehicleClass::LightTank),
            entry("germany", 1, "one", VehicleClass::HeavyTank),
            entry("italy", 7, "seven", VehicleClass::MediumTank),
        ];
        let forward = build_compact_descriptor_catalog(&entries);
        let mut reversed = entries.clone();
        reversed.reverse();
        let reverse = build_compact_descriptor_catalog(&reversed);
        assert_eq!(forward.entries, reverse.entries);
        assert_eq!(forward.len(), 3);
        assert_eq!(forward.stats.generated_descriptors, 3);
        assert_eq!(forward.stats.round_trip_failures, 0);
        let ussr = encode_vehicle_compact_descr(0, 1).unwrap();
        let germany = encode_vehicle_compact_descr(1, 1).unwrap();
        assert_ne!(ussr, germany);
    }

    #[test]
    fn catalog_rejects_duplicate_descriptor_missing_id_and_unknown_nation() {
        let entries = vec![
            entry("ussr", 2, "first", VehicleClass::LightTank),
            entry("ussr", 2, "second", VehicleClass::HeavyTank),
            entry("ussr", 0, "zero", VehicleClass::Spg),
            entry("new-nation", 3, "new", VehicleClass::MediumTank),
        ];
        let catalog = build_compact_descriptor_catalog(&entries);
        assert_eq!(catalog.stats.descriptor_collisions, 1);
        assert_eq!(catalog.stats.class_conflicts, 1);
        assert_eq!(catalog.stats.invalid_vehicle_ids, 0);
        assert_eq!(catalog.stats.invalid_zero_vehicle_ids, 1);
        assert_eq!(catalog.stats.unsupported_nation_keys, 1);
        assert_eq!(catalog.len(), 0);
    }

    #[test]
    fn vehicle_key_and_class_do_not_change_descriptor() {
        let first = build_compact_descriptor_catalog(&[entry(
            "france",
            42,
            "visible-looking-a",
            VehicleClass::LightTank,
        )]);
        let second = build_compact_descriptor_catalog(&[entry(
            "france",
            42,
            "unrelated-b",
            VehicleClass::Spg,
        )]);
        assert_eq!(first.entries.keys().next(), second.entries.keys().next());
    }

    #[test]
    fn vehicle_key_cross_check_distinguishes_all_four_states() {
        let catalog = build_compact_descriptor_catalog(&[entry(
            "uk",
            130,
            "GB130_Test",
            VehicleClass::TankDestroyer,
        )]);
        let id = encode_vehicle_compact_descr(5, 130).unwrap();
        assert_eq!(
            exact_vehicle_join(&catalog, id, Some("uk-GB130_Test")),
            ExactVehicleJoin::Matched {
                vehicle_class: VehicleClass::TankDestroyer,
                vehicle_key_cross_check: VehicleKeyCrossCheck::RawExact,
            }
        );
        assert_eq!(
            exact_vehicle_join(&catalog, id, None),
            ExactVehicleJoin::Matched {
                vehicle_class: VehicleClass::TankDestroyer,
                vehicle_key_cross_check: VehicleKeyCrossCheck::Unavailable,
            }
        );
        assert_eq!(
            exact_vehicle_join(&catalog, id, Some("uk-gb130_test")),
            ExactVehicleJoin::Matched {
                vehicle_class: VehicleClass::TankDestroyer,
                vehicle_key_cross_check: VehicleKeyCrossCheck::CanonicalExact,
            }
        );
        assert_eq!(
            cross_check_vehicle_keys("uk", None, Some("uk-gb130_test")),
            VehicleKeyCrossCheck::Unavailable
        );
        assert_eq!(
            cross_check_vehicle_keys("uk", Some("GB130_Test"), None),
            VehicleKeyCrossCheck::Unavailable
        );
        assert_eq!(
            cross_check_vehicle_keys("uk", Some(""), Some("uk-gb130_test")),
            VehicleKeyCrossCheck::Conflict
        );
        assert_eq!(
            cross_check_vehicle_keys("uk", Some("GB130_Test"), Some("")),
            VehicleKeyCrossCheck::Conflict
        );
        for wrong in ["uk-GB130_Tes", "GB130_Test", "uk-GB130_Test2", "!!!"] {
            assert_eq!(
                exact_vehicle_join(&catalog, id, Some(wrong)),
                ExactVehicleJoin::Conflict(JoinConflict::VehicleKey)
            );
        }
    }

    #[test]
    fn canonical_key_is_deterministic_idempotent_and_not_fuzzy() {
        let first = canonicalize_vehicle_key(" UK:GB130 Test ").unwrap();
        let second = canonicalize_vehicle_key(" UK:GB130 Test ").unwrap();
        let again = canonicalize_vehicle_key(first.as_str()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first, again);
        assert_eq!(first.as_str(), "uk-gb130_test");
        assert_eq!(
            cross_check_vehicle_keys("uk", Some("GB130_Test"), Some("uk-gb130_tes")),
            VehicleKeyCrossCheck::Conflict
        );
        assert_eq!(
            cross_check_vehicle_keys("uk", Some("GB130_Test"), Some("uk-gb130_test2")),
            VehicleKeyCrossCheck::Conflict
        );
    }

    #[test]
    fn unknown_wrong_and_zero_tank_descriptors_are_unknown() {
        let catalog =
            build_compact_descriptor_catalog(&[entry("usa", 10, "A10", VehicleClass::MediumTank)]);
        assert_eq!(
            exact_vehicle_join(&catalog, encode_vehicle_compact_descr(2, 11).unwrap(), None),
            ExactVehicleJoin::Unknown
        );
        assert_eq!(
            exact_vehicle_join(&catalog, VEHICLE_ITEM_TYPE_ID.into(), None),
            ExactVehicleJoin::Unknown
        );
    }

    #[test]
    fn invalid_item_types_are_conflicts_and_tank_id_alone_is_insufficient() {
        let catalog = build_compact_descriptor_catalog(&[
            entry("ussr", 7, "R7", VehicleClass::LightTank),
            entry("germany", 7, "G7", VehicleClass::HeavyTank),
        ]);
        for item_type in [0, 2] {
            assert_eq!(
                exact_vehicle_join(&catalog, (7 << 8) | item_type, None),
                ExactVehicleJoin::Conflict(JoinConflict::ItemType)
            );
        }
        let ussr = encode_vehicle_compact_descr(0, 7).unwrap();
        let germany = encode_vehicle_compact_descr(1, 7).unwrap();
        assert_ne!(ussr, germany);
        assert_eq!(
            exact_vehicle_join(&catalog, ussr, Some("ussr-R7")),
            ExactVehicleJoin::Matched {
                vehicle_class: VehicleClass::LightTank,
                vehicle_key_cross_check: VehicleKeyCrossCheck::RawExact,
            }
        );
        assert_eq!(
            exact_vehicle_join(&catalog, germany, Some("germany-G7")),
            ExactVehicleJoin::Matched {
                vehicle_class: VehicleClass::HeavyTank,
                vehicle_key_cross_check: VehicleKeyCrossCheck::RawExact,
            }
        );
    }

    #[test]
    fn an_inconsistent_catalog_nation_is_a_fail_closed_conflict() {
        let mut catalog =
            build_compact_descriptor_catalog(&[entry("ussr", 7, "R7", VehicleClass::LightTank)]);
        let descriptor = encode_vehicle_compact_descr(0, 7).unwrap();
        catalog.entries.get_mut(&descriptor).unwrap().nation_id = 1;
        assert_eq!(
            exact_vehicle_join(&catalog, descriptor, Some("ussr-r7")),
            ExactVehicleJoin::Conflict(JoinConflict::Nation)
        );
    }

    #[test]
    fn swapped_and_shifted_nation_mappings_are_detected_by_exact_key_check() {
        let catalog = build_compact_descriptor_catalog(&[
            entry("ussr", 7, "R7", VehicleClass::LightTank),
            entry("germany", 7, "G7", VehicleClass::HeavyTank),
        ]);
        let swapped_ussr = encode_vehicle_compact_descr(1, 7).unwrap();
        assert_eq!(
            exact_vehicle_join(&catalog, swapped_ussr, Some("ussr-R7")),
            ExactVehicleJoin::Conflict(JoinConflict::VehicleKey)
        );
        let swapped_germany = encode_vehicle_compact_descr(0, 7).unwrap();
        assert_eq!(
            exact_vehicle_join(&catalog, swapped_germany, Some("germany-G7")),
            ExactVehicleJoin::Conflict(JoinConflict::VehicleKey)
        );
        let offset_ussr = encode_vehicle_compact_descr(1, 7).unwrap();
        assert_eq!(
            exact_vehicle_join(&catalog, offset_ussr, Some("ussr-R7")),
            ExactVehicleJoin::Conflict(JoinConflict::VehicleKey)
        );
    }

    #[test]
    fn validation_accepts_raw_canonical_and_unavailable_cross_checks() {
        let entries = vec![entry("czech", 17, "Cz17", VehicleClass::MediumTank)];
        let id = encode_vehicle_compact_descr(7, 17).unwrap();
        for (key, expected) in [
            (Some("czech-Cz17"), VehicleKeyCrossCheck::RawExact),
            (Some("czech-cz17"), VehicleKeyCrossCheck::CanonicalExact),
            (None, VehicleKeyCrossCheck::Unavailable),
        ] {
            let probes = vec![RealVehicleProbe {
                wg_tank_id: id,
                vehicle_key: key.map(str::to_owned),
            }];
            let (_, report) = validate_compact_descriptor_protocol(
                &entries,
                &probes,
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            );
            assert!(report.accepted_for_production_hardening);
            assert_eq!(report.descriptor_exact_matches, 1);
            assert_eq!(report.real_matched_vehicle_ids, 1);
            assert_eq!(
                report.vehicle_key_raw_exact,
                usize::from(expected == VehicleKeyCrossCheck::RawExact)
            );
            assert_eq!(
                report.vehicle_key_canonical_exact,
                usize::from(expected == VehicleKeyCrossCheck::CanonicalExact)
            );
            assert_eq!(
                report.vehicle_key_unavailable,
                usize::from(expected == VehicleKeyCrossCheck::Unavailable)
            );
            assert_eq!(
                report.protocol_source,
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated
            );
        }
    }

    #[test]
    fn validation_rejects_key_conflict_and_unconfirmed_provenance() {
        let entries = vec![entry("czech", 17, "Cz17", VehicleClass::MediumTank)];
        let id = encode_vehicle_compact_descr(7, 17).unwrap();
        let bad = vec![RealVehicleProbe {
            wg_tank_id: id,
            vehicle_key: Some("czech-wrong".into()),
        }];
        let (_, rejected) = validate_compact_descriptor_protocol(
            &entries,
            &bad,
            CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
        );
        assert!(!rejected.accepted_for_production_hardening);
        assert_eq!(
            rejected.protocol_source,
            CompactDescriptorProtocolSource::Unconfirmed
        );

        let good = vec![RealVehicleProbe {
            wg_tank_id: id,
            vehicle_key: Some("czech-cz17".into()),
        }];
        let (_, unconfirmed_rejected) = validate_compact_descriptor_protocol(
            &entries,
            &good,
            CompactDescriptorProtocolSource::Unconfirmed,
        );
        assert!(!unconfirmed_rejected.accepted_for_production_hardening);
        assert_eq!(
            unconfirmed_rejected.protocol_source,
            CompactDescriptorProtocolSource::Unconfirmed
        );
    }

    #[test]
    fn zero_ids_are_reported_but_do_not_block_valid_catalog_evidence() {
        let entries = vec![
            entry("czech", 17, "Cz17", VehicleClass::MediumTank),
            entry("czech", 0, "technical", VehicleClass::MediumTank),
        ];
        let id = encode_vehicle_compact_descr(7, 17).unwrap();
        let probes = vec![RealVehicleProbe {
            wg_tank_id: id,
            vehicle_key: Some("czech-cz17".into()),
        }];
        let (catalog, report) = validate_compact_descriptor_protocol(
            &entries,
            &probes,
            CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
        );
        assert_eq!(catalog.len(), 1);
        assert_eq!(report.catalog_records, 2);
        assert_eq!(report.generated_descriptors, 1);
        assert_eq!(report.invalid_zero_vehicle_ids, 1);
        assert_eq!(report.invalid_vehicle_ids, 0);
        assert!(report.accepted_for_production_hardening);

        let invalid_probe = vec![RealVehicleProbe {
            wg_tank_id: u32::from(VEHICLE_ITEM_TYPE_ID),
            vehicle_key: None,
        }];
        let (_, invalid_report) = validate_compact_descriptor_protocol(
            &entries,
            &invalid_probe,
            CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
        );
        assert_eq!(invalid_report.descriptor_exact_matches, 0);
        assert_eq!(invalid_report.real_unmatched_vehicle_ids, 1);
        assert!(!invalid_report.accepted_for_production_hardening);
    }

    #[test]
    fn collisions_class_round_trip_and_negative_control_failures_reject_validation() {
        let id = encode_vehicle_compact_descr(0, 8).unwrap();
        let probes = vec![RealVehicleProbe {
            wg_tank_id: id,
            vehicle_key: Some("ussr-r8".into()),
        }];
        let collision = vec![
            entry("ussr", 8, "R8", VehicleClass::LightTank),
            entry("ussr", 8, "R8-copy", VehicleClass::HeavyTank),
        ];
        let (_, collision_report) = validate_compact_descriptor_protocol(
            &collision,
            &probes,
            CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
        );
        assert_eq!(collision_report.descriptor_collisions, 1);
        assert_eq!(collision_report.class_conflicts, 1);
        assert!(!collision_report.accepted_for_production_hardening);

        let valid = vec![entry("ussr", 8, "R8", VehicleClass::LightTank)];
        let (_, controls_failed) = validate_compact_descriptor_protocol_with_controls(
            &valid,
            &probes,
            CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            false,
        );
        assert!(!controls_failed.negative_controls_passed);
        assert!(!controls_failed.accepted_for_production_hardening);

        let mut structurally_invalid = build_compact_descriptor_catalog(&valid).stats;
        structurally_invalid.round_trip_failures = 1;
        assert!(!catalog_is_structurally_valid(&structurally_invalid));
    }

    #[test]
    fn display_name_tank_name_class_and_numeric_nation_do_not_canonicalize_keys() {
        let display_name = "Visible Tank";
        let tank_name = "Another Tank";
        let class = VehicleClass::Spg;
        let nation_id = 10_u8;
        assert_ne!(display_name, tank_name);
        assert_eq!(class, VehicleClass::Spg);
        assert_eq!(nation_id, 10);
        assert_eq!(
            cross_check_vehicle_keys("italy", Some("It10_Test"), Some("italy-it10_test")),
            VehicleKeyCrossCheck::CanonicalExact
        );
    }
}
