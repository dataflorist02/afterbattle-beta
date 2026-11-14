use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    bigworld_packed::{
        PackedDocument, PackedError, PackedSection, PackedValue, parse_packed_document,
    },
    models::VehicleClass,
};

pub(crate) const NATION_KEYS: [&str; 11] = [
    "china", "czech", "france", "germany", "italy", "japan", "poland", "sweden", "uk", "usa",
    "ussr",
];
const MAX_PACKAGE_ENTRY_SIZE: u64 = 32 * 1024 * 1024;
const VEHICLE_LIST_PREFIX: &str = "scripts/item_defs/vehicles/";
const VEHICLE_LIST_SUFFIX: &str = "/list.xml";
/// Highest vehicle tier in the client (tier XI exists since client 2.x).
pub(crate) const MAX_VEHICLE_TIER: u8 = 11;
/// Upper bound on nation folders considered in one package (B5: new nations are reported, not
/// fatal, but the count stays bounded).
const MAX_NATION_FOLDERS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VehicleCatalogProbeEntry {
    /// Raw, nation-local `id` from the catalog. This is not a typeCompDescr.
    pub vehicle_id: u32,
    pub vehicle_class: VehicleClass,
    pub nation_key: String,
    pub source_record_id: u32,
    pub vehicle_key: String,
    pub source_tag: String,
    /// `level` of the record (1..=11); `None` when missing or out of range.
    pub tier: Option<u8>,
    /// Sorted, unique `role_*` tags of the record.
    pub roles: Vec<String>,
    /// Translation key of the full name, e.g. `#nation_vehicles:Tag` (E5-ui §2). Kept only in RAM.
    pub user_string: Option<String>,
    /// Translation key of the short name, when the record has one.
    pub short_user_string: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CatalogExtraction {
    pub total_records: usize,
    pub non_section_records: usize,
    pub missing_id: usize,
    /// Records whose `id` is a decimal string (accepted).
    pub string_ids: usize,
    pub missing_tier: usize,
    pub invalid_tier: usize,
    pub missing_tags: usize,
    pub unknown_class: usize,
    pub class_conflicts: usize,
    pub duplicate_id_groups: usize,
    pub duplicate_id_class_conflicts: usize,
    pub user_string_records: usize,
    pub short_user_string_records: usize,
    pub non_section_field_names: BTreeSet<String>,
    pub direct_record_field_names: BTreeSet<String>,
    pub entries: Vec<VehicleCatalogProbeEntry>,
}

impl CatalogExtraction {
    #[cfg(test)]
    pub(crate) fn class_counts(&self) -> BTreeMap<VehicleClass, usize> {
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            *counts.entry(entry.vehicle_class).or_default() += 1;
        }
        counts
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CatalogProbeReport {
    pub source_fingerprint: String,
    pub files_read: usize,
    pub format_versions: BTreeSet<u8>,
    /// Known nations whose `list.xml` is absent from the package (B5: not fatal).
    pub missing_nations: Vec<String>,
    /// Nation folders the app does not know yet (B5: reported, skipped).
    pub unknown_nations: Vec<String>,
    pub total_records: usize,
    pub non_section_records: usize,
    pub missing_id: usize,
    pub string_ids: usize,
    pub missing_tier: usize,
    pub invalid_tier: usize,
    pub missing_tags: usize,
    pub unknown_class: usize,
    pub class_conflicts: usize,
    pub duplicate_id_groups: usize,
    pub duplicate_id_class_conflicts: usize,
    pub user_string_records: usize,
    pub short_user_string_records: usize,
    pub non_section_field_names: BTreeSet<String>,
    pub direct_record_field_names: BTreeSet<String>,
    pub entries: Vec<VehicleCatalogProbeEntry>,
}

impl CatalogProbeReport {
    #[cfg(test)]
    pub(crate) fn class_counts(&self) -> BTreeMap<VehicleClass, usize> {
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            *counts.entry(entry.vehicle_class).or_default() += 1;
        }
        counts
    }
}

#[derive(Debug, Error)]
pub(crate) enum CatalogProbeError {
    #[error("catalog package could not be opened")]
    PackageOpen,
    #[error("catalog package is not a supported ZIP archive")]
    InvalidPackage,
    #[error("required catalog entry is missing")]
    MissingCatalogEntry,
    #[error("catalog entry compression method is unsupported by this spike")]
    UnsupportedCompression,
    #[error("catalog entry exceeds the configured size limit")]
    CatalogEntryTooLarge,
    #[error("catalog entry could not be read completely")]
    CatalogEntryRead,
    #[error("catalog PackedSection is invalid: {0}")]
    Packed(#[from] PackedError),
}

fn fingerprint_start(metadata: &std::fs::Metadata) -> Sha256 {
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"afterbattle-vehicle-catalog-source-v2\0");
    fingerprint.update(metadata.len().to_le_bytes());
    if let Ok(modified) = metadata.modified()
        && let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH)
    {
        fingerprint.update(duration.as_nanos().to_le_bytes());
    }
    fingerprint
}

