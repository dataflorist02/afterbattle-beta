use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    compact_descriptor::{build_compact_descriptor_catalog, decode_vehicle_compact_descr},
    local_tracker::canonicalize_vehicle_key,
    models::{
        CompactDescriptorProtocolSource, VehicleClass, VehicleClassCatalogAvailability,
        VehicleClassCatalogRefreshOutcome, VehicleClassCatalogStatus, VehicleClassResolution,
        VehicleClassResolutionState, VehicleClassSource, VehicleKeyCrossCheck,
    },
    vehicle_catalog_probe::{MAX_VEHICLE_TIER, NATION_KEYS, fingerprint_package, probe_package},
};

/// 2 (spec 20, E1b): entries carry the vehicle tier and `role_*` tags. Schema 1 catalogs are
/// still loaded (no tiers) as a safe fallback and are rebuilt by the next refresh.
pub const VEHICLE_CLASS_CATALOG_SCHEMA_VERSION: u32 = 2;
const LEGACY_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const VEHICLE_CLASS_RESOLVER_VERSION: &str = "vehicle-class-resolver-v1";
const CACHE_STATE_SCHEMA_VERSION: u32 = 1;
const MAX_CATALOG_BYTES: usize = 2 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 64 * 1024;
const MAX_STATE_BYTES: usize = 16 * 1024;
const MIN_CATALOG_ENTRIES: usize = 500;
const MAX_CATALOG_ENTRIES: usize = 5_000;
const MAX_INVALID_ZERO_IDS: usize = 100;
const MAX_EXCLUDED_MISSING_IDS: usize = 100;
const MAX_UNKNOWN_NATIONS: usize = 32;
const STALE_LOCK_MS: i64 = 10 * 60 * 1_000;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

