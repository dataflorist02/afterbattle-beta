use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime},
};

use chrono::{Local, NaiveDateTime, TimeZone, Utc};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    battle_pace::{BattlePaceInput, resolve_battle_pace},
    clock::{Clock, system_clock},
    error::AppError,
    events::{AppEvent, EventSink, no_events},
    foundations::{CatalogVehicle, ImportExtras, extract_battle_roster, extract_own_player_facts},
    models::{
        Battle, BattleOutcome, BattlePaceDestroyedScore, BattlePaceProvenance, BattlePaceReason,
        BattlePaceRecord, BattlePaceTeamSizes, DataQuality, ImportProgress, LocalImportResult,
        ParserState, Session, SourceKind, SourceStatus,
    },
    storage::{Database, ReplayFileState, ReplayUpsertOutcome},
    team_relative::{TeamRelativeFactsV1, TeamRelativeParseInput, extract_team_relative_facts},
    vehicle_class_catalog::{LoadedVehicleClassCatalog, VehicleClassCatalogRuntime},
};

const REPLAY_MAGIC: u32 = 0x1134_3212;
/// 8 (E1): `battleType`/`bonusType` are read as strict JSON integers and the import gate uses
/// the number (spec 20, D4). Team-relative facts are unchanged from version 7.
/// 9 (E2): the complete v10 foundations are stored with every battle: own-player columns
/// (spec 20 E2), vehicle tier and catalog fingerprint, the anonymous roster and team
/// aggregates (D17) and `packet_stream_bytes`. Team-relative facts are unchanged.
const REPLAY_PARSER_VERSION: i64 = 9;
const FEATURE_SCHEMA_VERSION: i64 = 3;
const MAX_REPLAY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 2 * 1024 * 1024;
const MAX_RESULTS_BYTES: usize = 8 * 1024 * 1024;
const MAX_REPLAY_BLOCKS: u32 = 16;
const MAX_FINAL_FRAGS: i64 = 1_000;
/// Replays per transaction of the full import (§4.1).
pub(crate) const IMPORT_BATCH_SIZE: usize = 100;
/// Settings key of the full import cursor (`<mtime_ms>:<path key>` of the last stored batch).
pub(crate) const IMPORT_CURSOR_KEY: &str = "import_cursor";
const MAX_TRACKED_FILES: usize = 2_048;
const MAX_DIRTY_FILES: usize = 256;
const MAX_REPLAY_RETRIES: u8 = 5;
const SOURCE_CREDENTIAL_SERVICE: &str = "app.afterbattle.desktop.local-source";
const SOURCE_CREDENTIAL_USER: &str = "wot-root";

#[derive(Debug)]
struct ReplayMetadata {
    battle: Battle,
    client_version: Option<String>,
    pace: ReplayPaceMetadata,
}

#[derive(Debug, Clone, Default)]
struct ReplayPaceMetadata {
    battle_type_raw: Option<i64>,
    initial_team_sizes: Option<BattlePaceTeamSizes>,
    vehicle_teams: Option<HashMap<String, i64>>,
    extraction_error: Option<BattlePaceReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BattleResults {
    arena_unique_id: i64,
    wg_tank_id: Option<i64>,
    outcome: BattleOutcome,
    player_team: Option<i64>,
    winner_team_raw: Option<i64>,
    damage: Option<i64>,
    assist: Option<i64>,
    assist_spotting: Option<i64>,
    assist_tracking: Option<i64>,
    assist_stun: Option<i64>,
    blocked: Option<i64>,
    damage_received: Option<i64>,
    frags: Option<i64>,
    spots: Option<i64>,
    shots: Option<i64>,
    hits: Option<i64>,
    direct_enemy_hits: Option<i64>,
    direct_team_hits: Option<i64>,
    penetrations: Option<i64>,
    piercing_enemy_hits: Option<i64>,
    explosion_hits: Option<i64>,
    survived: Option<bool>,
    xp: Option<i64>,
    original_xp: Option<i64>,
    duration_seconds: Option<i64>,
    defense_points: Option<i64>,
    moving_avg_damage: Option<i64>,
    damage_rating: Option<i64>,
    pace_resolution: crate::models::BattlePaceResolution,
    parser_error_code: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BattleResultsError {
    InvalidShape,
}

impl BattleResultsError {
    fn code(self) -> &'static str {
        match self {
            Self::InvalidShape => "invalid_results_shape",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FinalFragsResolution {
    value: Option<i64>,
    parser_error_code: Option<&'static str>,
}

#[derive(Debug)]
struct ParsedReplay {
    metadata: ReplayMetadata,
    results: Option<BattleResults>,
    block_count: u32,
    parser_version: i64,
    parser_error_code: Option<&'static str>,
    file_state: ReplayFileState,
    team_relative_facts: TeamRelativeFactsV1,
    extras: ImportExtras,
}

/// A parsed replay ready to be stored.
#[derive(Debug)]
struct ImportedReplay {
    battle: Battle,
    file_state: ReplayFileState,
    client_version: Option<String>,
    team_relative_facts: TeamRelativeFactsV1,
    extras: ImportExtras,
}

impl ParsedReplay {
    #[cfg(test)]
    fn into_battle(self) -> (Battle, ReplayFileState, Option<String>) {
        let (battle, file_state, client_version, _) = self.into_import();
        (battle, file_state, client_version)
    }

    #[cfg(test)]
    fn into_import(self) -> (Battle, ReplayFileState, Option<String>, TeamRelativeFactsV1) {
        let imported = self.into_imported(None);
        (
            imported.battle,
            imported.file_state,
            imported.client_version,
            imported.team_relative_facts,
        )
    }

    /// Final battle and extras; the player's vehicle tier comes from `catalog` by the resolved
    /// `typeCompDescr`.
    fn into_imported(mut self, catalog: Option<&LoadedVehicleClassCatalog>) -> ImportedReplay {
        let mut extras = std::mem::take(&mut self.extras);
        let (battle, file_state, client_version, team_relative_facts) = self.into_parts();
        if let Some(catalog) = catalog {
            extras.catalog_fingerprint = Some(catalog.fingerprint().to_owned());
            extras.vehicle_tier = battle
                .wg_tank_id
                .and_then(|id| catalog.vehicle(id))
                .and_then(|vehicle| vehicle.tier);
        }
        ImportedReplay {
            battle,
            file_state,
            client_version,
            team_relative_facts,
            extras,
        }
    }

    fn into_parts(self) -> (Battle, ReplayFileState, Option<String>, TeamRelativeFactsV1) {
        let partial_pace = resolve_battle_pace(&BattlePaceInput {
            data_quality: DataQuality::Partial,
            feature_schema_version: None,
            battle_type_raw: self.metadata.pace.battle_type_raw,
            bonus_type_raw: None,
            gameplay_id: self.metadata.battle.gameplay_id.clone(),
            duration_seconds: None,
            finish_reason_raw: None,
            initial_team_sizes: self.metadata.pace.initial_team_sizes,
            destroyed_score: None,
            player_team: None,
            winner_team_raw: None,
            outcome: BattleOutcome::Unknown,
            extraction_error: self.metadata.pace.extraction_error,
            provenance: BattlePaceProvenance::ReplayMetadata,
        });
        let mut battle = self.metadata.battle;
        battle.parser_version = Some(self.parser_version);
        battle.parser_error_code = self.parser_error_code.map(str::to_owned);
        battle.battle_pace = BattlePaceRecord::from_resolution(&partial_pace);
        if let Some(results) = self.results {
            battle.arena_unique_id = Some(results.arena_unique_id);
            let metadata_tank_id = battle.wg_tank_id;
            battle.wg_tank_id = match (results.wg_tank_id, metadata_tank_id) {
                (Some(results_id), Some(metadata_id)) if results_id != metadata_id => {
                    battle.parser_error_code = Some("vehicle_id_conflict".into());
                    None
                }
                (Some(results_id), _) => Some(results_id),
                (None, metadata_id) => metadata_id,
            };
            battle.source_kind = SourceKind::Replay;
            battle.data_quality = DataQuality::Full;
            battle.feature_schema_version = Some(FEATURE_SCHEMA_VERSION);
            battle.data_completeness = Some("results".into());
            battle.replay_source = Some("replay_results".into());
            battle.outcome = results.outcome;
            battle.player_team = results.player_team;
            battle.winner_team_raw = results.winner_team_raw;
            battle.damage = results.damage;
            battle.assist = results.assist;
            battle.assist_spotting = results.assist_spotting;
            battle.assist_tracking = results.assist_tracking;
            battle.assist_stun = results.assist_stun;
            battle.blocked = results.blocked;
            battle.damage_received = results.damage_received;
            battle.frags = results.frags;
            battle.spots = results.spots;
            battle.shots = results.shots;
            battle.hits = results.hits;
            battle.direct_enemy_hits = results.direct_enemy_hits;
            battle.direct_team_hits = results.direct_team_hits;
            battle.penetrations = results.penetrations;
            battle.piercing_enemy_hits = results.piercing_enemy_hits;
            battle.explosion_hits = results.explosion_hits;
            battle.survived = results.survived;
            battle.xp = results.xp;
            battle.original_xp = results.original_xp;
            battle.duration_seconds = results.duration_seconds;
            battle.defense_points = results.defense_points;
            battle.moving_avg_damage = results.moving_avg_damage;
            battle.damage_rating = results.damage_rating;
            battle.battle_pace = BattlePaceRecord::from_resolution(&results.pace_resolution);
        }
        debug_assert_eq!(self.block_count as i64, self.file_state.block_count);
        (
            battle,
            self.file_state,
            self.metadata.client_version,
            self.team_relative_facts,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct RetryState {
    stamp: (u64, SystemTime),
    attempts: u8,
    next_retry: SystemTime,
}

#[derive(Clone)]
pub struct LocalTracker {
    database: Database,
    root: Arc<RwLock<Option<PathBuf>>>,
    paused: Arc<AtomicBool>,
    status: Arc<Mutex<SourceStatus>>,
    stable_files: Arc<Mutex<HashMap<PathBuf, (u64, SystemTime)>>>,
    processed_files: Arc<Mutex<HashMap<PathBuf, (u64, SystemTime)>>>,
    dirty_files: Arc<Mutex<HashSet<PathBuf>>>,
    retry_files: Arc<Mutex<HashMap<PathBuf, RetryState>>>,
    operation_lock: Arc<Mutex<()>>,
    challenge_last_error_code: Arc<RwLock<Option<String>>>,
    /// Client catalog for vehicle tiers and classes at import time (E1b, E2).
    vehicle_catalog: Option<VehicleClassCatalogRuntime>,
    /// Set while the full import runs; a second request is ignored.
    import_running: Arc<AtomicBool>,
    /// Injected clock (audit T8): tests move time instead of sleeping.
    clock: Arc<dyn Clock>,
    /// Set by `cancel_import`; the running full import stops after the current batch.
    cancel_requested: Arc<AtomicBool>,
    /// Backend → frontend events (E3), shared by every clone so the worker thread sees a sink
    /// set after it started.
    events: Arc<RwLock<Arc<dyn EventSink>>>,
    /// Recovery mode: the real database could not be opened, so nothing may be imported.
    disabled: bool,
}

impl std::fmt::Debug for LocalTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalTracker").finish_non_exhaustive()
    }
}

/// Resets the "full import running" flag when the run ends, also on an error.
struct ImportRunGuard(Arc<AtomicBool>);

impl Drop for ImportRunGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// One replay file waiting for the full import, in import order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PendingReplay {
    mtime_ms: i64,
    key: String,
    path: PathBuf,
    stamp_len: u64,
}

impl PendingReplay {
    fn cursor(&self) -> String {
        format!("{}:{}", self.mtime_ms, self.key)
    }
}

fn parse_cursor(value: &str) -> Option<(i64, String)> {
    let (mtime, key) = value.split_once(':')?;
    Some((mtime.parse().ok()?, key.to_owned()))
}

impl LocalTracker {
    fn build(
        database: Database,
        challenge_last_error_code: Arc<RwLock<Option<String>>>,
        root: Option<PathBuf>,
        vehicle_catalog: Option<VehicleClassCatalogRuntime>,
        disabled: bool,
    ) -> Self {
        Self {
            database,
            root: Arc::new(RwLock::new(root)),
            paused: Arc::new(AtomicBool::new(disabled)),
            status: Arc::new(Mutex::new(SourceStatus::default())),
            stable_files: Arc::new(Mutex::new(HashMap::new())),
            processed_files: Arc::new(Mutex::new(HashMap::new())),
            dirty_files: Arc::new(Mutex::new(HashSet::new())),
            retry_files: Arc::new(Mutex::new(HashMap::new())),
            operation_lock: Arc::new(Mutex::new(())),
            challenge_last_error_code,
            vehicle_catalog,
            import_running: Arc::new(AtomicBool::new(false)),
            clock: system_clock(),
            cancel_requested: Arc::new(AtomicBool::new(false)),
            events: Arc::new(RwLock::new(no_events())),
            disabled,
        }
    }

    /// Sends events to `events` (the Tauri app in the application), also from the worker thread
    /// that is already running.
    pub(crate) fn set_events(&self, events: Arc<dyn EventSink>) {
        *self.events.write().unwrap_or_else(|e| e.into_inner()) = events;
    }

    /// Sends one event to the frontend.
    pub(crate) fn emit(&self, event: AppEvent) {
        let sink = Arc::clone(&self.events.read().unwrap_or_else(|e| e.into_inner()));
        sink.emit(event);
    }

    /// Asks the running full import to stop after its current batch. The cursor stays, so the
    /// next run continues where it stopped.
    pub fn cancel_import(&self) {
        if self.import_running.load(Ordering::SeqCst) {
            self.cancel_requested.store(true, Ordering::SeqCst);
        }
    }

    #[cfg(test)]
    pub(crate) fn import_running(&self) -> bool {
        self.import_running.load(Ordering::SeqCst)
    }

    /// The single auto-end schedule (D6): ends a session past the inactivity limit and reports
    /// it to the frontend.
    pub(crate) fn end_stale_session(&self) -> Result<Option<Session>, AppError> {
        let minutes = self.database.inactivity_minutes().unwrap_or(60);
        let ended = self
            .database
            .auto_end_stale_at(minutes, self.clock.now_ms(), || Ok(()))?;
        if let Some(session) = &ended {
            self.emit(AppEvent::SessionEnded {
                session_id: session.id.clone(),
            });
            self.emit(AppEvent::DataChanged);
        }
        Ok(ended)
    }

    #[cfg(test)]
    fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn new(
        database: Database,
        challenge_last_error_code: Arc<RwLock<Option<String>>>,
        vehicle_catalog: Option<VehicleClassCatalogRuntime>,
    ) -> Self {
        let legacy = database.get_setting("wot_root").ok().flatten();
        let configured = load_source_root()
            .or_else(|| legacy.clone())
            .map(PathBuf::from);
        if let Some(path) = legacy {
            let _ = store_source_root(&path);
            let _ = database.remove_setting("wot_root");
        }
        let tracker = Self::build(
            database,
            challenge_last_error_code,
            configured,
            vehicle_catalog,
            false,
        );
        tracker.refresh_status();
        tracker.spawn_worker();
        tracker
    }

    /// A tracker for recovery mode: no worker, no configured source, every import refused.
    /// Battles must never be written into the recovery database (audit N1).
    pub fn disabled(
        database: Database,
        challenge_last_error_code: Arc<RwLock<Option<String>>>,
    ) -> Self {
        let tracker = Self::build(database, challenge_last_error_code, None, None, true);
        if let Ok(mut status) = tracker.status.lock() {
            status.parser_state = ParserState::Error;
            status.message = Some(AppError::RecoveryModeTrackerDisabled.to_string());
        }
        tracker
    }

    pub fn configure(&self, root: &str) -> Result<SourceStatus, AppError> {
        if self.disabled {
            return Err(AppError::RecoveryModeTrackerDisabled);
        }
        let canonical = validate_root(Path::new(root))?;
        {
            let _operation = self.operation_lock.lock().map_err(|_| AppError::Tracker)?;
            store_source_root(&canonical.to_string_lossy())?;
            self.reset_transient_state()?;
            *self.root.write().map_err(|_| AppError::Tracker)? = Some(canonical);
        }
        self.paused.store(false, Ordering::Relaxed);
        self.refresh_status();
        self.spawn_full_import();
        Ok(self.status())
    }

    pub(crate) fn configured_root(&self) -> Option<PathBuf> {
        self.root
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn clear_configuration(&self) -> Result<(), AppError> {
        let _guard = self.operation_lock.lock().map_err(|_| AppError::Tracker)?;
        delete_source_root()?;
        *self.root.write().map_err(|_| AppError::Tracker)? = None;
        self.stable_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .clear();
        self.processed_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .clear();
        self.dirty_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .clear();
        self.retry_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .clear();
        *self.status.lock().map_err(|_| AppError::Tracker)? = SourceStatus::default();
        Ok(())
    }

    pub fn pause(&self) -> SourceStatus {
        self.paused.store(true, Ordering::Relaxed);
        self.refresh_status();
        self.status()
    }
    pub fn resume(&self) -> SourceStatus {
        self.paused.store(false, Ordering::Relaxed);
        self.refresh_status();
        self.spawn_full_import();
        self.status()
    }
    pub fn status(&self) -> SourceStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Starts the full import in the background (D14). Progress is reported in the status read
    /// by `get_snapshot`; the returned counters are zero.
    pub fn import_history(&self) -> Result<LocalImportResult, AppError> {
        if self.disabled {
            return Err(AppError::RecoveryModeTrackerDisabled);
        }
        if self.configured_root().is_none() {
            return Err(AppError::NotConfigured);
        }
        self.spawn_full_import();
        Ok(LocalImportResult {
            scanned: 0,
            imported: 0,
            duplicates: 0,
            partial: 0,
        })
    }

    fn spawn_full_import(&self) {
        if self.disabled {
            return;
        }
        let tracker = self.clone();
        let _ = thread::Builder::new()
            .name("afterbattle-full-import".into())
            .spawn(move || {
                if let Err(error) = tracker.run_full_import() {
                    tracker.note_error(&error);
                }
            });
    }

    fn note_error(&self, error: &AppError) {
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        status.parser_state = ParserState::Error;
        status.message = Some(error.to_string());
        status.partial_mode = true;
    }

    fn reset_transient_state(&self) -> Result<(), AppError> {
        self.stable_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .clear();
        self.processed_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .clear();
        self.dirty_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .clear();
        self.retry_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .clear();
        Ok(())
    }

    pub fn detect_installations() -> Vec<String> {
        let mut candidates = Vec::new();
        for path in [
            r"C:\Games\World_of_Tanks_EU",
            r"C:\Games\World_of_Tanks",
            r"C:\Program Files (x86)\World_of_Tanks_EU",
            r"C:\Program Files\World_of_Tanks_EU",
        ] {
            if validate_root(Path::new(path)).is_ok() {
                candidates.push(path.to_owned());
            }
        }
        candidates
    }

    fn refresh_status(&self) {
        let root = self.root.read().ok().and_then(|r| r.clone());
        let paused = self.paused.load(Ordering::Relaxed);
        let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
        status.configured = root.is_some();
        status.active = root.is_some() && !paused;
        status.paused = paused;
        status.game_root_display = root
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|v| v.to_string_lossy().into_owned());
        if root.is_some() && status.message.is_none() {
            status.parser_state = ParserState::Waiting;
        }
    }

    fn spawn_worker(&self) {
        let tracker = self.clone();
        thread::Builder::new()
            .name("afterbattle-local-tracker".into())
            .spawn(move || {
                // The catalog is refreshed before the first import so every stored battle gets
                // its tier (E1b); then the startup scan reads files missing from the database or
                // changed since (D14).
                let root = tracker.configured_root();
                if let Some(catalog) = &tracker.vehicle_catalog {
                    let _ = catalog.refresh(root.as_deref());
                }
                if root.is_some()
                    && !tracker.paused.load(Ordering::Relaxed)
                    && let Err(error) = tracker.run_full_import()
                {
                    tracker.note_error(&error);
                }
                let (tx, rx) = std::sync::mpsc::sync_channel::<notify::Result<notify::Event>>(64);
                let make_watcher = || {
                    let event_tx = tx.clone();
                    notify::recommended_watcher(move |event| {
                        let _ = event_tx.try_send(event);
                    })
                    .ok()
                };
                let mut watcher: Option<RecommendedWatcher> = None;
                let mut watched: Option<PathBuf> = None;
                loop {
                    let root = tracker.root.read().ok().and_then(|r| r.clone());
                    if root != watched {
                        drop(watcher.take());
                        watcher = make_watcher();
                        if let (Some(w), Some(new)) = (&mut watcher, &root) {
                            let _ = w.watch(new, RecursiveMode::NonRecursive);
                            let _ = w.watch(&new.join("replays"), RecursiveMode::NonRecursive);
                        }
                        watched = root;
                    }
                    let mut events = Vec::new();
                    if let Ok(event) = rx.recv_timeout(Duration::from_secs(30)) {
                        events.push(event);
                    }
                    while let Ok(event) = rx.try_recv() {
                        events.push(event);
                    }
                    for event in events.into_iter().flatten() {
                        for path in event.paths {
                            if !path.extension().is_some_and(|extension| {
                                extension.eq_ignore_ascii_case("wotreplay")
                            }) {
                                continue;
                            }
                            let Ok(path) = fs::canonicalize(path) else {
                                continue;
                            };
                            if let Ok(mut dirty) = tracker.dirty_files.lock() {
                                if dirty.len() >= MAX_DIRTY_FILES && !dirty.contains(&path) {
                                    dirty.clear();
                                }
                                dirty.insert(path.clone());
                            }
                            if let Ok(mut stable) = tracker.stable_files.lock() {
                                stable.remove(&path);
                            }
                            if let Ok(mut processed) = tracker.processed_files.lock() {
                                processed.remove(&path);
                            }
                            if let Ok(mut retries) = tracker.retry_files.lock() {
                                retries.remove(&path);
                            }
                        }
                    }
                    if !tracker.paused.load(Ordering::Relaxed)
                        && let Err(error) = tracker.reconcile()
                    {
                        tracker.note_error(&error);
                    }
                    // Sessions end on time also while the tracker is paused.
                    let _ = tracker.end_stale_session();
                }
            })
            .ok();
    }

    fn replay_dir(&self) -> Result<PathBuf, AppError> {
        let root = self
            .root
            .read()
            .map_err(|_| AppError::Tracker)?
            .clone()
            .ok_or(AppError::NotConfigured)?;
        let root = validate_root(&root)?;
        Ok(fs::canonicalize(root.join("replays"))?)
    }

    fn catalog_snapshot(&self) -> Option<LoadedVehicleClassCatalog> {
        self.vehicle_catalog
            .as_ref()
            .and_then(VehicleClassCatalogRuntime::snapshot)
    }

    fn set_import_progress(&self, progress: Option<ImportProgress>) {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .import_progress = progress;
    }

    /// Replay files missing from the database or changed since they were stored (`size`,
    /// `mtime`), oldest first, without limits of age or count (D14).
    fn pending_replays(&self, replay_dir: &Path) -> Result<Vec<PendingReplay>, AppError> {
        let known = self.database.known_replay_files()?;
        let mut pending = Vec::new();
        for entry in fs::read_dir(replay_dir)?.filter_map(Result::ok) {
            if !entry
                .file_type()
                .is_ok_and(|kind| kind.is_file() && !kind.is_symlink())
            {
                continue;
            }
            let path = entry.path();
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wotreplay"))
            {
                continue;
            }
            let Ok(path) = fs::canonicalize(&path) else {
                continue;
            };
            if !path.starts_with(replay_dir) {
                continue;
            }
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let mtime_ms = file_mtime_ms(&metadata);
            let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
            let key = source_path_key(&path);
            if known.get(&key) == Some(&(size, mtime_ms)) {
                continue;
            }
            pending.push(PendingReplay {
                mtime_ms,
                key,
                path,
                stamp_len: metadata.len(),
            });
        }
        pending.sort();
        Ok(pending)
    }

    /// The full import engine (D14, D5): every pending replay, in batches of
    /// [`IMPORT_BATCH_SIZE`]. Each batch is stored in one transaction together with the cursor,
    /// so an interrupted run resumes after the last committed batch without duplicates. Battles
    /// older than the inactivity limit do not create sessions. At the end of a complete pass the
    /// v10 rebuild report is written.
    pub(crate) fn run_full_import(&self) -> Result<LocalImportResult, AppError> {
        if self.disabled {
            return Err(AppError::RecoveryModeTrackerDisabled);
        }
        if self.import_running.swap(true, Ordering::SeqCst) {
            return Ok(LocalImportResult {
                scanned: 0,
                imported: 0,
                duplicates: 0,
                partial: 0,
            });
        }
        let _running = ImportRunGuard(Arc::clone(&self.import_running));
        self.cancel_requested.store(false, Ordering::SeqCst);
        let replay_dir = self.replay_dir()?;
        let mut pending = self.pending_replays(&replay_dir)?;
        if let Some((mtime, key)) = self
            .database
            .get_setting(IMPORT_CURSOR_KEY)?
            .as_deref()
            .and_then(parse_cursor)
        {
            pending.retain(|file| (file.mtime_ms, file.key.as_str()) > (mtime, key.as_str()));
        }
        let mut result = LocalImportResult {
            scanned: pending.len(),
            imported: 0,
            duplicates: 0,
            partial: 0,
        };
        let total = pending.len();
        self.set_import_progress(Some(ImportProgress { done: 0, total }));
        let inactivity_threshold = i64::from(self.database.inactivity_minutes()?) * 60_000;
        let mut done = 0;
        self.emit(AppEvent::ImportProgress {
            done: 0,
            total,
            finished: false,
            cancelled: false,
        });
        let mut cancelled = false;
        for batch in pending.chunks(IMPORT_BATCH_SIZE) {
            if self.paused.load(Ordering::Relaxed)
                || self.cancel_requested.swap(false, Ordering::SeqCst)
            {
                cancelled = true;
                break;
            }
            let catalog = self.catalog_snapshot();
            let mut parsed = batch
                .iter()
                .map(|file| {
                    let imported = parse_replay_file_with(&file.path, catalog.as_ref())
                        .map(|parsed| parsed.into_imported(catalog.as_ref()));
                    (file, imported)
                })
                .collect::<Vec<_>>();
            let cursor = batch.last().map(PendingReplay::cursor);
            let session_cutoff = self.clock.now_ms() - inactivity_threshold;
            let stored = {
                let _operation = self.operation_lock.lock().map_err(|_| AppError::Tracker)?;
                self.database.with_transaction(|tx| {
                    let mut stored = Vec::new();
                    for (_, imported) in &mut parsed {
                        let Ok(imported) = imported else {
                            continue;
                        };
                        if !passes_import_gate(&imported.battle) {
                            Database::record_skipped_replay_tx(
                                tx,
                                &imported.file_state,
                                &imported.battle,
                            )?;
                            continue;
                        }
                        let attach_session = imported.battle.battle_time >= session_cutoff;
                        let outcome = Database::upsert_replay_tx(
                            tx,
                            &mut imported.battle,
                            &imported.file_state,
                            attach_session,
                            Some(&imported.team_relative_facts),
                            &imported.extras,
                            inactivity_threshold,
                        )?;
                        stored.push((
                            imported.battle.arena_id.clone(),
                            outcome,
                            imported.battle.data_quality.clone(),
                        ));
                    }
                    if let Some(cursor) = &cursor {
                        Database::set_setting_tx(tx, IMPORT_CURSOR_KEY, cursor)?;
                    }
                    Ok(stored)
                })?
            };
            for (arena_id, outcome, quality) in stored {
                match outcome {
                    ReplayUpsertOutcome::Inserted | ReplayUpsertOutcome::Updated => {
                        result.imported += 1;
                        if quality == DataQuality::Partial {
                            result.partial += 1;
                        } else {
                            self.process_challenge(&arena_id);
                        }
                    }
                    ReplayUpsertOutcome::Duplicate => result.duplicates += 1,
                }
            }
            if let Ok(mut processed) = self.processed_files.lock() {
                for (file, imported) in &parsed {
                    if imported.is_ok()
                        && let Ok(metadata) = fs::metadata(&file.path)
                    {
                        processed.insert(
                            file.path.clone(),
                            (
                                file.stamp_len,
                                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                            ),
                        );
                    }
                }
            }
            for (_, imported) in &parsed {
                if let Ok(imported) = imported {
                    let mut status = self.status.lock().unwrap_or_else(|e| e.into_inner());
                    status.client_version = imported
                        .client_version
                        .clone()
                        .or(status.client_version.clone());
                }
            }
            done += batch.len();
            self.set_import_progress(Some(ImportProgress { done, total }));
            self.emit(AppEvent::ImportProgress {
                done,
                total,
                finished: false,
                cancelled: false,
            });
            if result.imported > 0 {
                self.emit(AppEvent::DataChanged);
            }
        }
        if done == total {
            self.database.remove_setting(IMPORT_CURSOR_KEY)?;
            self.database.finish_legacy_rebuild()?;
        }
        self.set_import_progress(None);
        self.emit(AppEvent::ImportProgress {
            done,
            total,
            finished: true,
            cancelled,
        });
        self.update_parser_state()?;
        Ok(result)
    }

    fn process_challenge(&self, arena_id: &str) {
        if self
            .database
            .process_battle_for_active_challenge(arena_id)
            .is_err()
        {
            *self
                .challenge_last_error_code
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some("challenge_progress_failed".to_owned());
        }
    }

    fn update_parser_state(&self) -> Result<(), AppError> {
        let replay_results_ready = self
            .database
            .recent_battles(1)?
            .first()
            .is_some_and(|battle| battle.replay_source.as_deref() == Some("replay_results"));
        let parser_state = if replay_results_ready {
            ParserState::Ready
        } else {
            ParserState::ReplayOnly
        };
        let mut s = self.status.lock().unwrap_or_else(|e| e.into_inner());
        s.configured = true;
        s.active = !self.paused.load(Ordering::Relaxed);
        s.paused = self.paused.load(Ordering::Relaxed);
        s.partial_mode = parser_state != ParserState::Ready;
        if s.parser_state != ParserState::Error {
            s.parser_state = parser_state;
        }
        if s.parser_state == ParserState::Ready {
            s.message = None;
        }
        Ok(())
    }

    /// Live path: replays written in the last 15 minutes, imported once their size and time are
    /// stable across two scans.
    fn reconcile(&self) -> Result<LocalImportResult, AppError> {
        if self.disabled {
            return Err(AppError::RecoveryModeTrackerDisabled);
        }
        if self.paused.load(Ordering::Relaxed) {
            return Ok(LocalImportResult {
                scanned: 0,
                imported: 0,
                duplicates: 0,
                partial: 0,
            });
        }
        let _operation = self.operation_lock.lock().map_err(|_| AppError::Tracker)?;
        let replay_dir = self.replay_dir()?;
        prune_file_cache(&self.stable_files)?;
        prune_file_cache(&self.processed_files)?;
        if self
            .retry_files
            .lock()
            .map_err(|_| AppError::Tracker)?
            .len()
            > MAX_TRACKED_FILES
        {
            self.retry_files
                .lock()
                .map_err(|_| AppError::Tracker)?
                .clear();
        }
        let mut files = fs::read_dir(&replay_dir)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_type()
                    .is_ok_and(|kind| kind.is_file() && !kind.is_symlink())
            })
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("wotreplay"))
            })
            .collect::<Vec<_>>();
        files.sort_by_key(|p| {
            fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        });
        let now = self.clock.now();
        {
            let processed_files = self.processed_files.lock().map_err(|_| AppError::Tracker)?;
            files.retain(|p| {
                let Ok(metadata) = fs::metadata(p) else {
                    return false;
                };
                let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                let stamp = (metadata.len(), modified);
                let already_processed = fs::canonicalize(p)
                    .ok()
                    .is_some_and(|safe_path| processed_files.get(&safe_path) == Some(&stamp));
                !already_processed
                    && now.duration_since(modified).is_ok_and(|age| {
                        age >= Duration::from_secs(2) && age <= Duration::from_secs(15 * 60)
                    })
            });
        }
        if files.len() > 20 {
            files = files.split_off(files.len() - 20);
        }
        let mut result = LocalImportResult {
            scanned: files.len(),
            imported: 0,
            duplicates: 0,
            partial: 0,
        };
        for path in files {
            let Ok(safe_path) = fs::canonicalize(&path) else {
                continue;
            };
            self.dirty_files
                .lock()
                .map_err(|_| AppError::Tracker)?
                .remove(&safe_path);
            if !safe_path.starts_with(&replay_dir) {
                continue;
            }
            let Ok(metadata) = fs::metadata(&safe_path) else {
                continue;
            };
            let stamp = (
                metadata.len(),
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            );
            if self
                .processed_files
                .lock()
                .map_err(|_| AppError::Tracker)?
                .get(&safe_path)
                == Some(&stamp)
            {
                continue;
            }
            {
                let mut stable = self.stable_files.lock().map_err(|_| AppError::Tracker)?;
                if stable.insert(safe_path.clone(), stamp) != Some(stamp) {
                    continue;
                }
            }
            {
                let mut retries = self.retry_files.lock().map_err(|_| AppError::Tracker)?;
                if let Some(retry) = retries.get(&safe_path).copied() {
                    if retry.stamp != stamp {
                        retries.remove(&safe_path);
                    } else if retry.attempts >= MAX_REPLAY_RETRIES
                        || self.clock.now() < retry.next_retry
                    {
                        continue;
                    }
                }
            }
            let catalog = self.catalog_snapshot();
            match parse_replay_file_with(&safe_path, catalog.as_ref()) {
                Ok(parsed) => {
                    self.retry_files
                        .lock()
                        .map_err(|_| AppError::Tracker)?
                        .remove(&safe_path);
                    let mut imported = parsed.into_imported(catalog.as_ref());
                    if !passes_import_gate(&imported.battle) {
                        self.database.with_transaction(|tx| {
                            Database::record_skipped_replay_tx(
                                tx,
                                &imported.file_state,
                                &imported.battle,
                            )
                        })?;
                        self.processed_files
                            .lock()
                            .map_err(|_| AppError::Tracker)?
                            .insert(safe_path, stamp);
                        continue;
                    }
                    match self.database.upsert_imported(
                        &mut imported.battle,
                        &imported.file_state,
                        true,
                        Some(&imported.team_relative_facts),
                        &imported.extras,
                    )? {
                        ReplayUpsertOutcome::Inserted | ReplayUpsertOutcome::Updated => {
                            result.imported += 1;
                            if imported.battle.data_quality == DataQuality::Partial {
                                result.partial += 1;
                            } else {
                                self.process_challenge(&imported.battle.arena_id);
                            }
                        }
                        ReplayUpsertOutcome::Duplicate => result.duplicates += 1,
                    }
                    let mut s = self.status.lock().unwrap_or_else(|e| e.into_inner());
                    s.last_replay_at = Some(imported.battle.battle_time);
                    s.client_version = imported.client_version.or(s.client_version.clone());
                    drop(s);
                    self.processed_files
                        .lock()
                        .map_err(|_| AppError::Tracker)?
                        .insert(safe_path, stamp);
                }
                Err(AppError::ReplayIncomplete) => {
                    self.note_replay_retry(&safe_path, stamp)?;
                }
                Err(_) => {
                    self.note_replay_retry(&safe_path, stamp)?;
                    let mut s = self.status.lock().unwrap_or_else(|e| e.into_inner());
                    s.message = Some("replay_parse_failed".into());
                }
            }
        }
        self.update_parser_state()?;
        if result.imported > 0 {
            self.emit(AppEvent::DataChanged);
        }
        Ok(result)
    }

    fn note_replay_retry(&self, path: &Path, stamp: (u64, SystemTime)) -> Result<(), AppError> {
        let mut retries = self.retry_files.lock().map_err(|_| AppError::Tracker)?;
        let attempts = retries
            .get(path)
            .filter(|retry| retry.stamp == stamp)
            .map_or(1, |retry| retry.attempts.saturating_add(1));
        let delay_seconds = 1_u64 << u32::from(attempts.min(5));
        retries.insert(
            path.to_path_buf(),
            RetryState {
                stamp,
                attempts,
                next_retry: self.clock.now() + Duration::from_secs(delay_seconds),
            },
        );
        Ok(())
    }
}