fn update_fingerprint(fingerprint: &mut Sha256, entry_name: &str, bytes: &[u8]) {
    fingerprint.update((entry_name.len() as u64).to_le_bytes());
    fingerprint.update(entry_name.as_bytes());
    fingerprint.update((bytes.len() as u64).to_le_bytes());
    fingerprint.update(bytes);
}

/// Nation folders of the package (B5): the known nations present, the known nations missing,
/// and folders the app does not know. Names come only from the fixed path pattern.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct NationLayout {
    pub present: Vec<&'static str>,
    pub missing: Vec<String>,
    pub unknown: Vec<String>,
}

pub(crate) fn nation_layout<R: std::io::Read + std::io::Seek>(
    archive: &zip::ZipArchive<R>,
) -> Result<NationLayout, CatalogProbeError> {
    let mut folders = BTreeSet::new();
    for name in archive.file_names() {
        let Ok(name) = name else {
            continue;
        };
        let Some(rest) = name.strip_prefix(VEHICLE_LIST_PREFIX) else {
            continue;
        };
        let Some(nation) = rest.strip_suffix(VEHICLE_LIST_SUFFIX) else {
            continue;
        };
        if nation.is_empty()
            || nation.len() > 32
            || !nation
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            continue;
        }
        folders.insert(nation.to_owned());
        if folders.len() > MAX_NATION_FOLDERS {
            return Err(CatalogProbeError::InvalidPackage);
        }
    }
    let mut layout = NationLayout::default();
    for nation in NATION_KEYS {
        if folders.remove(nation) {
            layout.present.push(nation);
        } else {
            layout.missing.push(nation.to_owned());
        }
    }
    layout.unknown = folders.into_iter().collect();
    if layout.present.is_empty() {
        return Err(CatalogProbeError::MissingCatalogEntry);
    }
    Ok(layout)
}

fn list_entry_name(nation_key: &str) -> String {
    format!("{VEHICLE_LIST_PREFIX}{nation_key}{VEHICLE_LIST_SUFFIX}")
}

pub(crate) fn fingerprint_package(package_path: &Path) -> Result<String, CatalogProbeError> {
    let file = File::open(package_path).map_err(|_| CatalogProbeError::PackageOpen)?;
    let metadata = file
        .metadata()
        .map_err(|_| CatalogProbeError::PackageOpen)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| CatalogProbeError::InvalidPackage)?;
    let layout = nation_layout(&archive)?;
    let mut fingerprint = fingerprint_start(&metadata);
    for nation_key in &layout.present {
        let entry_name = list_entry_name(nation_key);
        let bytes = read_catalog_entry(&mut archive, &entry_name)?;
        update_fingerprint(&mut fingerprint, &entry_name, &bytes);
    }
    for nation_key in layout.unknown.iter().chain(&layout.missing) {
        fingerprint.update(b"nation-folder\0");
        fingerprint.update(nation_key.as_bytes());
    }
    Ok(crate::hashing::hex_lower(fingerprint.finalize()))
}