static ACTIVE_CATALOG_ROOTS: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VehicleClassCatalogEntry {
    pub vehicle_id: u32,
    pub vehicle_class: VehicleClass,
    pub nation_id: u8,
    pub tank_id: u16,
    pub canonical_vehicle_key: String,
    /// Vehicle tier 1..=11 at the time the catalog was built; `None` in schema 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<u8>,
    /// `role_*` tags; empty in schema 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VehicleClassCatalogObject {
    pub schema_version: u32,
    pub resolver_version: String,
    pub protocol_source: CompactDescriptorProtocolSource,
    pub source_version: String,
    pub source_fingerprint: String,
    pub generated_at: i64,
    pub valid_entry_count: usize,
    pub invalid_zero_id_count: usize,
    pub excluded_missing_id_count: usize,
    /// Known nations missing from the client package (B5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_nations: Vec<String>,
    /// Nation folders the app does not know yet (B5); their vehicles are not in the catalog.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown_nations: Vec<String>,
    pub entries: Vec<VehicleClassCatalogEntry>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogActivation {
    schema_version: u32,
    object_sha256: String,
    source_version: String,
    source_fingerprint: String,
    generated_at: i64,
    valid_entry_count: usize,
    invalid_zero_id_count: usize,
    excluded_missing_id_count: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedVehicleClassCatalog {
    pub object: VehicleClassCatalogObject,
    entries: Arc<HashMap<u32, VehicleClassCatalogEntry>>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CatalogErrorCode {
    NoClient,
    InvalidSource,
    InvalidCatalog,
    Io,
}

impl CatalogErrorCode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::NoClient => "no_client",
            Self::InvalidSource => "invalid_source",
            Self::InvalidCatalog => "invalid_catalog",
            Self::Io => "io",
        }
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogUpdateState {
    schema_version: u32,
    generation: u64,
    last_attempt_at: Option<i64>,
    last_success_at: Option<i64>,
    last_error_code: Option<CatalogErrorCode>,
}

impl Default for CatalogUpdateState {
    fn default() -> Self {
        Self {
            schema_version: CACHE_STATE_SCHEMA_VERSION,
            generation: 0,
            last_attempt_at: None,
            last_success_at: None,
            last_error_code: None,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum VehicleClassCatalogError {
    #[error("vehicle class catalog filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("vehicle class catalog cache entry is invalid")]
    InvalidCache,
    #[error("vehicle class catalog source is unavailable")]
    SourceUnavailable,
    #[error("vehicle class catalog source is invalid")]
    InvalidSource,
    #[error("vehicle class catalog candidate is invalid")]
    InvalidCandidate,
}

#[derive(Debug, Clone)]
struct SourceIdentity {
    source_version: String,
    source_fingerprint: String,
    package_fingerprint: String,
    package_path: PathBuf,
}

fn sha256_hex(bytes: &[u8]) -> String {
    crate::hashing::hex_lower(Sha256::digest(bytes))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, VehicleClassCatalogError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > limit as u64 {
        return Err(VehicleClassCatalogError::InvalidCache);
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(VehicleClassCatalogError::InvalidCache);
    }
    Ok(bytes)
}

fn create_synced(path: &Path, bytes: &[u8]) -> Result<(), VehicleClassCatalogError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

fn cache_paths(root: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf) {
    (
        root.join("objects"),
        root.join("activations"),
        root.join("tmp"),
        root.join("locks"),
        root.join("update-state"),
    )
}

fn safe_regular_file(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
    }
    #[cfg(not(windows))]
    {
        true
    }
}

fn cleanup_part_files(root: &Path) -> Result<(), VehicleClassCatalogError> {
    let tmp = cache_paths(root).2;
    if !tmp.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(tmp)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("part")
            && safe_regular_file(&path)
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn activation_files(root: &Path) -> Result<Vec<PathBuf>, VehicleClassCatalogError> {
    let activations = cache_paths(root).1;
    if !activations.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(activations)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json")
            && safe_regular_file(&path)
        {
            files.push(path);
        }
    }
    files.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    Ok(files)
}

fn source_version(root: &Path) -> Result<String, VehicleClassCatalogError> {
    let bytes = read_limited(&root.join("version.xml"), 16 * 1024)
        .map_err(|_| VehicleClassCatalogError::SourceUnavailable)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| VehicleClassCatalogError::InvalidSource)?;
    let start = text
        .find("<version>")
        .map(|index| index + "<version>".len())
        .ok_or(VehicleClassCatalogError::InvalidSource)?;
    let end = text[start..]
        .find("</version>")
        .map(|index| start + index)
        .ok_or(VehicleClassCatalogError::InvalidSource)?;
    let version = text[start..end].trim();
    if version.is_empty()
        || version.len() > 128
        || !version
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        return Err(VehicleClassCatalogError::InvalidSource);
    }
    Ok(version.to_owned())
}

fn source_identity(root: &Path) -> Result<SourceIdentity, VehicleClassCatalogError> {
    if !root.is_dir() {
        return Err(VehicleClassCatalogError::SourceUnavailable);
    }
    let package_path = root.join("res").join("packages").join("scripts.pkg");
    let version = source_version(root)?;
    let package_fingerprint =
        fingerprint_package(&package_path).map_err(|_| VehicleClassCatalogError::InvalidSource)?;
    let mut fingerprint = Sha256::new();
    fingerprint.update(b"afterbattle-vehicle-class-source-v1\0");
    fingerprint.update((version.len() as u64).to_le_bytes());
    fingerprint.update(version.as_bytes());
    fingerprint.update(package_fingerprint.as_bytes());
    Ok(SourceIdentity {
        source_version: version,
        source_fingerprint: crate::hashing::hex_lower(fingerprint.finalize()),
        package_fingerprint,
        package_path,
    })
}

fn validate_catalog_object(
    object: &VehicleClassCatalogObject,
) -> Result<(), VehicleClassCatalogError> {
    let legacy = object.schema_version == LEGACY_CATALOG_SCHEMA_VERSION;
    if !(legacy || object.schema_version == VEHICLE_CLASS_CATALOG_SCHEMA_VERSION)
        || object.resolver_version != VEHICLE_CLASS_RESOLVER_VERSION
        || object.protocol_source
            != CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated
        || object.generated_at <= 0
        || object.source_version.trim().is_empty()
        || object.source_version.len() > 128
        || !valid_sha256(&object.source_fingerprint)
        || object.valid_entry_count != object.entries.len()
        || !(MIN_CATALOG_ENTRIES..=MAX_CATALOG_ENTRIES).contains(&object.entries.len())
        || object.invalid_zero_id_count > MAX_INVALID_ZERO_IDS
        || object.excluded_missing_id_count > MAX_EXCLUDED_MISSING_IDS
        || object.missing_nations.len() > NATION_KEYS.len()
        || object.unknown_nations.len() > MAX_UNKNOWN_NATIONS
    {
        return Err(VehicleClassCatalogError::InvalidCandidate);
    }

    let mut vehicle_ids = HashSet::with_capacity(object.entries.len());
    let mut vehicle_keys = HashSet::with_capacity(object.entries.len());
    let mut classes = HashSet::with_capacity(5);
    let mut previous_id = None;
    for entry in &object.entries {
        let tier_ok = match entry.tier {
            None => true,
            Some(tier) => !legacy && (1..=MAX_VEHICLE_TIER).contains(&tier),
        };
        let roles_ok = (!legacy || entry.roles.is_empty())
            && entry.roles.len() <= 8
            && entry
                .roles
                .iter()
                .all(|role| role.starts_with("role_") && role.len() <= 32);
        if !entry.vehicle_class.is_known()
            || !tier_ok
            || !roles_ok
            || entry.vehicle_id == 0
            || !vehicle_ids.insert(entry.vehicle_id)
            || !vehicle_keys.insert(entry.canonical_vehicle_key.as_str())
            || previous_id.is_some_and(|previous| previous >= entry.vehicle_id)
        {
            return Err(VehicleClassCatalogError::InvalidCandidate);
        }
        let canonical = canonicalize_vehicle_key(&entry.canonical_vehicle_key)
            .map_err(|_| VehicleClassCatalogError::InvalidCandidate)?;
        if canonical.as_str() != entry.canonical_vehicle_key {
            return Err(VehicleClassCatalogError::InvalidCandidate);
        }
        let decoded = decode_vehicle_compact_descr(entry.vehicle_id)
            .map_err(|_| VehicleClassCatalogError::InvalidCandidate)?;
        if decoded.nation_id != entry.nation_id || decoded.tank_id != entry.tank_id {
            return Err(VehicleClassCatalogError::InvalidCandidate);
        }
        classes.insert(entry.vehicle_class);
        previous_id = Some(entry.vehicle_id);
    }
    if classes.len() != 5 {
        return Err(VehicleClassCatalogError::InvalidCandidate);
    }
    Ok(())
}

fn build_catalog_from_client(
    root: &Path,
    now: i64,
) -> Result<VehicleClassCatalogObject, VehicleClassCatalogError> {
    let identity = source_identity(root)?;
    let report = probe_package(&identity.package_path)
        .map_err(|_| VehicleClassCatalogError::InvalidSource)?;
    let all_root_records_accounted_for = report
        .entries
        .len()
        .saturating_add(report.missing_id)
        .saturating_add(report.non_section_records)
        == report.total_records;
    let only_known_structural_nodes = report.non_section_records <= report.files_read
        && report
            .non_section_field_names
            .iter()
            .all(|name| name == "xmlns:xmlref");
    if report.source_fingerprint != identity.package_fingerprint
        || report.files_read == 0
        || report.files_read + report.missing_nations.len() != NATION_KEYS.len()
        || !all_root_records_accounted_for
        || !only_known_structural_nodes
        || report.missing_id > MAX_EXCLUDED_MISSING_IDS
        || report.missing_tags != 0
        || report.unknown_class != 0
        || report.class_conflicts != 0
        || report.duplicate_id_groups != 0
        || report.duplicate_id_class_conflicts != 0
    {
        return Err(VehicleClassCatalogError::InvalidSource);
    }

    let catalog = build_compact_descriptor_catalog(&report.entries);
    if !catalog.is_structurally_valid()
        || catalog.invalid_vehicle_ids() != 0
        || catalog.descriptor_collisions() != 0
        || catalog.unsupported_nation_keys() != 0
        || catalog.class_conflicts() != 0
        || catalog.round_trip_failures() != 0
    {
        return Err(VehicleClassCatalogError::InvalidSource);
    }

    let mut entries = Vec::with_capacity(catalog.len());
    for entry in catalog.entries() {
        let raw_key = format!("{}-{}", entry.nation_key, entry.vehicle_key);
        let canonical_vehicle_key = canonicalize_vehicle_key(&raw_key)
            .map_err(|_| VehicleClassCatalogError::InvalidSource)?
            .as_str()
            .to_owned();
        entries.push(VehicleClassCatalogEntry {
            vehicle_id: entry.type_comp_descr,
            vehicle_class: entry.vehicle_class,
            nation_id: entry.nation_id,
            tank_id: entry.tank_id,
            canonical_vehicle_key,
            tier: entry.tier,
            roles: entry.roles.clone(),
        });
    }
    entries.sort_by_key(|entry| entry.vehicle_id);
    let object = VehicleClassCatalogObject {
        schema_version: VEHICLE_CLASS_CATALOG_SCHEMA_VERSION,
        resolver_version: VEHICLE_CLASS_RESOLVER_VERSION.to_owned(),
        protocol_source: CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
        source_version: identity.source_version,
        source_fingerprint: identity.source_fingerprint,
        generated_at: now,
        valid_entry_count: entries.len(),
        invalid_zero_id_count: catalog.invalid_zero_vehicle_ids(),
        excluded_missing_id_count: report.missing_id,
        missing_nations: report.missing_nations,
        unknown_nations: report.unknown_nations,
        entries,
    };
    validate_catalog_object(&object)?;
    Ok(object)
}

fn load_activation(
    root: &Path,
    path: &Path,
) -> Result<LoadedVehicleClassCatalog, VehicleClassCatalogError> {
    if !safe_regular_file(path) {
        return Err(VehicleClassCatalogError::InvalidCache);
    }
    let manifest: CatalogActivation =
        serde_json::from_slice(&read_limited(path, MAX_MANIFEST_BYTES)?)
            .map_err(|_| VehicleClassCatalogError::InvalidCache)?;
    if !matches!(
        manifest.schema_version,
        LEGACY_CATALOG_SCHEMA_VERSION | VEHICLE_CLASS_CATALOG_SCHEMA_VERSION
    ) || !valid_sha256(&manifest.object_sha256)
        || !valid_sha256(&manifest.source_fingerprint)
    {
        return Err(VehicleClassCatalogError::InvalidCache);
    }
    let object_path = cache_paths(root)
        .0
        .join(format!("{}.json", manifest.object_sha256));
    if !safe_regular_file(&object_path) {
        return Err(VehicleClassCatalogError::InvalidCache);
    }
    let bytes = read_limited(&object_path, MAX_CATALOG_BYTES)?;
    if sha256_hex(&bytes) != manifest.object_sha256 {
        return Err(VehicleClassCatalogError::InvalidCache);
    }
    let object: VehicleClassCatalogObject =
        serde_json::from_slice(&bytes).map_err(|_| VehicleClassCatalogError::InvalidCache)?;
    validate_catalog_object(&object).map_err(|_| VehicleClassCatalogError::InvalidCache)?;
    if object.schema_version != manifest.schema_version
        || object.source_version != manifest.source_version
        || object.source_fingerprint != manifest.source_fingerprint
        || object.generated_at != manifest.generated_at
        || object.valid_entry_count != manifest.valid_entry_count
        || object.invalid_zero_id_count != manifest.invalid_zero_id_count
        || object.excluded_missing_id_count != manifest.excluded_missing_id_count
    {
        return Err(VehicleClassCatalogError::InvalidCache);
    }
    let entries = object
        .entries
        .iter()
        .cloned()
        .map(|entry| (entry.vehicle_id, entry))
        .collect();
    Ok(LoadedVehicleClassCatalog {
        object,
        entries: Arc::new(entries),
    })
}

pub(crate) fn load_current_or_previous_catalog(
    root: &Path,
) -> Result<Option<LoadedVehicleClassCatalog>, VehicleClassCatalogError> {
    for activation in activation_files(root)? {
        if let Ok(catalog) = load_activation(root, &activation) {
            return Ok(Some(catalog));
        }
    }
    Ok(None)
}

fn prune_old_activations(root: &Path) -> Result<(), VehicleClassCatalogError> {
    let mut valid_kept = 0usize;
    for path in activation_files(root)? {
        let keep = load_activation(root, &path).is_ok() && {
            valid_kept += 1;
            valid_kept <= 2
        };
        if !keep && safe_regular_file(&path) {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn activate_catalog_object(
    root: &Path,
    object: &VehicleClassCatalogObject,
) -> Result<(LoadedVehicleClassCatalog, bool), VehicleClassCatalogError> {
    validate_catalog_object(object)?;
    if let Some(current) = load_current_or_previous_catalog(root)?
        && current.object.source_fingerprint == object.source_fingerprint
    {
        return Ok((current, false));
    }
    let bytes = serde_json::to_vec_pretty(object)
        .map_err(|_| VehicleClassCatalogError::InvalidCandidate)?;
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(VehicleClassCatalogError::InvalidCandidate);
    }
    let object_sha256 = sha256_hex(&bytes);
    let (objects, activations, tmp, _, _) = cache_paths(root);
    fs::create_dir_all(&objects)?;
    fs::create_dir_all(&activations)?;
    fs::create_dir_all(&tmp)?;
    cleanup_part_files(root)?;

    let nonce = Uuid::new_v4();
    let part = tmp.join(format!("catalog-{nonce}.part"));
    create_synced(&part, &bytes)?;
    let object_path = objects.join(format!("{object_sha256}.json"));
    if object_path.exists() {
        if !safe_regular_file(&object_path)
            || read_limited(&object_path, MAX_CATALOG_BYTES)? != bytes
        {
            let _ = fs::remove_file(&part);
            return Err(VehicleClassCatalogError::InvalidCache);
        }
    } else if let Err(error) = create_synced(&object_path, &bytes) {
        let _ = fs::remove_file(&part);
        return Err(error);
    }
    fs::remove_file(&part)?;

    let manifest = CatalogActivation {
        schema_version: object.schema_version,
        object_sha256: object_sha256.clone(),
        source_version: object.source_version.clone(),
        source_fingerprint: object.source_fingerprint.clone(),
        generated_at: object.generated_at,
        valid_entry_count: object.valid_entry_count,
        invalid_zero_id_count: object.invalid_zero_id_count,
        excluded_missing_id_count: object.excluded_missing_id_count,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| VehicleClassCatalogError::InvalidCandidate)?;
    let activation_path = activations.join(format!(
        "{:020}-{nonce}-{object_sha256}.json",
        object.generated_at
    ));
    create_synced(&activation_path, &manifest_bytes)?;
    let loaded = load_activation(root, &activation_path)?;
    prune_old_activations(root)?;
    Ok((loaded, true))
}

fn state_files(root: &Path) -> Result<Vec<PathBuf>, VehicleClassCatalogError> {
    let state = cache_paths(root).4;
    if !state.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(state)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) == Some("json")
            && safe_regular_file(&path)
        {
            files.push(path);
        }
    }
    files.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    Ok(files)
}

fn load_update_state(root: &Path) -> Result<CatalogUpdateState, VehicleClassCatalogError> {
    for path in state_files(root)? {
        let Ok(bytes) = read_limited(&path, MAX_STATE_BYTES) else {
            continue;
        };
        let Ok(state) = serde_json::from_slice::<CatalogUpdateState>(&bytes) else {
            continue;
        };
        let prefix = format!("{:020}-", state.generation);
        if state.schema_version == CACHE_STATE_SCHEMA_VERSION
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        {
            return Ok(state);
        }
    }
    Ok(CatalogUpdateState::default())
}

fn persist_update_state(
    root: &Path,
    state: &CatalogUpdateState,
) -> Result<CatalogUpdateState, VehicleClassCatalogError> {
    let mut next = state.clone();
    next.generation = next
        .generation
        .checked_add(1)
        .ok_or(VehicleClassCatalogError::InvalidCache)?;
    let bytes =
        serde_json::to_vec_pretty(&next).map_err(|_| VehicleClassCatalogError::InvalidCache)?;
    let tmp = cache_paths(root).2;
    let state_dir = cache_paths(root).4;
    fs::create_dir_all(&tmp)?;
    fs::create_dir_all(&state_dir)?;
    let nonce = Uuid::new_v4();
    let part = tmp.join(format!("state-{nonce}.part"));
    create_synced(&part, &bytes)?;
    let final_path = state_dir.join(format!("{:020}-{nonce}.json", next.generation));
    let result = create_synced(&final_path, &bytes);
    let _ = fs::remove_file(part);
    result?;
    for old in state_files(root)?.into_iter().skip(8) {
        if safe_regular_file(&old) {
            fs::remove_file(old)?;
        }
    }
    Ok(next)
}

fn absolute_lock_key(root: &Path) -> Result<PathBuf, VehicleClassCatalogError> {
    if root.is_absolute() {
        Ok(root.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(root))
    }
}

struct ProcessUpdateLock {
    key: PathBuf,
}

impl ProcessUpdateLock {
    fn try_acquire(root: &Path) -> Result<Option<Self>, VehicleClassCatalogError> {
        let key = absolute_lock_key(root)?;
        let mut active = ACTIVE_CATALOG_ROOTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !active.insert(key.clone()) {
            return Ok(None);
        }
        Ok(Some(Self { key }))
    }
}

impl Drop for ProcessUpdateLock {
    fn drop(&mut self) {
        ACTIVE_CATALOG_ROOTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.key);
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Serialize, Deserialize)]
struct FileLockRecord {
    token: String,
    created_at: i64,
}

fn modified_at_millis(path: &Path) -> Option<i64> {
    path.metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()
}

fn lock_is_stale(bytes: &[u8], now: i64, modified_at: Option<i64>) -> bool {
    match serde_json::from_slice::<FileLockRecord>(bytes) {
        Ok(lock) => now.saturating_sub(lock.created_at) >= STALE_LOCK_MS,
        Err(_) => modified_at.is_some_and(|modified| now.saturating_sub(modified) >= STALE_LOCK_MS),
    }
}

struct ReclaimLock {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl ReclaimLock {
    fn try_acquire(locks: &Path, now: i64) -> Result<Option<Self>, VehicleClassCatalogError> {
        let path = locks.join("update.lock.reclaim");
        let bytes = serde_json::to_vec(&FileLockRecord {
            token: Uuid::new_v4().to_string(),
            created_at: now,
        })
        .map_err(|_| VehicleClassCatalogError::InvalidCache)?;
        match create_synced(&path, &bytes) {
            Ok(()) => Ok(Some(Self { path, bytes })),
            Err(VehicleClassCatalogError::Io(error))
                if error.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for ReclaimLock {
    fn drop(&mut self) {
        if fs::read(&self.path).ok().as_deref() == Some(self.bytes.as_slice()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

struct FileUpdateLock {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl FileUpdateLock {
    fn try_acquire(root: &Path, now: i64) -> Result<Option<Self>, VehicleClassCatalogError> {
        let locks = cache_paths(root).3;
        fs::create_dir_all(&locks)?;
        let path = locks.join("update.lock");
        let reclaim_path = locks.join("update.lock.reclaim");
        if reclaim_path.exists() {
            let existing = match read_limited(&reclaim_path, MAX_STATE_BYTES) {
                Ok(value) => value,
                Err(_) => return Ok(None),
            };
            if !lock_is_stale(&existing, now, modified_at_millis(&reclaim_path))
                || fs::read(&reclaim_path).ok().as_deref() != Some(existing.as_slice())
            {
                return Ok(None);
            }
            match fs::remove_file(&reclaim_path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let bytes = serde_json::to_vec(&FileLockRecord {
            token: Uuid::new_v4().to_string(),
            created_at: now,
        })
        .map_err(|_| VehicleClassCatalogError::InvalidCache)?;
        match create_synced(&path, &bytes) {
            Ok(()) => Ok(Some(Self { path, bytes })),
            Err(VehicleClassCatalogError::Io(error))
                if error.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                let existing = match read_limited(&path, MAX_STATE_BYTES) {
                    Ok(value) => value,
                    Err(_) => return Ok(None),
                };
                if !lock_is_stale(&existing, now, modified_at_millis(&path)) {
                    return Ok(None);
                }
                let Some(_reclaim) = ReclaimLock::try_acquire(&locks, now)? else {
                    return Ok(None);
                };
                if fs::read(&path).ok().as_deref() != Some(existing.as_slice()) {
                    return Ok(None);
                }
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(error.into()),
                }
                create_synced(&path, &bytes)?;
                Ok(Some(Self { path, bytes }))
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for FileUpdateLock {
    fn drop(&mut self) {
        if fs::read(&self.path).ok().as_deref() == Some(self.bytes.as_slice()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn outcome_for_failure(has_current: bool) -> VehicleClassCatalogRefreshOutcome {
    if has_current {
        VehicleClassCatalogRefreshOutcome::FailedUsingCurrent
    } else {
        VehicleClassCatalogRefreshOutcome::Unavailable
    }
}

fn error_code(error: &VehicleClassCatalogError) -> CatalogErrorCode {
    match error {
        VehicleClassCatalogError::SourceUnavailable => CatalogErrorCode::NoClient,
        VehicleClassCatalogError::InvalidSource => CatalogErrorCode::InvalidSource,
        VehicleClassCatalogError::InvalidCandidate | VehicleClassCatalogError::InvalidCache => {
            CatalogErrorCode::InvalidCatalog
        }
        VehicleClassCatalogError::Io(_) => CatalogErrorCode::Io,
    }
}

/// A catalog needs no rebuild only when it has the current schema and the same client source.
/// A schema 1 catalog (no tiers) is rebuilt even for an unchanged client (E1b).
fn catalog_is_current(catalog: &LoadedVehicleClassCatalog, source_fingerprint: &str) -> bool {
    catalog.object.schema_version == VEHICLE_CLASS_CATALOG_SCHEMA_VERSION
        && catalog.object.source_fingerprint == source_fingerprint
}

fn refresh_catalog_cache(
    cache_root: &Path,
    game_root: Option<&Path>,
    now: i64,
) -> Result<VehicleClassCatalogRefreshOutcome, VehicleClassCatalogError> {
    let current = load_current_or_previous_catalog(cache_root)?;
    let Some(_process_lock) = ProcessUpdateLock::try_acquire(cache_root)? else {
        return Ok(VehicleClassCatalogRefreshOutcome::Busy);
    };
    let Some(_file_lock) = FileUpdateLock::try_acquire(cache_root, now)? else {
        return Ok(VehicleClassCatalogRefreshOutcome::Busy);
    };
    cleanup_part_files(cache_root)?;
    let mut state = load_update_state(cache_root)?;
    state.last_attempt_at = Some(now);
    state.last_error_code = None;
    state = persist_update_state(cache_root, &state)?;

    let result = (|| {
        let root = game_root.ok_or(VehicleClassCatalogError::SourceUnavailable)?;
        let identity = source_identity(root)?;
        if current
            .as_ref()
            .is_some_and(|catalog| catalog_is_current(catalog, &identity.source_fingerprint))
        {
            return Ok(VehicleClassCatalogRefreshOutcome::Unchanged);
        }
        let candidate = build_catalog_from_client(root, now)?;
        let (_, activated) = activate_catalog_object(cache_root, &candidate)?;
        Ok(if activated {
            VehicleClassCatalogRefreshOutcome::Activated
        } else {
            VehicleClassCatalogRefreshOutcome::Unchanged
        })
    })();

    match result {
        Ok(outcome) => {
            state.last_success_at = Some(now);
            state.last_error_code = None;
            let _ = persist_update_state(cache_root, &state)?;
            Ok(outcome)
        }
        Err(error) => {
            state.last_error_code = Some(error_code(&error));
            let _ = persist_update_state(cache_root, &state)?;
            Ok(outcome_for_failure(current.is_some()))
        }
    }
}

impl LoadedVehicleClassCatalog {
    /// Exact lookup by `typeCompDescr` (E1b): class, tier and roles of one vehicle.
    pub(crate) fn vehicle(&self, type_comp_descr: i64) -> Option<&VehicleClassCatalogEntry> {
        let id = u32::try_from(type_comp_descr).ok()?;
        self.entries.get(&id)
    }

    /// Fingerprint of the client data this catalog was built from; stored with each battle so
    /// a tier can be traced to the catalog version (E2).
    pub(crate) fn fingerprint(&self) -> &str {
        &self.object.source_fingerprint
    }

    pub(crate) fn resolve(
        &self,
        vehicle_id: i64,
        vehicle_key: Option<&str>,
    ) -> VehicleClassResolution {
        let base = || VehicleClassResolution {
            vehicle_id,
            vehicle_class: VehicleClass::Unknown,
            state: VehicleClassResolutionState::Unknown,
            source: VehicleClassSource::Unknown,
            catalog_schema_version: Some(self.object.schema_version),
            catalog_source_version: Some(self.object.source_version.clone()),
            catalog_fingerprint: Some(self.fingerprint().to_owned()),
            protocol_source: self.object.protocol_source,
            vehicle_key_cross_check: VehicleKeyCrossCheck::Unavailable,
        };
        let Some(entry) = self.vehicle(vehicle_id) else {
            return base();
        };
        let cross_check = match vehicle_key {
            None => VehicleKeyCrossCheck::Unavailable,
            Some(key) if key == entry.canonical_vehicle_key => VehicleKeyCrossCheck::RawExact,
            Some(key) => match canonicalize_vehicle_key(key) {
                Ok(canonical) if canonical.as_str() == entry.canonical_vehicle_key => {
                    VehicleKeyCrossCheck::CanonicalExact
                }
                _ => VehicleKeyCrossCheck::Conflict,
            },
        };
        if cross_check == VehicleKeyCrossCheck::Conflict {
            return VehicleClassResolution {
                state: VehicleClassResolutionState::Conflict,
                vehicle_key_cross_check: cross_check,
                ..base()
            };
        }
        VehicleClassResolution {
            vehicle_id,
            vehicle_class: entry.vehicle_class,
            state: VehicleClassResolutionState::Resolved,
            source: VehicleClassSource::WgClientPackedCatalog,
            catalog_schema_version: Some(self.object.schema_version),
            catalog_source_version: Some(self.object.source_version.clone()),
            catalog_fingerprint: Some(self.fingerprint().to_owned()),
            protocol_source: self.object.protocol_source,
            vehicle_key_cross_check: cross_check,
        }
    }
}

pub(crate) fn unavailable_resolution(vehicle_id: i64) -> VehicleClassResolution {
    VehicleClassResolution {
        vehicle_id,
        vehicle_class: VehicleClass::Unknown,
        state: if vehicle_id > 0 {
            VehicleClassResolutionState::CatalogUnavailable
        } else {
            VehicleClassResolutionState::Unknown
        },
        source: VehicleClassSource::Unknown,
        catalog_schema_version: None,
        catalog_source_version: None,
        catalog_fingerprint: None,
        protocol_source: CompactDescriptorProtocolSource::Unconfirmed,
        vehicle_key_cross_check: VehicleKeyCrossCheck::Unavailable,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct VehicleClassCatalogRuntime {
    cache_root: PathBuf,
    snapshot: Arc<RwLock<Option<LoadedVehicleClassCatalog>>>,
    active_updates: Arc<AtomicUsize>,
}

struct RuntimeUpdateGuard {
    updates: Arc<AtomicUsize>,
}

impl Drop for RuntimeUpdateGuard {
    fn drop(&mut self) {
        self.updates.store(0, Ordering::SeqCst);
    }
}

impl VehicleClassCatalogRuntime {
    pub(crate) fn load(cache_root: PathBuf) -> Self {
        let snapshot = load_current_or_previous_catalog(&cache_root).ok().flatten();
        Self {
            cache_root,
            snapshot: Arc::new(RwLock::new(snapshot)),
            active_updates: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub(crate) fn snapshot(&self) -> Option<LoadedVehicleClassCatalog> {
        self.snapshot
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn refresh(
        &self,
        game_root: Option<&Path>,
    ) -> Result<VehicleClassCatalogRefreshOutcome, VehicleClassCatalogError> {
        if self
            .active_updates
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(VehicleClassCatalogRefreshOutcome::Busy);
        }
        let _guard = RuntimeUpdateGuard {
            updates: Arc::clone(&self.active_updates),
        };
        let outcome =
            refresh_catalog_cache(&self.cache_root, game_root, Utc::now().timestamp_millis())?;
        if matches!(
            outcome,
            VehicleClassCatalogRefreshOutcome::Activated
                | VehicleClassCatalogRefreshOutcome::Unchanged
        ) && let Some(catalog) = load_current_or_previous_catalog(&self.cache_root)?
        {
            *self
                .snapshot
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(catalog);
        }
        Ok(outcome)
    }

    pub(crate) fn status(
        &self,
        unresolved_current_vehicle_count: usize,
        conflict_current_vehicle_count: usize,
        installed_client_version: Option<&str>,
    ) -> VehicleClassCatalogStatus {
        let updating = self.active_updates.load(Ordering::SeqCst) > 0;
        let snapshot = self.snapshot();
        let state = load_update_state(&self.cache_root).unwrap_or_default();
        let has_error = state.last_error_code.is_some();
        let availability = if updating {
            VehicleClassCatalogAvailability::Updating
        } else if has_error {
            VehicleClassCatalogAvailability::Error
        } else if snapshot.is_some() {
            VehicleClassCatalogAvailability::Available
        } else {
            VehicleClassCatalogAvailability::Unavailable
        };
        VehicleClassCatalogStatus {
            availability,
            source_version: snapshot
                .as_ref()
                .map(|catalog| catalog.object.source_version.clone()),
            vehicle_count: snapshot
                .as_ref()
                .map_or(0, |catalog| catalog.object.valid_entry_count),
            invalid_record_count: snapshot.as_ref().map_or(0, |catalog| {
                catalog
                    .object
                    .invalid_zero_id_count
                    .saturating_add(catalog.object.excluded_missing_id_count)
            }),
            unresolved_current_vehicle_count,
            conflict_current_vehicle_count,
            last_attempt_at: state.last_attempt_at,
            last_success_at: state.last_success_at,
            last_error_code: state.last_error_code.map(|code| code.as_str().to_owned()),
            update_in_progress: updating,
            protocol_source: snapshot
                .as_ref()
                .map_or(CompactDescriptorProtocolSource::Unconfirmed, |catalog| {
                    catalog.object.protocol_source
                }),
            schema_version: snapshot
                .as_ref()
                .map(|catalog| catalog.object.schema_version),
            vehicles_with_tier: snapshot.as_ref().map_or(0, |catalog| {
                catalog
                    .object
                    .entries
                    .iter()
                    .filter(|entry| entry.tier.is_some())
                    .count()
            }),
            missing_nations: snapshot
                .as_ref()
                .map(|catalog| catalog.object.missing_nations.clone())
                .unwrap_or_default(),
            unknown_nations: snapshot
                .as_ref()
                .map(|catalog| catalog.object.unknown_nations.clone())
                .unwrap_or_default(),
            stale_for_client_version: stale_for_client_version(
                snapshot.as_ref(),
                installed_client_version,
            ),
        }
    }
}

/// B5: the installed client version when the active catalog was built from another version
/// (or is a legacy schema without tiers); `None` when current or when no client is known.
fn stale_for_client_version(
    catalog: Option<&LoadedVehicleClassCatalog>,
    installed_client_version: Option<&str>,
) -> Option<String> {
    let installed = installed_client_version?;
    let catalog = catalog?;
    (catalog.object.source_version != installed
        || catalog.object.schema_version != VEHICLE_CLASS_CATALOG_SCHEMA_VERSION)
        .then(|| installed.to_owned())
}

/// The client version from `version.xml` of an approved game folder, if readable.
pub(crate) fn installed_client_version(game_root: &Path) -> Option<String> {
    source_version(game_root).ok()
}

pub(crate) fn resolution_counts(
    snapshot: Option<&LoadedVehicleClassCatalog>,
    vehicles: &[(i64, Option<String>)],
) -> (usize, usize) {
    let mut by_id = HashMap::<i64, (bool, bool)>::new();
    for (vehicle_id, vehicle_key) in vehicles {
        let resolution = snapshot.map_or_else(
            || unavailable_resolution(*vehicle_id),
            |catalog| catalog.resolve(*vehicle_id, vehicle_key.as_deref()),
        );
        let entry = by_id.entry(*vehicle_id).or_default();
        match resolution.state {
            VehicleClassResolutionState::Conflict => entry.1 = true,
            VehicleClassResolutionState::Unknown
            | VehicleClassResolutionState::CatalogUnavailable => entry.0 = true,
            VehicleClassResolutionState::Resolved => {}
        }
    }
    by_id.values().fold((0, 0), |(unknown, conflict), item| {
        (
            unknown + usize::from(item.0),
            conflict + usize::from(item.1),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compact_descriptor::encode_vehicle_compact_descr;
    use std::{hint::black_box, time::Instant};
    use tempfile::TempDir;

    fn candidate(fingerprint_byte: char, generated_at: i64) -> VehicleClassCatalogObject {
        let classes = [
            VehicleClass::LightTank,
            VehicleClass::MediumTank,
            VehicleClass::HeavyTank,
            VehicleClass::TankDestroyer,
            VehicleClass::Spg,
        ];
        let mut entries = (0..500_u32)
            .map(|index| {
                let nation_id = (index % 11) as u8;
                let tank_id = (index / 11 + 1) as u16;
                VehicleClassCatalogEntry {
                    vehicle_id: encode_vehicle_compact_descr(nation_id, u32::from(tank_id))
                        .unwrap(),
                    vehicle_class: classes[index as usize % classes.len()],
                    nation_id,
                    tank_id,
                    canonical_vehicle_key: format!("nation-vehicle_{index}"),
                    tier: Some((index % 11 + 1) as u8),
                    roles: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.vehicle_id);
        VehicleClassCatalogObject {
            schema_version: VEHICLE_CLASS_CATALOG_SCHEMA_VERSION,
            resolver_version: VEHICLE_CLASS_RESOLVER_VERSION.to_owned(),
            protocol_source:
                CompactDescriptorProtocolSource::ExternallyDocumentedAndRuntimeValidated,
            source_version: format!("v.{generated_at}"),
            source_fingerprint: fingerprint_byte.to_string().repeat(64),
            generated_at,
            valid_entry_count: entries.len(),
            invalid_zero_id_count: 9,
            excluded_missing_id_count: 0,
            missing_nations: Vec::new(),
            unknown_nations: Vec::new(),
            entries,
        }
    }

    fn first_entry(object: &VehicleClassCatalogObject) -> &VehicleClassCatalogEntry {
        object.entries.first().unwrap()
    }

    fn activate(root: &Path, fingerprint: char, at: i64) -> LoadedVehicleClassCatalog {
        activate_catalog_object(root, &candidate(fingerprint, at))
            .unwrap()
            .0
    }

    #[test]
    fn cold_start_without_cache_is_unavailable_and_does_not_create_files() {
        let temp = TempDir::new().unwrap();
        assert!(
            load_current_or_previous_catalog(temp.path())
                .unwrap()
                .is_none()
        );
        let runtime = VehicleClassCatalogRuntime::load(temp.path().to_path_buf());
        let status = runtime.status(0, 0, None);
        assert_eq!(
            status.availability,
            VehicleClassCatalogAvailability::Unavailable
        );
        assert_eq!(status.vehicle_count, 0);
        assert!(runtime.snapshot().is_none());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn valid_catalog_activates_and_reloads_after_restart() {
        let temp = TempDir::new().unwrap();
        let activated = activate(temp.path(), 'a', 1_000);
        assert_eq!(activated.object.valid_entry_count, 500);
        assert_eq!(activated.object.invalid_zero_id_count, 9);
        let reloaded = VehicleClassCatalogRuntime::load(temp.path().to_path_buf());
        assert_eq!(reloaded.snapshot().unwrap().object, activated.object);
        assert_eq!(activation_files(temp.path()).unwrap().len(), 1);
    }

    #[test]
    fn corrupt_current_falls_back_to_previous_and_two_corrupt_generations_are_unavailable() {
        let temp = TempDir::new().unwrap();
        activate(temp.path(), 'a', 1_000);
        activate(temp.path(), 'b', 2_000);
        let activations = activation_files(temp.path()).unwrap();
        fs::write(&activations[0], b"corrupt").unwrap();
        let fallback = load_current_or_previous_catalog(temp.path())
            .unwrap()
            .unwrap();
        assert_eq!(fallback.object.source_fingerprint, "a".repeat(64));
        fs::write(&activations[1], b"corrupt").unwrap();
        assert!(
            load_current_or_previous_catalog(temp.path())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn corrupt_current_object_falls_back_without_modifying_previous() {
        let temp = TempDir::new().unwrap();
        let previous = activate(temp.path(), 'a', 1_000);
        activate(temp.path(), 'b', 2_000);
        let current_manifest: CatalogActivation = serde_json::from_slice(
            &read_limited(
                &activation_files(temp.path()).unwrap()[0],
                MAX_MANIFEST_BYTES,
            )
            .unwrap(),
        )
        .unwrap();
        fs::write(
            cache_paths(temp.path())
                .0
                .join(format!("{}.json", current_manifest.object_sha256)),
            b"corrupt",
        )
        .unwrap();
        assert_eq!(
            load_current_or_previous_catalog(temp.path())
                .unwrap()
                .unwrap()
                .object,
            previous.object
        );
    }

    #[test]
    fn activation_prunes_corrupt_generation_but_retains_a_valid_previous() {
        let temp = TempDir::new().unwrap();
        let previous = activate(temp.path(), 'a', 1_000);
        activate(temp.path(), 'b', 2_000);
        let corrupt = activation_files(temp.path()).unwrap()[0].clone();
        fs::write(&corrupt, b"corrupt").unwrap();
        activate(temp.path(), 'c', 3_000);
        assert!(!corrupt.exists());
        let activations = activation_files(temp.path()).unwrap();
        assert_eq!(activations.len(), 2);
        fs::write(&activations[0], b"corrupt-newest").unwrap();
        assert_eq!(
            load_current_or_previous_catalog(temp.path())
                .unwrap()
                .unwrap()
                .object,
            previous.object
        );
    }

    #[test]
    fn identical_fingerprint_is_unchanged_and_new_fingerprint_activates() {
        let temp = TempDir::new().unwrap();
        let first = candidate('a', 1_000);
        assert!(activate_catalog_object(temp.path(), &first).unwrap().1);
        let mut same_source = first.clone();
        same_source.generated_at = 2_000;
        assert!(
            !activate_catalog_object(temp.path(), &same_source)
                .unwrap()
                .1
        );
        assert!(
            activate_catalog_object(temp.path(), &candidate('b', 3_000))
                .unwrap()
                .1
        );
        assert_eq!(activation_files(temp.path()).unwrap().len(), 2);
    }

    fn legacy(fingerprint: char, at: i64) -> VehicleClassCatalogObject {
        let mut object = candidate(fingerprint, at);
        object.schema_version = 1;
        for entry in &mut object.entries {
            entry.tier = None;
            entry.roles.clear();
        }
        object
    }

    // E1b: a schema 1 catalog from an earlier version still loads (classes, no tiers) as the
    // safe fallback, and is rebuilt by the next refresh even if the client did not change.
    #[test]
    fn legacy_schema_one_catalog_loads_without_tiers_and_is_never_current() {
        let temp = TempDir::new().unwrap();
        let (loaded, activated) =
            activate_catalog_object(temp.path(), &legacy('a', 1_000)).unwrap();
        assert!(activated);
        assert_eq!(loaded.object.schema_version, 1);
        let reloaded = VehicleClassCatalogRuntime::load(temp.path().to_path_buf())
            .snapshot()
            .unwrap();
        assert!(
            reloaded
                .object
                .entries
                .iter()
                .all(|entry| entry.tier.is_none())
        );
        let fingerprint = reloaded.object.source_fingerprint.clone();
        assert!(!catalog_is_current(&reloaded, &fingerprint));

        let current = activate(temp.path(), 'b', 2_000);
        let fingerprint = current.object.source_fingerprint.clone();
        assert!(catalog_is_current(&current, &fingerprint));
        assert!(!catalog_is_current(&current, &"c".repeat(64)));
    }

    #[test]
    fn tiers_and_roles_are_validated_per_schema() {
        let mut out_of_range = candidate('a', 1_000);
        out_of_range.entries[0].tier = Some(12);
        assert!(validate_catalog_object(&out_of_range).is_err());
        let mut zero = candidate('a', 1_000);
        zero.entries[0].tier = Some(0);
        assert!(validate_catalog_object(&zero).is_err());
        let mut eleven = candidate('a', 1_000);
        eleven.entries[0].tier = Some(11);
        eleven.entries[0].roles = vec!["role_HT_break".into()];
        assert!(validate_catalog_object(&eleven).is_ok());
        let mut bad_role = candidate('a', 1_000);
        bad_role.entries[0].roles = vec!["premium".into()];
        assert!(validate_catalog_object(&bad_role).is_err());
        let mut legacy_with_tier = legacy('a', 1_000);
        legacy_with_tier.entries[0].tier = Some(5);
        assert!(validate_catalog_object(&legacy_with_tier).is_err());
        let mut future = candidate('a', 1_000);
        future.schema_version = 3;
        assert!(validate_catalog_object(&future).is_err());
    }

    #[test]
    fn vehicle_lookup_returns_tier_and_status_reports_stale_client_and_coverage() {
        let temp = TempDir::new().unwrap();
        let catalog = activate(temp.path(), 'a', 1_000);
        let first = first_entry(&catalog.object).clone();
        let found = catalog.vehicle(i64::from(first.vehicle_id)).unwrap();
        assert_eq!(found.tier, first.tier);
        assert!(catalog.vehicle(-1).is_none());
        assert!(catalog.vehicle(i64::from(u32::MAX) + 1).is_none());

        let runtime = VehicleClassCatalogRuntime::load(temp.path().to_path_buf());
        let version = catalog.object.source_version.clone();
        let current = runtime.status(0, 0, Some(&version));
        assert_eq!(current.stale_for_client_version, None);
        assert_eq!(
            current.schema_version,
            Some(VEHICLE_CLASS_CATALOG_SCHEMA_VERSION)
        );
        assert_eq!(current.vehicles_with_tier, 500);
        let stale = runtime.status(0, 0, Some("v.9.9.9"));
        assert_eq!(stale.stale_for_client_version.as_deref(), Some("v.9.9.9"));
        assert_eq!(runtime.status(0, 0, None).stale_for_client_version, None);

        activate_catalog_object(temp.path(), &legacy('b', 2_000)).unwrap();
        let runtime = VehicleClassCatalogRuntime::load(temp.path().to_path_buf());
        let legacy_status = runtime.status(0, 0, Some("v.2000"));
        assert_eq!(legacy_status.vehicles_with_tier, 0);
        assert_eq!(
            legacy_status.stale_for_client_version.as_deref(),
            Some("v.2000")
        );
    }

    #[test]
    fn invalid_candidate_never_changes_the_active_catalog() {
        let temp = TempDir::new().unwrap();
        let current = activate(temp.path(), 'a', 1_000);
        let mut invalid = candidate('b', 2_000);
        invalid.entries[0].vehicle_class = VehicleClass::Unknown;
        assert!(activate_catalog_object(temp.path(), &invalid).is_err());
        assert_eq!(
            load_current_or_previous_catalog(temp.path())
                .unwrap()
                .unwrap()
                .object,
            current.object
        );
    }

    #[test]
    fn content_addressed_object_is_never_overwritten() {
        let temp = TempDir::new().unwrap();
        let object = candidate('a', 1_000);
        let bytes = serde_json::to_vec_pretty(&object).unwrap();
        let path = cache_paths(temp.path())
            .0
            .join(format!("{}.json", sha256_hex(&bytes)));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"different").unwrap();
        assert!(activate_catalog_object(temp.path(), &object).is_err());
        assert_eq!(fs::read(path).unwrap(), b"different");
        assert!(activation_files(temp.path()).unwrap().is_empty());
    }

    #[test]
    fn part_files_are_cleaned_and_interrupted_update_cannot_become_current() {
        let temp = TempDir::new().unwrap();
        let tmp = cache_paths(temp.path()).2;
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("interrupted.part"), b"private game bytes").unwrap();
        cleanup_part_files(temp.path()).unwrap();
        assert!(fs::read_dir(&tmp).unwrap().next().is_none());
        assert!(
            load_current_or_previous_catalog(temp.path())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn fresh_file_lock_blocks_and_stale_lock_is_recovered_without_helpers() {
        let temp = TempDir::new().unwrap();
        let first = FileUpdateLock::try_acquire(temp.path(), 1_000)
            .unwrap()
            .unwrap();
        assert!(
            FileUpdateLock::try_acquire(temp.path(), 1_001)
                .unwrap()
                .is_none()
        );
        drop(first);
        let locks = cache_paths(temp.path()).3;
        let stale = serde_json::to_vec(&FileLockRecord {
            token: "stale".into(),
            created_at: 1,
        })
        .unwrap();
        create_synced(&locks.join("update.lock"), &stale).unwrap();
        let recovered = FileUpdateLock::try_acquire(temp.path(), STALE_LOCK_MS + 2)
            .unwrap()
            .unwrap();
        drop(recovered);
        assert!(!locks.join("update.lock").exists());
        assert!(!locks.join("update.lock.reclaim").exists());
    }

    #[test]
    fn process_lock_is_raii_and_prevents_parallel_updates() {
        let temp = TempDir::new().unwrap();
        let first = ProcessUpdateLock::try_acquire(temp.path())
            .unwrap()
            .unwrap();
        assert!(
            ProcessUpdateLock::try_acquire(temp.path())
                .unwrap()
                .is_none()
        );
        drop(first);
        assert!(
            ProcessUpdateLock::try_acquire(temp.path())
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn missing_client_keeps_current_and_progress_always_resets() {
        let temp = TempDir::new().unwrap();
        activate(temp.path(), 'a', 1_000);
        let runtime = VehicleClassCatalogRuntime::load(temp.path().to_path_buf());
        assert_eq!(
            runtime.refresh(None).unwrap(),
            VehicleClassCatalogRefreshOutcome::FailedUsingCurrent
        );
        assert!(!runtime.status(0, 0, None).update_in_progress);
        assert!(runtime.snapshot().is_some());
        assert_eq!(
            runtime.status(0, 0, None).last_error_code.as_deref(),
            Some("no_client")
        );
    }

    #[test]
    fn no_client_without_cache_is_non_blocking_unavailable() {
        let temp = TempDir::new().unwrap();
        let runtime = VehicleClassCatalogRuntime::load(temp.path().to_path_buf());
        assert_eq!(
            runtime.refresh(None).unwrap(),
            VehicleClassCatalogRefreshOutcome::Unavailable
        );
        assert!(!runtime.status(0, 0, None).update_in_progress);
        assert!(runtime.snapshot().is_none());
    }

    #[test]
    fn resolver_is_exact_fail_closed_and_preserves_cross_check_provenance() {
        let temp = TempDir::new().unwrap();
        let catalog = activate(temp.path(), 'a', 1_000);
        let entry = first_entry(&catalog.object);
        let without_key = catalog.resolve(i64::from(entry.vehicle_id), None);
        assert_eq!(without_key.state, VehicleClassResolutionState::Resolved);
        assert_eq!(
            without_key.vehicle_key_cross_check,
            VehicleKeyCrossCheck::Unavailable
        );
        let raw = catalog.resolve(
            i64::from(entry.vehicle_id),
            Some(&entry.canonical_vehicle_key),
        );
        assert_eq!(raw.state, VehicleClassResolutionState::Resolved);
        assert_eq!(raw.vehicle_key_cross_check, VehicleKeyCrossCheck::RawExact);
        let canonical = catalog.resolve(
            i64::from(entry.vehicle_id),
            Some(&entry.canonical_vehicle_key.to_ascii_uppercase()),
        );
        assert_eq!(canonical.state, VehicleClassResolutionState::Resolved);
        assert_eq!(
            canonical.vehicle_key_cross_check,
            VehicleKeyCrossCheck::CanonicalExact
        );
        let conflict = catalog.resolve(i64::from(entry.vehicle_id), Some("unrelated-name"));
        assert_eq!(conflict.state, VehicleClassResolutionState::Conflict);
        assert_eq!(conflict.vehicle_class, VehicleClass::Unknown);
        assert_eq!(conflict.source, VehicleClassSource::Unknown);
        assert_eq!(
            catalog.resolve(i64::from(u32::MAX), None).state,
            VehicleClassResolutionState::Unknown
        );
        assert_eq!(
            catalog.resolve(0, None).state,
            VehicleClassResolutionState::Unknown
        );
        assert_eq!(
            unavailable_resolution(i64::from(entry.vehicle_id)).state,
            VehicleClassResolutionState::CatalogUnavailable
        );
    }

    #[test]
    fn resolver_does_not_use_display_names_fuzzy_matching_disk_or_sql() {
        let temp = TempDir::new().unwrap();
        let catalog = activate(temp.path(), 'a', 1_000);
        let entry = first_entry(&catalog.object).clone();
        fs::remove_dir_all(temp.path()).unwrap();
        for misleading in [
            "heavy_tank",
            "tank name",
            "nation-vehicle_",
            "nation-vehicle_999999",
        ] {
            let resolution = catalog.resolve(i64::from(entry.vehicle_id), Some(misleading));
            assert_eq!(resolution.state, VehicleClassResolutionState::Conflict);
            assert_eq!(resolution.vehicle_class, VehicleClass::Unknown);
        }
        let first = catalog.resolve(i64::from(entry.vehicle_id), None);
        let second = catalog.resolve(i64::from(entry.vehicle_id), None);
        assert_eq!(first, second);
    }

    #[test]
    fn active_map_excludes_zero_ids_and_cache_contains_only_processed_metadata() {
        let temp = TempDir::new().unwrap();
        let catalog = activate(temp.path(), 'a', 1_000);
        assert_eq!(catalog.entries.len(), 500);
        assert_eq!(catalog.object.invalid_zero_id_count, 9);
        assert!(!catalog.entries.contains_key(&0));
        let root_text = temp.path().to_string_lossy().into_owned();
        for directory in [cache_paths(temp.path()).0, cache_paths(temp.path()).1] {
            for entry in fs::read_dir(directory).unwrap() {
                let bytes = fs::read(entry.unwrap().path()).unwrap();
                let text = String::from_utf8(bytes).unwrap();
                assert!(!text.contains("list.xml"));
                assert!(!text.contains("scripts.pkg"));
                assert!(!text.contains(&root_text));
            }
        }
    }

    #[test]
    fn malformed_candidate_rules_reject_duplicates_bad_keys_and_missing_class_distribution() {
        let mut duplicate = candidate('a', 1_000);
        duplicate.entries[1].vehicle_id = duplicate.entries[0].vehicle_id;
        duplicate.entries.sort_by_key(|entry| entry.vehicle_id);
        assert!(validate_catalog_object(&duplicate).is_err());
        let mut bad_key = candidate('a', 1_000);
        bad_key.entries[0].canonical_vehicle_key = "Bad Key".into();
        assert!(validate_catalog_object(&bad_key).is_err());
        let mut missing_class = candidate('a', 1_000);
        for entry in &mut missing_class.entries {
            if entry.vehicle_class == VehicleClass::Spg {
                entry.vehicle_class = VehicleClass::LightTank;
            }
        }
        assert!(validate_catalog_object(&missing_class).is_err());
    }

    #[test]
    fn resolution_counts_are_per_vehicle_and_conflicts_are_not_hidden() {
        let temp = TempDir::new().unwrap();
        let catalog = activate(temp.path(), 'a', 1_000);
        let first = first_entry(&catalog.object);
        let vehicles = vec![
            (
                i64::from(first.vehicle_id),
                Some(first.canonical_vehicle_key.clone()),
            ),
            (i64::from(first.vehicle_id), Some("wrong".into())),
            (i64::from(u32::MAX), None),
        ];
        assert_eq!(resolution_counts(Some(&catalog), &vehicles), (1, 1));
    }

    #[test]
    fn ten_thousand_lookups_share_one_snapshot_and_are_fast_without_io() {
        let temp = TempDir::new().unwrap();
        let catalog = activate(temp.path(), 'a', 1_000);
        let ids = catalog.entries.keys().copied().collect::<Vec<_>>();
        fs::remove_dir_all(temp.path()).unwrap();
        let entries_ptr = Arc::as_ptr(&catalog.entries);
        let started = Instant::now();
        for index in 0..10_000 {
            let id = ids[index % ids.len()];
            black_box(catalog.resolve(i64::from(id), None));
        }
        assert_eq!(Arc::as_ptr(&catalog.entries), entries_ptr);
        assert!(started.elapsed().as_secs_f64() < 1.0);
    }

    #[test]
    fn status_contains_only_safe_codes_and_no_source_path() {
        let temp = TempDir::new().unwrap();
        let runtime = VehicleClassCatalogRuntime::load(temp.path().to_path_buf());
        runtime.refresh(None).unwrap();
        let serialized = serde_json::to_string(&runtime.status(3, 1, None)).unwrap();
        assert!(serialized.contains("no_client"));
        assert!(!serialized.contains(&temp.path().to_string_lossy().into_owned()));
        assert!(!serialized.contains("scripts.pkg"));
    }

    /// Known-answer test (FIPS 180-2 vectors) so a digest library bump cannot silently change
    /// the hex format or the values used for snapshot and catalog identities.
    #[test]
    fn sha256_hex_matches_known_answer_vectors() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