fn prune_file_cache(cache: &Mutex<HashMap<PathBuf, (u64, SystemTime)>>) -> Result<(), AppError> {
    let mut cache = cache.lock().map_err(|_| AppError::Tracker)?;
    cache.retain(|path, _| path.exists());
    if cache.len() > MAX_TRACKED_FILES {
        cache.clear();
    }
    Ok(())
}

fn source_credential() -> Result<keyring::Entry, AppError> {
    keyring::Entry::new(SOURCE_CREDENTIAL_SERVICE, SOURCE_CREDENTIAL_USER)
        .map_err(|_| AppError::Credential)
}

/// The configured game's `replays` directory, for the P8 gate of the v10 rebuild at startup.
pub(crate) fn configured_replay_dir() -> Option<PathBuf> {
    let root = validate_root(Path::new(&load_source_root()?)).ok()?;
    fs::canonicalize(root.join("replays")).ok()
}

fn load_source_root() -> Option<String> {
    source_credential().ok()?.get_password().ok()
}

fn store_source_root(path: &str) -> Result<(), AppError> {
    source_credential()?
        .set_password(path)
        .map_err(|_| AppError::Credential)
}

fn delete_source_root() -> Result<(), AppError> {
    match source_credential()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(AppError::Credential),
    }
}

fn validate_root(path: &Path) -> Result<PathBuf, AppError> {
    let root = fs::canonicalize(path).map_err(|_| AppError::InvalidGameDirectory)?;
    let has_executable = root.join("WorldOfTanks.exe").is_file()
        || root.join("win64").join("WorldOfTanks.exe").is_file();
    if !root.is_dir() || !root.join("replays").is_dir() || !has_executable {
        return Err(AppError::InvalidGameDirectory);
    }
    let replays =
        fs::canonicalize(root.join("replays")).map_err(|_| AppError::InvalidGameDirectory)?;
    if !replays.starts_with(&root) {
        return Err(AppError::InvalidGameDirectory);
    }
    Ok(root)
}

#[cfg(test)]
fn parse_replay_file(path: &Path) -> Result<ParsedReplay, AppError> {
    parse_replay_file_with(path, None)
}

/// Parses one replay. `catalog` gives the tier and class of every roster vehicle.
fn parse_replay_file_with(
    path: &Path,
    catalog: Option<&LoadedVehicleClassCatalog>,
) -> Result<ParsedReplay, AppError> {
    let file_metadata = fs::metadata(path)?;
    let file_size = file_metadata.len();
    if file_size > MAX_REPLAY_BYTES {
        return Err(AppError::ReplayTooLarge);
    }
    if file_size < 12 {
        return Err(AppError::ReplayIncomplete);
    }
    let mut file = File::open(path)?;
    let magic = read_replay_u32(&mut file)?;
    if magic != REPLAY_MAGIC {
        return Err(AppError::InvalidReplay);
    }
    let block_count = read_replay_u32(&mut file)?;
    if block_count == 0 || block_count > MAX_REPLAY_BLOCKS {
        return Err(AppError::InvalidReplay);
    }
    // E3: the content fingerprint covers the header and the JSON blocks only; the packet stream
    // after them is never read.
    let mut hasher = Sha256::new();
    hasher.update(magic.to_le_bytes());
    hasher.update(block_count.to_le_bytes());
    let mut json_blocks = Vec::with_capacity(usize::min(block_count as usize, 2));
    for index in 0..block_count {
        let block_length = read_replay_u32(&mut file)? as usize;
        hasher.update((block_length as u32).to_le_bytes());
        let limit = if index == 0 {
            MAX_HEADER_BYTES
        } else {
            MAX_RESULTS_BYTES
        };
        if block_length == 0 || block_length > limit {
            return Err(AppError::InvalidReplay);
        }
        let position = file.stream_position()?;
        let end = position
            .checked_add(block_length as u64)
            .ok_or(AppError::InvalidReplay)?;
        if end > file_size {
            return Err(AppError::ReplayIncomplete);
        }
        if index < 2 {
            let mut block = vec![0_u8; block_length];
            file.read_exact(&mut block)?;
            hasher.update(&block);
            json_blocks.push(block);
        } else {
            file.seek(SeekFrom::Start(end))?;
        }
    }
    let packet_stream_bytes =
        crate::foundations::packet_stream_bytes(file_size, file.stream_position()?);
    let metadata_json: Value =
        serde_json::from_slice(json_blocks.first().ok_or(AppError::ReplayIncomplete)?)
            .map_err(|_| AppError::InvalidReplay)?;
    let client_version = text(
        &metadata_json,
        &["clientVersionFromXml", "clientVersion", "version"],
    );
    let mut battle =
        replay_from_metadata(String::new(), &metadata_json, file_metadata.modified().ok())?;
    battle.game_version = client_version.clone();
    battle.arena_id = provisional_battle_id(&battle);
    let pace_metadata = parse_replay_pace_metadata(&metadata_json);

    let (results, parser_error_code, results_value) = if let Some(results_json) = json_blocks.get(1)
    {
        match serde_json::from_slice::<Value>(results_json) {
            Ok(value) => match parse_battle_results(&value, &pace_metadata, &battle) {
                Ok(results) => {
                    let parser_error_code = results.parser_error_code;
                    (Some(results), parser_error_code, Some(value))
                }
                Err(error) => (None, Some(error.code()), Some(value)),
            },
            Err(_) => (None, Some("invalid_results_json"), None),
        }
    } else {
        (None, None, None)
    };
    let team_relative_facts = extract_team_relative_facts(&TeamRelativeParseInput {
        data_quality: if results.is_some() {
            DataQuality::Full
        } else {
            DataQuality::Partial
        },
        data_completeness: results.as_ref().map(|_| "results"),
        replay_source: Some(if results.is_some() {
            "replay_results"
        } else {
            "replay_metadata"
        }),
        parser_version: Some(REPLAY_PARSER_VERSION),
        feature_schema_version: results.as_ref().map(|_| FEATURE_SCHEMA_VERSION),
        metadata: &metadata_json,
        results: results_value.as_ref(),
        canonical_damage: results.as_ref().and_then(|results| results.damage),
        canonical_frags: results.as_ref().and_then(|results| results.frags),
        canonical_original_xp: results.as_ref().and_then(|results| results.original_xp),
    });
    let lookup = |type_comp_descr: i64| {
        catalog
            .and_then(|catalog| catalog.vehicle(type_comp_descr))
            .map(|vehicle| CatalogVehicle {
                tier: vehicle.tier,
                class: vehicle.vehicle_class,
            })
    };
    let extras = match (results.as_ref(), results_value.as_ref()) {
        (Some(_), Some(value)) => ImportExtras {
            own: Some(extract_own_player_facts(value)),
            roster: extract_battle_roster(&metadata_json, value, &lookup).ok(),
            packet_stream_bytes,
            ..ImportExtras::default()
        },
        _ => ImportExtras {
            packet_stream_bytes,
            ..ImportExtras::default()
        },
    };

    let content_fingerprint = hex_digest(hasher.finalize());
    let source_path_key = source_path_key(path);
    let file_mtime_ms = file_mtime_ms(&file_metadata);
    Ok(ParsedReplay {
        metadata: ReplayMetadata {
            battle,
            client_version,
            pace: pace_metadata,
        },
        results,
        block_count,
        parser_version: REPLAY_PARSER_VERSION,
        parser_error_code,
        file_state: ReplayFileState {
            source_path_key,
            file_size: i64::try_from(file_size).map_err(|_| AppError::ReplayTooLarge)?,
            file_mtime_ms,
            content_fingerprint,
            block_count: i64::from(block_count),
        },
        team_relative_facts,
        extras,
    })
}