pub(crate) fn probe_package(package_path: &Path) -> Result<CatalogProbeReport, CatalogProbeError> {
    let file = File::open(package_path).map_err(|_| CatalogProbeError::PackageOpen)?;
    let metadata = file
        .metadata()
        .map_err(|_| CatalogProbeError::PackageOpen)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| CatalogProbeError::InvalidPackage)?;
    let layout = nation_layout(&archive)?;
    let mut report = CatalogProbeReport {
        missing_nations: layout.missing.clone(),
        unknown_nations: layout.unknown.clone(),
        ..CatalogProbeReport::default()
    };
    let mut fingerprint = fingerprint_start(&metadata);

    for nation_key in layout.present {
        let entry_name = list_entry_name(nation_key);
        let bytes = read_catalog_entry(&mut archive, &entry_name)?;
        update_fingerprint(&mut fingerprint, &entry_name, &bytes);
        let document = parse_packed_document(&bytes)?;
        let extraction = extract_catalog(nation_key, &document);
        report.files_read += 1;
        report.format_versions.insert(document.version);
        report.total_records += extraction.total_records;
        report.non_section_records += extraction.non_section_records;
        report.missing_id += extraction.missing_id;
        report.string_ids += extraction.string_ids;
        report.missing_tier += extraction.missing_tier;
        report.invalid_tier += extraction.invalid_tier;
        report.missing_tags += extraction.missing_tags;
        report.unknown_class += extraction.unknown_class;
        report.class_conflicts += extraction.class_conflicts;
        report.duplicate_id_groups += extraction.duplicate_id_groups;
        report.duplicate_id_class_conflicts += extraction.duplicate_id_class_conflicts;
        report.user_string_records += extraction.user_string_records;
        report.short_user_string_records += extraction.short_user_string_records;
        report
            .non_section_field_names
            .extend(extraction.non_section_field_names);
        report
            .direct_record_field_names
            .extend(extraction.direct_record_field_names);
        report.entries.extend(extraction.entries);
    }
    report.entries.sort_by(|left, right| {
        (&left.nation_key, left.source_record_id, left.vehicle_class).cmp(&(
            &right.nation_key,
            right.source_record_id,
            right.vehicle_class,
        ))
    });
    for nation_key in layout.unknown.iter().chain(&layout.missing) {
        fingerprint.update(b"nation-folder\0");
        fingerprint.update(nation_key.as_bytes());
    }
    report.source_fingerprint = crate::hashing::hex_lower(fingerprint.finalize());
    Ok(report)
}

fn read_catalog_entry<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    entry_name: &str,
) -> Result<Vec<u8>, CatalogProbeError> {
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|_| CatalogProbeError::MissingCatalogEntry)?;
    if entry.compression() != zip::CompressionMethod::Stored {
        return Err(CatalogProbeError::UnsupportedCompression);
    }
    let declared_size = entry.size();
    if declared_size > MAX_PACKAGE_ENTRY_SIZE {
        return Err(CatalogProbeError::CatalogEntryTooLarge);
    }
    let capacity =
        usize::try_from(declared_size).map_err(|_| CatalogProbeError::CatalogEntryTooLarge)?;
    let mut bytes = Vec::with_capacity(capacity);
    entry
        .by_ref()
        .take(MAX_PACKAGE_ENTRY_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CatalogProbeError::CatalogEntryRead)?;
    let actual_size =
        u64::try_from(bytes.len()).map_err(|_| CatalogProbeError::CatalogEntryTooLarge)?;
    if actual_size > MAX_PACKAGE_ENTRY_SIZE {
        return Err(CatalogProbeError::CatalogEntryTooLarge);
    }
    if actual_size != declared_size {
        return Err(CatalogProbeError::CatalogEntryRead);
    }
    Ok(bytes)
}

