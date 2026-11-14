use chrono::{NaiveDate, Utc};
use reqwest::{StatusCode, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

pub const XVM_WARGAMING_SOURCE_URL: &str =
    "https://static.modxvm.com/wn8-data-exp/json/wg/wn8exp.json";
pub const CACHE_SCHEMA_VERSION: u32 = 1;
pub const VALIDATOR_VERSION: u32 = 1;
const MAX_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;
const MIN_VEHICLES: usize = 500;
const MAX_VEHICLES: usize = 5_000;
const MAX_SKIPPED_ROWS_PERCENT: usize = 1;
const UPDATE_STATE_SCHEMA_VERSION: u32 = 1;
const UPDATE_CADENCE_MS: i64 = 24 * 60 * 60 * 1_000;
const RETRY_AFTER_FAILURE_MS: i64 = 60 * 60 * 1_000;
const STALE_LOCK_MS: i64 = 10 * 60 * 1_000;
const MAX_UPDATE_STATE_BYTES: usize = 16 * 1024;

static ACTIVE_UPDATE_ROOTS: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedValuesSource {
    XvmWargaming,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedValuesRow {
    pub vehicle_id: i64,
    pub exp_damage: f64,
    pub exp_frags: f64,
    pub exp_spots: f64,
    pub exp_defense: f64,
    pub exp_win_rate: f64,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotValidationStatus {
    Validated,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedSnapshotManifest {
    pub cache_schema_version: u32,
    pub validator_version: u32,
    pub source: ExpectedValuesSource,
    pub source_url: String,
    pub downloaded_at: i64,
    pub source_version: String,
    pub source_date: String,
    pub sha256: String,
    pub byte_length: u64,
    pub vehicle_count: usize,
    /// Rows skipped as invalid (M8); manifests written before this field read as 0.
    #[serde(default)]
    pub skipped_row_count: usize,
    pub validation_status: SnapshotValidationStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedExpectedSnapshot {
    pub manifest: ExpectedSnapshotManifest,
    pub rows: HashMap<i64, ExpectedValuesRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivationResult {
    pub snapshot: LoadedExpectedSnapshot,
    pub created_activation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedUpdateMode {
    Automatic,
    ManualForce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedUpdateOutcome {
    NotDue,
    Unchanged,
    Activated,
    OfflineUsingCurrent,
    RejectedUsingCurrent,
    Unavailable,
    Busy,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedUpdateErrorCode {
    Offline,
    HttpStatus,
    TooLarge,
    InvalidDataset,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedUpdateState {
    pub schema_version: u32,
    pub generation: u64,
    pub last_attempt_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_error_code: Option<ExpectedUpdateErrorCode>,
}

impl Default for ExpectedUpdateState {
    fn default() -> Self {
        Self {
            schema_version: UPDATE_STATE_SCHEMA_VERSION,
            generation: 0,
            last_attempt_at: None,
            last_success_at: None,
            last_error_code: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum ExpectedValuesError {
    #[error("expected-values snapshot is too large")]
    TooLarge,
    #[error("expected-values snapshot is not valid UTF-8 JSON")]
    InvalidJson,
    #[error("expected-values snapshot has an invalid header")]
    InvalidHeader,
    #[error("expected-values snapshot has an invalid row")]
    InvalidRow,
    #[error("expected-values snapshot contains a duplicate vehicle ID")]
    DuplicateVehicleId,
    #[error("expected-values snapshot vehicle count is outside safety bounds")]
    InvalidVehicleCount,
    #[error("expected-values snapshot is older than the active snapshot")]
    OlderVersion,
    #[error("expected-values snapshot reuses a version with different content")]
    VersionHashConflict,
    #[error("expected-values snapshot vehicle count dropped too far")]
    VehicleCountDrop,
    #[error("expected-values snapshot vehicle overlap dropped too far")]
    VehicleOverlapDrop,
    #[error("expected-values cache entry is invalid")]
    InvalidCacheEntry,
    #[error("expected-values filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("expected-values HTTP client could not be initialized")]
    HttpClient(#[from] reqwest::Error),
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Deserialize)]
struct RawDataset {
    header: RawHeader,
    data: Vec<Value>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Deserialize)]
struct RawHeader {
    source: String,
    version: String,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct RawRow {
    IDNum: i64,
    expDef: f64,
    expFrag: f64,
    expSpot: f64,
    expDamage: f64,
    expWinRate: f64,
}

fn sha256_hex(bytes: &[u8]) -> String {
    crate::hashing::hex_lower(Sha256::digest(bytes))
}

fn strict_date(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .is_ok_and(|date| date.format("%Y-%m-%d").to_string() == value)
}

fn valid_row(row: &RawRow) -> bool {
    let values = [
        row.expDef,
        row.expFrag,
        row.expSpot,
        row.expDamage,
        row.expWinRate,
    ];
    row.IDNum > 0
        && values.iter().all(|value| value.is_finite())
        && row.expDef > 0.0
        && row.expFrag > 0.0
        && row.expSpot > 0.0
        && row.expDamage > 0.0
        && 0.0 < row.expWinRate
        && row.expWinRate <= 100.0
}

fn parse_intrinsic(bytes: &[u8]) -> Result<LoadedExpectedSnapshot, ExpectedValuesError> {
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(ExpectedValuesError::TooLarge);
    }
    std::str::from_utf8(bytes).map_err(|_| ExpectedValuesError::InvalidJson)?;
    let raw: RawDataset =
        serde_json::from_slice(bytes).map_err(|_| ExpectedValuesError::InvalidJson)?;
    if raw.header.source != "XVM" || !strict_date(&raw.header.version) {
        return Err(ExpectedValuesError::InvalidHeader);
    }
    let total_rows = raw.data.len();
    if !(MIN_VEHICLES..=MAX_VEHICLES).contains(&total_rows) {
        return Err(ExpectedValuesError::InvalidVehicleCount);
    }

    // M8: a bad row is counted and skipped instead of rejecting the whole file, as long as bad
    // rows stay within MAX_SKIPPED_ROWS_PERCENT of the file.
    let mut rows = HashMap::with_capacity(total_rows);
    let mut skipped_rows = 0_usize;
    for value in raw.data {
        let Some(row) = serde_json::from_value::<RawRow>(value)
            .ok()
            .filter(valid_row)
        else {
            skipped_rows += 1;
            continue;
        };
        let parsed = ExpectedValuesRow {
            vehicle_id: row.IDNum,
            exp_damage: row.expDamage,
            exp_frags: row.expFrag,
            exp_spots: row.expSpot,
            exp_defense: row.expDef,
            exp_win_rate: row.expWinRate,
        };
        if rows.insert(parsed.vehicle_id, parsed).is_some() {
            return Err(ExpectedValuesError::DuplicateVehicleId);
        }
    }
    if skipped_rows * 100 > total_rows * MAX_SKIPPED_ROWS_PERCENT {
        return Err(ExpectedValuesError::InvalidRow);
    }
    if rows.len() < MIN_VEHICLES {
        return Err(ExpectedValuesError::InvalidVehicleCount);
    }

    let hash = sha256_hex(bytes);
    let version = raw.header.version;
    Ok(LoadedExpectedSnapshot {
        manifest: ExpectedSnapshotManifest {
            cache_schema_version: CACHE_SCHEMA_VERSION,
            validator_version: VALIDATOR_VERSION,
            source: ExpectedValuesSource::XvmWargaming,
            source_url: XVM_WARGAMING_SOURCE_URL.to_owned(),
            downloaded_at: 0,
            source_version: version.clone(),
            source_date: version,
            sha256: hash,
            byte_length: bytes.len() as u64,
            vehicle_count: rows.len(),
            skipped_row_count: skipped_rows,
            validation_status: SnapshotValidationStatus::Validated,
        },
        rows,
    })
}

pub fn validate_snapshot_bytes(
    bytes: &[u8],
    current: Option<&LoadedExpectedSnapshot>,
) -> Result<LoadedExpectedSnapshot, ExpectedValuesError> {
    let candidate = parse_intrinsic(bytes)?;
    if let Some(current) = current {
        if candidate.manifest.source_version < current.manifest.source_version {
            return Err(ExpectedValuesError::OlderVersion);
        }
        if candidate.manifest.source_version == current.manifest.source_version
            && candidate.manifest.sha256 != current.manifest.sha256
        {
            return Err(ExpectedValuesError::VersionHashConflict);
        }
        if candidate.manifest.vehicle_count * 100 < current.manifest.vehicle_count * 90 {
            return Err(ExpectedValuesError::VehicleCountDrop);
        }
        let common = current
            .rows
            .keys()
            .filter(|id| candidate.rows.contains_key(id))
            .count();
        if common * 100 < current.rows.len() * 90 {
            return Err(ExpectedValuesError::VehicleOverlapDrop);
        }
    }
    Ok(candidate)
}

fn cache_paths(cache_root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    (
        cache_root.join("objects"),
        cache_root.join("activations"),
        cache_root.join("tmp"),
    )
}

fn create_synced(path: &Path, bytes: &[u8]) -> Result<(), ExpectedValuesError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

fn read_limited(path: &Path, limit: usize) -> Result<Vec<u8>, ExpectedValuesError> {
    let file = File::open(path)?;
    if file.metadata()?.len() > limit as u64 {
        return Err(ExpectedValuesError::InvalidCacheEntry);
    }
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(ExpectedValuesError::InvalidCacheEntry);
    }
    Ok(bytes)
}

fn valid_manifest_shape(manifest: &ExpectedSnapshotManifest) -> bool {
    manifest.cache_schema_version == CACHE_SCHEMA_VERSION
        && manifest.validator_version == VALIDATOR_VERSION
        && manifest.source == ExpectedValuesSource::XvmWargaming
        && manifest.source_url == XVM_WARGAMING_SOURCE_URL
        && strict_date(&manifest.source_version)
        && manifest.source_date == manifest.source_version
        && manifest.sha256.len() == 64
        && manifest
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        && (MIN_VEHICLES..=MAX_VEHICLES).contains(&manifest.vehicle_count)
        && manifest.validation_status == SnapshotValidationStatus::Validated
}

fn activation_files(cache_root: &Path) -> Result<Vec<PathBuf>, ExpectedValuesError> {
    let activations = cache_paths(cache_root).1;
    if !activations.exists() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(activations)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    Ok(paths)
}

fn load_activation(
    cache_root: &Path,
    activation: &Path,
) -> Result<LoadedExpectedSnapshot, ExpectedValuesError> {
    let manifest_bytes = read_limited(activation, 64 * 1024)?;
    let manifest: ExpectedSnapshotManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| ExpectedValuesError::InvalidCacheEntry)?;
    if !valid_manifest_shape(&manifest) {
        return Err(ExpectedValuesError::InvalidCacheEntry);
    }
    let object_path = cache_paths(cache_root)
        .0
        .join(format!("{}.json", manifest.sha256));
    let object_bytes = read_limited(&object_path, MAX_SNAPSHOT_BYTES)?;
    if object_bytes.len() as u64 != manifest.byte_length
        || sha256_hex(&object_bytes) != manifest.sha256
    {
        return Err(ExpectedValuesError::InvalidCacheEntry);
    }
    let mut snapshot = parse_intrinsic(&object_bytes)?;
    if snapshot.manifest.source_version != manifest.source_version
        || snapshot.manifest.vehicle_count != manifest.vehicle_count
    {
        return Err(ExpectedValuesError::InvalidCacheEntry);
    }
    snapshot.manifest = manifest;
    Ok(snapshot)
}

pub fn load_current_snapshot(
    cache_root: &Path,
) -> Result<Option<LoadedExpectedSnapshot>, ExpectedValuesError> {
    let Some(path) = activation_files(cache_root)?.into_iter().next() else {
        return Ok(None);
    };
    load_activation(cache_root, &path).map(Some)
}

pub fn load_current_or_previous_snapshot(
    cache_root: &Path,
) -> Result<Option<LoadedExpectedSnapshot>, ExpectedValuesError> {
    for path in activation_files(cache_root)? {
        if let Ok(snapshot) = load_activation(cache_root, &path) {
            return Ok(Some(snapshot));
        }
    }
    Ok(None)
}

pub fn activate_snapshot_bytes(
    cache_root: &Path,
    bytes: &[u8],
    downloaded_at: i64,
) -> Result<ActivationResult, ExpectedValuesError> {
    let current = load_current_or_previous_snapshot(cache_root)?;
    let mut candidate = validate_snapshot_bytes(bytes, current.as_ref())?;
    if current.as_ref().is_some_and(|snapshot| {
        snapshot.manifest.source_version == candidate.manifest.source_version
            && snapshot.manifest.sha256 == candidate.manifest.sha256
    }) {
        return Ok(ActivationResult {
            snapshot: current.expect("checked as present"),
            created_activation: false,
        });
    }
    candidate.manifest.downloaded_at = downloaded_at;

    let (objects, activations, tmp) = cache_paths(cache_root);
    fs::create_dir_all(&objects)?;
    fs::create_dir_all(&activations)?;
    fs::create_dir_all(&tmp)?;

    let nonce = Uuid::new_v4();
    let temp_path = tmp.join(format!("{nonce}.part"));
    create_synced(&temp_path, bytes)?;

    let object_path = objects.join(format!("{}.json", candidate.manifest.sha256));
    if object_path.exists() {
        let existing = read_limited(&object_path, MAX_SNAPSHOT_BYTES)?;
        if existing != bytes {
            let _ = fs::remove_file(&temp_path);
            return Err(ExpectedValuesError::InvalidCacheEntry);
        }
    } else if let Err(error) = create_synced(&object_path, bytes) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    fs::remove_file(&temp_path)?;

    let activation_name = format!(
        "{:020}-{}-{}.json",
        downloaded_at, nonce, candidate.manifest.sha256
    );
    let activation_path = activations.join(activation_name);
    let manifest_bytes = serde_json::to_vec_pretty(&candidate.manifest)
        .map_err(|_| ExpectedValuesError::InvalidCacheEntry)?;
    create_synced(&activation_path, &manifest_bytes)?;

    Ok(ActivationResult {
        snapshot: candidate,
        created_activation: true,
    })
}

fn update_state_dir(cache_root: &Path) -> PathBuf {
    cache_root.join("update-state")
}

fn update_state_files(cache_root: &Path) -> Result<Vec<PathBuf>, ExpectedValuesError> {
    let state_dir = update_state_dir(cache_root);
    if !state_dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(state_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    Ok(paths)
}

pub fn load_expected_update_state(
    cache_root: &Path,
) -> Result<ExpectedUpdateState, ExpectedValuesError> {
    for path in update_state_files(cache_root)? {
        let Ok(bytes) = read_limited(&path, MAX_UPDATE_STATE_BYTES) else {
            continue;
        };
        let Ok(state) = serde_json::from_slice::<ExpectedUpdateState>(&bytes) else {
            continue;
        };
        let expected_prefix = format!("{:020}-", state.generation);
        if state.schema_version == UPDATE_STATE_SCHEMA_VERSION
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with(&expected_prefix))
        {
            return Ok(state);
        }
    }
    Ok(ExpectedUpdateState::default())
}

fn persist_update_state(
    cache_root: &Path,
    state: &ExpectedUpdateState,
) -> Result<ExpectedUpdateState, ExpectedValuesError> {
    let mut next = state.clone();
    next.generation = next
        .generation
        .checked_add(1)
        .ok_or(ExpectedValuesError::InvalidCacheEntry)?;
    let bytes =
        serde_json::to_vec_pretty(&next).map_err(|_| ExpectedValuesError::InvalidCacheEntry)?;
    let (_, _, tmp) = cache_paths(cache_root);
    let state_dir = update_state_dir(cache_root);
    fs::create_dir_all(&tmp)?;
    fs::create_dir_all(&state_dir)?;
    let nonce = Uuid::new_v4();
    let temp_path = tmp.join(format!("update-state-{nonce}.part"));
    create_synced(&temp_path, &bytes)?;
    let final_path = state_dir.join(format!("{:020}-{nonce}.json", next.generation));
    let result = create_synced(&final_path, &bytes);
    let _ = fs::remove_file(&temp_path);
    result?;
    Ok(next)
}

fn absolute_lock_key(cache_root: &Path) -> Result<PathBuf, ExpectedValuesError> {
    if cache_root.is_absolute() {
        Ok(cache_root.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(cache_root))
    }
}

struct ProcessUpdateLock {
    key: PathBuf,
}

impl ProcessUpdateLock {
    fn try_acquire(cache_root: &Path) -> Result<Option<Self>, ExpectedValuesError> {
        let key = absolute_lock_key(cache_root)?;
        let mut active = ACTIVE_UPDATE_ROOTS
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
        ACTIVE_UPDATE_ROOTS
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

struct FileUpdateLock {
    path: PathBuf,
    bytes: Vec<u8>,
}

fn lock_is_stale(bytes: &[u8], now: i64, modified_at: Option<i64>) -> bool {
    match serde_json::from_slice::<FileLockRecord>(bytes) {
        Ok(lock) => now.saturating_sub(lock.created_at) >= STALE_LOCK_MS,
        Err(_) => modified_at.is_some_and(|modified| now.saturating_sub(modified) >= STALE_LOCK_MS),
    }
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

fn clear_stale_reclaim(cache_root: &Path, now: i64) -> Result<bool, ExpectedValuesError> {
    let path = cache_root.join("update.lock.reclaim");
    if !path.exists() {
        return Ok(true);
    }
    let existing = match read_limited(&path, MAX_UPDATE_STATE_BYTES) {
        Ok(existing) => existing,
        Err(_) => return Ok(false),
    };
    if !lock_is_stale(&existing, now, modified_at_millis(&path))
        || fs::read(&path).ok().as_deref() != Some(existing.as_slice())
    {
        return Ok(false);
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error.into()),
    }
}

struct ReclaimLock {
    path: PathBuf,
    bytes: Vec<u8>,
}

impl ReclaimLock {
    fn try_acquire(cache_root: &Path, now: i64) -> Result<Option<Self>, ExpectedValuesError> {
        let path = cache_root.join("update.lock.reclaim");
        let bytes = serde_json::to_vec(&FileLockRecord {
            token: Uuid::new_v4().to_string(),
            created_at: now,
        })
        .map_err(|_| ExpectedValuesError::InvalidCacheEntry)?;
        match create_synced(&path, &bytes) {
            Ok(()) => Ok(Some(Self { path, bytes })),
            Err(ExpectedValuesError::Io(error))
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

impl FileUpdateLock {
    fn try_acquire(cache_root: &Path, now: i64) -> Result<Option<Self>, ExpectedValuesError> {
        fs::create_dir_all(cache_root)?;
        let path = cache_root.join("update.lock");
        if !clear_stale_reclaim(cache_root, now)? {
            return Ok(None);
        }
        let record = FileLockRecord {
            token: Uuid::new_v4().to_string(),
            created_at: now,
        };
        let bytes =
            serde_json::to_vec(&record).map_err(|_| ExpectedValuesError::InvalidCacheEntry)?;
        match create_synced(&path, &bytes) {
            Ok(()) => Ok(Some(Self { path, bytes })),
            Err(ExpectedValuesError::Io(error))
                if error.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                let existing = match read_limited(&path, MAX_UPDATE_STATE_BYTES) {
                    Ok(existing) => existing,
                    Err(_) => return Ok(None),
                };
                if !lock_is_stale(&existing, now, modified_at_millis(&path)) {
                    return Ok(None);
                }
                let Some(_reclaim_lock) = ReclaimLock::try_acquire(cache_root, now)? else {
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

struct DownloadTemp {
    path: PathBuf,
    file: File,
}

impl DownloadTemp {
    fn create(cache_root: &Path) -> Result<Self, ExpectedValuesError> {
        let tmp = cache_paths(cache_root).2;
        fs::create_dir_all(&tmp)?;
        let path = tmp.join(format!("download-{}.part", Uuid::new_v4()));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok(Self { path, file })
    }
}

impl Drop for DownloadTemp {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Clone)]
struct HttpConfig {
    url: String,
    https_only: bool,
    connect_timeout: Duration,
    total_timeout: Duration,
}

enum DownloadProblem {
    Offline,
    Rejected(ExpectedUpdateErrorCode),
    Local(ExpectedValuesError),
}

async fn download_snapshot(
    cache_root: &Path,
    config: &HttpConfig,
) -> Result<Vec<u8>, DownloadProblem> {
    let client = reqwest::Client::builder()
        .https_only(config.https_only)
        .redirect(Policy::none())
        .connect_timeout(config.connect_timeout)
        .timeout(config.total_timeout)
        .user_agent("Afterbattle/0.1")
        .build()
        .map_err(ExpectedValuesError::from)
        .map_err(DownloadProblem::Local)?;
    let mut response = client
        .get(&config.url)
        .send()
        .await
        .map_err(|_| DownloadProblem::Offline)?;
    if response.status() != StatusCode::OK {
        return Err(DownloadProblem::Rejected(
            ExpectedUpdateErrorCode::HttpStatus,
        ));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_SNAPSHOT_BYTES as u64)
    {
        return Err(DownloadProblem::Rejected(ExpectedUpdateErrorCode::TooLarge));
    }

    let mut temp = DownloadTemp::create(cache_root).map_err(DownloadProblem::Local)?;
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or(0)
            .min(MAX_SNAPSHOT_BYTES as u64) as usize,
    );
    let mut hasher = Sha256::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| DownloadProblem::Offline)?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_SNAPSHOT_BYTES {
            return Err(DownloadProblem::Rejected(ExpectedUpdateErrorCode::TooLarge));
        }
        temp.file
            .write_all(&chunk)
            .map_err(ExpectedValuesError::from)
            .map_err(DownloadProblem::Local)?;
        hasher.update(&chunk);
        bytes.extend_from_slice(&chunk);
    }
    temp.file
        .sync_all()
        .map_err(ExpectedValuesError::from)
        .map_err(DownloadProblem::Local)?;
    let _download_sha256 = crate::hashing::hex_lower(hasher.finalize());
    Ok(bytes)
}

fn validation_error(error: &ExpectedValuesError) -> bool {
    matches!(
        error,
        ExpectedValuesError::TooLarge
            | ExpectedValuesError::InvalidJson
            | ExpectedValuesError::InvalidHeader
            | ExpectedValuesError::InvalidRow
            | ExpectedValuesError::DuplicateVehicleId
            | ExpectedValuesError::InvalidVehicleCount
            | ExpectedValuesError::OlderVersion
            | ExpectedValuesError::VersionHashConflict
            | ExpectedValuesError::VehicleCountDrop
            | ExpectedValuesError::VehicleOverlapDrop
    )
}

/// D3: an automatic download is due 24 h after a successful attempt and 1 h after any other
/// attempt (an error, or a marker left by a download that never finished).
fn automatic_update_due(state: &ExpectedUpdateState, now: i64) -> bool {
    let Some(last) = state.last_attempt_at else {
        return true;
    };
    let succeeded = state.last_error_code.is_none() && state.last_success_at == Some(last);
    let wait = if succeeded {
        UPDATE_CADENCE_MS
    } else {
        RETRY_AFTER_FAILURE_MS
    };
    now.saturating_sub(last) >= wait
}

fn failure_outcome(has_current: bool, offline: bool) -> ExpectedUpdateOutcome {
    match (has_current, offline) {
        (true, true) => ExpectedUpdateOutcome::OfflineUsingCurrent,
        (true, false) => ExpectedUpdateOutcome::RejectedUsingCurrent,
        (false, _) => ExpectedUpdateOutcome::Unavailable,
    }
}

async fn update_expected_values_with(
    cache_root: &Path,
    mode: ExpectedUpdateMode,
    now: i64,
    config: &HttpConfig,
) -> Result<ExpectedUpdateOutcome, ExpectedValuesError> {
    let current = load_current_or_previous_snapshot(cache_root)?;
    let mut state = load_expected_update_state(cache_root)?;
    let Some(_process_lock) = ProcessUpdateLock::try_acquire(cache_root)? else {
        return Ok(ExpectedUpdateOutcome::Busy);
    };
    if mode == ExpectedUpdateMode::Automatic && !automatic_update_due(&state, now) {
        return Ok(ExpectedUpdateOutcome::NotDue);
    }

    let Some(_file_lock) = FileUpdateLock::try_acquire(cache_root, now)? else {
        return Ok(ExpectedUpdateOutcome::Busy);
    };

    // D3: the attempt marker is written once the attempt has ended, with its result.
    state.last_attempt_at = Some(now);
    let bytes = match download_snapshot(cache_root, config).await {
        Ok(bytes) => bytes,
        Err(DownloadProblem::Local(error)) => return Err(error),
        Err(DownloadProblem::Offline) => {
            state.last_error_code = Some(ExpectedUpdateErrorCode::Offline);
            let _ = persist_update_state(cache_root, &state)?;
            return Ok(failure_outcome(current.is_some(), true));
        }
        Err(DownloadProblem::Rejected(code)) => {
            state.last_error_code = Some(code);
            let _ = persist_update_state(cache_root, &state)?;
            return Ok(failure_outcome(current.is_some(), false));
        }
    };

    let activation = match activate_snapshot_bytes(cache_root, &bytes, now) {
        Ok(activation) => activation,
        Err(error) if validation_error(&error) => {
            state.last_error_code = Some(ExpectedUpdateErrorCode::InvalidDataset);
            let _ = persist_update_state(cache_root, &state)?;
            return Ok(failure_outcome(current.is_some(), false));
        }
        Err(error) => return Err(error),
    };

    state.last_success_at = Some(now);
    state.last_error_code = None;
    let _ = persist_update_state(cache_root, &state)?;
    Ok(if activation.created_activation {
        ExpectedUpdateOutcome::Activated
    } else {
        ExpectedUpdateOutcome::Unchanged
    })
}

pub async fn update_expected_values(
    cache_root: &Path,
    mode: ExpectedUpdateMode,
) -> Result<ExpectedUpdateOutcome, ExpectedValuesError> {
    let parsed = reqwest::Url::parse(XVM_WARGAMING_SOURCE_URL)
        .map_err(|_| ExpectedValuesError::InvalidCacheEntry)?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("static.modxvm.com")
        || parsed.path() != "/wn8-data-exp/json/wg/wn8exp.json"
        || parsed.query().is_some()
    {
        return Err(ExpectedValuesError::InvalidCacheEntry);
    }
    update_expected_values_with(
        cache_root,
        mode,
        Utc::now().timestamp_millis(),
        &HttpConfig {
            url: XVM_WARGAMING_SOURCE_URL.to_owned(),
            https_only: true,
            connect_timeout: Duration::from_secs(5),
            total_timeout: Duration::from_secs(15),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };
    use tempfile::TempDir;

    struct MockServer {
        url: String,
        requests: Arc<AtomicUsize>,
    }

    impl MockServer {
        fn response(
            status: &str,
            body: Vec<u8>,
            delay: Duration,
            headers: &[(&str, String)],
            include_content_length: bool,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let requests = Arc::new(AtomicUsize::new(0));
            let thread_requests = Arc::clone(&requests);
            let status = status.to_owned();
            let headers = headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), value.clone()))
                .collect::<Vec<_>>();
            thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                thread_requests.fetch_add(1, Ordering::SeqCst);
                thread::sleep(delay);
                let mut response = format!("HTTP/1.1 {status}\r\nConnection: close\r\n");
                if include_content_length {
                    response.push_str(&format!("Content-Length: {}\r\n", body.len()));
                }
                for (name, value) in headers {
                    response.push_str(&format!("{name}: {value}\r\n"));
                }
                response.push_str("\r\n");
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&body);
            });
            Self {
                url: format!("http://{address}/wn8exp.json"),
                requests,
            }
        }

        fn ok(body: Vec<u8>) -> Self {
            Self::response("200 OK", body, Duration::ZERO, &[], true)
        }

        /// A server that reports each received request on the first channel and answers it with
        /// `200 OK` only after a message arrives on the returned sender.
        fn gated(
            body: Vec<u8>,
        ) -> (
            Self,
            std::sync::mpsc::Receiver<()>,
            std::sync::mpsc::Sender<()>,
        ) {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let requests = Arc::new(AtomicUsize::new(0));
            let thread_requests = Arc::clone(&requests);
            let (received_tx, received_rx) = std::sync::mpsc::channel();
            let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
            thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                thread_requests.fetch_add(1, Ordering::SeqCst);
                let _ = received_tx.send(());
                if release_rx.recv().is_err() {
                    return;
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(&body);
            });
            (
                Self {
                    url: format!("http://{address}/wn8exp.json"),
                    requests,
                },
                received_rx,
                release_tx,
            )
        }
    }

    fn http_config(url: &str) -> HttpConfig {
        HttpConfig {
            url: url.to_owned(),
            https_only: false,
            connect_timeout: Duration::from_millis(200),
            total_timeout: Duration::from_secs(2),
        }
    }

    fn update(
        root: &Path,
        mode: ExpectedUpdateMode,
        now: i64,
        config: &HttpConfig,
    ) -> ExpectedUpdateOutcome {
        tauri::async_runtime::block_on(update_expected_values_with(root, mode, now, config))
            .unwrap()
    }

    fn unavailable_url() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        format!("http://{address}/wn8exp.json")
    }

    fn row(id: i64) -> Value {
        json!({
            "IDNum": id,
            "expDef": 0.5,
            "expFrag": 0.8,
            "expSpot": 1.1,
            "expDamage": 900.0,
            "expWinRate": 49.5
        })
    }

    fn dataset(version: &str, ids: impl IntoIterator<Item = i64>) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "header": {"source": "XVM", "version": version},
            "data": ids.into_iter().map(row).collect::<Vec<_>>()
        }))
        .unwrap()
    }

    fn ids(start: i64, count: usize) -> std::ops::Range<i64> {
        start..start + count as i64
    }

    fn valid() -> Vec<u8> {
        let mut all = ids(1, 499).collect::<Vec<_>>();
        all.push(33345);
        dataset("2026-07-10", all)
    }

    fn activate(root: &Path, bytes: &[u8], timestamp: i64) -> ActivationResult {
        activate_snapshot_bytes(root, bytes, timestamp).unwrap()
    }

    #[test]
    fn activates_and_loads_valid_dataset_with_exact_vehicle_and_defense() {
        let temp = TempDir::new().unwrap();
        let result = activate(temp.path(), &valid(), 1_000);
        assert!(result.created_activation);
        let loaded = load_current_snapshot(temp.path()).unwrap().unwrap();
        assert_eq!(loaded.rows.keys().filter(|id| **id == 33345).count(), 1);
        assert_eq!(loaded.rows[&33345].exp_defense, 0.5);
    }

    /// P4 (2026-11-12): the live XVM file puts `header` after `data` and adds a `url` field.
    #[test]
    fn header_after_data_with_extra_fields_is_accepted() {
        let rows = ids(1, 999)
            .chain(std::iter::once(72193))
            .map(row)
            .map(|row| serde_json::to_string(&row).unwrap())
            .collect::<Vec<_>>()
            .join(",");
        let text = format!(
            "{{\"data\":[{rows}],\"header\":{{\"url\":\"https://example.invalid/wn8exp-2026-11-11.json\",\"source\":\"XVM\",\"version\":\"2026-11-11\"}}}}"
        );
        let loaded = validate_snapshot_bytes(text.as_bytes(), None).unwrap();
        assert_eq!(loaded.manifest.source_version, "2026-11-11");
        assert_eq!(loaded.manifest.vehicle_count, 1_000);
        assert!(loaded.rows.contains_key(&1));
        assert!(loaded.rows.contains_key(&72193));
    }

    /// 1000 rows with the given rows broken; a bad row is skipped and counted (M8).
    fn with_broken_rows(broken: &[(usize, &str, Option<Value>)]) -> Vec<u8> {
        let mut parsed: Value = serde_json::from_slice(&dataset("2026-07-10", ids(1, 1_000)))
            .expect("synthetic dataset");
        for (index, field, value) in broken {
            let row = parsed["data"][*index].as_object_mut().unwrap();
            match value {
                Some(value) => {
                    row.insert((*field).to_owned(), value.clone());
                }
                None => {
                    row.remove(*field);
                }
            }
        }
        serde_json::to_vec(&parsed).unwrap()
    }

    #[test]
    fn a_row_missing_any_required_field_is_skipped_and_counted() {
        for field in [
            "IDNum",
            "expDef",
            "expFrag",
            "expSpot",
            "expDamage",
            "expWinRate",
        ] {
            let loaded = validate_snapshot_bytes(&with_broken_rows(&[(0, field, None)]), None)
                .unwrap_or_else(|error| panic!("{field}: {error}"));
            assert_eq!(loaded.manifest.skipped_row_count, 1, "{field}");
            assert_eq!(loaded.manifest.vehicle_count, 999, "{field}");
            assert!(!loaded.rows.contains_key(&1), "{field}");
        }
    }

    #[test]
    fn duplicate_ids_reject_the_file_and_non_positive_ids_are_skipped() {
        let duplicate = dataset("2026-07-10", ids(1, 500).chain(std::iter::once(1)));
        assert!(matches!(
            validate_snapshot_bytes(&duplicate, None),
            Err(ExpectedValuesError::DuplicateVehicleId)
        ));
        for id in [0, -1] {
            let loaded =
                validate_snapshot_bytes(&with_broken_rows(&[(0, "IDNum", Some(json!(id)))]), None)
                    .unwrap();
            assert_eq!(loaded.manifest.skipped_row_count, 1);
            assert!(!loaded.rows.contains_key(&id));
        }
    }

    #[test]
    fn each_invalid_numeric_value_skips_only_its_row() {
        for (field, value) in [
            ("expDef", json!(0)),
            ("expFrag", json!(-1)),
            ("expSpot", json!(0)),
            ("expDamage", json!(-1)),
            ("expWinRate", json!(0)),
            ("expWinRate", json!(101)),
            ("expDamage", json!("900")),
            ("IDNum", json!(1.5)),
        ] {
            let loaded = validate_snapshot_bytes(
                &with_broken_rows(&[(3, field, Some(value.clone()))]),
                None,
            )
            .unwrap_or_else(|error| panic!("{field}={value}: {error}"));
            assert_eq!(loaded.manifest.skipped_row_count, 1, "{field}={value}");
            assert!(!loaded.rows.contains_key(&4), "{field}={value}");
            assert!(loaded.rows.contains_key(&3), "{field}={value}");
        }
        // Boundary values that stay valid.
        let edge = with_broken_rows(&[(0, "expWinRate", Some(json!(100)))]);
        assert_eq!(
            validate_snapshot_bytes(&edge, None)
                .unwrap()
                .manifest
                .skipped_row_count,
            0
        );
        let non_finite = String::from_utf8(valid())
            .unwrap()
            .replace("900.0", "1e9999");
        assert!(validate_snapshot_bytes(non_finite.as_bytes(), None).is_err());
    }

    #[test]
    fn bad_rows_above_one_percent_reject_the_file() {
        let at_limit = (0..10)
            .map(|index| (index, "expDef", Some(json!(0))))
            .collect::<Vec<_>>();
        let loaded = validate_snapshot_bytes(&with_broken_rows(&at_limit), None).unwrap();
        assert_eq!(loaded.manifest.skipped_row_count, 10);
        assert_eq!(loaded.manifest.vehicle_count, 990);
        let above = (0..11)
            .map(|index| (index, "expDef", Some(json!(0))))
            .collect::<Vec<_>>();
        assert!(matches!(
            validate_snapshot_bytes(&with_broken_rows(&above), None),
            Err(ExpectedValuesError::InvalidRow)
        ));
    }

    #[test]
    fn skipping_below_the_minimum_vehicle_count_rejects_the_file() {
        let mut parsed: Value = serde_json::from_slice(&valid()).unwrap();
        parsed["data"][0]["expDef"] = json!(0);
        assert!(matches!(
            validate_snapshot_bytes(&serde_json::to_vec(&parsed).unwrap(), None),
            Err(ExpectedValuesError::InvalidVehicleCount)
        ));
    }

    #[test]
    fn a_snapshot_with_skipped_rows_survives_a_restart() {
        let temp = TempDir::new().unwrap();
        let bytes = with_broken_rows(&[(0, "expDef", Some(json!(0)))]);
        activate(temp.path(), &bytes, 1_000);
        let loaded = load_current_snapshot(temp.path()).unwrap().unwrap();
        assert_eq!(loaded.manifest.vehicle_count, 999);
        assert_eq!(loaded.manifest.skipped_row_count, 1);
    }

    #[test]
    fn enforces_vehicle_count_bounds() {
        assert!(matches!(
            validate_snapshot_bytes(&dataset("2026-07-10", ids(1, 499)), None),
            Err(ExpectedValuesError::InvalidVehicleCount)
        ));
        assert!(validate_snapshot_bytes(&dataset("2026-07-10", ids(1, 500)), None).is_ok());
        assert!(validate_snapshot_bytes(&dataset("2026-07-10", ids(1, 5_000)), None).is_ok());
        assert!(matches!(
            validate_snapshot_bytes(&dataset("2026-07-10", ids(1, 5_001)), None),
            Err(ExpectedValuesError::InvalidVehicleCount)
        ));
    }

    #[test]
    fn rejects_older_version_and_version_hash_conflict() {
        let current = validate_snapshot_bytes(&dataset("2026-07-10", ids(1, 600)), None).unwrap();
        assert!(matches!(
            validate_snapshot_bytes(&dataset("2026-07-09", ids(1, 600)), Some(&current)),
            Err(ExpectedValuesError::OlderVersion)
        ));
        assert!(matches!(
            validate_snapshot_bytes(&dataset("2026-07-10", ids(2, 600)), Some(&current)),
            Err(ExpectedValuesError::VersionHashConflict)
        ));
    }

    #[test]
    fn enforces_count_drop_and_overlap_thresholds() {
        let current = validate_snapshot_bytes(&dataset("2026-07-10", ids(1, 1_000)), None).unwrap();
        assert!(
            validate_snapshot_bytes(&dataset("2026-07-11", ids(1, 910)), Some(&current)).is_ok()
        );
        assert!(matches!(
            validate_snapshot_bytes(&dataset("2026-07-11", ids(1, 890)), Some(&current)),
            Err(ExpectedValuesError::VehicleCountDrop)
        ));
        let low_overlap = dataset("2026-07-11", ids(102, 1_000));
        assert!(matches!(
            validate_snapshot_bytes(&low_overlap, Some(&current)),
            Err(ExpectedValuesError::VehicleOverlapDrop)
        ));
    }

    #[test]
    fn identical_version_and_hash_is_not_activated_twice() {
        let temp = TempDir::new().unwrap();
        let bytes = valid();
        activate(temp.path(), &bytes, 1_000);
        let second = activate(temp.path(), &bytes, 2_000);
        assert!(!second.created_activation);
        assert_eq!(activation_files(temp.path()).unwrap().len(), 1);
    }

    #[test]
    fn orphan_object_is_ignored() {
        let temp = TempDir::new().unwrap();
        let objects = cache_paths(temp.path()).0;
        fs::create_dir_all(&objects).unwrap();
        create_synced(
            &objects.join(format!("{}.json", sha256_hex(&valid()))),
            &valid(),
        )
        .unwrap();
        assert!(
            load_current_or_previous_snapshot(temp.path())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn corrupt_newest_activation_falls_back_to_previous() {
        let temp = TempDir::new().unwrap();
        let first = dataset("2026-07-10", ids(1, 500));
        activate(temp.path(), &first, 1_000);
        activate(temp.path(), &dataset("2026-07-11", ids(1, 500)), 2_000);
        let newest = activation_files(temp.path()).unwrap().remove(0);
        fs::write(newest, b"not json").unwrap();
        let loaded = load_current_or_previous_snapshot(temp.path())
            .unwrap()
            .unwrap();
        assert_eq!(loaded.manifest.source_version, "2026-07-10");
    }

    #[test]
    fn corrupt_current_object_falls_back_to_previous() {
        let temp = TempDir::new().unwrap();
        activate(temp.path(), &dataset("2026-07-10", ids(1, 500)), 1_000);
        let current = activate(temp.path(), &dataset("2026-07-11", ids(1, 500)), 2_000);
        fs::write(
            cache_paths(temp.path())
                .0
                .join(format!("{}.json", current.snapshot.manifest.sha256)),
            b"corrupt",
        )
        .unwrap();
        let loaded = load_current_or_previous_snapshot(temp.path())
            .unwrap()
            .unwrap();
        assert_eq!(loaded.manifest.source_version, "2026-07-10");
    }

    #[test]
    fn no_snapshots_is_unavailable() {
        let temp = TempDir::new().unwrap();
        assert!(load_current_snapshot(temp.path()).unwrap().is_none());
        assert!(
            load_current_or_previous_snapshot(temp.path())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn restart_load_preserves_hash_version_and_rows() {
        let temp = TempDir::new().unwrap();
        let activated = activate(temp.path(), &valid(), 1_000).snapshot;
        let loaded = load_current_or_previous_snapshot(temp.path())
            .unwrap()
            .unwrap();
        assert_eq!(loaded, activated);
    }

    #[test]
    fn invalid_activation_does_not_change_current() {
        let temp = TempDir::new().unwrap();
        let first = activate(temp.path(), &dataset("2026-07-10", ids(1, 600)), 1_000).snapshot;
        let invalid = dataset("2026-07-11", ids(1, 500));
        assert!(activate_snapshot_bytes(temp.path(), &invalid, 2_000).is_err());
        assert_eq!(
            load_current_or_previous_snapshot(temp.path())
                .unwrap()
                .unwrap(),
            first
        );
    }

    #[test]
    fn existing_final_file_is_never_overwritten() {
        let temp = TempDir::new().unwrap();
        let bytes = valid();
        let hash = sha256_hex(&bytes);
        let objects = cache_paths(temp.path()).0;
        fs::create_dir_all(&objects).unwrap();
        let object = objects.join(format!("{hash}.json"));
        fs::write(&object, b"different").unwrap();
        assert!(activate_snapshot_bytes(temp.path(), &bytes, 1_000).is_err());
        assert_eq!(fs::read(object).unwrap(), b"different");
    }

    #[test]
    fn invalid_utf8_oversize_and_header_are_rejected() {
        assert!(matches!(
            validate_snapshot_bytes(&[0xff], None),
            Err(ExpectedValuesError::InvalidJson)
        ));
        assert!(matches!(
            validate_snapshot_bytes(&vec![b' '; MAX_SNAPSHOT_BYTES + 1], None),
            Err(ExpectedValuesError::TooLarge)
        ));
        let wrong_source = dataset("2026-07-10", ids(1, 500))
            .into_iter()
            .collect::<Vec<_>>();
        let mut value: Value = serde_json::from_slice(&wrong_source).unwrap();
        value["header"]["source"] = json!("OTHER");
        assert!(matches!(
            validate_snapshot_bytes(&serde_json::to_vec(&value).unwrap(), None),
            Err(ExpectedValuesError::InvalidHeader)
        ));
        assert!(matches!(
            validate_snapshot_bytes(&dataset("2026-7-10", ids(1, 500)), None),
            Err(ExpectedValuesError::InvalidHeader)
        ));
    }

    #[test]
    fn http_200_activates_and_identical_bytes_are_unchanged() {
        let temp = TempDir::new().unwrap();
        let bytes = valid();
        let first = MockServer::ok(bytes.clone());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::ManualForce,
                1_000,
                &http_config(&first.url)
            ),
            ExpectedUpdateOutcome::Activated
        );
        let second = MockServer::ok(bytes);
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::ManualForce,
                2_000,
                &http_config(&second.url)
            ),
            ExpectedUpdateOutcome::Unchanged
        );
        assert_eq!(activation_files(temp.path()).unwrap().len(), 1);
    }

    #[test]
    fn automatic_honors_persisted_cadence_and_manual_force_bypasses_it() {
        let temp = TempDir::new().unwrap();
        let first = MockServer::ok(valid());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                10_000,
                &http_config(&first.url)
            ),
            ExpectedUpdateOutcome::Activated
        );
        let unreachable = http_config(&unavailable_url());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                20_000,
                &unreachable
            ),
            ExpectedUpdateOutcome::NotDue
        );
        let forced = MockServer::ok(valid());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::ManualForce,
                20_000,
                &http_config(&forced.url)
            ),
            ExpectedUpdateOutcome::Unchanged
        );
        assert_eq!(forced.requests.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn timeout_and_connection_failure_use_current_or_report_unavailable() {
        let with_current = TempDir::new().unwrap();
        activate(with_current.path(), &valid(), 1_000);
        let slow = MockServer::response("200 OK", valid(), Duration::from_millis(250), &[], true);
        let mut slow_config = http_config(&slow.url);
        slow_config.total_timeout = Duration::from_millis(40);
        assert_eq!(
            update(
                with_current.path(),
                ExpectedUpdateMode::ManualForce,
                2_000,
                &slow_config
            ),
            ExpectedUpdateOutcome::OfflineUsingCurrent
        );

        let without_current = TempDir::new().unwrap();
        assert_eq!(
            update(
                without_current.path(),
                ExpectedUpdateMode::ManualForce,
                2_000,
                &http_config(&unavailable_url())
            ),
            ExpectedUpdateOutcome::Unavailable
        );
    }

    #[test]
    fn error_statuses_use_current_or_report_unavailable() {
        for status in ["404 Not Found", "500 Internal Server Error"] {
            let with_current = TempDir::new().unwrap();
            activate(with_current.path(), &valid(), 1_000);
            let server = MockServer::response(status, Vec::new(), Duration::ZERO, &[], true);
            assert_eq!(
                update(
                    with_current.path(),
                    ExpectedUpdateMode::ManualForce,
                    2_000,
                    &http_config(&server.url)
                ),
                ExpectedUpdateOutcome::RejectedUsingCurrent
            );

            let without_current = TempDir::new().unwrap();
            let server = MockServer::response(status, Vec::new(), Duration::ZERO, &[], true);
            assert_eq!(
                update(
                    without_current.path(),
                    ExpectedUpdateMode::ManualForce,
                    2_000,
                    &http_config(&server.url)
                ),
                ExpectedUpdateOutcome::Unavailable
            );
        }
    }

    #[test]
    fn redirects_and_oversized_responses_are_rejected_without_following() {
        let redirect_root = TempDir::new().unwrap();
        let redirect = MockServer::response(
            "302 Found",
            Vec::new(),
            Duration::ZERO,
            &[("Location", unavailable_url())],
            true,
        );
        assert_eq!(
            update(
                redirect_root.path(),
                ExpectedUpdateMode::ManualForce,
                1_000,
                &http_config(&redirect.url)
            ),
            ExpectedUpdateOutcome::Unavailable
        );
        assert_eq!(redirect.requests.load(Ordering::SeqCst), 1);

        let oversized_root = TempDir::new().unwrap();
        let oversized = MockServer::response(
            "200 OK",
            vec![b'x'; MAX_SNAPSHOT_BYTES + 1],
            Duration::ZERO,
            &[],
            false,
        );
        assert_eq!(
            update(
                oversized_root.path(),
                ExpectedUpdateMode::ManualForce,
                1_000,
                &http_config(&oversized.url)
            ),
            ExpectedUpdateOutcome::Unavailable
        );
    }

    #[test]
    fn invalid_downloads_never_replace_current_and_leave_no_temp() {
        for body in [b"not json".to_vec(), dataset("2026-07-11", ids(1, 499))] {
            let temp = TempDir::new().unwrap();
            let current = activate(temp.path(), &valid(), 1_000).snapshot;
            let server = MockServer::ok(body);
            assert_eq!(
                update(
                    temp.path(),
                    ExpectedUpdateMode::ManualForce,
                    2_000,
                    &http_config(&server.url)
                ),
                ExpectedUpdateOutcome::RejectedUsingCurrent
            );
            assert_eq!(
                load_current_or_previous_snapshot(temp.path())
                    .unwrap()
                    .unwrap(),
                current
            );
            assert!(
                fs::read_dir(cache_paths(temp.path()).2)
                    .unwrap()
                    .all(|entry| !entry.unwrap().path().to_string_lossy().ends_with(".part"))
            );
            assert!(!temp.path().join("update.lock").exists());
            assert!(!temp.path().join("update.lock.reclaim").exists());
        }
    }

    #[test]
    fn state_records_attempts_and_only_success_changes_success_timestamp() {
        let temp = TempDir::new().unwrap();
        let success = MockServer::ok(valid());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::ManualForce,
                1_000,
                &http_config(&success.url)
            ),
            ExpectedUpdateOutcome::Activated
        );
        let after_success = load_expected_update_state(temp.path()).unwrap();
        assert_eq!(after_success.last_attempt_at, Some(1_000));
        assert_eq!(after_success.last_success_at, Some(1_000));

        let failure = MockServer::response("500 Error", Vec::new(), Duration::ZERO, &[], true);
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::ManualForce,
                2_000,
                &http_config(&failure.url)
            ),
            ExpectedUpdateOutcome::RejectedUsingCurrent
        );
        let after_failure = load_expected_update_state(temp.path()).unwrap();
        assert_eq!(after_failure.last_attempt_at, Some(2_000));
        assert_eq!(after_failure.last_success_at, Some(1_000));
        assert_eq!(
            after_failure.last_error_code,
            Some(ExpectedUpdateErrorCode::HttpStatus)
        );
    }

    /// D3: the attempt is recorded when it ends, not before the download starts.
    #[test]
    fn attempt_is_recorded_only_after_the_attempt_ends() {
        let temp = TempDir::new().unwrap();
        let (server, received, release) = MockServer::gated(valid());
        let root = temp.path().to_path_buf();
        let mut config = http_config(&server.url);
        config.connect_timeout = Duration::from_secs(60);
        config.total_timeout = Duration::from_secs(120);
        let thread_root = root.clone();
        let first = thread::spawn(move || {
            update(&thread_root, ExpectedUpdateMode::Automatic, 1_000, &config)
        });
        received
            .recv_timeout(Duration::from_secs(120))
            .expect("caller reached the server");
        assert_eq!(
            load_expected_update_state(&root).unwrap().last_attempt_at,
            None,
            "no attempt marker while the download is in flight"
        );
        release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), ExpectedUpdateOutcome::Activated);
        let state = load_expected_update_state(&root).unwrap();
        assert_eq!(state.last_attempt_at, Some(1_000));
        assert_eq!(state.last_success_at, Some(1_000));
        assert_eq!(state.last_error_code, None);
    }

    /// D3: an automatic retry after a failure waits at least 1 h; after a success, 24 h.
    #[test]
    fn automatic_retry_waits_one_hour_after_failure_and_a_day_after_success() {
        const HOUR: i64 = 60 * 60 * 1_000;
        let temp = TempDir::new().unwrap();
        let failure = MockServer::response("500 Error", Vec::new(), Duration::ZERO, &[], true);
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                10 * HOUR,
                &http_config(&failure.url)
            ),
            ExpectedUpdateOutcome::Unavailable
        );
        let never = http_config(&unavailable_url());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                11 * HOUR - 1,
                &never
            ),
            ExpectedUpdateOutcome::NotDue
        );
        let success = MockServer::ok(valid());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                11 * HOUR,
                &http_config(&success.url)
            ),
            ExpectedUpdateOutcome::Activated
        );
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                35 * HOUR - 1,
                &never
            ),
            ExpectedUpdateOutcome::NotDue
        );
        let next = MockServer::ok(valid());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                35 * HOUR,
                &http_config(&next.url)
            ),
            ExpectedUpdateOutcome::Unchanged
        );
        assert_eq!(next.requests.load(Ordering::SeqCst), 1);
    }

    /// A marker written by the old code before a download that never finished (no success and
    /// no error at the same time) counts as a failed attempt, so it is retried after 1 h.
    #[test]
    fn unfinished_legacy_marker_is_retried_after_one_hour() {
        const HOUR: i64 = 60 * 60 * 1_000;
        let temp = TempDir::new().unwrap();
        persist_update_state(
            temp.path(),
            &ExpectedUpdateState {
                last_attempt_at: Some(5 * HOUR),
                last_success_at: Some(HOUR),
                ..ExpectedUpdateState::default()
            },
        )
        .unwrap();
        let success = MockServer::ok(valid());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                6 * HOUR,
                &http_config(&success.url)
            ),
            ExpectedUpdateOutcome::Activated
        );
    }

    #[test]
    fn concurrent_update_returns_busy_for_second_caller() {
        let temp = TempDir::new().unwrap();
        // The server answers only after the second caller got `Busy`: ordering by channels, not
        // by elapsed time, so the test holds under any load.
        let (server, received, release) = MockServer::gated(valid());
        let root = temp.path().to_path_buf();
        let mut config = http_config(&server.url);
        // Generous limits only so a loaded machine cannot time the first request out; the
        // ordering does not depend on them.
        config.connect_timeout = Duration::from_secs(60);
        config.total_timeout = Duration::from_secs(120);
        let thread_root = root.clone();
        let thread_config = config.clone();
        let first = thread::spawn(move || {
            update(
                &thread_root,
                ExpectedUpdateMode::ManualForce,
                1_000,
                &thread_config,
            )
        });
        received
            .recv_timeout(Duration::from_secs(120))
            .expect("first caller reached the server");
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            update(&root, ExpectedUpdateMode::ManualForce, 1_001, &config),
            ExpectedUpdateOutcome::Busy
        );
        release.send(()).unwrap();
        assert_eq!(first.join().unwrap(), ExpectedUpdateOutcome::Activated);
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn fresh_file_lock_is_preserved_and_stale_lock_is_recovered() {
        let fresh_root = TempDir::new().unwrap();
        let fresh_path = fresh_root.path().join("update.lock");
        let fresh = serde_json::to_vec(&FileLockRecord {
            token: "fresh".to_owned(),
            created_at: 1_000,
        })
        .unwrap();
        create_synced(&fresh_path, &fresh).unwrap();
        assert_eq!(
            update(
                fresh_root.path(),
                ExpectedUpdateMode::ManualForce,
                1_000 + STALE_LOCK_MS - 1,
                &http_config(&unavailable_url())
            ),
            ExpectedUpdateOutcome::Busy
        );
        assert_eq!(fs::read(fresh_path).unwrap(), fresh);

        let stale_root = TempDir::new().unwrap();
        let stale_path = stale_root.path().join("update.lock");
        create_synced(
            &stale_path,
            &serde_json::to_vec(&FileLockRecord {
                token: "stale".to_owned(),
                created_at: 1_000,
            })
            .unwrap(),
        )
        .unwrap();
        let server = MockServer::ok(valid());
        assert_eq!(
            update(
                stale_root.path(),
                ExpectedUpdateMode::ManualForce,
                1_000 + STALE_LOCK_MS,
                &http_config(&server.url)
            ),
            ExpectedUpdateOutcome::Activated
        );
        assert!(!stale_path.exists());
        assert!(!stale_root.path().join("update.lock.reclaim").exists());
    }

    #[test]
    fn malformed_lock_uses_file_age_and_stale_reclaim_is_recovered() {
        assert!(!lock_is_stale(
            b"truncated",
            1_000 + STALE_LOCK_MS - 1,
            Some(1_000)
        ));
        assert!(lock_is_stale(
            b"truncated",
            1_000 + STALE_LOCK_MS,
            Some(1_000)
        ));

        let fresh_root = TempDir::new().unwrap();
        let fresh_reclaim = fresh_root.path().join("update.lock.reclaim");
        let fresh_bytes = serde_json::to_vec(&FileLockRecord {
            token: "fresh-reclaim".to_owned(),
            created_at: 1_000,
        })
        .unwrap();
        create_synced(&fresh_reclaim, &fresh_bytes).unwrap();
        assert_eq!(
            update(
                fresh_root.path(),
                ExpectedUpdateMode::ManualForce,
                1_000 + STALE_LOCK_MS - 1,
                &http_config(&unavailable_url())
            ),
            ExpectedUpdateOutcome::Busy
        );
        assert_eq!(fs::read(fresh_reclaim).unwrap(), fresh_bytes);

        let stale_root = TempDir::new().unwrap();
        let stale_reclaim = stale_root.path().join("update.lock.reclaim");
        create_synced(
            &stale_reclaim,
            &serde_json::to_vec(&FileLockRecord {
                token: "stale-reclaim".to_owned(),
                created_at: 1_000,
            })
            .unwrap(),
        )
        .unwrap();
        let server = MockServer::ok(valid());
        assert_eq!(
            update(
                stale_root.path(),
                ExpectedUpdateMode::ManualForce,
                1_000 + STALE_LOCK_MS,
                &http_config(&server.url)
            ),
            ExpectedUpdateOutcome::Activated
        );
        assert!(!stale_reclaim.exists());
        assert!(!stale_root.path().join("update.lock").exists());
    }

    #[test]
    fn restart_preserves_snapshot_and_cadence() {
        let temp = TempDir::new().unwrap();
        let server = MockServer::ok(valid());
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                5_000,
                &http_config(&server.url)
            ),
            ExpectedUpdateOutcome::Activated
        );

        let reloaded = load_current_or_previous_snapshot(temp.path())
            .unwrap()
            .unwrap();
        let state = load_expected_update_state(temp.path()).unwrap();
        assert_eq!(reloaded.manifest.source_version, "2026-07-10");
        assert_eq!(state.last_attempt_at, Some(5_000));
        assert_eq!(
            update(
                temp.path(),
                ExpectedUpdateMode::Automatic,
                6_000,
                &http_config(&unavailable_url())
            ),
            ExpectedUpdateOutcome::NotDue
        );
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