fn file_mtime_ms(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

/// Stable key of a replay file: hash of its canonical, lower-cased path.
pub(crate) fn source_path_key(canonical_path: &Path) -> String {
    hash_text(&canonical_path.to_string_lossy().to_ascii_lowercase())
}

/// Keys of every `.wotreplay` file in `replay_dir` (for the P8 gate of the v10 rebuild).
/// `None` when the directory cannot be read.
pub(crate) fn replay_source_keys(replay_dir: &Path) -> Option<HashSet<String>> {
    let entries = fs::read_dir(replay_dir).ok()?;
    Some(
        entries
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_type()
                    .is_ok_and(|kind| kind.is_file() && !kind.is_symlink())
            })
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("wotreplay"))
            })
            .filter_map(|path| fs::canonicalize(path).ok())
            .map(|path| source_path_key(&path))
            .collect(),
    )
}

/// Diagnostics of the P8 gate only: `arenaUniqueID` values read from the result blocks of the
/// `.wotreplay` files directly in `replay_dir` (the same files as [`replay_source_keys`]).
/// Returns (files seen, files with a result block, distinct ids).
#[cfg(test)]
pub(crate) fn replay_arena_unique_ids(replay_dir: &Path) -> Option<(usize, usize, HashSet<i64>)> {
    let entries = fs::read_dir(replay_dir).ok()?;
    let (mut files, mut with_results, mut ids) = (0, 0, HashSet::new());
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let is_replay = entry
            .file_type()
            .is_ok_and(|kind| kind.is_file() && !kind.is_symlink())
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("wotreplay"));
        if !is_replay {
            continue;
        }
        files += 1;
        if let Ok(parsed) = parse_replay_file(&path)
            && let Some(id) = parsed.into_battle().0.arena_unique_id
        {
            with_results += 1;
            ids.insert(id);
        }
    }
    Some((files, with_results, ids))
}

fn read_replay_u32(file: &mut File) -> Result<u32, AppError> {
    let mut bytes = [0_u8; 4];
    file.read_exact(&mut bytes)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::UnexpectedEof => AppError::ReplayIncomplete,
            _ => AppError::Io(error),
        })?;
    Ok(u32::from_le_bytes(bytes))
}

fn hash_text(value: &str) -> String {
    hex_digest(Sha256::digest(value.as_bytes()))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn provisional_battle_id(battle: &Battle) -> String {
    format!(
        "local:{}",
        hash_text(&format!(
            "{}|{}|{}|{}",
            battle.battle_time,
            battle.vehicle_key,
            battle.map_name.as_deref().unwrap_or_default(),
            battle.battle_type.as_deref().unwrap_or_default()
        ))
    )
}

fn parse_replay_pace_metadata(root: &Value) -> ReplayPaceMetadata {
    // Strict JSON integer: a string or float `battleType` is not a battle mode (audit M1).
    let battle_type_raw = root.get("battleType").and_then(Value::as_i64);
    let Some(vehicles) = root.get("vehicles").and_then(Value::as_object) else {
        return ReplayPaceMetadata {
            battle_type_raw,
            extraction_error: Some(BattlePaceReason::MissingFinalRoster),
            ..Default::default()
        };
    };
    if vehicles.is_empty() {
        return ReplayPaceMetadata {
            battle_type_raw,
            extraction_error: Some(BattlePaceReason::MissingFinalRoster),
            ..Default::default()
        };
    }
    let mut vehicle_teams = HashMap::with_capacity(vehicles.len());
    let mut team1 = 0_i64;
    let mut team2 = 0_i64;
    for (vehicle_id, vehicle) in vehicles {
        let Some(team) = vehicle.get("team").and_then(as_i64) else {
            return ReplayPaceMetadata {
                battle_type_raw,
                extraction_error: Some(BattlePaceReason::MissingTeamAssignment),
                ..Default::default()
            };
        };
        match team {
            1 => team1 += 1,
            2 => team2 += 1,
            _ => {
                return ReplayPaceMetadata {
                    battle_type_raw,
                    extraction_error: Some(BattlePaceReason::MissingTeamAssignment),
                    ..Default::default()
                };
            }
        }
        vehicle_teams.insert(vehicle_id.clone(), team);
    }
    ReplayPaceMetadata {
        battle_type_raw,
        initial_team_sizes: Some(BattlePaceTeamSizes { team1, team2 }),
        vehicle_teams: Some(vehicle_teams),
        extraction_error: None,
    }
}

fn final_destroyed_score(
    summary: &serde_json::Map<String, Value>,
    metadata: &ReplayPaceMetadata,
) -> (Option<BattlePaceDestroyedScore>, Option<BattlePaceReason>) {
    if let Some(reason) = metadata.extraction_error {
        return (None, Some(reason));
    }
    let Some(initial_teams) = metadata.vehicle_teams.as_ref() else {
        return (None, Some(BattlePaceReason::MissingFinalRoster));
    };
    let Some(final_vehicles) = summary.get("vehicles").and_then(Value::as_object) else {
        return (None, Some(BattlePaceReason::MissingFinalRoster));
    };
    if final_vehicles.len() != initial_teams.len() {
        return (None, Some(BattlePaceReason::ConflictingData));
    }

    let mut team1 = 0_i64;
    let mut team2 = 0_i64;
    for (vehicle_id, initial_team) in initial_teams {
        let Some(rollouts) = final_vehicles.get(vehicle_id).and_then(Value::as_array) else {
            return (None, Some(BattlePaceReason::MissingFinalRoster));
        };
        if rollouts.len() != 1 {
            return (None, Some(BattlePaceReason::ConflictingData));
        }
        let Some(rollout) = rollouts.first().and_then(Value::as_object) else {
            return (None, Some(BattlePaceReason::MissingFinalRoster));
        };
        let Some(final_team) = rollout.get("team").and_then(as_i64) else {
            return (None, Some(BattlePaceReason::MissingTeamAssignment));
        };
        if final_team != *initial_team {
            return (None, Some(BattlePaceReason::ConflictingData));
        }
        let Some(death_reason) = rollout.get("deathReason").and_then(as_i64) else {
            return (None, Some(BattlePaceReason::MissingFinalRoster));
        };
        if !(-1..=64).contains(&death_reason) {
            return (None, Some(BattlePaceReason::ConflictingData));
        }
        if death_reason != -1 {
            if final_team == 1 {
                team2 += 1;
            } else if final_team == 2 {
                team1 += 1;
            } else {
                return (None, Some(BattlePaceReason::MissingTeamAssignment));
            }
        }
    }
    (Some(BattlePaceDestroyedScore { team1, team2 }), None)
}

fn parse_battle_results(
    root: &Value,
    pace_metadata: &ReplayPaceMetadata,
    metadata_battle: &Battle,
) -> Result<BattleResults, BattleResultsError> {
    let sections = root.as_array().ok_or(BattleResultsError::InvalidShape)?;
    let summary = sections
        .first()
        .and_then(Value::as_object)
        .ok_or(BattleResultsError::InvalidShape)?;
    let arena_unique_id = summary
        .get("arenaUniqueID")
        .and_then(as_i64)
        .filter(|value| *value > 0)
        .ok_or(BattleResultsError::InvalidShape)?;
    let personal = summary
        .get("personal")
        .and_then(Value::as_object)
        .ok_or(BattleResultsError::InvalidShape)?;
    let mut personal_entries = personal
        .iter()
        .filter(|(key, value)| key.as_str() != "avatar" && value.is_object());
    let (_, personal_stats) = personal_entries
        .next()
        .ok_or(BattleResultsError::InvalidShape)?;
    if personal_entries.next().is_some() {
        return Err(BattleResultsError::InvalidShape);
    }
    let personal_stats = personal_stats
        .as_object()
        .ok_or(BattleResultsError::InvalidShape)?;
    let account_id = personal_stats.get("accountDBID").and_then(as_i64);
    let arena_vehicle_id = account_id.and_then(|account_id| {
        summary
            .get("vehicles")?
            .as_object()?
            .iter()
            .find_map(|(vehicle_id, entries)| {
                entries
                    .as_array()?
                    .iter()
                    .any(|entry| entry.get("accountDBID").and_then(as_i64) == Some(account_id))
                    .then(|| vehicle_id.clone())
            })
    });
    let frags = final_player_frags(sections, arena_vehicle_id.as_deref(), personal_stats);
    // D8 / audit M4: assist is spotting (radio) + tracking, as in the game's results screen.
    // Stun is stored separately; smoke and inspire are not part of assist.
    let assist_fields = ["damageAssistedRadio", "damageAssistedTrack"];
    let assist_values = assist_fields
        .iter()
        .filter_map(|key| personal_stats.get(*key).and_then(as_i64))
        .filter(|value| *value >= 0)
        .collect::<Vec<_>>();
    let assist = (!assist_values.is_empty())
        .then(|| assist_values.into_iter().try_fold(0_i64, i64::checked_add))
        .flatten()
        .filter(|value| *value <= 200_000);
    let player_team = bounded_object_field(personal_stats, &["team"], 2);
    let winner_team_raw = summary
        .get("common")
        .and_then(|value| bounded_field(value, &["winnerTeam"], 2));
    let outcome = BattleOutcome::from_replay_teams(player_team, winner_team_raw);
    let common = summary.get("common").and_then(Value::as_object);
    let bonus_type_raw = common
        .and_then(|value| value.get("bonusType"))
        .and_then(Value::as_i64);
    let finish_reason_raw = common
        .and_then(|value| value.get("finishReason"))
        .and_then(as_i64);
    let duration_seconds = common
        .and_then(|value| value.get("duration"))
        .and_then(as_i64)
        .filter(|value| (0..=7_200).contains(value));
    let (destroyed_score, roster_error) = final_destroyed_score(summary, pace_metadata);
    let pace_resolution = resolve_battle_pace(&BattlePaceInput {
        data_quality: DataQuality::Full,
        feature_schema_version: Some(FEATURE_SCHEMA_VERSION),
        battle_type_raw: pace_metadata.battle_type_raw,
        bonus_type_raw,
        gameplay_id: metadata_battle.gameplay_id.clone(),
        duration_seconds,
        finish_reason_raw,
        initial_team_sizes: pace_metadata.initial_team_sizes,
        destroyed_score,
        player_team,
        winner_team_raw,
        outcome,
        extraction_error: roster_error.or(pace_metadata.extraction_error),
        provenance: BattlePaceProvenance::ReplayResults,
    });
    let survived = personal_stats
        .get("deathReason")
        .and_then(as_i64)
        .filter(|value| (-1..=64).contains(value))
        .map(|value| value == -1);
    Ok(BattleResults {
        arena_unique_id,
        wg_tank_id: bounded_object_field(personal_stats, &["typeCompDescr"], i64::MAX),
        outcome,
        player_team,
        winner_team_raw,
        damage: bounded_object_field(personal_stats, &["damageDealt"], 200_000),
        assist,
        assist_spotting: bounded_object_field(personal_stats, &["damageAssistedRadio"], 200_000),
        assist_tracking: bounded_object_field(personal_stats, &["damageAssistedTrack"], 200_000),
        assist_stun: bounded_object_field(personal_stats, &["damageAssistedStun"], 200_000),
        blocked: bounded_object_field(personal_stats, &["damageBlockedByArmor"], 500_000),
        damage_received: bounded_object_field(personal_stats, &["damageReceived"], 500_000),
        frags: frags.value,
        spots: bounded_object_field(personal_stats, &["spotted"], 30),
        shots: bounded_object_field(personal_stats, &["shots"], 2_000),
        hits: bounded_object_field(personal_stats, &["directHits"], 2_000),
        direct_enemy_hits: strict_counter_field(personal_stats, "directEnemyHits", 2_000),
        direct_team_hits: strict_counter_field(personal_stats, "directTeamHits", 2_000),
        penetrations: bounded_object_field(personal_stats, &["piercings"], 2_000),
        piercing_enemy_hits: strict_counter_field(personal_stats, "piercingEnemyHits", 2_000),
        explosion_hits: strict_counter_field(personal_stats, "explosionHits", 2_000),
        survived,
        xp: bounded_object_field(personal_stats, &["originalXP", "xp"], 100_000),
        original_xp: bounded_object_field(personal_stats, &["originalXP"], 100_000),
        duration_seconds,
        defense_points: bounded_object_field(personal_stats, &["droppedCapturePoints"], 1_000),
        moving_avg_damage: bounded_object_field(personal_stats, &["movingAvgDamage"], 200_000),
        damage_rating: bounded_object_field(personal_stats, &["damageRating"], 100),
        pace_resolution,
        parser_error_code: frags.parser_error_code,
    })
}

fn final_player_frags(
    sections: &[Value],
    arena_vehicle_id: Option<&str>,
    personal_stats: &serde_json::Map<String, Value>,
) -> FinalFragsResolution {
    let Some(personal_frags) = bounded_object_field(personal_stats, &["kills"], MAX_FINAL_FRAGS)
    else {
        return FinalFragsResolution {
            value: None,
            parser_error_code: None,
        };
    };
    let (Some(vehicle_id), Some(final_section)) = (arena_vehicle_id, sections.get(2)) else {
        return FinalFragsResolution {
            value: Some(personal_frags),
            parser_error_code: None,
        };
    };
    let Some(final_section) = final_section.as_object() else {
        return FinalFragsResolution {
            value: None,
            parser_error_code: Some("invalid_final_frags_confirmation"),
        };
    };
    let Some(own_entry) = final_section.get(vehicle_id) else {
        return FinalFragsResolution {
            value: Some(personal_frags),
            parser_error_code: None,
        };
    };
    let Some(own_entry) = own_entry.as_object() else {
        return FinalFragsResolution {
            value: None,
            parser_error_code: Some("invalid_final_frags_confirmation"),
        };
    };
    let Some(value) = own_entry.get("frags") else {
        return FinalFragsResolution {
            value: Some(personal_frags),
            parser_error_code: None,
        };
    };
    let Some(confirmed) = as_i64(value).filter(|value| (0..=MAX_FINAL_FRAGS).contains(value))
    else {
        return FinalFragsResolution {
            value: None,
            parser_error_code: Some("invalid_final_frags_confirmation"),
        };
    };
    if confirmed != personal_frags {
        return FinalFragsResolution {
            value: None,
            parser_error_code: Some("final_frags_conflict"),
        };
    }
    FinalFragsResolution {
        value: Some(personal_frags),
        parser_error_code: None,
    }
}

fn bounded_object_field(
    value: &serde_json::Map<String, Value>,
    keys: &[&str],
    maximum: i64,
) -> Option<i64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(as_i64)
        .filter(|value| (0..=maximum).contains(value))
}

fn strict_counter_field(
    value: &serde_json::Map<String, Value>,
    key: &str,
    maximum: i64,
) -> Option<i64> {
    let number = value.get(key)?.as_number()?;
    number
        .as_i64()
        .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
        .filter(|value| (0..=maximum).contains(value))
}

fn bounded_field(value: &Value, keys: &[&str], maximum: i64) -> Option<i64> {
    value
        .as_object()
        .and_then(|object| bounded_object_field(object, keys, maximum))
}