pub(crate) fn extract_catalog(nation_key: &str, document: &PackedDocument) -> CatalogExtraction {
    #[derive(Debug)]
    struct Candidate {
        id: u32,
        class: VehicleClass,
        vehicle_key: String,
        tier: Option<u8>,
        roles: Vec<String>,
        user_string: Option<String>,
        short_user_string: Option<String>,
    }

    let mut result = CatalogExtraction::default();
    let mut candidates: BTreeMap<u32, Vec<Candidate>> = BTreeMap::new();
    result.total_records = document.root.children.len();

    for record in &document.root.children {
        let Some(section) = record.value.as_section() else {
            result.non_section_records += 1;
            result.non_section_field_names.insert(record.name.clone());
            continue;
        };
        result
            .direct_record_field_names
            .extend(section.children.iter().map(|child| child.name.clone()));
        if unique_direct_value(section, "userString").is_some() {
            result.user_string_records += 1;
        }
        if unique_direct_value(section, "shortUserString").is_some() {
            result.short_user_string_records += 1;
        }

        let id = match unique_direct_value(section, "id") {
            Some(value) => match record_id(value) {
                Some((id, from_string)) => {
                    if from_string {
                        result.string_ids += 1;
                    }
                    Some(id)
                }
                None => None,
            },
            None => None,
        };
        let tags = unique_direct_value(section, "tags").and_then(PackedValue::as_string);
        if id.is_none() {
            result.missing_id += 1;
        }
        if tags.is_none() {
            result.missing_tags += 1;
        }
        let (Some(id), Some(tags)) = (id, tags) else {
            continue;
        };
        let class = match class_from_tags(tags) {
            Ok(class) => class,
            Err(ClassTagError::Missing) => {
                result.unknown_class += 1;
                continue;
            }
            Err(ClassTagError::Conflict) => {
                result.class_conflicts += 1;
                continue;
            }
        };
        let tier = match unique_direct_value(section, "level") {
            None => {
                result.missing_tier += 1;
                None
            }
            Some(value) => {
                let tier = value
                    .as_integer()
                    .and_then(|level| u8::try_from(level).ok())
                    .filter(|level| (1..=MAX_VEHICLE_TIER).contains(level));
                if tier.is_none() {
                    result.invalid_tier += 1;
                }
                tier
            }
        };
        candidates.entry(id).or_default().push(Candidate {
            id,
            class,
            vehicle_key: record.name.clone(),
            tier,
            roles: roles_from_tags(tags),
            user_string: unique_direct_value(section, "userString")
                .and_then(PackedValue::as_string)
                .map(str::to_owned),
            short_user_string: unique_direct_value(section, "shortUserString")
                .and_then(PackedValue::as_string)
                .map(str::to_owned),
        });
    }

    for group in candidates.into_values() {
        if group.len() > 1 {
            result.duplicate_id_groups += 1;
            let classes: BTreeSet<_> = group.iter().map(|candidate| candidate.class).collect();
            if classes.len() > 1 {
                result.duplicate_id_class_conflicts += 1;
            }
            continue;
        }
        let candidate = &group[0];
        result.entries.push(VehicleCatalogProbeEntry {
            vehicle_id: candidate.id,
            vehicle_class: candidate.class,
            nation_key: nation_key.to_owned(),
            source_record_id: candidate.id,
            vehicle_key: candidate.vehicle_key.clone(),
            source_tag: candidate.class.source_tag().to_owned(),
            tier: candidate.tier,
            roles: candidate.roles.clone(),
            user_string: candidate.user_string.clone(),
            short_user_string: candidate.short_user_string.clone(),
        });
    }
    result
        .entries
        .sort_by_key(|entry| (entry.source_record_id, entry.vehicle_class));
    result
}

fn unique_direct_value<'a>(section: &'a PackedSection, name: &str) -> Option<&'a PackedValue> {
    let mut matches = section
        .children
        .iter()
        .filter(|child| child.name == name)
        .map(|child| &child.value);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

/// The record `id`: an integer, or (9 records in client 2.4.1.1) a string holding only decimal
/// digits. Anything else is missing. Returns the id and whether it came from a string.
fn record_id(value: &PackedValue) -> Option<(u32, bool)> {
    if let Some(integer) = value.as_integer() {
        return u32::try_from(integer).ok().map(|id| (id, false));
    }
    let text = value.as_string()?.trim();
    // Only decimal digits: `u32::from_str` alone would also accept a leading `+`. Empty strings
    // and values above `u32::MAX` fail the parse.
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>().ok().map(|id| (id, true))
}

/// Maximum number of role tags kept per vehicle.
const MAX_ROLE_TAGS: usize = 8;

fn roles_from_tags(tags: &str) -> Vec<String> {
    let roles = tags
        .split_whitespace()
        .filter(|tag| {
            tag.len() <= 32
                && tag.starts_with("role_")
                && tag.len() > "role_".len()
                && tag
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    roles.into_iter().take(MAX_ROLE_TAGS).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassTagError {
    Missing,
    Conflict,
}

fn class_from_tags(tags: &str) -> Result<VehicleClass, ClassTagError> {
    let mut found = None;
    for tag in tags.split_whitespace() {
        let class = match tag {
            "lightTank" => Some(VehicleClass::LightTank),
            "mediumTank" => Some(VehicleClass::MediumTank),
            "heavyTank" => Some(VehicleClass::HeavyTank),
            "AT-SPG" => Some(VehicleClass::TankDestroyer),
            "SPG" => Some(VehicleClass::Spg),
            _ => None,
        };
        if let Some(class) = class
            && found.replace(class).is_some()
        {
            return Err(ClassTagError::Conflict);
        }
    }
    found.ok_or(ClassTagError::Missing)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::bigworld_packed::{PackedNode, PackedSection, PackedValue};
    use std::io::{Cursor, Write};

    fn record(key: &str, id: Option<i64>, tags: Option<&str>) -> PackedNode {
        let mut children = Vec::new();
        if let Some(id) = id {
            children.push(PackedNode {
                name: "id".into(),
                value: PackedValue::Integer(id),
            });
        }
        if let Some(tags) = tags {
            children.push(PackedNode {
                name: "tags".into(),
                value: PackedValue::String(tags.into()),
            });
        }
        children.push(PackedNode {
            name: "userString".into(),
            value: PackedValue::String("display-only".into()),
        });
        children.push(PackedNode {
            name: "shortUserString".into(),
            value: PackedValue::String("short-display-only".into()),
        });
        PackedNode {
            name: key.into(),
            value: PackedValue::Section(Box::new(PackedSection {
                value: PackedValue::String(String::new()),
                children,
            })),
        }
    }

    fn document(records: Vec<PackedNode>) -> PackedDocument {
        PackedDocument {
            version: 0,
            string_count: 4,
            root: PackedSection {
                value: PackedValue::String(String::new()),
                children: records,
            },
        }
    }

    pub(crate) fn minimal_packed_catalog() -> Vec<u8> {
        fn descriptor(kind: u8, offset: u32) -> [u8; 4] {
            ((u32::from(kind) << 28) | offset).to_le_bytes()
        }

        let mut nested = Vec::new();
        nested.extend_from_slice(&2_u16.to_le_bytes());
        nested.extend_from_slice(&descriptor(1, 0));
        nested.extend_from_slice(&1_u16.to_le_bytes());
        nested.extend_from_slice(&descriptor(2, 1));
        nested.extend_from_slice(&2_u16.to_le_bytes());
        nested.extend_from_slice(&descriptor(1, 10));
        nested.push(1);
        nested.extend_from_slice(b"heavyTank");

        let mut bytes = crate::bigworld_packed::PACKED_SECTION_MAGIC
            .to_le_bytes()
            .to_vec();
        bytes.push(0);
        for name in [b"vehicle".as_slice(), b"id", b"tags"] {
            bytes.extend_from_slice(name);
            bytes.push(0);
        }
        bytes.push(0);
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&descriptor(1, 0));
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&descriptor(0, nested.len() as u32));
        bytes.extend_from_slice(&nested);
        bytes
    }

    #[test]
    fn maps_all_five_exact_class_tags() {
        let cases = [
            ("lightTank", VehicleClass::LightTank),
            ("mediumTank", VehicleClass::MediumTank),
            ("heavyTank", VehicleClass::HeavyTank),
            ("AT-SPG", VehicleClass::TankDestroyer),
            ("SPG", VehicleClass::Spg),
        ];
        for (tag, expected) in cases {
            assert_eq!(class_from_tags(tag), Ok(expected));
        }
    }

    #[test]
    fn class_tag_need_not_be_first_and_unknown_tags_are_ignored() {
        assert_eq!(
            class_from_tags("premium event mediumTank secret"),
            Ok(VehicleClass::MediumTank)
        );
    }

    #[test]
    fn missing_or_multiple_class_tags_are_rejected() {
        assert_eq!(
            class_from_tags("premium event"),
            Err(ClassTagError::Missing)
        );
        assert_eq!(
            class_from_tags("lightTank premium heavyTank"),
            Err(ClassTagError::Conflict)
        );
    }

    #[test]
    fn missing_id_and_tags_are_reported_without_guessing() {
        let extracted = extract_catalog(
            "test",
            &document(vec![
                record("name-suggests-heavyTank", None, Some("heavyTank")),
                record("medium-looking-key", Some(2), None),
                record("missing-both", None, None),
            ]),
        );
        assert_eq!(extracted.missing_id, 2);
        assert_eq!(extracted.missing_tags, 2);
        assert!(extracted.entries.is_empty());
    }

    #[test]
    fn duplicate_id_is_rejected_even_when_class_matches() {
        let extracted = extract_catalog(
            "test",
            &document(vec![
                record("first", Some(7), Some("lightTank")),
                record("second", Some(7), Some("lightTank")),
            ]),
        );
        assert_eq!(extracted.duplicate_id_groups, 1);
        assert_eq!(extracted.duplicate_id_class_conflicts, 0);
        assert!(extracted.entries.is_empty());
    }

    #[test]
    fn duplicate_id_with_two_classes_is_an_explicit_conflict() {
        let extracted = extract_catalog(
            "test",
            &document(vec![
                record("first", Some(7), Some("lightTank")),
                record("second", Some(7), Some("SPG")),
            ]),
        );
        assert_eq!(extracted.duplicate_id_groups, 1);
        assert_eq!(extracted.duplicate_id_class_conflicts, 1);
        assert!(extracted.entries.is_empty());
    }

    #[test]
    fn name_and_vehicle_key_do_not_influence_class() {
        let extracted = extract_catalog(
            "test",
            &document(vec![record(
                "SPG_heavyTank_misleading",
                Some(42),
                Some("premium lightTank"),
            )]),
        );
        assert_eq!(extracted.entries[0].vehicle_class, VehicleClass::LightTank);
        assert_eq!(extracted.entries[0].vehicle_id, 42);
    }

    #[test]
    fn extraction_is_independent_of_record_order() {
        let first = record("one", Some(1), Some("lightTank"));
        let second = record("two", Some(2), Some("AT-SPG"));
        let forward = extract_catalog("test", &document(vec![first.clone(), second.clone()]));
        let reverse = extract_catalog("test", &document(vec![second, first]));
        assert_eq!(forward, reverse);
        assert_eq!(
            forward.class_counts(),
            BTreeMap::from([
                (VehicleClass::LightTank, 1),
                (VehicleClass::TankDestroyer, 1),
            ])
        );
    }

    #[test]
    fn unknown_and_conflicting_class_records_are_counted() {
        let extracted = extract_catalog(
            "test",
            &document(vec![
                record("unknown", Some(1), Some("premium")),
                record("conflict", Some(2), Some("SPG lightTank")),
            ]),
        );
        assert_eq!(extracted.unknown_class, 1);
        assert_eq!(extracted.class_conflicts, 1);
        assert!(extracted.entries.is_empty());
    }

    #[test]
    fn display_fields_are_counted_and_kept_as_translation_keys() {
        let extracted = extract_catalog(
            "test",
            &document(vec![record("technical-key", Some(1), Some("heavyTank"))]),
        );
        assert_eq!(extracted.user_string_records, 1);
        assert_eq!(extracted.short_user_string_records, 1);
        let entry = &extracted.entries[0];
        assert_eq!(entry.source_record_id, 1);
        assert_eq!(entry.vehicle_key, "technical-key");
        assert_eq!(entry.source_tag, "heavyTank");
        assert_eq!(entry.nation_key, "test");
        assert_eq!(entry.user_string.as_deref(), Some("display-only"));
        assert_eq!(
            entry.short_user_string.as_deref(),
            Some("short-display-only")
        );
    }

    #[test]
    fn package_probe_reads_only_the_eleven_named_catalogs() {
        let fixture = minimal_packed_catalog();
        let output = Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(output);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for nation in NATION_KEYS {
            writer
                .start_file(
                    format!("scripts/item_defs/vehicles/{nation}/list.xml"),
                    options,
                )
                .unwrap();
            writer.write_all(&fixture).unwrap();
        }
        writer.start_file("unrelated/private.bin", options).unwrap();
        writer.write_all(b"must not be parsed").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("synthetic.pkg");
        std::fs::write(&package, bytes).unwrap();

        let report = probe_package(&package).unwrap();
        assert_eq!(report.files_read, 11);
        assert_eq!(report.format_versions, BTreeSet::from([0]));
        assert_eq!(report.total_records, 11);
        assert_eq!(report.entries.len(), 11);
        assert_eq!(
            report.class_counts(),
            BTreeMap::from([(VehicleClass::HeavyTank, 11)])
        );
    }

    fn with_child(mut node: PackedNode, name: &str, value: PackedValue) -> PackedNode {
        if let PackedValue::Section(section) = &mut node.value {
            section.children.push(PackedNode {
                name: name.into(),
                value,
            });
        }
        node
    }

    fn tiered(key: &str, id: i64, tags: &str, level: Option<PackedValue>) -> PackedNode {
        let node = record(key, Some(id), Some(tags));
        match level {
            Some(level) => with_child(node, "level", level),
            None => node,
        }
    }

    // E1b: the `level` field becomes the tier (1..=11), out-of-range and missing values stay
    // missing and are counted.
    #[test]
    fn tier_is_read_from_level_and_invalid_values_stay_missing() {
        let extraction = extract_catalog(
            "test",
            &document(vec![
                tiered("t1", 1, "lightTank", Some(PackedValue::Integer(1))),
                tiered("t10", 2, "mediumTank", Some(PackedValue::Integer(10))),
                tiered("t11", 3, "heavyTank", Some(PackedValue::Integer(11))),
                tiered("t0", 4, "AT-SPG", Some(PackedValue::Integer(0))),
                tiered("t12", 5, "SPG", Some(PackedValue::Integer(12))),
                tiered("neg", 6, "SPG", Some(PackedValue::Integer(-3))),
                tiered("text", 7, "SPG", Some(PackedValue::String("10".into()))),
                tiered("none", 8, "SPG", None),
            ]),
        );
        let tiers = extraction
            .entries
            .iter()
            .map(|entry| (entry.vehicle_key.as_str(), entry.tier))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(tiers["t1"], Some(1));
        assert_eq!(tiers["t10"], Some(10));
        assert_eq!(tiers["t11"], Some(11));
        for key in ["t0", "t12", "neg", "text", "none"] {
            assert_eq!(tiers[key], None, "{key}");
        }
        assert_eq!(extraction.invalid_tier, 4);
        assert_eq!(extraction.missing_tier, 1);
        assert_eq!(
            extraction.entries.len(),
            8,
            "a bad tier never drops the class"
        );
    }

    #[test]
    fn role_tags_are_collected_sorted_unique_and_bounded() {
        let extraction = extract_catalog(
            "test",
            &document(vec![
                record(
                    "roles",
                    Some(1),
                    Some(
                        "role_MT_sniper mediumTank role_MT_universal role_MT_sniper role_ role_x-y",
                    ),
                ),
                record("none", Some(2), Some("heavyTank premiumIGR")),
            ]),
        );
        let by_key = extraction
            .entries
            .iter()
            .map(|entry| (entry.vehicle_key.as_str(), entry.roles.clone()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            by_key["roles"],
            vec!["role_MT_sniper".to_owned(), "role_MT_universal".to_owned()]
        );
        assert!(by_key["none"].is_empty());
        let many = (0..20)
            .map(|index| format!("role_r{index:02}"))
            .collect::<Vec<_>>();
        assert_eq!(roles_from_tags(&many.join(" ")).len(), 8);
    }

    // E1b: 9 records of client 2.4.1.1 store `id` as a string; a decimal string is accepted.
    #[test]
    fn string_ids_with_only_digits_are_accepted_and_others_are_missing() {
        let string_id = |key: &str, id: &str| {
            with_child(
                record(key, None, Some("lightTank")),
                "id",
                PackedValue::String(id.into()),
            )
        };
        let extraction = extract_catalog(
            "test",
            &document(vec![
                string_id("digits", "42"),
                string_id("padded", " 43 "),
                string_id("letters", "4a"),
                string_id("negative", "-5"),
                string_id("empty", ""),
                string_id("huge", "99999999999"),
                string_id("plus", "+44"),
                string_id("ten-digits", "0000000045"),
                string_id("u32-max", "4294967295"),
                string_id("u32-max-plus-one", "4294967296"),
            ]),
        );
        let ids = extraction
            .entries
            .iter()
            .map(|entry| (entry.vehicle_key.as_str(), entry.source_record_id))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            ids,
            BTreeMap::from([
                ("digits", 42),
                ("padded", 43),
                ("ten-digits", 45),
                ("u32-max", u32::MAX),
            ])
        );
        assert_eq!(extraction.string_ids, 4);
        assert_eq!(extraction.missing_id, 6);
    }

    fn package_with(nations: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
        let fixture = minimal_packed_catalog();
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for nation in nations {
            writer
                .start_file(
                    format!("scripts/item_defs/vehicles/{nation}/list.xml"),
                    options,
                )
                .unwrap();
            writer.write_all(&fixture).unwrap();
        }
        writer
            .start_file("scripts/item_defs/vehicles/common/shared.xml", options)
            .unwrap();
        writer.write_all(b"not a vehicle list").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("synthetic.pkg");
        std::fs::write(&package, bytes).unwrap();
        (temp, package)
    }

    // B5: a missing known nation or a new nation does not reject the whole catalog.
    #[test]
    fn missing_and_new_nations_are_reported_not_fatal() {
        let mut nations = NATION_KEYS
            .iter()
            .copied()
            .filter(|nation| *nation != "italy")
            .collect::<Vec<_>>();
        nations.push("atlantis");
        let (_temp, package) = package_with(&nations);
        let report = probe_package(&package).unwrap();
        assert_eq!(report.files_read, 10);
        assert_eq!(report.missing_nations, vec!["italy".to_owned()]);
        assert_eq!(report.unknown_nations, vec!["atlantis".to_owned()]);
        assert_eq!(report.entries.len(), 10);
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.nation_key != "atlantis")
        );
        assert_eq!(
            fingerprint_package(&package).unwrap(),
            report.source_fingerprint
        );

        let (_temp_all, all) = package_with(&NATION_KEYS);
        assert_ne!(
            fingerprint_package(&all).unwrap(),
            report.source_fingerprint,
            "nation layout is part of the fingerprint"
        );
    }

    #[test]
    fn package_without_any_known_nation_is_rejected() {
        let (_temp, package) = package_with(&["atlantis"]);
        assert!(matches!(
            probe_package(&package),
            Err(CatalogProbeError::MissingCatalogEntry)
        ));
        assert!(fingerprint_package(&package).is_err());
    }

    #[test]
    fn nation_folder_names_accept_lowercase_digits_and_underscore_up_to_32_bytes() {
        let longest = "a".repeat(32);
        let too_long = "b".repeat(33);
        let (_temp, package) = package_with(&["usa", "new_land2", &longest, &too_long]);
        let report = probe_package(&package).unwrap();
        assert_eq!(
            report.unknown_nations,
            vec![longest, "new_land2".to_owned()]
        );
    }

    #[test]
    fn nation_folder_count_is_bounded() {
        let names = (0..64)
            .map(|index| format!("n{index:02}"))
            .collect::<Vec<_>>();
        let mut at_limit = names.iter().map(String::as_str).collect::<Vec<_>>();
        at_limit[0] = "usa";
        let (_temp, package) = package_with(&at_limit);
        assert!(probe_package(&package).is_ok());
        let mut over = names.iter().map(String::as_str).collect::<Vec<_>>();
        over[0] = "usa";
        over.push("n64");
        let (_temp_over, package_over) = package_with(&over);
        assert!(matches!(
            probe_package(&package_over),
            Err(CatalogProbeError::InvalidPackage)
        ));
    }

    #[test]
    fn nation_folder_names_outside_the_fixed_pattern_are_ignored() {
        let (_temp, package) = package_with(&["usa", "Bad-Name", "x".repeat(40).as_str()]);
        let report = probe_package(&package).unwrap();
        assert_eq!(report.files_read, 1);
        assert!(report.unknown_nations.is_empty());
        assert_eq!(report.missing_nations.len(), 10);
    }

    fn stored_archive(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file(name, options).unwrap();
        writer.write_all(payload).unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn stored_entry_is_read_with_exact_declared_size() {
        let bytes = stored_archive("scripts/list.xml", b"synthetic payload");
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            read_catalog_entry(&mut archive, "scripts/list.xml").unwrap(),
            b"synthetic payload"
        );
        assert!(matches!(
            read_catalog_entry(&mut archive, "scripts/missing.xml"),
            Err(CatalogProbeError::MissingCatalogEntry)
        ));
    }

    #[test]
    fn deflated_entry_is_rejected() {
        // The crate is built without the deflate feature, so craft the archive by marking a
        // stored entry as method 8 (deflate) in both its local and central headers.
        let mut bytes = stored_archive("scripts/list.xml", b"synthetic payload");
        let mut patched = 0;
        for window_start in 0..bytes.len().saturating_sub(12) {
            match &bytes[window_start..window_start + 4] {
                b"PK" => {
                    bytes[window_start + 8] = 8;
                    patched += 1;
                }
                b"PK" => {
                    bytes[window_start + 10] = 8;
                    patched += 1;
                }
                _ => {}
            }
        }
        assert_eq!(patched, 2);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(matches!(
            read_catalog_entry(&mut archive, "scripts/list.xml"),
            Err(CatalogProbeError::UnsupportedCompression)
        ));
    }

    #[test]
    fn oversized_entry_is_rejected_before_it_is_read() {
        let payload = vec![0_u8; usize::try_from(MAX_PACKAGE_ENTRY_SIZE).unwrap() + 1];
        let bytes = stored_archive("scripts/list.xml", &payload);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(matches!(
            read_catalog_entry(&mut archive, "scripts/list.xml"),
            Err(CatalogProbeError::CatalogEntryTooLarge)
        ));
    }
}