fn replay_from_metadata(
    id: String,
    v: &Value,
    modified: Option<SystemTime>,
) -> Result<Battle, AppError> {
    let vehicle =
        text(v, &["playerVehicle", "vehicle", "vehicleName"]).ok_or(AppError::InvalidReplay)?;
    let metadata_battle_time =
        text(v, &["dateTime", "battleTime"]).and_then(|d| parse_replay_time(&d));
    let (battle_time, battle_time_source) = if let Some(value) = metadata_battle_time {
        (value, "metadata")
    } else {
        let value = modified
            .and_then(|m| m.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .ok_or(AppError::InvalidReplay)?;
        (value, "file_mtime")
    };
    let map = text(v, &["mapDisplayName", "mapName"]);
    let battle_type = text(v, &["gameplayID", "battleType"]);
    Ok(Battle {
        arena_id: id,
        arena_unique_id: None,
        battle_time,
        battle_time_source: Some(battle_time_source.into()),
        vehicle_key: canonicalize_vehicle_key(&vehicle)
            .map(|key| key.as_str().to_owned())
            .unwrap_or_default(),
        wg_tank_id: integer(v, &["vehicleCompDescr", "tankId"]),
        tank_name: Some(vehicle),
        map_name: map,
        map_key: text(v, &["mapName"]),
        battle_type,
        gameplay_id: text(v, &["gameplayID"]),
        source_kind: SourceKind::Replay,
        data_quality: DataQuality::Partial,
        parser_version: Some(REPLAY_PARSER_VERSION),
        feature_schema_version: None,
        game_version: text(v, &["clientVersionFromXml", "clientVersion", "version"]),
        data_completeness: Some("metadata_only".into()),
        replay_source: Some("replay_metadata".into()),
        parser_error_code: None,
        outcome: BattleOutcome::Unknown,
        battle_pace: Default::default(),
        player_team: None,
        winner_team_raw: None,
        damage: None,
        assist: None,
        assist_spotting: None,
        assist_tracking: None,
        assist_stun: None,
        blocked: None,
        damage_received: None,
        frags: None,
        spots: None,
        shots: None,
        hits: None,
        direct_enemy_hits: None,
        direct_team_hits: None,
        penetrations: None,
        piercing_enemy_hits: None,
        explosion_hits: None,
        defense_points: None,
        moving_avg_damage: None,
        damage_rating: None,
        survived: None,
        xp: None,
        original_xp: None,
        duration_seconds: None,
        session_id: None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CanonicalVehicleKey(String);

impl CanonicalVehicleKey {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanonicalVehicleKeyError {
    Empty,
}

pub(crate) fn canonicalize_vehicle_key(
    value: &str,
) -> Result<CanonicalVehicleKey, CanonicalVehicleKeyError> {
    let canonical: String = value
        .trim()
        .to_ascii_lowercase()
        .replace(':', "-")
        .replace(' ', "_")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(128)
        .collect();
    if canonical.is_empty() {
        Err(CanonicalVehicleKeyError::Empty)
    } else {
        Ok(CanonicalVehicleKey(canonical))
    }
}
/// Spec 20, D4: the import gate reads the numeric `battleType` from the replay metadata; text
/// such as `gameplayID` never classifies a battle (audit M1). `bonusType` is checked later by
/// `stats_eligibility`: such battles are stored but never counted. Partial replays have no
/// `bonusType` and pass the gate with `battleType == 1`.
fn passes_import_gate(battle: &Battle) -> bool {
    battle.battle_pace.battle_type_raw == Some(1)
}
fn parse_replay_time(v: &str) -> Option<i64> {
    parse_replay_time_in(v, &Local)
}

fn parse_replay_time_in<Tz: TimeZone>(v: &str, zone: &Tz) -> Option<i64> {
    ["%d.%m.%Y %H:%M:%S", "%Y-%m-%d %H:%M:%S"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(v, f).ok())
        // Audit D10: in the repeated autumn hour take the earlier instant instead of dropping it.
        .and_then(|n| zone.from_local_datetime(&n).earliest())
        .map(|d| d.timestamp_millis())
        .filter(|timestamp| {
            *timestamp >= 1_262_304_000_000
                && *timestamp <= Utc::now().timestamp_millis() + 24 * 60 * 60 * 1000
        })
}
fn text(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| v.get(*k))
        .and_then(|v| v.as_str())
        .map(|v| v.trim().chars().take(256).collect())
}
fn integer(v: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|k| v.get(*k)).and_then(as_i64)
}
fn as_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().and_then(|n| i64::try_from(n).ok()))
        .or_else(|| v.as_f64().map(|n| n as i64))
        .or_else(|| v.as_str()?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{battle_pace::BATTLE_PACE_RESOLVER_VERSION, models::BattlePaceContext};
    use serde_json::json;
    use tempfile::tempdir;

    fn tracker(database: Database) -> LocalTracker {
        LocalTracker::build(database, Arc::new(RwLock::new(None)), None, None, false)
    }

    // N1: in recovery mode a replay in the game folder must never reach the recovery database.
    #[test]
    fn disabled_tracker_refuses_import_and_configuration() {
        let directory = tempdir().unwrap();
        let replay_dir = directory.path().join("replays");
        fs::create_dir(&replay_dir).unwrap();
        fs::write(directory.path().join("WorldOfTanks.exe"), []).unwrap();
        fs::write(
            replay_dir.join("synthetic.wotreplay"),
            completed_replay_bytes(
                json!({"battleType": 1, "dateTime": "01.01.2026 12:00:00",
                       "mapName": "map_a", "playerVehicle": "nation-Vehicle_A"}),
                realistic_results(),
            ),
        )
        .unwrap();
        let database = Database::initialize(directory.path().join("recovery.sqlite3")).unwrap();
        let tracker = LocalTracker::disabled(database.clone(), Arc::new(RwLock::new(None)));
        *tracker.root.write().unwrap() = Some(fs::canonicalize(directory.path()).unwrap());
        tracker.paused.store(false, Ordering::Relaxed);
        for _ in 0..2 {
            assert!(matches!(
                tracker.import_history(),
                Err(AppError::RecoveryModeTrackerDisabled)
            ));
            assert!(matches!(
                tracker.reconcile(),
                Err(AppError::RecoveryModeTrackerDisabled)
            ));
        }
        assert!(matches!(
            tracker.configure(&directory.path().to_string_lossy()),
            Err(AppError::RecoveryModeTrackerDisabled)
        ));
        assert!(database.recent_battles(10).unwrap().is_empty());
        assert_eq!(tracker.status().parser_state, ParserState::Error);
        assert!(tracker.status().message.is_some());
    }

    fn replay_bytes(metadata: Value) -> Vec<u8> {
        let j = serde_json::to_vec(&metadata).unwrap();
        let mut b = Vec::new();
        b.extend(REPLAY_MAGIC.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend((j.len() as u32).to_le_bytes());
        b.extend(j);
        b
    }
    fn captured_replay_bytes(metadata: Value, blocks: u32) -> Vec<u8> {
        let json = serde_json::to_vec(&metadata).unwrap();
        let mut bytes = Vec::new();
        bytes.extend([0x12, 0x32, 0x34, 0x11]);
        bytes.extend(blocks.to_le_bytes());
        bytes.extend((json.len() as u32).to_le_bytes());
        bytes.extend(json);
        bytes
    }
    fn completed_replay_bytes(metadata: Value, results: Value) -> Vec<u8> {
        let metadata = serde_json::to_vec(&metadata).unwrap();
        let results = serde_json::to_vec(&results).unwrap();
        replay_with_raw_second_block(metadata, results)
    }
    fn replay_with_raw_second_block(metadata: Vec<u8>, results: Vec<u8>) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(REPLAY_MAGIC.to_le_bytes());
        bytes.extend(2u32.to_le_bytes());
        bytes.extend((metadata.len() as u32).to_le_bytes());
        bytes.extend(metadata);
        bytes.extend((results.len() as u32).to_le_bytes());
        bytes.extend(results);
        bytes
    }
    fn realistic_results() -> Value {
        json!([{
            "personal": {
                "33345": {
                    "accountDBID": 1001,
                    "typeCompDescr": 33345,
                    "damageDealt": 9061,
                    "damageReceived": 2738,
                    "damageBlockedByArmor": 0,
                    "damageAssistedRadio": 254,
                    "damageAssistedTrack": 0,
                    "damageAssistedStun": 0,
                    "damageAssistedSmoke": 0,
                    "droppedCapturePoints": 7,
                    "movingAvgDamage": 4490,
                    "damageRating": 84,
                    "shots": 27,
                    "directHits": 23,
                    "directEnemyHits": 22,
                    "directTeamHits": 1,
                    "piercings": 20,
                    "piercingEnemyHits": 20,
                    "explosionHits": 2,
                    "originalXP": 1439,
                    "kills": 4,
                    "deathReason": -1,
                    "spotted": 1,
                    "team": 2
                },
                "avatar": {}
            },
            "vehicles": {
                "854161": [{"accountDBID": 1001, "typeCompDescr": 33345}]
            },
            "common": {"winnerTeam": 2},
            "arenaUniqueID": 1111222233334444_i64
        }, {
            "854161": {"frags": 4, "team": 2}
        }, {
            "854161": {"frags": 4}
        }])
    }

    fn frag_results(
        personal_kills: Option<Value>,
        initial_frags: Value,
        final_section: Option<Value>,
    ) -> Value {
        let mut results = realistic_results();
        let personal = results[0]["personal"]["33345"].as_object_mut().unwrap();
        match personal_kills {
            Some(value) => {
                personal.insert("kills".into(), value);
            }
            None => {
                personal.remove("kills");
            }
        }
        results[1]["854161"]["frags"] = initial_frags;
        match final_section {
            Some(value) => results[2] = value,
            None => {
                results.as_array_mut().unwrap().pop();
            }
        }
        results
    }

    fn parse_frag_battle(results: Value) -> Battle {
        let directory = tempdir().unwrap();
        let path = directory.path().join("frags.wotreplay");
        fs::write(
            &path,
            completed_replay_bytes(
                json!({
                    "battleType": 1,
                    "playerVehicle": "synthetic-vehicle",
                    "dateTime": "11.07.2026 11:16:00"
                }),
                results,
            ),
        )
        .unwrap();
        parse_replay_file(&path).unwrap().into_battle().0
    }

    #[test]
    fn final_frags_use_personal_kills_instead_of_initial_section() {
        for (personal, initial) in [(3, 0), (2, 7), (0, 0)] {
            let battle = parse_frag_battle(frag_results(
                Some(json!(personal)),
                json!(initial),
                Some(json!({"854161": {"frags": personal}})),
            ));
            assert_eq!(battle.data_quality, DataQuality::Full);
            assert_eq!(battle.frags, Some(personal));
            assert_eq!(battle.parser_error_code, None);
        }
    }

    #[test]
    fn final_frags_accept_personal_kills_without_final_confirmation() {
        let battle = parse_frag_battle(frag_results(Some(json!(2)), json!(0), None));
        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.frags, Some(2));
        assert_eq!(battle.parser_error_code, None);
    }

    #[test]
    fn final_frags_conflict_is_explicit_and_fail_closed() {
        let battle = parse_frag_battle(frag_results(
            Some(json!(2)),
            json!(0),
            Some(json!({"854161": {"frags": 3}})),
        ));
        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.frags, None);
        assert_eq!(
            battle.parser_error_code.as_deref(),
            Some("final_frags_conflict")
        );
    }

    #[test]
    fn final_frags_never_fall_back_when_personal_kills_are_missing() {
        let battle = parse_frag_battle(frag_results(
            None,
            json!(4),
            Some(json!({"854161": {"frags": 4}})),
        ));
        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.frags, None);
    }

    #[test]
    fn final_frags_join_only_the_own_player_entry() {
        let battle = parse_frag_battle(frag_results(
            Some(json!(2)),
            json!(7),
            Some(json!({
                "111": {"frags": 15},
                "854161": {"frags": 2},
                "999": {"frags": 14},
                "tkills": {"111": 1}
            })),
        ));
        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.frags, Some(2));
    }

    #[test]
    fn final_frags_handle_missing_extra_and_malformed_confirmation_safely() {
        let missing_own = parse_frag_battle(frag_results(
            Some(json!(2)),
            json!(7),
            Some(json!({"111": {"frags": 9}})),
        ));
        assert_eq!(missing_own.data_quality, DataQuality::Full);
        assert_eq!(missing_own.frags, Some(2));

        let malformed_section =
            parse_frag_battle(frag_results(Some(json!(2)), json!(7), Some(json!("bad"))));
        assert_eq!(malformed_section.data_quality, DataQuality::Full);
        assert_eq!(malformed_section.frags, None);
        assert_eq!(
            malformed_section.parser_error_code.as_deref(),
            Some("invalid_final_frags_confirmation")
        );

        let malformed_value = parse_frag_battle(frag_results(
            Some(json!(2)),
            json!(7),
            Some(json!({"854161": {"frags": "two"}})),
        ));
        assert_eq!(malformed_value.data_quality, DataQuality::Full);
        assert_eq!(malformed_value.frags, None);
        assert_eq!(
            malformed_value.parser_error_code.as_deref(),
            Some("invalid_final_frags_confirmation")
        );
    }

    #[test]
    fn final_frags_bounds_do_not_overflow_or_truncate() {
        let special_mode = parse_frag_battle(frag_results(
            Some(json!(29)),
            json!(0),
            Some(json!({"854161": {"frags": 29}})),
        ));
        assert_eq!(special_mode.data_quality, DataQuality::Full);
        assert_eq!(special_mode.frags, Some(29));

        let malformed_personal = parse_frag_battle(frag_results(
            Some(json!("two")),
            json!(4),
            Some(json!({"854161": {"frags": 4}})),
        ));
        assert_eq!(malformed_personal.data_quality, DataQuality::Full);
        assert_eq!(malformed_personal.frags, None);

        let oversized_personal = parse_frag_battle(frag_results(
            Some(json!(i64::MAX)),
            json!(4),
            Some(json!({"854161": {"frags": 4}})),
        ));
        assert_eq!(oversized_personal.data_quality, DataQuality::Full);
        assert_eq!(oversized_personal.frags, None);

        let oversized_confirmation = parse_frag_battle(frag_results(
            Some(json!(2)),
            json!(0),
            Some(json!({"854161": {"frags": i64::MAX}})),
        ));
        assert_eq!(oversized_confirmation.data_quality, DataQuality::Full);
        assert_eq!(oversized_confirmation.frags, None);
        assert_eq!(
            oversized_confirmation.parser_error_code.as_deref(),
            Some("invalid_final_frags_confirmation")
        );
    }

    fn pace_metadata() -> Value {
        let mut vehicles = serde_json::Map::new();
        for vehicle_id in 1..=30 {
            vehicles.insert(
                vehicle_id.to_string(),
                json!({"team": if vehicle_id <= 15 { 1 } else { 2 }}),
            );
        }
        json!({
            "battleType": 1,
            "dateTime": "11.07.2026 11:16:00",
            "gameplayID": "ctf",
            "mapDisplayName": "Synthetic map",
            "mapName": "synthetic_map",
            "playerVehicle": "nation-Synthetic_Tank",
            "vehicles": vehicles
        })
    }

    fn pace_results(duration: i64, loser_destroyed_score: i64) -> Value {
        let mut vehicles = serde_json::Map::new();
        for vehicle_id in 1..=30 {
            let team = if vehicle_id <= 15 { 1 } else { 2 };
            let destroyed = team == 1 || (vehicle_id - 15) <= loser_destroyed_score;
            let account_id = if vehicle_id == 30 {
                1001
            } else {
                10_000 + vehicle_id
            };
            vehicles.insert(
                vehicle_id.to_string(),
                json!([{
                    "accountDBID": account_id,
                    "deathReason": if destroyed { 0 } else { -1 },
                    "team": team,
                    "typeCompDescr": if vehicle_id == 30 { 33345 } else { 100_000 + vehicle_id },
                    "damageDealt": if vehicle_id == 30 { 1234 } else { vehicle_id * 10 },
                    "kills": 0,
                    "xp": if vehicle_id == 30 { 777 } else { 400 + vehicle_id }
                }]),
            );
        }
        json!([{
            "personal": {
                "33345": {
                    "accountDBID": 1001,
                    "typeCompDescr": 33345,
                    "damageDealt": 1234,
                    "damageAssistedRadio": 10,
                    "damageAssistedTrack": 20,
                    "damageAssistedStun": 0,
                    "deathReason": -1,
                    "originalXP": 777,
                    "shots": 10,
                    "directHits": 8,
                    "piercings": 6,
                    "spotted": 2,
                    "team": 2
                },
                "avatar": {}
            },
            "vehicles": vehicles,
            "common": {
                "bonusType": 1,
                "duration": duration,
                "finishReason": 1,
                "winnerTeam": 2
            },
            "arenaUniqueID": 123456789_i64
        }, {
            "30": {"frags": 15, "isAlive": false, "team": 2}
        }])
    }

    #[test]
    fn parses_all_pace_inputs_and_derives_destroyed_score_only_from_death_reason() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("pace-inputs.wotreplay");
        fs::write(
            &path,
            completed_replay_bytes(pace_metadata(), pace_results(205, 0)),
        )
        .unwrap();

        let (battle, _, _) = parse_replay_file(&path).unwrap().into_battle();

        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(
            battle.battle_pace.context,
            Some(BattlePaceContext::BorderlineSteamroll)
        );
        assert_eq!(battle.battle_pace.reason, None);
        assert_eq!(battle.duration_seconds, Some(205));
        assert_eq!(battle.gameplay_id.as_deref(), Some("ctf"));
        assert_eq!(battle.battle_pace.battle_type_raw, Some(1));
        assert_eq!(battle.battle_pace.bonus_type_raw, Some(1));
        assert_eq!(battle.battle_pace.finish_reason_raw, Some(1));
        assert_eq!(battle.battle_pace.initial_team1_size, Some(15));
        assert_eq!(battle.battle_pace.initial_team2_size, Some(15));
        assert_eq!(battle.battle_pace.destroyed_team1_score, Some(0));
        assert_eq!(battle.battle_pace.destroyed_team2_score, Some(15));
        assert_eq!(battle.outcome, BattleOutcome::Victory);
    }

    #[test]
    fn parser_seven_emits_schema_three_and_anonymous_team_relative_facts() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("team-relative.wotreplay");
        fs::write(
            &path,
            completed_replay_bytes(pace_metadata(), pace_results(205, 0)),
        )
        .unwrap();

        let (battle, _, _, facts) = parse_replay_file(&path).unwrap().into_import();
        assert_eq!(battle.parser_version, Some(REPLAY_PARSER_VERSION));
        assert_eq!(battle.feature_schema_version, Some(3));
        assert_eq!(facts.common_reason, None);
        assert_eq!(facts.team_player_count, Some(15));
        assert_eq!(facts.team_damage_total, Some(4_384));
        assert_eq!(facts.damage_greater_count, Some(0));
        assert_eq!(facts.damage_equal_count, Some(1));
        assert_eq!(facts.team_frags_total, None);
        assert_eq!(
            facts.frags_reason,
            Some(crate::team_relative::TeamRelativeFragsReason::Missing)
        );
        assert_eq!(facts.xp_greater_count, Some(0));
        assert_eq!(facts.xp_equal_count, Some(1));
    }

    #[test]
    fn parser_fails_closed_for_duplicate_rollout_team_conflict_and_missing_roster() {
        let directory = tempdir().unwrap();
        let cases = [
            (
                "duplicate-rollout",
                {
                    let mut results = pace_results(179, 2);
                    let duplicate = results[0]["vehicles"]["1"][0].clone();
                    results[0]["vehicles"]["1"]
                        .as_array_mut()
                        .unwrap()
                        .push(duplicate);
                    results
                },
                BattlePaceReason::ConflictingData,
            ),
            (
                "team-conflict",
                {
                    let mut results = pace_results(179, 2);
                    results[0]["vehicles"]["1"][0]["team"] = json!(2);
                    results
                },
                BattlePaceReason::ConflictingData,
            ),
            (
                "missing-final-roster",
                {
                    let mut results = pace_results(179, 2);
                    results[0].as_object_mut().unwrap().remove("vehicles");
                    results
                },
                BattlePaceReason::MissingFinalRoster,
            ),
        ];

        for (name, results, expected_reason) in cases {
            let path = directory.path().join(format!("{name}.wotreplay"));
            fs::write(&path, completed_replay_bytes(pace_metadata(), results)).unwrap();
            let (battle, _, _) = parse_replay_file(&path).unwrap().into_battle();
            assert_eq!(battle.data_quality, DataQuality::Full);
            assert_eq!(battle.battle_pace.context, Some(BattlePaceContext::Unknown));
            assert_eq!(battle.battle_pace.reason, Some(expected_reason));
        }
    }

    #[test]
    fn death_reason_minus_one_is_alive_and_other_supported_values_are_destroyed() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("death-reason.wotreplay");
        let mut results = pace_results(179, 2);
        results[0]["vehicles"]["1"][0]["deathReason"] = json!(-1);
        results[0]["vehicles"]["16"][0]["deathReason"] = json!(3);
        fs::write(&path, completed_replay_bytes(pace_metadata(), results)).unwrap();

        let (battle, _, _) = parse_replay_file(&path).unwrap().into_battle();

        assert_eq!(battle.battle_pace.destroyed_team2_score, Some(14));
        assert_eq!(battle.battle_pace.destroyed_team1_score, Some(2));
        assert_eq!(battle.battle_pace.context, Some(BattlePaceContext::Unknown));
        assert_eq!(
            battle.battle_pace.reason,
            Some(BattlePaceReason::ConflictingData)
        );
    }

    #[test]
    fn canonical_vehicle_key_preserves_the_existing_normalization_contract() {
        let key = canonicalize_vehicle_key("  FRANCE:F136 AMX!_67  ").unwrap();
        assert_eq!(key.as_str(), "france-f136_amx_67");

        let capped = canonicalize_vehicle_key(&"A".repeat(140)).unwrap();
        assert_eq!(capped.as_str().len(), 128);
        assert!(capped.as_str().bytes().all(|byte| byte == b'a'));
    }

    #[test]
    fn canonical_vehicle_key_is_idempotent_and_rejects_empty_results() {
        let once = canonicalize_vehicle_key("UK:GB147 FV4025").unwrap();
        let twice = canonicalize_vehicle_key(once.as_str()).unwrap();
        assert_eq!(once, twice);
        assert_eq!(
            canonicalize_vehicle_key("  !Ż!  "),
            Err(CanonicalVehicleKeyError::Empty)
        );
    }

    #[test]
    fn parses_valid_replay_header() {
        let b = replay_bytes(
            json!({"playerVehicle":"france-AMX_13","mapDisplayName":"Steppes","mapName":"internal_map","gameplayID":"random","dateTime":"10.07.2026 12:00:00"}),
        );
        let len = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        let v: Value = serde_json::from_slice(&b[12..12 + len]).unwrap();
        let battle = replay_from_metadata("id".into(), &v, None).unwrap();
        assert_eq!(battle.vehicle_key, "france-amx_13");
        assert_eq!(battle.map_name.as_deref(), Some("Steppes"));
        assert_eq!(battle.map_key.as_deref(), Some("internal_map"));
        assert_eq!(battle.gameplay_id.as_deref(), Some("random"));
        assert_eq!(battle.battle_time_source.as_deref(), Some("metadata"));
        assert_eq!(battle.feature_schema_version, None);
    }

    #[test]
    fn metadata_time_falls_back_to_file_mtime_with_provenance() {
        let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let battle = replay_from_metadata(
            "id".into(),
            &json!({"playerVehicle":"synthetic-vehicle"}),
            Some(modified),
        )
        .unwrap();

        assert_eq!(battle.battle_time, 1_700_000_000_000);
        assert_eq!(battle.battle_time_source.as_deref(), Some("file_mtime"));
    }
    #[test]
    fn parses_realistic_second_block_as_full_results() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("complete.wotreplay");
        fs::write(
            &path,
            completed_replay_bytes(
                json!({
                    "playerVehicle": "france-F136_AMX_67_Imbattable",
                    "mapDisplayName": "Cliff",
                    "mapName": "synthetic_cliff_key",
                    "gameplayID": "ctf",
                    "dateTime": "11.07.2026 11:16:00",
                    "clientVersionFromXml": "World of Tanks v.2.3.0.1 #892"
                }),
                realistic_results(),
            ),
        )
        .unwrap();

        let (battle, _, _) = parse_replay_file(&path).unwrap().into_battle();

        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.damage, Some(9061));
        assert_eq!(battle.frags, Some(4));
        assert_eq!(battle.shots, Some(27));
        assert_eq!(battle.hits, Some(23));
        assert_eq!(battle.direct_enemy_hits, Some(22));
        assert_eq!(battle.direct_team_hits, Some(1));
        assert_eq!(battle.penetrations, Some(20));
        assert_eq!(battle.piercing_enemy_hits, Some(20));
        assert_eq!(battle.explosion_hits, Some(2));
        assert_eq!(battle.assist, Some(254));
        assert_eq!(battle.assist_spotting, Some(254));
        assert_eq!(battle.assist_tracking, Some(0));
        assert_eq!(battle.assist_stun, Some(0));
        assert_eq!(battle.xp, Some(1439));
        assert_eq!(battle.original_xp, Some(1439));
        assert_eq!(battle.survived, Some(true));
        assert_eq!(battle.outcome, BattleOutcome::Victory);
        assert_eq!(battle.player_team, Some(2));
        assert_eq!(battle.winner_team_raw, Some(2));
        assert_eq!(battle.wg_tank_id, Some(33345));
        assert_eq!(battle.map_key.as_deref(), Some("synthetic_cliff_key"));
        assert_eq!(battle.gameplay_id.as_deref(), Some("ctf"));
        assert_eq!(battle.battle_time_source.as_deref(), Some("metadata"));
        assert_eq!(battle.feature_schema_version, Some(3));
        assert_eq!(battle.arena_unique_id, Some(1_111_222_233_334_444));
        assert_eq!(battle.defense_points, Some(7));
        assert_eq!(battle.moving_avg_damage, Some(4490));
        assert_eq!(battle.damage_rating, Some(84));
        assert_eq!(battle.parser_version, Some(REPLAY_PARSER_VERSION));
        assert_eq!(battle.data_completeness.as_deref(), Some("results"));
        assert_eq!(battle.replay_source.as_deref(), Some("replay_results"));
    }

    fn raw_counter(battle: &Battle, key: &str) -> Option<i64> {
        match key {
            "directEnemyHits" => battle.direct_enemy_hits,
            "directTeamHits" => battle.direct_team_hits,
            "piercingEnemyHits" => battle.piercing_enemy_hits,
            "explosionHits" => battle.explosion_hits,
            _ => panic!("unexpected synthetic counter"),
        }
    }

    #[test]
    fn attempts_conversion_counters_preserve_zero_missing_and_do_not_use_broad_fallbacks() {
        let mut zero_results = realistic_results();
        let personal = zero_results[0]["personal"]["33345"]
            .as_object_mut()
            .unwrap();
        for key in [
            "directEnemyHits",
            "directTeamHits",
            "piercingEnemyHits",
            "explosionHits",
        ] {
            personal.insert(key.into(), json!(0));
        }
        let zero = parse_frag_battle(zero_results);
        assert_eq!(zero.data_quality, DataQuality::Full);
        assert_eq!(zero.feature_schema_version, Some(3));
        assert_eq!(zero.direct_enemy_hits, Some(0));
        assert_eq!(zero.direct_team_hits, Some(0));
        assert_eq!(zero.piercing_enemy_hits, Some(0));
        assert_eq!(zero.explosion_hits, Some(0));

        let mut missing_results = realistic_results();
        let personal = missing_results[0]["personal"]["33345"]
            .as_object_mut()
            .unwrap();
        personal.remove("directEnemyHits");
        personal.remove("piercingEnemyHits");
        let missing = parse_frag_battle(missing_results);
        assert_eq!(missing.data_quality, DataQuality::Full);
        assert_eq!(missing.hits, Some(23));
        assert_eq!(missing.penetrations, Some(20));
        assert_eq!(missing.direct_enemy_hits, None);
        assert_eq!(missing.piercing_enemy_hits, None);
        assert_eq!(missing.direct_team_hits, Some(1));
        assert_eq!(missing.explosion_hits, Some(2));
    }

    #[test]
    fn attempts_conversion_counters_reject_malformed_values_without_panics_or_truncation() {
        let malformed = [
            json!(-1),
            json!(1.5),
            json!("1"),
            json!(true),
            Value::Null,
            json!({"value": 1}),
            json!([1]),
            json!(u64::MAX),
            json!(2_001),
        ];
        for key in [
            "directEnemyHits",
            "directTeamHits",
            "piercingEnemyHits",
            "explosionHits",
        ] {
            for value in &malformed {
                let mut results = realistic_results();
                results[0]["personal"]["33345"]
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), value.clone());
                let battle = parse_frag_battle(results);
                assert_eq!(battle.data_quality, DataQuality::Full);
                assert_eq!(battle.feature_schema_version, Some(3));
                assert_eq!(raw_counter(&battle, key), None, "key={key}, value={value}");
            }
        }
    }

    #[test]
    fn attempts_conversion_raw_facts_do_not_apply_false_causal_invariants() {
        let mut results = realistic_results();
        let personal = results[0]["personal"]["33345"].as_object_mut().unwrap();
        personal.insert("shots".into(), json!(0));
        personal.insert("directHits".into(), json!(0));
        personal.insert("directEnemyHits".into(), json!(0));
        personal.insert("directTeamHits".into(), json!(0));
        personal.insert("piercings".into(), json!(0));
        personal.insert("piercingEnemyHits".into(), json!(0));
        personal.insert("explosionHits".into(), json!(3));
        personal.insert("damageDealt".into(), json!(900));
        let splash = parse_frag_battle(results);
        assert_eq!(splash.data_quality, DataQuality::Full);
        assert_eq!(splash.shots, Some(0));
        assert_eq!(splash.direct_enemy_hits, Some(0));
        assert_eq!(splash.piercing_enemy_hits, Some(0));
        assert_eq!(splash.explosion_hits, Some(3));
        assert_eq!(splash.damage, Some(900));

        let mut no_damage_results = realistic_results();
        let personal = no_damage_results[0]["personal"]["33345"]
            .as_object_mut()
            .unwrap();
        personal.insert("damageDealt".into(), json!(0));
        personal.insert("directEnemyHits".into(), json!(5));
        personal.insert("piercingEnemyHits".into(), json!(2));
        let no_damage = parse_frag_battle(no_damage_results);
        assert_eq!(no_damage.data_quality, DataQuality::Full);
        assert_eq!(no_damage.damage, Some(0));
        assert_eq!(no_damage.direct_enemy_hits, Some(5));
        assert_eq!(no_damage.piercing_enemy_hits, Some(2));
    }

    #[test]
    fn special_mode_keeps_raw_counters_without_deciding_eligibility() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("special-mode.wotreplay");
        fs::write(
            &path,
            completed_replay_bytes(
                json!({
                    "battleType": 99,
                    "gameplayID": "synthetic_special",
                    "playerVehicle": "synthetic-spg",
                    "dateTime": "11.07.2026 11:16:00"
                }),
                realistic_results(),
            ),
        )
        .unwrap();
        let battle = parse_replay_file(&path).unwrap().into_battle().0;
        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.feature_schema_version, Some(3));
        assert_eq!(battle.direct_enemy_hits, Some(22));
        assert_eq!(battle.direct_team_hits, Some(1));
        assert_eq!(battle.piercing_enemy_hits, Some(20));
        assert_eq!(battle.explosion_hits, Some(2));
    }

    #[test]
    fn missing_raw_features_stay_missing_and_xp_fallback_is_not_original_xp() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("missing-raw-features.wotreplay");
        let mut results = realistic_results();
        let stats = results[0]["personal"]["33345"].as_object_mut().unwrap();
        for key in [
            "typeCompDescr",
            "team",
            "damageAssistedRadio",
            "damageAssistedTrack",
            "damageAssistedStun",
            "damageAssistedSmoke",
            "damageAssistedInspire",
            "originalXP",
        ] {
            stats.remove(key);
        }
        stats.insert("xp".into(), json!(777));
        results[0]["common"]
            .as_object_mut()
            .unwrap()
            .remove("winnerTeam");
        fs::write(
            &path,
            completed_replay_bytes(
                json!({
                    "playerVehicle": "synthetic-vehicle",
                    "dateTime": "11.07.2026 11:16:00"
                }),
                results,
            ),
        )
        .unwrap();

        let (battle, _, _) = parse_replay_file(&path).unwrap().into_battle();

        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.feature_schema_version, Some(3));
        assert_eq!(battle.map_key, None);
        assert_eq!(battle.gameplay_id, None);
        assert_eq!(battle.wg_tank_id, None);
        assert_eq!(battle.player_team, None);
        assert_eq!(battle.winner_team_raw, None);
        assert_eq!(battle.outcome, BattleOutcome::Unknown);
        assert_eq!(battle.assist, None);
        assert_eq!(battle.assist_spotting, None);
        assert_eq!(battle.assist_tracking, None);
        assert_eq!(battle.assist_stun, None);
        assert_eq!(battle.xp, Some(777));
        assert_eq!(battle.original_xp, None);
    }

    #[test]
    fn zero_raw_values_and_independent_assist_components_are_preserved() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("zero-raw-features.wotreplay");
        let mut results = realistic_results();
        let stats = results[0]["personal"]["33345"].as_object_mut().unwrap();
        stats.insert("team".into(), json!(0));
        stats.insert("damageAssistedRadio".into(), json!(120));
        stats.remove("damageAssistedTrack");
        stats.insert("damageAssistedStun".into(), json!(30));
        stats.remove("damageAssistedSmoke");
        stats.remove("damageAssistedInspire");
        stats.insert("originalXP".into(), json!(0));
        results[0]["common"]["winnerTeam"] = json!(0);
        fs::write(
            &path,
            completed_replay_bytes(
                json!({
                    "playerVehicle": "synthetic-vehicle",
                    "dateTime": "11.07.2026 11:16:00"
                }),
                results,
            ),
        )
        .unwrap();

        let (battle, _, _) = parse_replay_file(&path).unwrap().into_battle();

        assert_eq!(battle.player_team, Some(0));
        assert_eq!(battle.winner_team_raw, Some(0));
        assert_eq!(battle.outcome, BattleOutcome::Draw);
        assert_eq!(battle.assist_spotting, Some(120));
        assert_eq!(battle.assist_tracking, None);
        assert_eq!(battle.assist_stun, Some(30));
        // D8: stun is not assist.
        assert_eq!(battle.assist, Some(120));
        assert_eq!(battle.original_xp, Some(0));
        assert_eq!(battle.xp, Some(0));
    }

    #[test]
    fn maps_confirmed_winner_team_contract_to_defeat_draw_and_unknown() {
        let directory = tempdir().unwrap();
        for (name, winner_team, expected) in [
            ("defeat", 1, BattleOutcome::Defeat),
            ("draw", 0, BattleOutcome::Draw),
            ("unknown", 9, BattleOutcome::Unknown),
        ] {
            let path = directory.path().join(format!("{name}.wotreplay"));
            let mut results = realistic_results();
            results[0]["common"]["winnerTeam"] = json!(winner_team);
            fs::write(
                &path,
                completed_replay_bytes(
                    json!({
                        "playerVehicle": "synthetic-vehicle",
                        "dateTime": "11.07.2026 11:16:00"
                    }),
                    results,
                ),
            )
            .unwrap();

            let (battle, _, _) = parse_replay_file(&path).unwrap().into_battle();
            assert_eq!(battle.outcome, expected);
        }
    }

    #[test]
    fn results_vehicle_id_is_primary_metadata_is_fallback_and_conflict_is_explicit() {
        let directory = tempdir().unwrap();
        let results_primary = directory.path().join("results-primary.wotreplay");
        fs::write(
            &results_primary,
            completed_replay_bytes(
                json!({
                    "playerVehicle": "synthetic-vehicle",
                    "dateTime": "11.07.2026 11:16:00"
                }),
                realistic_results(),
            ),
        )
        .unwrap();
        let (primary, _, _) = parse_replay_file(&results_primary).unwrap().into_battle();
        assert_eq!(primary.wg_tank_id, Some(33345));
        assert_eq!(primary.parser_error_code, None);

        let fallback = directory.path().join("metadata-fallback.wotreplay");
        let mut no_results_id = realistic_results();
        no_results_id[0]["personal"]["33345"]
            .as_object_mut()
            .unwrap()
            .remove("typeCompDescr");
        fs::write(
            &fallback,
            completed_replay_bytes(
                json!({
                    "playerVehicle": "synthetic-vehicle",
                    "vehicleCompDescr": 444,
                    "dateTime": "11.07.2026 11:16:00"
                }),
                no_results_id,
            ),
        )
        .unwrap();
        let (fallback, _, _) = parse_replay_file(&fallback).unwrap().into_battle();
        assert_eq!(fallback.wg_tank_id, Some(444));
        assert_eq!(fallback.parser_error_code, None);

        let conflict = directory.path().join("vehicle-conflict.wotreplay");
        fs::write(
            &conflict,
            completed_replay_bytes(
                json!({
                    "playerVehicle": "synthetic-vehicle",
                    "vehicleCompDescr": 444,
                    "dateTime": "11.07.2026 11:16:00"
                }),
                realistic_results(),
            ),
        )
        .unwrap();
        let (conflict, _, _) = parse_replay_file(&conflict).unwrap().into_battle();
        assert_eq!(conflict.data_quality, DataQuality::Full);
        assert_eq!(conflict.wg_tank_id, None);
        assert_eq!(
            conflict.parser_error_code.as_deref(),
            Some("vehicle_id_conflict")
        );
        assert_eq!(conflict.damage, Some(9061));
        assert_eq!(conflict.feature_schema_version, Some(3));
    }

    #[test]
    fn absent_assist_components_remain_missing() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("missing-assist.wotreplay");
        let mut results = realistic_results();
        let stats = results[0]["personal"]["33345"].as_object_mut().unwrap();
        for key in [
            "damageAssistedRadio",
            "damageAssistedTrack",
            "damageAssistedStun",
            "damageAssistedSmoke",
        ] {
            stats.remove(key);
        }
        fs::write(
            &path,
            completed_replay_bytes(
                json!({
                    "playerVehicle": "france-F136_AMX_67_Imbattable",
                    "mapDisplayName": "Cliff",
                    "gameplayID": "ctf",
                    "dateTime": "11.07.2026 11:16:00"
                }),
                results,
            ),
        )
        .unwrap();

        let (battle, _, _) = parse_replay_file(&path).unwrap().into_battle();
        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.assist, None);
    }
    #[test]
    fn imports_anonymized_captured_replay_headers() {
        let directory = tempdir().unwrap();
        let replay_dir = directory.path().join("replays");
        fs::create_dir(&replay_dir).unwrap();
        fs::write(directory.path().join("WorldOfTanks.exe"), []).unwrap();
        let battle_time = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
        fs::write(
            replay_dir.join("captured-one.wotreplay"),
            captured_replay_bytes(
                json!({
                    "battleType": 1,
                    "clientVersionFromXml": "World of Tanks v.2.3.0.1 #892",
                    "dateTime": battle_time,
                    "gameplayID": "ctf",
                    "mapDisplayName": "Map A",
                    "mapName": "map_a",
                    "playerVehicle": "nation-Vehicle_A"
                }),
                1,
            ),
        )
        .unwrap();
        fs::write(
            replay_dir.join("captured-two.wotreplay"),
            captured_replay_bytes(
                json!({
                    "battleType": 1,
                    "clientVersionFromXml": "World of Tanks v.2.3.0.1 #892",
                    "dateTime": battle_time,
                    "gameplayID": "ctf",
                    "mapDisplayName": "Map B",
                    "mapName": "map_b",
                    "playerVehicle": "nation-Vehicle_B"
                }),
                1,
            ),
        )
        .unwrap();
        let database = Database::initialize(directory.path().join("test.sqlite3")).unwrap();
        let clock = crate::clock::ManualClock::starting_now();
        let tracker = tracker(database.clone()).with_clock(Arc::new(clock.clone()));
        *tracker.root.write().unwrap() = Some(fs::canonicalize(directory.path()).unwrap());

        clock.advance(Duration::from_millis(2_100));
        let first_observation = tracker.reconcile().unwrap();
        assert_eq!(first_observation.imported, 0);
        assert!(database.recent_battles(10).unwrap().is_empty());

        let result = tracker.reconcile().unwrap();
        let battles = database.recent_battles(10).unwrap();

        assert_eq!(result.imported, 2);
        assert_eq!(battles.len(), 2);
        assert!(battles.iter().all(
            |battle| battle.data_quality == DataQuality::Partial && battle.session_id.is_some()
        ));
        let session_id = battles[0].session_id.as_deref().unwrap();
        assert!(
            battles
                .iter()
                .all(|battle| battle.session_id.as_deref() == Some(session_id))
        );
        let session = database.active_session().unwrap().unwrap();
        assert_eq!(session.id, session_id);
        assert_eq!(session.battle_count, 2);
        assert!(
            battles
                .iter()
                .any(|battle| battle.vehicle_key == "nation-vehicle_a")
        );
        assert!(
            battles
                .iter()
                .any(|battle| battle.vehicle_key == "nation-vehicle_b")
        );
    }
    #[test]
    fn appended_replay_content_does_not_duplicate_battle() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("appended.wotreplay");
        let metadata = json!({
            "battleType": 1,
            "dateTime": Local::now().format("%d.%m.%Y %H:%M:%S").to_string(),
            "gameplayID": "ctf",
            "mapDisplayName": "Map",
            "mapName": "map",
            "playerVehicle": "nation-Vehicle"
        });
        fs::write(&path, captured_replay_bytes(metadata.clone(), 1)).unwrap();
        let database = Database::initialize(directory.path().join("test.sqlite3")).unwrap();
        let (mut first, first_state, _) = parse_replay_file(&path).unwrap().into_battle();
        assert_eq!(first.data_quality, DataQuality::Partial);
        assert_eq!(
            database
                .upsert_replay(&mut first, &first_state, true)
                .unwrap(),
            ReplayUpsertOutcome::Inserted
        );
        let session_id = first.session_id.clone().unwrap();

        fs::write(&path, completed_replay_bytes(metadata, realistic_results())).unwrap();
        let (mut completed, completed_state, _) = parse_replay_file(&path).unwrap().into_battle();
        assert_eq!(first.arena_id, completed.arena_id);
        assert_ne!(
            first_state.content_fingerprint,
            completed_state.content_fingerprint
        );
        assert_eq!(
            database
                .upsert_replay(&mut completed, &completed_state, true)
                .unwrap(),
            ReplayUpsertOutcome::Updated
        );

        let battles = database.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].data_quality, DataQuality::Full);
        assert_eq!(battles[0].damage, Some(9061));
        assert_eq!(battles[0].feature_schema_version, Some(3));
        assert_eq!(battles[0].player_team, Some(2));
        assert_eq!(battles[0].winner_team_raw, Some(2));
        assert_eq!(battles[0].assist_spotting, Some(254));
        assert_eq!(battles[0].assist_tracking, Some(0));
        assert_eq!(battles[0].assist_stun, Some(0));
        assert_eq!(battles[0].original_xp, Some(1439));
        assert_eq!(battles[0].direct_enemy_hits, Some(22));
        assert_eq!(battles[0].direct_team_hits, Some(1));
        assert_eq!(battles[0].piercing_enemy_hits, Some(20));
        assert_eq!(battles[0].explosion_hits, Some(2));
        assert_eq!(battles[0].session_id.as_deref(), Some(session_id.as_str()));
        assert_eq!(database.active_session().unwrap().unwrap().battle_count, 1);
        assert_eq!(
            database
                .upsert_replay(&mut completed, &completed_state, true)
                .unwrap(),
            ReplayUpsertOutcome::Duplicate
        );
        assert_eq!(database.active_session().unwrap().unwrap().battle_count, 1);
    }

    #[test]
    fn partial_to_full_persists_pace_enrichment_without_duplicate_or_outcome_change() {
        let directory = tempdir().unwrap();
        let database_path = directory.path().join("pace-enrichment.sqlite3");
        let replay_path = directory.path().join("pace-enrichment.wotreplay");
        let metadata = pace_metadata();
        fs::write(&replay_path, captured_replay_bytes(metadata.clone(), 1)).unwrap();
        let database = Database::initialize(database_path.clone()).unwrap();

        let (mut partial, partial_state, _) =
            parse_replay_file(&replay_path).unwrap().into_battle();
        assert_eq!(partial.data_quality, DataQuality::Partial);
        assert_eq!(
            partial.battle_pace.context,
            Some(BattlePaceContext::Unknown)
        );
        assert_eq!(
            database
                .upsert_replay(&mut partial, &partial_state, true)
                .unwrap(),
            ReplayUpsertOutcome::Inserted
        );

        fs::write(
            &replay_path,
            completed_replay_bytes(metadata, pace_results(205, 0)),
        )
        .unwrap();
        let (mut full, full_state, _) = parse_replay_file(&replay_path).unwrap().into_battle();
        assert_eq!(full.outcome, BattleOutcome::Victory);
        assert_eq!(
            full.battle_pace.context,
            Some(BattlePaceContext::BorderlineSteamroll)
        );
        assert_eq!(
            database
                .upsert_replay(&mut full, &full_state, true)
                .unwrap(),
            ReplayUpsertOutcome::Updated
        );
        assert_eq!(database.active_session().unwrap().unwrap().battle_count, 1);
        drop(database);

        let reopened = Database::initialize(database_path).unwrap();
        let battles = reopened.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].data_quality, DataQuality::Full);
        assert_eq!(battles[0].outcome, BattleOutcome::Victory);
        assert_eq!(
            battles[0].battle_pace.context,
            Some(BattlePaceContext::BorderlineSteamroll)
        );
        assert_eq!(
            battles[0].battle_pace.resolver_version.as_deref(),
            Some(BATTLE_PACE_RESOLVER_VERSION)
        );
        assert_eq!(battles[0].battle_pace.destroyed_team1_score, Some(0));
        assert_eq!(battles[0].battle_pace.destroyed_team2_score, Some(15));
        assert_eq!(reopened.active_session().unwrap().unwrap().battle_count, 1);
    }

    #[test]
    fn copied_complete_replay_is_deduplicated_by_arena_unique_id() {
        let directory = tempdir().unwrap();
        let first_path = directory.path().join("first.wotreplay");
        let copied_path = directory.path().join("renamed-copy.wotreplay");
        let bytes = completed_replay_bytes(
            json!({
                "playerVehicle": "france-F136_AMX_67_Imbattable",
                "mapDisplayName": "Cliff",
                "gameplayID": "ctf",
                "dateTime": "11.07.2026 11:16:00"
            }),
            realistic_results(),
        );
        fs::write(&first_path, &bytes).unwrap();
        fs::write(&copied_path, &bytes).unwrap();
        let database = Database::initialize(directory.path().join("test.sqlite3")).unwrap();
        let (mut first, first_state, _) = parse_replay_file(&first_path).unwrap().into_battle();
        assert_eq!(
            database
                .upsert_replay(&mut first, &first_state, false)
                .unwrap(),
            ReplayUpsertOutcome::Inserted
        );
        let (mut copied, copied_state, _) = parse_replay_file(&copied_path).unwrap().into_battle();
        assert_ne!(first_state.source_path_key, copied_state.source_path_key);
        assert_eq!(
            database
                .upsert_replay(&mut copied, &copied_state, false)
                .unwrap(),
            ReplayUpsertOutcome::Duplicate
        );
        assert_eq!(database.recent_battles(10).unwrap().len(), 1);
    }

    #[test]
    fn vehicle_id_conflict_upgrades_the_same_record_without_losing_results() {
        let directory = tempdir().unwrap();
        let replay_path = directory.path().join("vehicle-conflict-upgrade.wotreplay");
        let metadata = json!({
            "playerVehicle": "synthetic-vehicle",
            "vehicleCompDescr": 444,
            "mapDisplayName": "Synthetic map",
            "mapName": "synthetic_map",
            "gameplayID": "ctf",
            "dateTime": "11.07.2026 11:16:00"
        });
        fs::write(&replay_path, replay_bytes(metadata.clone())).unwrap();
        let database = Database::initialize(directory.path().join("test.sqlite3")).unwrap();
        let (mut partial, partial_state, _) =
            parse_replay_file(&replay_path).unwrap().into_battle();
        database
            .upsert_replay(&mut partial, &partial_state, true)
            .unwrap();

        fs::write(
            &replay_path,
            completed_replay_bytes(metadata, realistic_results()),
        )
        .unwrap();
        let (mut full, full_state, _) = parse_replay_file(&replay_path).unwrap().into_battle();
        assert_eq!(full.wg_tank_id, None);
        assert_eq!(
            database
                .upsert_replay(&mut full, &full_state, true)
                .unwrap(),
            ReplayUpsertOutcome::Updated
        );

        let battles = database.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].data_quality, DataQuality::Full);
        assert_eq!(battles[0].wg_tank_id, None);
        assert_eq!(
            battles[0].parser_error_code.as_deref(),
            Some("vehicle_id_conflict")
        );
        assert_eq!(battles[0].damage, Some(9061));
        assert_eq!(battles[0].feature_schema_version, Some(3));
        assert_eq!(database.active_session().unwrap().unwrap().battle_count, 1);
    }

    #[test]
    fn partial_to_full_upgrade_survives_database_restart() {
        let directory = tempdir().unwrap();
        let replay_path = directory.path().join("restart.wotreplay");
        let database_path = directory.path().join("test.sqlite3");
        let metadata = json!({
            "battleType": 1,
            "playerVehicle": "france-F136_AMX_67_Imbattable",
            "mapDisplayName": "Cliff",
            "gameplayID": "ctf",
            "dateTime": "11.07.2026 11:16:00"
        });
        fs::write(&replay_path, replay_bytes(metadata.clone())).unwrap();
        let database = Database::initialize(database_path.clone()).unwrap();
        let (mut partial, state, _) = parse_replay_file(&replay_path).unwrap().into_battle();
        database.upsert_replay(&mut partial, &state, true).unwrap();
        drop(database);

        fs::write(
            &replay_path,
            completed_replay_bytes(metadata, realistic_results()),
        )
        .unwrap();
        let reopened = Database::initialize(database_path).unwrap();
        let (mut full, state, _) = parse_replay_file(&replay_path).unwrap().into_battle();
        assert_eq!(
            reopened.upsert_replay(&mut full, &state, true).unwrap(),
            ReplayUpsertOutcome::Updated
        );
        let battles = reopened.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].data_quality, DataQuality::Full);
        assert_eq!(battles[0].feature_schema_version, Some(3));
        assert_eq!(battles[0].player_team, Some(2));
        assert_eq!(battles[0].winner_team_raw, Some(2));
        assert_eq!(battles[0].assist_spotting, Some(254));
        assert_eq!(battles[0].assist_tracking, Some(0));
        assert_eq!(battles[0].assist_stun, Some(0));
        assert_eq!(battles[0].original_xp, Some(1439));
        assert_eq!(battles[0].direct_enemy_hits, Some(22));
        assert_eq!(battles[0].direct_team_hits, Some(1));
        assert_eq!(battles[0].piercing_enemy_hits, Some(20));
        assert_eq!(battles[0].explosion_hits, Some(2));
        assert_eq!(reopened.active_session().unwrap().unwrap().battle_count, 1);
    }

    #[test]
    fn watcher_reprocesses_a_replay_after_results_are_appended() {
        let directory = tempdir().unwrap();
        let replay_directory = directory.path().join("replays");
        fs::create_dir(&replay_directory).unwrap();
        fs::write(directory.path().join("WorldOfTanks.exe"), []).unwrap();
        let replay_path = replay_directory.join("growing.wotreplay");
        let metadata = json!({
            "battleType": 1,
            "playerVehicle": "france-F136_AMX_67_Imbattable",
            "mapDisplayName": "Cliff",
            "gameplayID": "ctf",
            "dateTime": Local::now().format("%d.%m.%Y %H:%M:%S").to_string()
        });
        fs::write(&replay_path, replay_bytes(metadata.clone())).unwrap();
        let database = Database::initialize(directory.path().join("test.sqlite3")).unwrap();
        let clock = crate::clock::ManualClock::starting_now();
        let tracker = tracker(database.clone()).with_clock(Arc::new(clock.clone()));
        *tracker.root.write().unwrap() = Some(fs::canonicalize(directory.path()).unwrap());

        clock.advance(Duration::from_millis(2_100));
        tracker.reconcile().unwrap();
        tracker.reconcile().unwrap();
        assert_eq!(
            database.recent_battles(10).unwrap()[0].data_quality,
            DataQuality::Partial
        );
        assert_eq!(database.active_session().unwrap().unwrap().battle_count, 1);

        fs::write(
            &replay_path,
            completed_replay_bytes(metadata, realistic_results()),
        )
        .unwrap();
        tracker.reconcile().unwrap();
        clock.advance(Duration::from_millis(2_100));
        tracker.reconcile().unwrap();
        tracker.reconcile().unwrap();

        let battles = database.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].data_quality, DataQuality::Full);
        assert_eq!(battles[0].damage, Some(9061));
        assert_eq!(battles[0].parser_version, Some(REPLAY_PARSER_VERSION));
        assert_eq!(battles[0].feature_schema_version, Some(3));
        assert_eq!(battles[0].direct_enemy_hits, Some(22));
        assert_eq!(battles[0].direct_team_hits, Some(1));
        assert_eq!(battles[0].piercing_enemy_hits, Some(20));
        assert_eq!(battles[0].explosion_hits, Some(2));
        assert_eq!(database.active_session().unwrap().unwrap().battle_count, 1);
    }

    #[test]
    fn rejects_bad_and_truncated_headers() {
        let mut b = replay_bytes(json!({"playerVehicle":"tank"}));
        b[0] = 0;
        assert_ne!(
            u32::from_le_bytes(b[0..4].try_into().unwrap()),
            REPLAY_MAGIC
        );
        assert!(serde_json::from_slice::<Value>(&b[12..13]).is_err());
    }
    #[test]
    fn replay_file_enforces_signature_json_and_size() {
        let directory = tempdir().unwrap();
        let valid = directory.path().join("valid.wotreplay");
        fs::write(&valid, replay_bytes(json!({"playerVehicle":"tank","gameplayID":"random","dateTime":"10.07.2026 12:00:00","clientVersion":"2.3"}))).unwrap();
        let (battle, _, version) = parse_replay_file(&valid).unwrap().into_battle();
        assert_eq!(battle.vehicle_key, "tank");
        assert_eq!(version.as_deref(), Some("2.3"));
        assert_eq!(battle.data_quality, DataQuality::Partial);
        assert_eq!(battle.damage, None);

        let bad_magic = directory.path().join("bad.wotreplay");
        let mut bytes = replay_bytes(json!({"playerVehicle":"tank"}));
        bytes[0] = 0;
        fs::write(&bad_magic, bytes).unwrap();
        assert!(matches!(
            parse_replay_file(&bad_magic),
            Err(AppError::InvalidReplay)
        ));

        let broken_json = directory.path().join("broken.wotreplay");
        let mut bytes = replay_bytes(json!({"playerVehicle":"tank"}));
        bytes[12] = b'!';
        fs::write(&broken_json, bytes).unwrap();
        assert!(matches!(
            parse_replay_file(&broken_json),
            Err(AppError::InvalidReplay)
        ));

        let partial = directory.path().join("partial.wotreplay");
        fs::write(&partial, [0_u8; 8]).unwrap();
        assert!(matches!(
            parse_replay_file(&partial),
            Err(AppError::ReplayIncomplete)
        ));

        let oversized = directory.path().join("oversized.wotreplay");
        File::create(&oversized)
            .unwrap()
            .set_len(MAX_REPLAY_BYTES + 1)
            .unwrap();
        assert!(matches!(
            parse_replay_file(&oversized),
            Err(AppError::ReplayTooLarge)
        ));

        let absurd_block = directory.path().join("absurd-block.wotreplay");
        let metadata = serde_json::to_vec(&json!({"playerVehicle":"tank"})).unwrap();
        let mut bytes = Vec::new();
        bytes.extend(REPLAY_MAGIC.to_le_bytes());
        bytes.extend(2_u32.to_le_bytes());
        bytes.extend((metadata.len() as u32).to_le_bytes());
        bytes.extend(metadata);
        bytes.extend(((MAX_RESULTS_BYTES as u32) + 1).to_le_bytes());
        fs::write(&absurd_block, bytes).unwrap();
        assert!(matches!(
            parse_replay_file(&absurd_block),
            Err(AppError::InvalidReplay)
        ));

        let truncated_block = directory.path().join("truncated-block.wotreplay");
        let metadata = serde_json::to_vec(&json!({"playerVehicle":"tank"})).unwrap();
        let mut bytes = Vec::new();
        bytes.extend(REPLAY_MAGIC.to_le_bytes());
        bytes.extend(2_u32.to_le_bytes());
        bytes.extend((metadata.len() as u32).to_le_bytes());
        bytes.extend(metadata);
        bytes.extend(128_u32.to_le_bytes());
        bytes.extend(b"{}");
        fs::write(&truncated_block, bytes).unwrap();
        assert!(matches!(
            parse_replay_file(&truncated_block),
            Err(AppError::ReplayIncomplete)
        ));

        let invalid_results = directory.path().join("invalid-results.wotreplay");
        let metadata = serde_json::to_vec(&json!({
            "playerVehicle":"tank",
            "gameplayID":"random",
            "dateTime":"10.07.2026 12:00:00"
        }))
        .unwrap();
        fs::write(
            &invalid_results,
            replay_with_raw_second_block(metadata, b"not-json".to_vec()),
        )
        .unwrap();
        let (battle, _, _) = parse_replay_file(&invalid_results).unwrap().into_battle();
        assert_eq!(battle.data_quality, DataQuality::Partial);
        assert_eq!(
            battle.parser_error_code.as_deref(),
            Some("invalid_results_json")
        );
        assert_eq!(battle.damage, None);
    }
    fn parse_synthetic(metadata: Value, results: Value) -> Battle {
        let directory = tempdir().unwrap();
        let path = directory.path().join("characterization.wotreplay");
        fs::write(&path, completed_replay_bytes(metadata, results)).unwrap();
        parse_replay_file(&path).unwrap().into_battle().0
    }

    fn metadata_with(gameplay_id: Option<&str>, battle_type: i64) -> Value {
        let mut metadata = json!({
            "battleType": battle_type,
            "playerVehicle": "synthetic-vehicle",
            "dateTime": "11.07.2026 11:16:00"
        });
        if let Some(gameplay_id) = gameplay_id {
            metadata["gameplayID"] = json!(gameplay_id);
        }
        metadata
    }

    /// A time zone with a one-hour autumn fall-back at 02:00-03:00 local time on 26.10.2025, so
    /// local times in that hour are ambiguous (two instants). Deterministic on any machine.
    #[derive(Clone, Copy)]
    struct FallBackZone;

    impl TimeZone for FallBackZone {
        type Offset = chrono::FixedOffset;
        fn from_offset(_: &chrono::FixedOffset) -> Self {
            FallBackZone
        }
        fn offset_from_local_date(
            &self,
            _: &chrono::NaiveDate,
        ) -> chrono::LocalResult<chrono::FixedOffset> {
            chrono::LocalResult::Single(chrono::FixedOffset::east_opt(3_600).unwrap())
        }
        fn offset_from_local_datetime(
            &self,
            local: &NaiveDateTime,
        ) -> chrono::LocalResult<chrono::FixedOffset> {
            let summer = chrono::FixedOffset::east_opt(7_200).unwrap();
            let winter = chrono::FixedOffset::east_opt(3_600).unwrap();
            let start =
                NaiveDateTime::parse_from_str("26.10.2025 02:00:00", "%d.%m.%Y %H:%M:%S").unwrap();
            let end = start + chrono::Duration::hours(1);
            if *local >= start && *local < end {
                chrono::LocalResult::Ambiguous(summer, winter)
            } else if *local < start {
                chrono::LocalResult::Single(summer)
            } else {
                chrono::LocalResult::Single(winter)
            }
        }
        fn offset_from_utc_date(&self, _: &chrono::NaiveDate) -> chrono::FixedOffset {
            chrono::FixedOffset::east_opt(3_600).unwrap()
        }
        fn offset_from_utc_datetime(&self, _: &NaiveDateTime) -> chrono::FixedOffset {
            chrono::FixedOffset::east_opt(3_600).unwrap()
        }
    }

    /// D10: a battle that starts in the repeated autumn hour keeps a time (the earlier
    /// instant) instead of losing it.
    #[test]
    fn ambiguous_dst_local_time_resolves_to_the_earliest_instant() {
        let parsed = parse_replay_time_in("26.10.2025 02:30:00", &FallBackZone);
        // 02:30 in summer time (UTC+2) is 00:30 UTC.
        assert_eq!(parsed, Some(1_761_438_600_000));
        assert_eq!(
            parse_replay_time_in("26.10.2025 01:30:00", &FallBackZone),
            Some(1_761_435_000_000)
        );
        assert_eq!(
            parse_replay_time_in("2025-10-26 03:30:00", &FallBackZone),
            Some(1_761_445_800_000)
        );
    }

    /// Regression (M3): a `python.log` with `sessionStats` next to a one-block replay used to
    /// turn the partial battle into a full/ambiguous row without a feature schema, which broke the
    /// whole team-relative session report. The log is now ignored entirely.
    #[test]
    fn python_log_is_ignored_and_partial_battle_keeps_session_report_working() {
        let directory = tempdir().unwrap();
        let replay_dir = directory.path().join("replays");
        fs::create_dir(&replay_dir).unwrap();
        fs::write(directory.path().join("WorldOfTanks.exe"), []).unwrap();
        let battle_time = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
        fs::write(
            replay_dir.join("partial.wotreplay"),
            captured_replay_bytes(
                json!({"battleType": 1, "dateTime": battle_time, "gameplayID": "ctf",
                       "mapName": "map_a", "playerVehicle": "nation-Vehicle_A"}),
                1,
            ),
        )
        .unwrap();
        let stats = json!({"sessionStats": {"nation-Vehicle_A": {"battlesCount": 1,
            "damage": 900, "wins": 1, "frags": 1, "spots": 1, "xp": 900}}});
        fs::write(
            directory.path().join("python.log"),
            format!(
                "INFO sessionStats {stats}
INFO sessionStats {stats}
"
            ),
        )
        .unwrap();
        let database = Database::initialize(directory.path().join("test.sqlite3")).unwrap();
        let clock = crate::clock::ManualClock::starting_now();
        let tracker = tracker(database.clone()).with_clock(Arc::new(clock.clone()));
        *tracker.root.write().unwrap() = Some(fs::canonicalize(directory.path()).unwrap());
        clock.advance(Duration::from_millis(2_100));
        tracker.reconcile().unwrap();
        tracker.reconcile().unwrap();
        let battles = database.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].data_quality, DataQuality::Partial);
        assert_eq!(battles[0].source_kind, SourceKind::Replay);
        assert_eq!(battles[0].damage, None);
        let session_id = battles[0]
            .session_id
            .clone()
            .expect("live battle opens a session");
        assert!(
            crate::team_relative_results::team_relative_session_report(&database, &session_id)
                .is_ok()
        );
    }

    fn partial_synthetic(metadata: Value) -> Battle {
        let directory = tempdir().unwrap();
        let path = directory.path().join("partial.wotreplay");
        fs::write(&path, captured_replay_bytes(metadata, 1)).unwrap();
        parse_replay_file(&path).unwrap().into_battle().0
    }

    fn with_bonus(bonus_type: Value) -> Value {
        let mut results = realistic_results();
        results[0]["common"]["bonusType"] = bonus_type;
        results
    }

    /// M1 and T1 (fixed in E1, were characterization tests): the import gate and the statistics
    /// use the numeric `battleType` and `bonusType` only. `gameplayID = "ctf"` is set everywhere,
    /// because training, tournament, clan and ranked battles carry it too.
    #[test]
    fn battle_type_and_bonus_type_table_decides_import_and_statistics() {
        // (battleType, bonusType, import gate, counts in statistics)
        let cases = [
            (json!(1), json!(1), true, true),
            (json!(1), json!(7), true, false),
            (json!(1), json!(22), true, false),
            (json!(2), json!(2), false, false),
            (json!(22), json!(22), false, false),
            (json!(29), json!(29), false, false),
            (json!(27), json!(27), false, false),
            (json!(43), json!(43), false, false),
            (json!("1"), json!(1), false, false),
            (json!(1.0), json!(1), false, false),
            (json!(1), json!("1"), true, false),
            (Value::Null, json!(1), false, false),
        ];
        let range = (101..=109).map(|kind| (json!(kind), json!(kind), false, false));
        for (battle_type, bonus_type, gate, counted) in cases.into_iter().chain(range) {
            let mut metadata = metadata_with(Some("ctf"), 1);
            metadata["battleType"] = battle_type.clone();
            let battle = parse_synthetic(metadata, with_bonus(bonus_type.clone()));
            assert_eq!(battle.data_quality, DataQuality::Full);
            assert_eq!(
                passes_import_gate(&battle),
                gate,
                "gate {battle_type}/{bonus_type}"
            );
            assert_eq!(
                crate::models::stats_eligibility(&battle),
                counted,
                "stats {battle_type}/{bonus_type}"
            );
        }
        // Missing fields.
        let mut metadata = metadata_with(Some("ctf"), 1);
        metadata.as_object_mut().unwrap().remove("battleType");
        let battle = parse_synthetic(metadata, realistic_results());
        assert!(!passes_import_gate(&battle));
        let mut results = realistic_results();
        results[0]["common"]
            .as_object_mut()
            .unwrap()
            .remove("bonusType");
        let battle = parse_synthetic(metadata_with(Some("ctf"), 1), results);
        assert!(passes_import_gate(&battle));
        assert!(!crate::models::stats_eligibility(&battle));
    }

    /// M1b (fixed in E1): a random battle is recognized from the number, with or without any
    /// `gameplayID` text; mode names in the text are irrelevant.
    #[test]
    fn gameplay_text_never_decides_the_import_gate() {
        for gameplay_id in [
            None,
            Some("ctf"),
            Some("ctf_ranked"),
            Some("onslaught"),
            Some("x"),
        ] {
            let battle = parse_synthetic(metadata_with(gameplay_id, 1), with_bonus(json!(1)));
            assert!(passes_import_gate(&battle), "{gameplay_id:?}");
            assert!(crate::models::stats_eligibility(&battle), "{gameplay_id:?}");
            let battle = parse_synthetic(metadata_with(gameplay_id, 2), with_bonus(json!(1)));
            assert!(!passes_import_gate(&battle), "{gameplay_id:?}");
        }
    }

    /// The import gate reads `battle_type_raw` from the pace metadata; every early return of
    /// the roster parser must keep it, and report its own extraction error.
    #[test]
    fn pace_metadata_keeps_battle_type_on_every_roster_error() {
        let cases = [
            (
                json!({"battleType": 1}),
                BattlePaceReason::MissingFinalRoster,
            ),
            (
                json!({"battleType": 1, "vehicles": {}}),
                BattlePaceReason::MissingFinalRoster,
            ),
            (
                json!({"battleType": 1, "vehicles": {"1": {"name": "x"}}}),
                BattlePaceReason::MissingTeamAssignment,
            ),
            (
                json!({"battleType": 1, "vehicles": {"1": {"team": 3}}}),
                BattlePaceReason::MissingTeamAssignment,
            ),
        ];
        for (metadata, reason) in cases {
            let pace = parse_replay_pace_metadata(&metadata);
            assert_eq!(pace.battle_type_raw, Some(1), "{metadata}");
            assert_eq!(pace.extraction_error, Some(reason), "{metadata}");
        }
        let valid = parse_replay_pace_metadata(
            &json!({"battleType": 1, "vehicles": {"1": {"team": 1}, "2": {"team": 2}}}),
        );
        assert_eq!(valid.battle_type_raw, Some(1));
        assert_eq!(valid.extraction_error, None);
    }

    #[test]
    fn results_with_non_positive_arena_id_are_not_full() {
        for arena_id in [json!(0), json!(-5)] {
            let mut results = realistic_results();
            results[0]["arenaUniqueID"] = arena_id.clone();
            let battle = parse_synthetic(metadata_with(Some("ctf"), 1), results);
            assert_eq!(battle.data_quality, DataQuality::Partial, "{arena_id}");
            assert_eq!(
                battle.parser_error_code.as_deref(),
                Some("invalid_results_shape")
            );
        }
    }

    #[test]
    fn partial_replays_pass_the_gate_by_number_and_never_count() {
        let partial = partial_synthetic(metadata_with(None, 1));
        assert_eq!(partial.data_quality, DataQuality::Partial);
        assert!(passes_import_gate(&partial));
        assert!(!crate::models::stats_eligibility(&partial));
        let other = partial_synthetic(metadata_with(Some("ctf"), 27));
        assert!(!passes_import_gate(&other));
    }

    /// M4 (fixed in E1, was a characterization test): `assist` is spotting + tracking; stun,
    /// smoke and inspire never enter it.
    #[test]
    fn stored_assist_is_spotting_plus_tracking() {
        let mut results = realistic_results();
        let personal = results[0]["personal"]["33345"].as_object_mut().unwrap();
        personal.insert("damageAssistedRadio".into(), json!(254));
        personal.insert("damageAssistedTrack".into(), json!(100));
        personal.insert("damageAssistedStun".into(), json!(40));
        personal.insert("damageAssistedSmoke".into(), json!(10));
        personal.insert("damageAssistedInspire".into(), json!(5));
        let battle = parse_synthetic(metadata_with(Some("ctf"), 1), results.clone());
        assert_eq!(battle.assist, Some(354));
        assert_eq!(battle.assist_spotting, Some(254));
        assert_eq!(battle.assist_tracking, Some(100));
        assert_eq!(battle.assist_stun, Some(40));

        let personal = results[0]["personal"]["33345"].as_object_mut().unwrap();
        personal.remove("damageAssistedTrack");
        let battle = parse_synthetic(metadata_with(Some("ctf"), 1), results.clone());
        assert_eq!(battle.assist, Some(254), "one component present");
        let personal = results[0]["personal"]["33345"].as_object_mut().unwrap();
        personal.remove("damageAssistedRadio");
        let battle = parse_synthetic(metadata_with(Some("ctf"), 1), results);
        assert_eq!(battle.assist, None, "stun alone is not assist");
    }

    /// M11 context: `xp` keeps the premium fallback for display, but `original_xp` stays empty
    /// when `originalXP` is absent, and averages use `original_xp` only (see baseline tests).
    #[test]
    fn xp_display_falls_back_but_original_xp_stays_missing() {
        let mut results = realistic_results();
        let personal = results[0]["personal"]["33345"].as_object_mut().unwrap();
        personal.remove("originalXP");
        personal.insert("xp".into(), json!(2_000));
        let battle = parse_synthetic(metadata_with(Some("ctf"), 1), results);
        assert_eq!(battle.xp, Some(2_000));
        assert_eq!(battle.original_xp, None);
    }

    #[derive(Default)]
    struct GoldenTotals {
        battles: u64,
        damage: i128,
        assist_five_components: i128,
        spotting_plus_tracking: i128,
        stun: i128,
        frags: i128,
        spots: i128,
        defense: i128,
        xp: i128,
        original_xp: i128,
        survived: u64,
        victories: u64,
        defeats: u64,
        draws: u64,
        unknown_outcomes: u64,
        rows: Vec<String>,
    }

    impl GoldenTotals {
        fn add(&mut self, battle: &Battle) {
            self.battles += 1;
            self.damage += i128::from(battle.damage.unwrap_or(0));
            self.assist_five_components += i128::from(battle.assist.unwrap_or(0));
            self.spotting_plus_tracking += i128::from(
                battle.assist_spotting.unwrap_or(0) + battle.assist_tracking.unwrap_or(0),
            );
            self.stun += i128::from(battle.assist_stun.unwrap_or(0));
            self.frags += i128::from(battle.frags.unwrap_or(0));
            self.spots += i128::from(battle.spots.unwrap_or(0));
            self.defense += i128::from(battle.defense_points.unwrap_or(0));
            self.xp += i128::from(battle.xp.unwrap_or(0));
            self.original_xp += i128::from(battle.original_xp.unwrap_or(0));
            self.survived += u64::from(battle.survived == Some(true));
            match battle.outcome {
                BattleOutcome::Victory => self.victories += 1,
                BattleOutcome::Defeat => self.defeats += 1,
                BattleOutcome::Draw => self.draws += 1,
                BattleOutcome::Unknown => self.unknown_outcomes += 1,
            }
            // Per-battle metric tuple without any identifier; only its sorted digest is printed.
            self.rows.push(format!(
                "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                battle.damage,
                battle.assist,
                battle.assist_spotting,
                battle.assist_tracking,
                battle.assist_stun,
                battle.frags,
                battle.spots,
                battle.defense_points,
                battle.xp,
                battle.survived,
            ));
        }

        fn print(&self, label: &str) {
            let mut rows = self.rows.clone();
            rows.sort();
            let mut digest = sha2::Sha256::new();
            for row in &rows {
                sha2::Digest::update(&mut digest, row.as_bytes());
                sha2::Digest::update(&mut digest, b"\n");
            }
            let digest = sha2::Digest::finalize(digest)
                .iter()
                .take(8)
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            println!("[{label}]");
            println!("  battles                 = {}", self.battles);
            println!("  sum damage              = {}", self.damage);
            println!(
                "  sum assist (stored)     = {}",
                self.assist_five_components
            );
            println!(
                "  sum spotting+tracking   = {}",
                self.spotting_plus_tracking
            );
            println!("  sum stun                = {}", self.stun);
            println!("  sum frags               = {}", self.frags);
            println!("  sum spots               = {}", self.spots);
            println!("  sum defense             = {}", self.defense);
            println!("  sum xp                  = {}", self.xp);
            println!("  sum original_xp         = {}", self.original_xp);
            println!("  survived                = {}", self.survived);
            println!(
                "  outcomes v/d/draw/unk   = {}/{}/{}/{}",
                self.victories, self.defeats, self.draws, self.unknown_outcomes
            );
            println!("  metrics digest (sha256) = {digest}");
        }
    }

    // ---- E2: v10 foundations, full import engine, legacy rebuild ----

    /// A synthetic game folder; returns the canonical root and its `replays` directory.
    fn game_folder(directory: &Path) -> (PathBuf, PathBuf) {
        let replays = directory.join("replays");
        fs::create_dir_all(&replays).unwrap();
        fs::write(directory.join("WorldOfTanks.exe"), []).unwrap();
        (
            fs::canonicalize(directory).unwrap(),
            fs::canonicalize(&replays).unwrap(),
        )
    }

    fn configured_tracker(database: Database, root: &Path) -> LocalTracker {
        let tracker = tracker(database);
        *tracker.root.write().unwrap() = Some(root.to_path_buf());
        tracker
    }

    /// The strict random 15 vs 15 battle of `foundations::tests::synthetic_battle` as a replay.
    fn foundation_replay(index: i64, date_time: &str, tail: &[u8]) -> Vec<u8> {
        let (mut metadata, mut results) = crate::foundations::tests::synthetic_battle();
        metadata["playerVehicle"] = json!("synthetic-vehicle");
        metadata["mapName"] = json!("synthetic_map");
        metadata["dateTime"] = json!(date_time);
        results[0]["arenaUniqueID"] = json!(12_345_678_901_i64 + index);
        let mut bytes = completed_replay_bytes(metadata, results);
        bytes.extend_from_slice(tail);
        bytes
    }

    fn write_with_mtime(path: &Path, bytes: &[u8], seconds_ago: u64) {
        fs::write(path, bytes).unwrap();
        let file = File::options().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(seconds_ago))
            .unwrap();
    }

    fn scalar(path: &Path, sql: &str) -> i64 {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn import_stores_parser_nine_foundations_roster_and_team_aggregates() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        let tail = b"synthetic-packet-stream".repeat(3);
        write_with_mtime(
            &replays.join("one.wotreplay"),
            &foundation_replay(0, "11.07.2025 11:16:00", &tail),
            3_600,
        );
        let tracker = configured_tracker(database, &root);
        let result = tracker.run_full_import().unwrap();
        assert_eq!(result.imported, 1);

        assert_eq!(
            scalar(&db_path, "SELECT parser_version FROM battles"),
            REPLAY_PARSER_VERSION
        );
        assert_eq!(REPLAY_PARSER_VERSION, 9);
        assert_eq!(
            scalar(&db_path, "SELECT packet_stream_bytes FROM replay_files"),
            tail.len() as i64
        );
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE own_platoon_size=2 AND life_time_seconds=120 AND death_reason_raw=-1 AND max_health=1500 AND health_remaining=-12 AND direct_hits_received=6 AND piercings_received=4 AND no_damage_hits_received=2 AND explosion_hits_received=1 AND potential_damage_received=900 AND damage_from_invisibles=80 AND assist_smoke=7 AND assist_inspire=3 AND stun_duration=12.5 AND stun_num=2 AND capture_points=15 AND mileage=1000 AND arena_type_id=393281"
            ),
            1
        );
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battle_roster"), 30);
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battle_roster WHERE is_own_player=1 AND side='own'"
            ),
            1
        );
        assert_eq!(
            scalar(&db_path, "SELECT COUNT(*) FROM battle_team_stats"),
            2
        );
        // Team aggregates equal the sums of the stored roster.
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battle_team_stats t WHERE
                   t.damage_total=(SELECT SUM(damage_dealt) FROM battle_roster r WHERE r.battle_id=t.battle_id AND r.side=t.side)
                   AND t.kills_total=(SELECT SUM(kills) FROM battle_roster r WHERE r.battle_id=t.battle_id AND r.side=t.side)
                   AND t.assist_radio_total=(SELECT SUM(assist_radio) FROM battle_roster r WHERE r.battle_id=t.battle_id AND r.side=t.side)
                   AND t.survivors=(SELECT SUM(death_reason_raw=-1) FROM battle_roster r WHERE r.battle_id=t.battle_id AND r.side=t.side)
                   AND t.life_time_total=(SELECT SUM(life_time) FROM battle_roster r WHERE r.battle_id=t.battle_id AND r.side=t.side)
                   AND t.platooned_players=(SELECT COUNT(*) FROM battle_roster r WHERE r.battle_id=t.battle_id AND r.side=t.side AND r.platoon_group>0)"
            ),
            2
        );
        // Two platoons on the enemy side, one on the own side; solo players are group 0.
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(DISTINCT platoon_group) FROM battle_roster WHERE platoon_group>0"
            ),
            3
        );
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battle_roster r WHERE killer_slot IS NOT NULL AND NOT EXISTS(SELECT 1 FROM battle_roster k WHERE k.battle_id=r.battle_id AND k.slot=r.killer_slot)"
            ),
            0
        );
    }

    #[test]
    fn no_identifier_is_stored_in_any_cell_or_named_by_any_new_column() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        write_with_mtime(
            &replays.join("one.wotreplay"),
            &foundation_replay(0, "11.07.2025 11:16:00", b""),
            3_600,
        );
        configured_tracker(database, &root)
            .run_full_import()
            .unwrap();

        let mut forbidden_text = vec![
            "SyntheticNick".to_owned(),
            "SyntheticReal".into(),
            "SYNTH".into(),
        ];
        let mut forbidden_numbers = Vec::new();
        for index in 0..30_i64 {
            forbidden_numbers.push(crate::foundations::tests::OWN_ACCOUNT + index);
            forbidden_numbers.push(444_000 + index);
            forbidden_numbers.push(777_000 + index);
        }
        forbidden_numbers.extend([555_001, 555_002, 555_003]);
        forbidden_text.extend(forbidden_numbers.iter().map(i64::to_string));
        let connection = rusqlite::Connection::open(&db_path).unwrap();
        let tables = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut cells = 0;
        for table in &tables {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {table}"))
                .unwrap();
            let width = statement.column_count();
            let mut rows = statement.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                for column in 0..width {
                    cells += 1;
                    match row.get_ref(column).unwrap() {
                        rusqlite::types::ValueRef::Integer(value) => {
                            assert!(
                                !forbidden_numbers.contains(&value),
                                "{table}.{column}={value}"
                            );
                        }
                        rusqlite::types::ValueRef::Text(text) => {
                            let text = String::from_utf8_lossy(text);
                            for marker in &forbidden_text {
                                assert!(!text.contains(marker.as_str()), "{table}.{column}");
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        assert!(cells > 30 * 28);

        let forbidden_columns = [
            "name",
            "nick",
            "account",
            "clan",
            "prebattle",
            "vehicle_id_in_battle",
            "killer_id",
            "marks",
            "avatar",
            "player_id",
        ];
        for table in ["battle_roster", "battle_team_stats", "battles"] {
            let columns = connection
                .prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            for column in columns {
                for forbidden in forbidden_columns {
                    assert!(
                        !column.contains(forbidden)
                            || column == "tank_name"
                            || column == "map_name",
                        "{table}.{column}"
                    );
                }
            }
        }
    }

    #[test]
    fn non_qualifying_battles_have_no_roster_but_keep_own_columns() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        // Random mode, but not a standard random battle (bonusType 2).
        let (mut metadata, mut results) = crate::foundations::tests::synthetic_battle();
        metadata["playerVehicle"] = json!("synthetic-vehicle");
        metadata["dateTime"] = json!("11.07.2025 11:16:00");
        results[0]["common"]["bonusType"] = json!(2);
        write_with_mtime(
            &replays.join("bonus.wotreplay"),
            &completed_replay_bytes(metadata.clone(), results.clone()),
            3_600,
        );
        // 14 vs 15: one vehicle removed.
        let mut short = results.clone();
        short[0]["vehicles"]
            .as_object_mut()
            .unwrap()
            .remove("777029");
        short[0]["common"]["bonusType"] = json!(1);
        short[0]["arenaUniqueID"] = json!(22_222_222_222_i64);
        metadata["dateTime"] = json!("11.07.2025 12:16:00");
        write_with_mtime(
            &replays.join("short.wotreplay"),
            &completed_replay_bytes(metadata.clone(), short),
            3_500,
        );
        // Partial: metadata only.
        metadata["dateTime"] = json!("11.07.2025 13:16:00");
        write_with_mtime(
            &replays.join("partial.wotreplay"),
            &replay_bytes(metadata),
            3_400,
        );
        let result = configured_tracker(database, &root)
            .run_full_import()
            .unwrap();
        assert_eq!(result.imported, 3);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battle_roster"), 0);
        assert_eq!(
            scalar(&db_path, "SELECT COUNT(*) FROM battle_team_stats"),
            0
        );
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE data_quality='full' AND life_time_seconds=120 AND arena_type_id=393281"
            ),
            2
        );
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE data_quality='partial' AND life_time_seconds IS NULL AND own_platoon_size IS NULL"
            ),
            1
        );
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM replay_files WHERE packet_stream_bytes=0"
            ),
            3
        );
    }

    #[test]
    fn full_import_reads_every_missing_file_without_limits_and_old_battles_get_no_session() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        // "Application closed → 3 new files → start → 3 battles", one of them older than 90 days.
        for (index, date) in [
            "11.07.2020 11:16:00",
            "12.07.2025 11:16:00",
            "13.07.2025 11:16:00",
        ]
        .iter()
        .enumerate()
        {
            write_with_mtime(
                &replays.join(format!("old-{index}.wotreplay")),
                &foundation_replay(index as i64, date, b""),
                200 * 86_400 - index as u64,
            );
        }
        // A non-random replay is skipped once and remembered.
        let mut training = metadata_with(Some("ctf"), 2);
        training["dateTime"] = json!("13.07.2025 12:16:00");
        write_with_mtime(
            &replays.join("training.wotreplay"),
            &replay_bytes(training),
            100,
        );
        let tracker = configured_tracker(database.clone(), &root);
        let first = tracker.run_full_import().unwrap();
        assert_eq!(first.scanned, 4);
        assert_eq!(first.imported, 3);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battles"), 3);
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE session_id IS NOT NULL"
            ),
            0
        );
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM sessions"), 0);
        assert_eq!(
            scalar(&db_path, "SELECT COUNT(*) FROM replay_file_skips"),
            1
        );

        // A battle played just now creates a session; nothing else is read again.
        let now = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
        write_with_mtime(
            &replays.join("recent.wotreplay"),
            &foundation_replay(9, &now, b""),
            5,
        );
        let second = tracker.run_full_import().unwrap();
        assert_eq!(second.scanned, 1);
        assert_eq!(second.imported, 1);
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE session_id IS NOT NULL"
            ),
            1
        );
        assert_eq!(tracker.run_full_import().unwrap().scanned, 0);
        assert!(database.get_setting(IMPORT_CURSOR_KEY).unwrap().is_none());
        assert_eq!(tracker.status().import_progress, None);
    }

    /// Records events and cancels the import from inside the progress event that reports `at`
    /// files done, like a click on "Cancel" while that batch is shown.
    #[derive(Debug)]
    struct CancelAt {
        at: usize,
        tracker: std::sync::Mutex<Option<LocalTracker>>,
        recorded: crate::events::RecordedEvents,
    }

    impl EventSink for CancelAt {
        fn emit(&self, event: AppEvent) {
            if let AppEvent::ImportProgress {
                done,
                finished: false,
                ..
            } = event
                && done == self.at
                && let Some(tracker) = self.tracker.lock().unwrap().take()
            {
                tracker.cancel_import();
            }
            self.recorded.emit(event);
        }
    }

    fn progress(events: &[AppEvent]) -> Vec<(usize, usize, bool, bool)> {
        events
            .iter()
            .filter_map(|event| match *event {
                AppEvent::ImportProgress {
                    done,
                    total,
                    finished,
                    cancelled,
                } => Some((done, total, finished, cancelled)),
                _ => None,
            })
            .collect()
    }

    /// A batch that stores no battle (here: only non-random replays) reports progress but does
    /// not tell the window that data changed.
    #[test]
    fn batch_without_new_battles_does_not_emit_data_changed() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let database = Database::initialize(directory.path().join("afterbattle.sqlite3")).unwrap();
        for index in 0..3_i64 {
            let (mut metadata, mut results) = crate::foundations::tests::synthetic_battle();
            metadata["battleType"] = json!(2);
            metadata["dateTime"] = json!("01.01.2025 10:00:00");
            results[0]["arenaUniqueID"] = json!(12_345_678_901_i64 + index);
            let bytes = completed_replay_bytes(metadata, results);
            write_with_mtime(
                &replays.join(format!("training-{index}.wotreplay")),
                &bytes,
                400 * 86_400,
            );
        }
        let tracker = configured_tracker(database, &root);
        let events = crate::events::RecordedEvents::default();
        tracker.set_events(Arc::new(events.clone()));
        let result = tracker.run_full_import().unwrap();
        assert_eq!(result.imported, 0);
        let events = events.take();
        assert!(!progress(&events).is_empty());
        assert!(!events.contains(&AppEvent::DataChanged), "{events:?}");
    }

    /// E3 checkpoint: 2000 synthetic replays, progress through events, cancel after 500, resume
    /// without duplicates, no sessions for old battles.
    #[test]
    fn two_thousand_replays_report_progress_cancel_and_resume_without_duplicates() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        let first_battle = chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        for index in 0..2_000_u64 {
            let played = first_battle + chrono::Duration::minutes(10 * index as i64);
            write_with_mtime(
                &replays.join(format!("synthetic-{index:04}.wotreplay")),
                &foundation_replay(
                    index as i64,
                    &played.format("%d.%m.%Y %H:%M:%S").to_string(),
                    b"",
                ),
                400 * 86_400 - index * 60,
            );
        }
        let tracker = configured_tracker(database.clone(), &root);
        let first_events = crate::events::RecordedEvents::default();
        tracker.set_events(Arc::new(CancelAt {
            at: 500,
            tracker: std::sync::Mutex::new(Some(tracker.clone())),
            recorded: first_events.clone(),
        }));

        let first = tracker.run_full_import().unwrap();
        assert_eq!(first.imported, 500);
        let events = first_events.take();
        let mut expected = (0..=500)
            .step_by(IMPORT_BATCH_SIZE)
            .map(|done| (done, 2_000, false, false))
            .collect::<Vec<_>>();
        expected.push((500, 2_000, true, true));
        assert_eq!(progress(&events), expected);
        assert_eq!(
            events
                .iter()
                .filter(|event| **event == AppEvent::DataChanged)
                .count(),
            5
        );
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battles"), 500);
        assert!(database.get_setting(IMPORT_CURSOR_KEY).unwrap().is_some());
        assert_eq!(tracker.status().import_progress, None);
        assert!(!tracker.import_running());

        let second_events = crate::events::RecordedEvents::default();
        tracker.set_events(Arc::new(second_events.clone()));
        let second = tracker.run_full_import().unwrap();
        assert_eq!(second.scanned, 1_500);
        assert_eq!(second.imported, 1_500);
        let resumed = progress(&second_events.take());
        assert_eq!(resumed.first(), Some(&(0, 1_500, false, false)));
        assert_eq!(resumed.last(), Some(&(1_500, 1_500, true, false)));
        assert_eq!(resumed.len(), 1_500 / IMPORT_BATCH_SIZE + 2);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battles"), 2_000);
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(DISTINCT arena_unique_id) FROM battles"
            ),
            2_000
        );
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM replay_files"), 2_000);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM sessions"), 0);
        assert!(database.get_setting(IMPORT_CURSOR_KEY).unwrap().is_none());
        assert_eq!(tracker.run_full_import().unwrap().scanned, 0);
    }

    #[test]
    fn cancel_without_a_running_import_does_not_stop_the_next_one() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let database = Database::initialize(directory.path().join("db.sqlite3")).unwrap();
        write_with_mtime(
            &replays.join("one.wotreplay"),
            &foundation_replay(1, "12.07.2025 11:16:00", b""),
            86_400,
        );
        let tracker = configured_tracker(database, &root);
        tracker.cancel_import();
        assert_eq!(tracker.run_full_import().unwrap().imported, 1);
    }

    /// Spec 29 §2: a single new battle during a running session, read by the live scan (not the
    /// full import), is reported to the window once.
    #[test]
    fn live_scan_reports_one_new_battle_of_the_running_session_once() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let database = Database::initialize(directory.path().join("db.sqlite3")).unwrap();
        let session = database.start_session().unwrap();
        let tracker = configured_tracker(database.clone(), &root);
        let events = crate::events::RecordedEvents::default();
        tracker.set_events(Arc::new(events.clone()));
        let now = Local::now().format("%d.%m.%Y %H:%M:%S").to_string();
        write_with_mtime(
            &replays.join("new.wotreplay"),
            &foundation_replay(1, &now, b""),
            5,
        );

        // First pass only notes the file; the second, with the same size and time, reads it.
        assert_eq!(tracker.reconcile().unwrap().imported, 0);
        assert!(events.take().is_empty());
        assert_eq!(tracker.reconcile().unwrap().imported, 1);
        assert_eq!(events.take(), vec![AppEvent::DataChanged]);
        let battles = database.recent_battles(10).unwrap();
        assert_eq!(battles.len(), 1);
        assert_eq!(battles[0].session_id.as_deref(), Some(session.id.as_str()));

        // Nothing new: no event.
        assert_eq!(tracker.reconcile().unwrap().imported, 0);
        assert!(events.take().is_empty());
    }

    #[test]
    fn stale_session_ends_on_the_injected_clock_and_is_reported() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("db.sqlite3")).unwrap();
        database.set_setting("inactivity_minutes", "15").unwrap();
        let session = database.start_session().unwrap();
        // The clock starts at or after the session start.
        let clock = crate::clock::ManualClock::starting_now();
        let tracker = tracker(database.clone()).with_clock(Arc::new(clock.clone()));
        let events = crate::events::RecordedEvents::default();
        tracker.set_events(Arc::new(events.clone()));

        clock.advance(Duration::from_secs(14 * 60));
        assert!(tracker.end_stale_session().unwrap().is_none());
        assert!(events.take().is_empty());

        clock.advance(Duration::from_secs(60));
        let ended = tracker.end_stale_session().unwrap().unwrap();
        assert_eq!(ended.id, session.id);
        assert_eq!(
            events.take(),
            vec![
                AppEvent::SessionEnded {
                    session_id: session.id.clone()
                },
                AppEvent::DataChanged
            ]
        );
        assert!(database.active_session().unwrap().is_none());
    }

    #[test]
    fn changed_file_is_read_again_and_upgrades_partial_to_full_without_a_new_session() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        let path = replays.join("growing.wotreplay");
        let (mut metadata, _) = crate::foundations::tests::synthetic_battle();
        metadata["playerVehicle"] = json!("synthetic-vehicle");
        metadata["mapName"] = json!("synthetic_map");
        metadata["dateTime"] = json!("11.07.2025 11:16:00");
        write_with_mtime(&path, &replay_bytes(metadata), 7_200);
        let tracker = configured_tracker(database, &root);
        tracker.run_full_import().unwrap();
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE data_quality='partial'"
            ),
            1
        );
        write_with_mtime(
            &path,
            &foundation_replay(0, "11.07.2025 11:16:00", b""),
            3_600,
        );
        let result = tracker.run_full_import().unwrap();
        assert_eq!(result.scanned, 1);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battles"), 1);
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE data_quality='full'"
            ),
            1
        );
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battle_roster"), 30);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM sessions"), 0);
    }

    #[test]
    fn interrupted_full_import_resumes_after_the_cursor_without_duplicates() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        let first = replays.join("a.wotreplay");
        write_with_mtime(
            &first,
            &foundation_replay(0, "11.07.2025 11:16:00", b""),
            9_000,
        );
        let tracker = configured_tracker(database.clone(), &root);
        tracker.run_full_import().unwrap();
        // A crash right after the batch with `a` was committed: the cursor is still stored.
        let metadata = fs::metadata(&first).unwrap();
        let cursor = format!(
            "{}:{}",
            file_mtime_ms(&metadata),
            source_path_key(&fs::canonicalize(&first).unwrap())
        );
        database.set_setting(IMPORT_CURSOR_KEY, &cursor).unwrap();
        for (index, name) in ["b", "c"].iter().enumerate() {
            write_with_mtime(
                &replays.join(format!("{name}.wotreplay")),
                &foundation_replay(index as i64 + 1, &format!("2{index}.07.2025 11:16:00"), b""),
                8_000 - index as u64,
            );
        }
        let resumed = tracker.run_full_import().unwrap();
        assert_eq!(resumed.scanned, 2);
        assert_eq!(resumed.imported, 2);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battles"), 3);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM replay_files"), 3);
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battle_roster"), 90);
        assert!(database.get_setting(IMPORT_CURSOR_KEY).unwrap().is_none());
    }

    #[test]
    fn full_import_counts_partial_and_duplicate_files_and_attaches_battles_within_the_limit() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        let full = foundation_replay(0, "11.07.2025 11:16:00", b"");
        write_with_mtime(&replays.join("full.wotreplay"), &full, 9_000);
        write_with_mtime(&replays.join("copy.wotreplay"), &full, 8_900);
        let (mut metadata, _) = crate::foundations::tests::synthetic_battle();
        metadata["playerVehicle"] = json!("synthetic-vehicle");
        metadata["dateTime"] = json!("12.07.2025 11:16:00");
        write_with_mtime(
            &replays.join("partial.wotreplay"),
            &replay_bytes(metadata),
            8_800,
        );
        // Played 30 minutes ago: within the default 60-minute limit, so it opens a session.
        let half_hour_ago = (Local::now() - chrono::Duration::minutes(30))
            .format("%d.%m.%Y %H:%M:%S")
            .to_string();
        write_with_mtime(
            &replays.join("recent.wotreplay"),
            &foundation_replay(5, &half_hour_ago, b""),
            1_800,
        );
        let result = configured_tracker(database, &root)
            .run_full_import()
            .unwrap();
        assert_eq!(result.scanned, 4);
        assert_eq!(result.imported, 3);
        assert_eq!(result.partial, 1);
        assert_eq!(result.duplicates, 1);
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE session_id IS NOT NULL"
            ),
            1
        );
    }

    /// E3: the fingerprint covers the header and the JSON blocks only, so the packet stream is
    /// never read; any change of a JSON block changes it.
    #[test]
    fn fingerprint_covers_json_blocks_only() {
        let directory = tempdir().unwrap();
        let fingerprint = |name: &str, bytes: &[u8]| {
            let path = directory.path().join(name);
            fs::write(&path, bytes).unwrap();
            parse_replay_file(&path)
                .unwrap()
                .file_state
                .content_fingerprint
        };
        let base = foundation_replay(0, "11.07.2025 11:16:00", b"");
        let a = fingerprint("a.wotreplay", &base);
        let mut with_tail = base.clone();
        with_tail.extend_from_slice(b"different packet stream");
        assert_eq!(fingerprint("b.wotreplay", &with_tail), a);
        let other = foundation_replay(1, "11.07.2025 11:16:00", b"");
        assert_ne!(fingerprint("c.wotreplay", &other), a);
        let (mut metadata, _) = crate::foundations::tests::synthetic_battle();
        metadata["playerVehicle"] = json!("synthetic-vehicle");
        metadata["dateTime"] = json!("11.07.2025 11:16:00");
        assert_ne!(fingerprint("d.wotreplay", &replay_bytes(metadata)), a);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn import_cursor_round_trips_and_rejects_malformed_values() {
        assert_eq!(parse_cursor("123:key"), Some((123, "key".to_owned())));
        assert_eq!(parse_cursor("-5:k:z"), Some((-5, "k:z".to_owned())));
        assert_eq!(parse_cursor("0:"), Some((0, String::new())));
        assert_eq!(parse_cursor("x:key"), None);
        assert_eq!(parse_cursor("no-separator"), None);
        let pending = PendingReplay {
            mtime_ms: 42,
            key: "abc".into(),
            path: PathBuf::from("a"),
            stamp_len: 1,
        };
        assert_eq!(
            parse_cursor(&pending.cursor()),
            Some((42, "abc".to_owned()))
        );
    }

    #[test]
    fn a_stored_cursor_skips_files_up_to_it_in_the_running_import() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        let older = replays.join("older.wotreplay");
        write_with_mtime(
            &older,
            &foundation_replay(0, "11.07.2025 11:16:00", b""),
            9_000,
        );
        write_with_mtime(
            &replays.join("newer.wotreplay"),
            &foundation_replay(1, "12.07.2025 11:16:00", b""),
            8_000,
        );
        // The cursor points at `older`: it counts as done in this run even though it was never
        // stored, and is picked up by the next run after the cursor is cleared.
        let metadata = fs::metadata(&older).unwrap();
        database
            .set_setting(
                IMPORT_CURSOR_KEY,
                &format!(
                    "{}:{}",
                    file_mtime_ms(&metadata),
                    source_path_key(&fs::canonicalize(&older).unwrap())
                ),
            )
            .unwrap();
        let tracker = configured_tracker(database, &root);
        let first = tracker.run_full_import().unwrap();
        assert_eq!((first.scanned, first.imported), (1, 1));
        let second = tracker.run_full_import().unwrap();
        assert_eq!((second.scanned, second.imported), (1, 1));
        assert_eq!(scalar(&db_path, "SELECT COUNT(*) FROM battles"), 2);
    }

    #[test]
    fn full_import_ignores_directories_and_other_files() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let database = Database::initialize(directory.path().join("afterbattle.sqlite3")).unwrap();
        fs::create_dir(replays.join("folder.wotreplay")).unwrap();
        fs::write(replays.join("notes.txt"), b"x").unwrap();
        write_with_mtime(
            &replays.join("one.wotreplay"),
            &foundation_replay(0, "11.07.2025 11:16:00", b""),
            9_000,
        );
        let result = configured_tracker(database, &root)
            .run_full_import()
            .unwrap();
        assert_eq!((result.scanned, result.imported), (1, 1));
    }

    #[test]
    fn rebuilt_battles_keep_legacy_ids_and_manual_sessions_and_the_report_is_written() {
        let directory = tempdir().unwrap();
        let (root, replays) = game_folder(directory.path());
        let db_path = directory.path().join("afterbattle.sqlite3");
        let database = Database::initialize(db_path.clone()).unwrap();
        let connection = rusqlite::Connection::open(&db_path).unwrap();
        connection
            .execute_batch(
                "INSERT INTO sessions(id,started_at,ended_at,status) VALUES('manual',1,2,'ended');
                 INSERT INTO legacy_battles(arena_id,arena_unique_id,session_id) VALUES('legacy-a',12345678901,'manual');
                 INSERT INTO legacy_battles(arena_id,arena_unique_id,session_id) VALUES('legacy-gone',99,NULL);
                 INSERT INTO legacy_battles(arena_id,arena_unique_id,session_id) VALUES('legacy-training',NULL,NULL);",
            )
            .unwrap();
        drop(connection);
        let report = crate::legacy::RebuildReport {
            legacy_version: 9,
            legacy_battles: 3,
            missing_at_start: 1,
            state: crate::legacy::RebuildState::Pending,
            started_at: 1,
            finished_at: None,
            recalculated: 0,
            not_random: 0,
            without_replay: 0,
            conflicts: 0,
            skipped_partial: 0,
            skipped_empty_sessions: 0,
        };
        database
            .set_setting(
                crate::legacy::REBUILD_REPORT_KEY,
                &serde_json::to_string(&report).unwrap(),
            )
            .unwrap();
        write_with_mtime(
            &replays.join("a.wotreplay"),
            &foundation_replay(0, "11.07.2025 11:16:00", b""),
            9_000,
        );
        let mut training = metadata_with(Some("ctf"), 2);
        training["dateTime"] = json!("12.07.2025 12:16:00");
        let training_bytes = replay_bytes(training);
        let training_path = replays.join("training.wotreplay");
        write_with_mtime(&training_path, &training_bytes, 8_000);
        let training_id = parse_replay_file(&training_path)
            .unwrap()
            .into_battle()
            .0
            .arena_id;
        rusqlite::Connection::open(&db_path)
            .unwrap()
            .execute(
                "UPDATE legacy_battles SET arena_id=?1 WHERE arena_id='legacy-training'",
                [training_id],
            )
            .unwrap();

        configured_tracker(database.clone(), &root)
            .run_full_import()
            .unwrap();
        assert_eq!(
            scalar(
                &db_path,
                "SELECT COUNT(*) FROM battles WHERE arena_id='legacy-a' AND session_id='manual'"
            ),
            1
        );
        let report = database.rebuild_report().unwrap().unwrap();
        assert_eq!(report.state, crate::legacy::RebuildState::Done);
        assert_eq!(report.recalculated, 1);
        assert_eq!(report.not_random, 1);
        assert_eq!(report.without_replay, 1);
        assert_eq!(report.conflicts, 0);
        assert!(report.finished_at.is_some());
    }

    /// Parses `AFTERBATTLE_GOLDEN_UNTIL`: local wall-clock time in ISO form, `YYYY-MM-DDTHH:MM`
    /// with optional `:SS`. A value that does not parse stops the test instead of silently
    /// counting the whole folder.
    fn golden_cutoff<Tz: TimeZone>(value: &str, zone: &Tz) -> SystemTime {
        let naive = ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"]
            .iter()
            .find_map(|format| chrono::NaiveDateTime::parse_from_str(value.trim(), format).ok())
            .unwrap_or_else(|| panic!("AFTERBATTLE_GOLDEN_UNTIL is not YYYY-MM-DDTHH:MM[:SS]"));
        let local = zone
            .from_local_datetime(&naive)
            .earliest()
            .expect("AFTERBATTLE_GOLDEN_UNTIL names a local time that does not exist");
        SystemTime::from(local.with_timezone(&Utc))
    }

    /// Optional `AFTERBATTLE_GOLDEN_UNTIL` (local time, ISO): files modified at or after it are
    /// left out, to reproduce an earlier measurement on a folder that has grown since. Without
    /// it every file counts.
    fn golden_included(metadata: &fs::Metadata) -> bool {
        let Ok(value) = std::env::var("AFTERBATTLE_GOLDEN_UNTIL") else {
            return true;
        };
        let cutoff = golden_cutoff(&value, &chrono::Local);
        metadata
            .modified()
            .ok()
            .is_some_and(|modified| modified < cutoff)
    }

    #[test]
    fn golden_cutoff_reads_local_iso_time() {
        let zone = chrono::FixedOffset::east_opt(3_600).unwrap();
        let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_762_891_200);
        // 2025-11-11T21:00 at UTC+1 is 20:00 UTC.
        assert_eq!(golden_cutoff("2025-11-11T21:00", &zone), expected);
        assert_eq!(golden_cutoff("2025-11-11T21:00:00", &zone), expected);
        assert_eq!(
            golden_cutoff(" 2025-11-11T21:00:30 ", &zone),
            expected + Duration::from_secs(30)
        );
    }

    #[test]
    #[should_panic(expected = "AFTERBATTLE_GOLDEN_UNTIL is not")]
    fn golden_cutoff_refuses_an_unreadable_value() {
        golden_cutoff("1762891200", &chrono::Utc);
    }

    fn golden_replay_paths(root: &Path, depth: usize, found: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if depth < 3 {
                    golden_replay_paths(&path, depth + 1, found);
                }
            } else if metadata.is_file()
                && golden_included(&metadata)
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("wotreplay"))
            {
                found.push(path);
            }
        }
    }

    /// Golden test on the author's real replays. Read-only; prints aggregates only (no paths,
    /// names, account or battle identifiers). Run manually:
    ///
    /// ```text
    /// $env:AFTERBATTLE_GOLDEN_DIR = "<replays folder>"
    /// cargo test --manifest-path src-tauri/Cargo.toml --lib golden_local_replays -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "reads the author's real replays; set AFTERBATTLE_GOLDEN_DIR and run with --ignored --nocapture"]
    fn golden_local_replays() {
        let root = std::env::var_os("AFTERBATTLE_GOLDEN_DIR")
            .map(PathBuf::from)
            .expect("set AFTERBATTLE_GOLDEN_DIR to the replays folder");
        assert!(root.is_dir(), "AFTERBATTLE_GOLDEN_DIR is not a directory");
        print_golden_report(&root);
        // E1b: the client package is read from the game folder (parent of `replays`) unless
        // AFTERBATTLE_GOLDEN_CLIENT_DIR points elsewhere. Read-only, aggregates only.
        let client = std::env::var_os("AFTERBATTLE_GOLDEN_CLIENT_DIR")
            .map(PathBuf::from)
            .or_else(|| root.parent().map(Path::to_path_buf));
        if let Some(client) = client {
            print_golden_tier_coverage(&root, &client);
        }
    }

    /// E1b acceptance: share of the player's vehicles (distinct `typeCompDescr` of counted
    /// battles) that have a tier in a catalog built from the installed client.
    fn print_golden_tier_coverage(replays: &Path, client: &Path) {
        let package = client.join("res").join("packages").join("scripts.pkg");
        println!("=== golden tier coverage (aggregates only) ===");
        let report = match crate::vehicle_catalog_probe::probe_package(&package) {
            Ok(report) => report,
            Err(error) => {
                println!("catalog unavailable: {error}");
                println!("=== end ===");
                return;
            }
        };
        let catalog = crate::compact_descriptor::build_compact_descriptor_catalog(&report.entries);
        let mut vehicles = std::collections::BTreeMap::<i64, u64>::new();
        // E2: v10 foundations, aggregates only.
        let mut counted_battles = 0_u64;
        let mut battles_with_roster = 0_u64;
        let mut roster_vehicles = Vec::<i64>::new();
        let (mut packet_files, mut packet_bytes) = (0_u64, 0_i64);
        let mut stack = vec![replays.to_path_buf()];
        while let Some(directory) = stack.pop() {
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("wotreplay"))
                    && fs::metadata(&path).is_ok_and(|metadata| golden_included(&metadata))
                    && let Ok(parsed) = parse_replay_file(&path)
                {
                    let extras = parsed.extras.clone();
                    let battle = parsed.into_battle().0;
                    if let Some(bytes) = extras.packet_stream_bytes {
                        packet_files += 1;
                        packet_bytes += bytes;
                    }
                    if crate::models::stats_eligibility(&battle) {
                        counted_battles += 1;
                        if let Some(roster) = &extras.roster {
                            battles_with_roster += 1;
                            roster_vehicles
                                .extend(roster.rows.iter().map(|row| row.type_comp_descr));
                        }
                        if let Some(id) = battle.wg_tank_id {
                            *vehicles.entry(id).or_default() += 1;
                        }
                    }
                }
            }
        }
        let tier_of = |id: i64| {
            u32::try_from(id)
                .ok()
                .and_then(|id| catalog.get(id))
                .and_then(|entry| entry.tier)
        };
        let distinct = vehicles.len();
        let with_tier = vehicles.keys().filter(|id| tier_of(**id).is_some()).count();
        let battles: u64 = vehicles.values().sum();
        let battles_with_tier: u64 = vehicles
            .iter()
            .filter(|(id, _)| tier_of(**id).is_some())
            .map(|(_, count)| *count)
            .sum();
        let mut tiers = std::collections::BTreeMap::<u8, u64>::new();
        for (id, count) in &vehicles {
            if let Some(tier) = tier_of(*id) {
                *tiers.entry(tier).or_default() += count;
            }
        }
        println!(
            "catalog nations read / missing / unknown = {} / {} / {}",
            report.files_read,
            report.missing_nations.len(),
            report.unknown_nations.len()
        );
        println!(
            "catalog records with tier / missing / invalid = {} / {} / {}",
            report
                .entries
                .iter()
                .filter(|entry| entry.tier.is_some())
                .count(),
            report.missing_tier,
            report.invalid_tier
        );
        println!("catalog string ids accepted   = {}", report.string_ids);
        println!("player vehicles (distinct)    = {distinct}");
        println!("  with tier                   = {with_tier}");
        if distinct > 0 {
            println!(
                "  share with tier             = {:.2}%",
                with_tier as f64 * 100.0 / distinct as f64
            );
        }
        println!("counted battles with tier     = {battles_with_tier} / {battles}");
        println!("counted battles by tier       = {tiers:?}");
        let roster_with_tier = roster_vehicles
            .iter()
            .filter(|id| tier_of(**id).is_some())
            .count();
        println!("--- E2 foundations ---");
        println!("counted battles with roster   = {battles_with_roster} / {counted_battles}");
        if counted_battles > 0 {
            println!(
                "  share with roster           = {:.2}%",
                battles_with_roster as f64 * 100.0 / counted_battles as f64
            );
        }
        println!(
            "roster vehicles with tier     = {roster_with_tier} / {}",
            roster_vehicles.len()
        );
        if !roster_vehicles.is_empty() {
            println!(
                "  share with tier             = {:.2}%",
                roster_with_tier as f64 * 100.0 / roster_vehicles.len() as f64
            );
        }
        println!("files with packet stream size = {packet_files}");
        println!("sum packet_stream_bytes       = {packet_bytes}");
        println!("=== end ===");
    }

    /// Keeps the golden report code compiled and exercised on a synthetic directory.
    #[test]
    fn golden_report_runs_on_a_synthetic_directory() {
        let directory = tempdir().unwrap();
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).unwrap();
        let mut results = realistic_results();
        results[0]["common"]["bonusType"] = json!(1);
        fs::write(
            directory.path().join("random.wotreplay"),
            completed_replay_bytes(metadata_with(Some("ctf"), 1), results.clone()),
        )
        .unwrap();
        results[0]["common"]["bonusType"] = json!(7);
        fs::write(
            nested.join("other-mode.WOTREPLAY"),
            completed_replay_bytes(metadata_with(Some("ctf"), 2), results),
        )
        .unwrap();
        fs::write(
            directory.path().join("metadata-only.wotreplay"),
            replay_bytes(metadata_with(Some("ctf"), 1)),
        )
        .unwrap();
        fs::write(
            directory.path().join("broken.wotreplay"),
            b"not a replay at all",
        )
        .unwrap();
        fs::write(directory.path().join("ignored.txt"), b"x").unwrap();
        let before = fs::read(directory.path().join("random.wotreplay")).unwrap();
        print_golden_report(directory.path());
        assert_eq!(
            before,
            fs::read(directory.path().join("random.wotreplay")).unwrap()
        );
    }

    fn print_golden_report(root: &Path) {
        let mut paths = Vec::new();
        golden_replay_paths(root, 0, &mut paths);

        let mut parse_errors = std::collections::BTreeMap::<String, u64>::new();
        let mut distribution =
            std::collections::BTreeMap::<(Option<i64>, Option<i64>, &'static str), u64>::new();
        let (mut full, mut partial) = (0_u64, 0_u64);
        let mut gate_accepted = 0_u64;
        let mut gate_accepted_not_strict_random = 0_u64;
        let mut strict_random_partial = 0_u64;
        let mut arena_ids = std::collections::BTreeSet::<i64>::new();
        let mut strict_arena_ids = std::collections::BTreeSet::<i64>::new();
        let mut gate_full = GoldenTotals::default();
        let mut strict_full = GoldenTotals::default();

        for path in &paths {
            let battle = match parse_replay_file(path) {
                Ok(parsed) => parsed.into_battle().0,
                Err(error) => {
                    // Variant name only: the Debug text could carry OS error details.
                    let name = format!("{error:?}");
                    let name = name
                        .split(|character: char| {
                            !character.is_ascii_alphanumeric() && character != '_'
                        })
                        .next()
                        .unwrap_or("unknown")
                        .to_owned();
                    *parse_errors.entry(name).or_default() += 1;
                    continue;
                }
            };
            let quality = match battle.data_quality {
                DataQuality::Full => {
                    full += 1;
                    "full"
                }
                _ => {
                    partial += 1;
                    "partial"
                }
            };
            let battle_type = battle.battle_pace.battle_type_raw;
            let bonus_type = battle.battle_pace.bonus_type_raw;
            *distribution
                .entry((battle_type, bonus_type, quality))
                .or_default() += 1;
            let strict_random = crate::models::stats_eligibility(&battle);
            let accepted = passes_import_gate(&battle);
            if accepted {
                gate_accepted += 1;
                if battle.data_quality != DataQuality::Full || !strict_random {
                    gate_accepted_not_strict_random += 1;
                }
            }
            if battle_type == Some(1) && battle.data_quality != DataQuality::Full {
                strict_random_partial += 1;
            }
            if battle.data_quality == DataQuality::Full {
                if let Some(id) = battle.arena_unique_id {
                    arena_ids.insert(id);
                    if strict_random {
                        strict_arena_ids.insert(id);
                    }
                }
                if accepted {
                    gate_full.add(&battle);
                }
                if strict_random {
                    strict_full.add(&battle);
                }
            }
        }

        println!("=== golden_local_replays (aggregates only) ===");
        println!("replay files found            = {}", paths.len());
        println!("parsed ok                     = {}", full + partial);
        println!("  full (results block)        = {full}");
        println!("  partial (metadata only)     = {partial}");
        println!("parse failures by kind        = {parse_errors:?}");
        println!("distinct arenaUniqueID (full) = {}", arena_ids.len());
        println!("battleType x bonusType x quality (None = field absent):");
        for ((battle_type, bonus_type, quality), count) in &distribution {
            println!(
                "  battleType={battle_type:?} bonusType={bonus_type:?} {quality:<7} : {count}"
            );
        }
        println!("accepted by the import gate (battleType == 1)    = {gate_accepted}");
        println!(
            "  of which not counted (partial or bonusType != 1) = {gate_accepted_not_strict_random}"
        );
        println!("partial replays with battleType == 1 (no bonus)   = {strict_random_partial}");
        println!(
            "stats population (stats_eligibility, distinct arena) = {}",
            strict_arena_ids.len()
        );
        gate_full.print("import gate, full battles");
        strict_full.print("stats_eligibility (strict random 1/1), full battles");
        println!("=== end ===");
    }

    /// Pins the non-ambiguous parts of the realistic synthetic result block.
    #[test]
    fn characterization_realistic_results_block_metrics() {
        let battle = parse_synthetic(metadata_with(Some("ctf"), 1), realistic_results());
        assert_eq!(battle.data_quality, DataQuality::Full);
        assert_eq!(battle.damage, Some(9_061));
        assert_eq!(battle.assist, Some(254));
        assert_eq!(battle.frags, Some(4));
        assert_eq!(battle.spots, Some(1));
        assert_eq!(battle.defense_points, Some(7));
        assert_eq!(battle.survived, Some(true));
        assert_eq!(battle.xp, Some(1_439));
        assert_eq!(battle.original_xp, Some(1_439));
        assert_eq!(battle.outcome, BattleOutcome::Victory);
    }
}
