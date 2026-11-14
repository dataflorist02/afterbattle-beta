mod analysis;
pub mod attempts_conversion;
pub mod attempts_conversion_api;
mod baseline;
mod battle_pace;
mod bigworld_packed;
mod challenges;
mod clock;
mod compact_descriptor;
mod diagnostics;
pub mod error;
mod events;
pub mod expected_values;
mod foundations;
mod game_images;
mod hashing;
mod history;
mod legacy;
mod legacy_credentials;
mod local_tracker;
mod map_picture;
pub mod models;
mod performance_trends;
pub mod ratings;
mod schema;
mod session_insights;
mod session_story;
pub mod storage;
mod team_relative;
mod team_relative_results;
mod vehicle_catalog_probe;
mod vehicle_class_catalog;
mod vehicle_names;
mod weekly;

const _: fn(
    &storage::Database,
    &str,
) -> Result<
    team_relative_results::TeamRelativeResultsReport,
    team_relative_results::TeamRelativeResultsError,
> = team_relative_results::team_relative_session_report;
const _: fn(
    &storage::Database,
    usize,
) -> Result<
    team_relative_results::TeamRelativeResultsReport,
    team_relative_results::TeamRelativeResultsError,
> = team_relative_results::team_relative_recent_report;

use chrono::Utc;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    future::Future,
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};
use tauri::{
    AppHandle, Emitter, Manager, State, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use analysis::{battle_feedback, session_feedback};
use attempts_conversion::analyze_with_catalog;
use attempts_conversion_api::{
    AttemptsConversionReportDto, AttemptsConversionScopeDto, report_to_dto,
};
use baseline::{
    player_baseline, session_baseline_comparison, session_baseline_context, vehicle_baseline,
};
use battle_pace::summarize_battle_pace;
use challenges::{build_vehicle_inputs, generate_suggestions};
use diagnostics::build_report;
use error::{AppError, CommandError, CommandResult};
use expected_values::{
    ExpectedUpdateErrorCode, ExpectedUpdateMode, ExpectedUpdateOutcome, ExpectedValuesError,
    LoadedExpectedSnapshot, load_current_or_previous_snapshot, load_expected_update_state,
    update_expected_values,
};
use history::{battle_history_page, vehicle_history_overview, vehicle_history_report};
use local_tracker::LocalTracker;
use models::{
    AdvancedStatus, AppSnapshot, Battle, BattleHistoryFilter, BattleHistoryPage, BattleView,
    BattleWn8Presentation, ChallengeHistoryPage, ChallengePresentation,
    ChallengeSuggestionResponse, ExpectedValuesAvailability, ExpectedValuesRefreshResult,
    ExpectedValuesStatus, ExpectedValuesUpdateOutcome, HistoricalSessionWn8Presentation,
    PerformanceTrendRange, PerformanceTrendReport, PlayerBaselinePresentation, Session,
    SessionBaselineComparison, SessionSummary, SessionWn8Presentation, SourceStatus,
    VehicleBaselinePresentation, VehicleClassCatalogRefreshResult, VehicleClassCatalogStatus,
    VehicleClassResolutionState, VehicleHistoryOverview, VehicleHistoryReport, Wn8Calculation,
    Wn8ExclusionCounts, Wn8Presentation, Wn8PresentationState, Wn8UnavailableReason,
};
use performance_trends::performance_trends;
use ratings::{WN8_FORMULA_VERSION, calculate_battle_wn8, calculate_wn8};
use storage::{Database, metrics};
use vehicle_class_catalog::{
    VehicleClassCatalogRuntime, resolution_counts, unavailable_resolution,
};

#[derive(Debug)]
struct AppState {
    database: Database,
    tracker: LocalTracker,
    expected_values: ExpectedValuesRuntime,
    vehicle_classes: VehicleClassCatalogRuntime,
    challenge_last_error_code: Arc<RwLock<Option<String>>>,
    startup_error_code: Option<String>,
    startup_notice_code: Option<String>,
    data_dir: PathBuf,
    game_images: game_images::GameImages,
    vehicle_names: vehicle_names::VehicleNames,
}

#[derive(Debug)]
struct ExpectedValuesRuntimeData {
    snapshot: Option<LoadedExpectedSnapshot>,
    last_attempt_at: Option<i64>,
    last_success_at: Option<i64>,
    last_error_code: Option<String>,
}

#[derive(Debug, Clone)]
struct ExpectedValuesRuntime {
    cache_root: PathBuf,
    data: Arc<RwLock<ExpectedValuesRuntimeData>>,
    active_updates: Arc<AtomicUsize>,
}

impl ExpectedValuesRuntime {
    fn load(cache_root: PathBuf) -> Self {
        let snapshot = load_current_or_previous_snapshot(&cache_root)
            .ok()
            .flatten();
        let update_state = load_expected_update_state(&cache_root).unwrap_or_default();
        Self {
            cache_root,
            data: Arc::new(RwLock::new(ExpectedValuesRuntimeData {
                snapshot,
                last_attempt_at: update_state.last_attempt_at,
                last_success_at: update_state.last_success_at,
                last_error_code: update_state.last_error_code.map(expected_error_code),
            })),
            active_updates: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn begin_update(&self) -> ExpectedValuesProgressGuard {
        self.active_updates.fetch_add(1, Ordering::SeqCst);
        ExpectedValuesProgressGuard {
            runtime: self.clone(),
            active: true,
        }
    }

    fn reload_after(&self, outcome: ExpectedUpdateOutcome) {
        let refreshed_snapshot = matches!(
            outcome,
            ExpectedUpdateOutcome::Activated | ExpectedUpdateOutcome::Unchanged
        )
        .then(|| {
            load_current_or_previous_snapshot(&self.cache_root)
                .ok()
                .flatten()
        })
        .flatten();
        let refreshed_state = load_expected_update_state(&self.cache_root).ok();
        let mut data = self
            .data
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(snapshot) = refreshed_snapshot {
            data.snapshot = Some(snapshot);
        }
        if let Some(state) = refreshed_state {
            data.last_attempt_at = state.last_attempt_at;
            data.last_success_at = state.last_success_at;
            data.last_error_code = state.last_error_code.map(expected_error_code);
        }
    }

    fn reload_state_only(&self) {
        if let Ok(state) = load_expected_update_state(&self.cache_root) {
            let mut data = self
                .data
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            data.last_attempt_at = state.last_attempt_at;
            data.last_success_at = state.last_success_at;
            data.last_error_code = state.last_error_code.map(expected_error_code);
        }
    }

    fn snapshot_and_status(&self) -> (Option<LoadedExpectedSnapshot>, ExpectedValuesStatus) {
        let updating = self.active_updates.load(Ordering::SeqCst) > 0;
        let data = self
            .data
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let availability = if updating {
            ExpectedValuesAvailability::Updating
        } else if data.snapshot.is_some() {
            ExpectedValuesAvailability::Available
        } else {
            ExpectedValuesAvailability::Unavailable
        };
        let manifest = data.snapshot.as_ref().map(|snapshot| &snapshot.manifest);
        let status = ExpectedValuesStatus {
            availability,
            source: manifest.map(|_| "xvm_wargaming".to_owned()),
            source_version: manifest.map(|manifest| manifest.source_version.clone()),
            sha256: manifest.map(|manifest| manifest.sha256.clone()),
            downloaded_at: manifest.map(|manifest| manifest.downloaded_at),
            vehicle_count: manifest.map(|manifest| manifest.vehicle_count),
            skipped_row_count: manifest.map(|manifest| manifest.skipped_row_count),
            last_attempt_at: data.last_attempt_at,
            last_success_at: data.last_success_at,
            last_error_code: data.last_error_code.clone(),
            update_in_progress: updating,
        };
        (data.snapshot.clone(), status)
    }

    fn status(&self) -> ExpectedValuesStatus {
        self.snapshot_and_status().1
    }
}

struct ExpectedValuesProgressGuard {
    runtime: ExpectedValuesRuntime,
    active: bool,
}

impl Drop for ExpectedValuesProgressGuard {
    fn drop(&mut self) {
        if self.active {
            self.runtime.active_updates.fetch_sub(1, Ordering::SeqCst);
            self.active = false;
        }
    }
}

fn expected_error_code(code: ExpectedUpdateErrorCode) -> String {
    match code {
        ExpectedUpdateErrorCode::Offline => "offline",
        ExpectedUpdateErrorCode::HttpStatus => "http_status",
        ExpectedUpdateErrorCode::TooLarge => "too_large",
        ExpectedUpdateErrorCode::InvalidDataset => "invalid_dataset",
    }
    .to_owned()
}

fn expected_outcome(outcome: ExpectedUpdateOutcome) -> ExpectedValuesUpdateOutcome {
    match outcome {
        ExpectedUpdateOutcome::NotDue => ExpectedValuesUpdateOutcome::NotDue,
        ExpectedUpdateOutcome::Unchanged => ExpectedValuesUpdateOutcome::Unchanged,
        ExpectedUpdateOutcome::Activated => ExpectedValuesUpdateOutcome::Activated,
        ExpectedUpdateOutcome::OfflineUsingCurrent => {
            ExpectedValuesUpdateOutcome::OfflineUsingCurrent
        }
        ExpectedUpdateOutcome::RejectedUsingCurrent => {
            ExpectedValuesUpdateOutcome::RejectedUsingCurrent
        }
        ExpectedUpdateOutcome::Unavailable => ExpectedValuesUpdateOutcome::Unavailable,
        ExpectedUpdateOutcome::Busy => ExpectedValuesUpdateOutcome::Busy,
    }
}

async fn run_expected_values_update_with<F, Fut>(
    runtime: &ExpectedValuesRuntime,
    mode: ExpectedUpdateMode,
    updater: F,
) -> Result<ExpectedValuesRefreshResult, ExpectedValuesError>
where
    F: FnOnce(PathBuf, ExpectedUpdateMode) -> Fut,
    Fut: Future<Output = Result<ExpectedUpdateOutcome, ExpectedValuesError>>,
{
    let progress = runtime.begin_update();
    let result = updater(runtime.cache_root.clone(), mode).await;
    match result {
        Ok(outcome) => runtime.reload_after(outcome),
        Err(_) => runtime.reload_state_only(),
    }
    drop(progress);
    result.map(|outcome| ExpectedValuesRefreshResult {
        outcome: expected_outcome(outcome),
        status: runtime.status(),
    })
}

/// Every XVM download (startup, opt-in switch, manual refresh) goes through this gate.
/// Returns `None` when XVM is disabled and no request was attempted (audit M9).
async fn gated_expected_values_update_with<F, Fut>(
    database: &Database,
    runtime: &ExpectedValuesRuntime,
    mode: ExpectedUpdateMode,
    updater: F,
) -> Result<Option<ExpectedValuesRefreshResult>, ExpectedValuesError>
where
    F: FnOnce(PathBuf, ExpectedUpdateMode) -> Fut,
    Fut: Future<Output = Result<ExpectedUpdateOutcome, ExpectedValuesError>>,
{
    if !xvm_enabled(database) {
        return Ok(None);
    }
    run_expected_values_update_with(runtime, mode, updater)
        .await
        .map(Some)
}

async fn gated_expected_values_update(
    database: &Database,
    runtime: &ExpectedValuesRuntime,
    mode: ExpectedUpdateMode,
) -> Result<Option<ExpectedValuesRefreshResult>, ExpectedValuesError> {
    gated_expected_values_update_with(database, runtime, mode, |cache_root, update_mode| async move {
        update_expected_values(&cache_root, update_mode).await
    })
    .await
}

fn expected_values_update_failed() -> CommandError {
    CommandError {
        code: "expected_values_update_failed",
        message: "Expected values update failed safely; the previous snapshot is unchanged."
            .to_owned(),
    }
}

/// XVM downloads are opt-in (PRIVACY.md). An unreadable setting counts as disabled.
fn xvm_enabled(database: &Database) -> bool {
    matches!(database.get_setting("xvm_enabled"), Ok(Some(value)) if value == "1")
}

const DATABASE_FILE: &str = "afterbattle.sqlite3";
const RECOVERY_DATABASE_FILE: &str = "afterbattle.recovery.sqlite3";

/// The database opened at startup, or the recovery database when the real one cannot be opened.
#[derive(Debug)]
struct StartupDatabase {
    database: Database,
    startup_error_code: Option<String>,
    /// Set when a corrupt file was moved aside and a new database was created (D16, step 2).
    startup_notice_code: Option<String>,
}

impl StartupDatabase {
    /// Whether the local tracker may start and write battles into `database`.
    fn tracker_may_start(&self) -> bool {
        self.startup_error_code.is_none()
    }
}

/// A file SQLite cannot read as a database, or one that fails its integrity check. Other
/// failures (newer schema, P8 gate, a read-only or locked file) keep the file where it is.
fn is_corrupt_database(error: &AppError) -> bool {
    match error {
        AppError::DatabaseIntegrity => true,
        AppError::Database(rusqlite::Error::SqliteFailure(failure, _)) => matches!(
            failure.code,
            rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt
        ),
        _ => false,
    }
}

/// Moves a corrupt database and its companion files to `<stem>.corrupt-<ts>.sqlite3[-wal…]`.
fn move_corrupt_database(path: &std::path::Path, timestamp: i64) -> Result<(), AppError> {
    let target = schema::sibling(path, &format!("corrupt-{timestamp}.sqlite3"));
    for suffix in ["-wal", "-shm", "-journal", ""] {
        let mut from = path.as_os_str().to_owned();
        from.push(suffix);
        let mut to = target.as_os_str().to_owned();
        to.push(suffix);
        match fs::rename(PathBuf::from(from), PathBuf::from(to)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Opens the real database (rebuilding a legacy one from the configured replays, D5). A corrupt
/// file is moved to `afterbattle.corrupt-<ts>.sqlite3` and a new database is created in its
/// place, so the import rebuilds the history from the replays (D16, step 2). Any other failure
/// opens an empty recovery database at one fixed path next to it (never in `%TEMP%`); it is
/// removed on the next successful start.
#[cfg(test)]
fn open_startup_database(data_dir: &std::path::Path) -> Result<StartupDatabase, AppError> {
    open_startup_database_with(data_dir, &legacy::LegacySource::default())
}

fn open_startup_database_with(
    data_dir: &std::path::Path,
    source: &legacy::LegacySource,
) -> Result<StartupDatabase, AppError> {
    let recovery = data_dir.join(RECOVERY_DATABASE_FILE);
    let path = data_dir.join(DATABASE_FILE);
    let mut notice = None;
    let mut opened = Database::initialize_with(path.clone(), source);
    if let Err(error) = &opened
        && is_corrupt_database(error)
    {
        move_corrupt_database(&path, Utc::now().timestamp_millis())?;
        notice = Some("database_recovered".to_owned());
        opened = Database::initialize_with(path, source);
    }
    match opened {
        Ok(database) => {
            remove_recovery_database(&recovery)?;
            Ok(StartupDatabase {
                database,
                startup_error_code: None,
                startup_notice_code: notice,
            })
        }
        Err(error) => {
            let code = CommandError::from(error).code.to_owned();
            remove_recovery_database(&recovery)?;
            let database = Database::initialize(recovery)?;
            Ok(StartupDatabase {
                database,
                startup_error_code: Some(code),
                startup_notice_code: notice,
            })
        }
    }
}

fn remove_recovery_database(path: &std::path::Path) -> Result<(), AppError> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        match fs::remove_file(PathBuf::from(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Refuses "delete all data" while the real database could not be opened: wiping the recovery
/// database would report success while the real history stays on disk.
fn ensure_delete_all_data_allowed(startup_error_code: Option<&str>) -> Result<(), AppError> {
    match startup_error_code {
        Some(_) => Err(AppError::RecoveryModeDeleteRefused),
        None => Ok(()),
    }
}

fn view(db: &Database, b: Battle) -> Result<BattleView, AppError> {
    let baseline = db.feedback_baseline_for_battle(&b)?;
    Ok(BattleView {
        feedback: battle_feedback(&b, &baseline),
        battle: b,
    })
}
fn summary(
    db: &Database,
    s: Session,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<SessionSummary, AppError> {
    let b = db.session_battles(&s.id)?;
    let m = metrics(&b);
    let h = db.historical_battles_excluding_session(&s.id, 200)?;
    let hm = (!h.is_empty()).then(|| metrics(&h));
    let battle_wn8 = b
        .iter()
        .map(|battle| battle_wn8_presentation(battle, expected))
        .collect();
    let battles = b
        .iter()
        .cloned()
        .map(|battle| BattleView {
            battle,
            feedback: Vec::new(),
        })
        .collect();
    Ok(SessionSummary {
        session: s,
        feedback: session_feedback(&m, hm.as_ref()),
        metrics: m,
        wn8: session_wn8_presentation(&b, expected),
        battles,
        battle_wn8,
        battle_pace: summarize_battle_pace(
            b.iter().map(|battle| battle.battle_pace.context.as_ref()),
        ),
    })
}

fn battle_unavailable_reason(exclusions: &Wn8ExclusionCounts) -> Wn8UnavailableReason {
    if exclusions.partial_or_incomplete > 0 {
        Wn8UnavailableReason::PartialOrIncomplete
    } else if exclusions.unsupported_feature_schema > 0 {
        Wn8UnavailableReason::UnsupportedFeatureSchema
    } else if exclusions.vehicle_id_conflict > 0 {
        Wn8UnavailableReason::VehicleIdConflict
    } else if exclusions.missing_vehicle_id > 0 {
        Wn8UnavailableReason::MissingVehicleId
    } else if exclusions.vehicle_not_in_expected_values > 0 {
        Wn8UnavailableReason::VehicleNotInExpectedValues
    } else if exclusions.missing_damage > 0 {
        Wn8UnavailableReason::MissingDamage
    } else if exclusions.missing_frags > 0 {
        Wn8UnavailableReason::MissingFrags
    } else if exclusions.missing_spots > 0 {
        Wn8UnavailableReason::MissingSpots
    } else if exclusions.missing_defense > 0 {
        Wn8UnavailableReason::MissingDefense
    } else if exclusions.missing_result > 0 {
        Wn8UnavailableReason::MissingResult
    } else {
        Wn8UnavailableReason::InvalidNumericInput
    }
}

pub(crate) fn session_wn8_presentation(
    battles: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
) -> SessionWn8Presentation {
    let Some(expected) = expected else {
        return SessionWn8Presentation {
            state: Wn8PresentationState::Unavailable,
            value: None,
            eligible_battles: 0,
            total_battles: battles.len(),
            coverage_ratio: 0.0,
            exclusions: Wn8ExclusionCounts::default(),
            formula_version: WN8_FORMULA_VERSION.to_owned(),
            snapshot_source_version: None,
            snapshot_sha256: None,
            unavailable_reason: Some(Wn8UnavailableReason::ExpectedValuesUnavailable),
        };
    };
    match calculate_wn8(battles, expected) {
        Wn8Calculation::Available(result) => SessionWn8Presentation {
            state: Wn8PresentationState::Available,
            value: Some(result.value),
            eligible_battles: result.eligible_battles,
            total_battles: result.total_battles,
            coverage_ratio: result.coverage_ratio,
            exclusions: result.exclusions,
            formula_version: result.formula_version,
            snapshot_source_version: Some(result.snapshot_source_version),
            snapshot_sha256: Some(result.snapshot_sha256),
            unavailable_reason: None,
        },
        Wn8Calculation::Unavailable(result) => SessionWn8Presentation {
            state: Wn8PresentationState::Unavailable,
            value: None,
            eligible_battles: result.eligible_battles,
            total_battles: result.total_battles,
            coverage_ratio: result.coverage_ratio,
            exclusions: result.exclusions,
            formula_version: result.formula_version,
            snapshot_source_version: Some(result.snapshot_source_version),
            snapshot_sha256: Some(result.snapshot_sha256),
            unavailable_reason: Some(Wn8UnavailableReason::NoEligibleBattles),
        },
    }
}

pub(crate) fn battle_wn8_presentation(
    battle: &Battle,
    expected: Option<&LoadedExpectedSnapshot>,
) -> BattleWn8Presentation {
    let Some(expected) = expected else {
        return BattleWn8Presentation {
            battle_id: battle.arena_id.clone(),
            state: Wn8PresentationState::Unavailable,
            value: None,
            exclusion_reason: Some(Wn8UnavailableReason::ExpectedValuesUnavailable),
            formula_version: WN8_FORMULA_VERSION.to_owned(),
            snapshot_source_version: None,
        };
    };
    match calculate_battle_wn8(battle, expected) {
        Wn8Calculation::Available(result) => BattleWn8Presentation {
            battle_id: battle.arena_id.clone(),
            state: Wn8PresentationState::Available,
            value: Some(result.value),
            exclusion_reason: None,
            formula_version: result.formula_version,
            snapshot_source_version: Some(result.snapshot_source_version),
        },
        Wn8Calculation::Unavailable(result) => BattleWn8Presentation {
            battle_id: battle.arena_id.clone(),
            state: Wn8PresentationState::Unavailable,
            value: None,
            exclusion_reason: Some(battle_unavailable_reason(&result.exclusions)),
            formula_version: result.formula_version,
            snapshot_source_version: Some(result.snapshot_source_version),
        },
    }
}

fn wn8_presentation(
    active_battles: Option<&[Battle]>,
    recent_battles: &[Battle],
    expected: Option<&LoadedExpectedSnapshot>,
    expected_status: ExpectedValuesStatus,
) -> Wn8Presentation {
    Wn8Presentation {
        session: active_battles.map(|battles| session_wn8_presentation(battles, expected)),
        battles: recent_battles
            .iter()
            .map(|battle| battle_wn8_presentation(battle, expected))
            .collect(),
        expected_values: expected_status,
    }
}

/// WN8 of each of `sessions`, read only when the history asks for it (E3: the snapshot no
/// longer loads the battles of 30 sessions on every refresh).
fn sessions_wn8(
    sessions: &[Session],
    historical_battles: Vec<Battle>,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Vec<HistoricalSessionWn8Presentation> {
    let mut grouped = HashMap::<String, Vec<Battle>>::new();
    for battle in historical_battles {
        if let Some(session_id) = &battle.session_id {
            grouped.entry(session_id.clone()).or_default().push(battle);
        }
    }
    sessions
        .iter()
        .map(|session| HistoricalSessionWn8Presentation {
            session_id: session.id.clone(),
            wn8: session_wn8_presentation(
                grouped.get(&session.id).map(Vec::as_slice).unwrap_or(&[]),
                expected,
            ),
        })
        .collect()
}

fn recent_sessions_wn8(
    state: &AppState,
) -> Result<Vec<HistoricalSessionWn8Presentation>, AppError> {
    let (expected, _) = state.expected_values.snapshot_and_status();
    let sessions = state.database.recent_sessions(30)?;
    let battles = state.database.battles_for_sessions(
        &sessions
            .iter()
            .map(|session| session.id.clone())
            .collect::<Vec<_>>(),
    )?;
    Ok(sessions_wn8(&sessions, battles, expected.as_ref()))
}

#[tauri::command]
async fn get_recent_sessions_wn8(
    app: AppHandle,
) -> CommandResult<Vec<HistoricalSessionWn8Presentation>> {
    on_state(app, move |state| {
        recent_sessions_wn8(state).map_err(Into::into)
    })
    .await
}

fn snapshot(state: &AppState) -> Result<AppSnapshot, AppError> {
    if state.startup_error_code.is_some() {
        return Err(AppError::DatabaseIntegrity);
    }
    // Clone the immutable dataset under a short read lock. No runtime lock is held while SQLite
    // is queried or WN8 is calculated.
    let (expected_snapshot, expected_status) = state.expected_values.snapshot_and_status();
    let configured = state.tracker.status().configured;
    let xvm_enabled = state.database.get_setting("xvm_enabled")?.as_deref() == Some("1");
    let active_session = state.database.active_session()?;
    let active_battles = active_session
        .as_ref()
        .map(|session| state.database.session_battles(&session.id))
        .transpose()?;
    let recent_battles = active_battles.clone().unwrap_or_default();
    let recent_sessions = state.database.recent_sessions(30)?;
    let wn8_presentation = wn8_presentation(
        active_battles.as_deref(),
        &recent_battles,
        expected_snapshot.as_ref(),
        expected_status,
    );
    Ok(AppSnapshot {
        configured,
        onboarding_complete: state
            .database
            .get_setting("onboarding_complete")?
            .as_deref()
            == Some("1"),
        inactivity_minutes: state.database.inactivity_minutes()?,
        source_status: state.tracker.status(),
        advanced_status: AdvancedStatus {
            xvm_enabled,
            xvm_last_updated_at: state
                .database
                .get_setting("xvm_updated_at")?
                .and_then(|value| value.parse().ok()),
        },
        active_session,
        recent_battles: recent_battles
            .into_iter()
            .map(|b| view(&state.database, b))
            .collect::<Result<_, _>>()?,
        recent_sessions,
        wn8_presentation,
        active_challenge: state.database.active_challenge()?,
        latest_finished_challenge: state.database.latest_finished_challenge()?,
    })
}

/// Runs `job` on the blocking pool so a long SQLite or file operation never stalls the window
/// (spec 20, D6). A job that panics is reported as a tracker error instead of crashing.
async fn off_thread<T, F>(job: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CommandResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(job)
        .await
        .unwrap_or_else(|_| Err(CommandError::from(AppError::Tracker)))
}

/// [`off_thread`] with the managed [`AppState`].
async fn on_state<T, F>(app: AppHandle, job: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> CommandResult<T> + Send + 'static,
{
    off_thread(move || job(&app.state::<AppState>())).await
}

/// Ends the active session on the player's request and tells the frontend.
fn end_session_now(state: &AppState) -> Result<Session, AppError> {
    let session = state.database.end_active_session("ended")?;
    state.tracker.emit(events::AppEvent::SessionEnded {
        session_id: session.id.clone(),
    });
    state.tracker.emit(events::AppEvent::DataChanged);
    Ok(session)
}

/// Sends backend events to the window (spec 20, D6).
#[derive(Debug)]
struct TauriEvents(AppHandle);

impl events::EventSink for TauriEvents {
    fn emit(&self, event: events::AppEvent) {
        let _ = self.0.emit(event.name(), event.payload());
    }
}

#[tauri::command]
async fn get_snapshot(app: AppHandle) -> CommandResult<AppSnapshot> {
    on_state(app, move |state| snapshot(state).map_err(Into::into)).await
}

#[tauri::command]
fn get_expected_values_status(state: State<'_, AppState>) -> ExpectedValuesStatus {
    state.expected_values.status()
}

fn vehicle_class_catalog_status(state: &AppState) -> Result<VehicleClassCatalogStatus, AppError> {
    let snapshot = state.vehicle_classes.snapshot();
    let vehicles = state.database.distinct_vehicle_class_inputs()?;
    let (unresolved, conflicts) = resolution_counts(snapshot.as_ref(), &vehicles);
    let installed = state
        .tracker
        .configured_root()
        .and_then(|root| vehicle_class_catalog::installed_client_version(&root));
    Ok(state
        .vehicle_classes
        .status(unresolved, conflicts, installed.as_deref()))
}

#[tauri::command]
async fn get_vehicle_class_catalog_status(
    app: AppHandle,
) -> CommandResult<VehicleClassCatalogStatus> {
    on_state(app, move |state| {
        vehicle_class_catalog_status(state).map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn refresh_vehicle_class_catalog(
    state: State<'_, AppState>,
) -> CommandResult<VehicleClassCatalogRefreshResult> {
    let runtime = state.vehicle_classes.clone();
    let game_root = state.tracker.configured_root();
    let outcome =
        tauri::async_runtime::spawn_blocking(move || runtime.refresh(game_root.as_deref()))
            .await
            .map_err(|_| CommandError {
                code: "vehicle_class_catalog_refresh_failed",
                message: "The local vehicle class catalog could not be refreshed safely."
                    .to_owned(),
            })?
            .map_err(|_| CommandError {
                code: "vehicle_class_catalog_refresh_failed",
                message: "The local vehicle class catalog could not be refreshed safely."
                    .to_owned(),
            })?;
    Ok(VehicleClassCatalogRefreshResult {
        outcome,
        status: vehicle_class_catalog_status(&state).map_err(CommandError::from)?,
    })
}

#[tauri::command]
async fn refresh_expected_values(
    state: State<'_, AppState>,
) -> CommandResult<ExpectedValuesRefreshResult> {
    gated_expected_values_update(
        &state.database,
        &state.expected_values,
        ExpectedUpdateMode::ManualForce,
    )
    .await
    .map_err(|_| expected_values_update_failed())?
    .ok_or_else(|| CommandError::from(AppError::XvmDisabled))
}

#[tauri::command]
async fn detect_wot_installations() -> CommandResult<Vec<String>> {
    off_thread(|| Ok(LocalTracker::detect_installations())).await
}
#[tauri::command]
async fn configure_local_source(
    game_root: String,
    language: String,
    app: AppHandle,
) -> CommandResult<SourceStatus> {
    on_state(app, move |state| {
        state
            .database
            .update_preferences(&language, 60)
            .map_err(CommandError::from)?;
        let status = state
            .tracker
            .configure(&game_root)
            .map_err(CommandError::from)?;
        let runtime = state.vehicle_classes.clone();
        let configured_root = state.tracker.configured_root();
        tauri::async_runtime::spawn_blocking(move || {
            let _ = runtime.refresh(configured_root.as_deref());
        });
        Ok(status)
    })
    .await
}
#[tauri::command]
fn pause_tracker(state: State<'_, AppState>) -> SourceStatus {
    state.tracker.pause()
}
#[tauri::command]
fn resume_tracker(state: State<'_, AppState>) -> SourceStatus {
    state.tracker.resume()
}
/// Starts the full import in the background; progress arrives as `import-progress` events.
#[tauri::command]
async fn start_import(app: AppHandle) -> CommandResult<()> {
    on_state(app, move |state| {
        state
            .tracker
            .import_history()
            .map(|_| ())
            .map_err(Into::into)
    })
    .await
}

/// Stops the running full import after its current batch; the next start continues from the
/// cursor.
#[tauri::command]
fn cancel_import(state: State<'_, AppState>) {
    state.tracker.cancel_import();
}

#[tauri::command]
async fn set_xvm_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> CommandResult<Option<ExpectedValuesRefreshResult>> {
    state
        .database
        .set_setting("xvm_enabled", if enabled { "1" } else { "0" })
        .map_err(CommandError::from)?;
    // Turning XVM on fetches the expected values once (respecting the daily schedule);
    // turning it off sends nothing.
    gated_expected_values_update(
        &state.database,
        &state.expected_values,
        ExpectedUpdateMode::Automatic,
    )
    .await
    .map_err(|_| expected_values_update_failed())
}

#[tauri::command]
async fn start_session(app: AppHandle) -> CommandResult<Session> {
    on_state(app, move |state| {
        state.database.start_session().map_err(Into::into)
    })
    .await
}
#[tauri::command]
async fn end_session(app: AppHandle) -> CommandResult<SessionSummary> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        let s = end_session_now(state).map_err(CommandError::from)?;
        summary(&state.database, s, expected.as_ref()).map_err(Into::into)
    })
    .await
}
#[tauri::command]
async fn get_session_summary(session_id: String, app: AppHandle) -> CommandResult<SessionSummary> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        let s = state
            .database
            .session(&session_id)
            .map_err(CommandError::from)?;
        summary(&state.database, s, expected.as_ref()).map_err(Into::into)
    })
    .await
}

fn attempts_conversion_report_for_battles(
    scope: AttemptsConversionScopeDto,
    battles: &[attempts_conversion::AttemptsConversionBattle],
    catalog: Option<&vehicle_class_catalog::LoadedVehicleClassCatalog>,
) -> Result<AttemptsConversionReportDto, AppError> {
    let report = analyze_with_catalog(battles, catalog)
        .map_err(|_| AppError::AttemptsConversionReportInvariant)?;
    report_to_dto(scope, report).map_err(|_| AppError::AttemptsConversionReportInvariant)
}

fn attempts_conversion_session_report(
    database: &Database,
    catalog: Option<&vehicle_class_catalog::LoadedVehicleClassCatalog>,
    session_id: &str,
) -> Result<AttemptsConversionReportDto, AppError> {
    let battles = database.attempts_conversion_session_battles(session_id)?;
    attempts_conversion_report_for_battles(
        AttemptsConversionScopeDto::Session {
            session_id: session_id.to_owned(),
        },
        &battles,
        catalog,
    )
}

fn attempts_conversion_window(window: u32) -> Result<usize, AppError> {
    match window {
        10 | 30 | 100 => Ok(window as usize),
        _ => Err(AppError::InvalidAttemptsConversionWindow),
    }
}

fn attempts_conversion_recent_report(
    database: &Database,
    catalog: Option<&vehicle_class_catalog::LoadedVehicleClassCatalog>,
    window: u32,
) -> Result<AttemptsConversionReportDto, AppError> {
    let limit = attempts_conversion_window(window)?;
    let battles = database.attempts_conversion_recent_battles(limit)?;
    attempts_conversion_report_for_battles(
        AttemptsConversionScopeDto::Recent {
            window: window as u16,
        },
        &battles,
        catalog,
    )
}

#[tauri::command]
async fn get_attempts_conversion_session_report(
    session_id: String,
    app: AppHandle,
) -> CommandResult<AttemptsConversionReportDto> {
    on_state(app, move |state| {
        let catalog = state.vehicle_classes.snapshot();
        attempts_conversion_session_report(&state.database, catalog.as_ref(), &session_id)
            .map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn get_attempts_conversion_recent_report(
    window: u32,
    app: AppHandle,
) -> CommandResult<AttemptsConversionReportDto> {
    on_state(app, move |state| {
        let catalog = state.vehicle_classes.snapshot();
        attempts_conversion_recent_report(&state.database, catalog.as_ref(), window)
            .map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn get_battle_history_page(
    filter: BattleHistoryFilter,
    app: AppHandle,
) -> CommandResult<BattleHistoryPage> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        battle_history_page(&state.database, &filter, expected.as_ref()).map_err(Into::into)
    })
    .await
}
#[tauri::command]
async fn get_vehicle_history_overview(app: AppHandle) -> CommandResult<VehicleHistoryOverview> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        let vehicle_classes = state.vehicle_classes.snapshot();
        vehicle_history_overview(&state.database, expected.as_ref(), vehicle_classes.as_ref())
            .map_err(Into::into)
    })
    .await
}
#[tauri::command]
async fn get_vehicle_history_report(
    vehicle_id: i64,
    app: AppHandle,
) -> CommandResult<VehicleHistoryReport> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        let vehicle_classes = state.vehicle_classes.snapshot();
        vehicle_history_report(
            &state.database,
            vehicle_id,
            expected.as_ref(),
            vehicle_classes.as_ref(),
        )
        .map_err(Into::into)
    })
    .await
}
#[tauri::command]
async fn get_player_baseline(app: AppHandle) -> CommandResult<PlayerBaselinePresentation> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        let active = state
            .database
            .active_session()
            .map_err(CommandError::from)?;
        let cutoff = active.as_ref().map_or_else(
            || Utc::now().timestamp_millis(),
            |session| session.started_at,
        );
        player_baseline(
            &state.database,
            cutoff,
            active.as_ref().map(|session| session.id.as_str()),
            expected.as_ref(),
        )
        .map_err(Into::into)
    })
    .await
}
#[tauri::command]
async fn get_vehicle_baseline(
    vehicle_id: i64,
    app: AppHandle,
) -> CommandResult<VehicleBaselinePresentation> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        let active = state
            .database
            .active_session()
            .map_err(CommandError::from)?;
        let cutoff = active.as_ref().map_or_else(
            || Utc::now().timestamp_millis(),
            |session| session.started_at,
        );
        vehicle_baseline(
            &state.database,
            vehicle_id,
            cutoff,
            active.as_ref().map(|session| session.id.as_str()),
            expected.as_ref(),
        )
        .map_err(Into::into)
    })
    .await
}
#[tauri::command]
async fn get_session_baseline_comparison(
    session_id: String,
    app: AppHandle,
) -> CommandResult<SessionBaselineComparison> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        session_baseline_comparison(&state.database, &session_id, expected.as_ref())
            .map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn get_performance_trends(
    range: PerformanceTrendRange,
    vehicle_id: Option<i64>,
    app: AppHandle,
) -> CommandResult<PerformanceTrendReport> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        performance_trends(
            &state.database,
            range,
            vehicle_id,
            expected.as_ref(),
            Utc::now().timestamp_millis(),
        )
        .map_err(Into::into)
    })
    .await
}

fn challenge_suggestions_for(
    state: &AppState,
    session_id: &str,
) -> Result<ChallengeSuggestionResponse, AppError> {
    let (expected, _) = state.expected_values.snapshot_and_status();
    let baseline = session_baseline_context(&state.database, session_id, expected.as_ref())?;
    let catalog = state.vehicle_classes.snapshot();
    let resolutions = baseline
        .comparison
        .vehicles
        .iter()
        .map(|vehicle| {
            let resolution = catalog.as_ref().map_or_else(
                || unavailable_resolution(vehicle.vehicle_id),
                |snapshot| snapshot.resolve(vehicle.vehicle_id, vehicle.vehicle_key.as_deref()),
            );
            (vehicle.vehicle_id, resolution)
        })
        .collect::<BTreeMap<_, _>>();
    let inputs = build_vehicle_inputs(
        &baseline.comparison,
        &baseline.session_battles,
        &baseline.vehicle_baseline_battles,
        &resolutions,
    );
    Ok(generate_suggestions(
        &baseline.comparison,
        &inputs,
        state.database.active_challenge()?.is_some(),
    ))
}

fn challenge_command_error(state: &AppState, error: AppError) -> CommandError {
    let command = CommandError::from(error);
    if matches!(
        command.code,
        "stale_suggestion" | "challenge_already_active"
    ) {
        *state
            .challenge_last_error_code
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(command.code.to_owned());
    }
    command
}

#[tauri::command]
async fn get_challenge_suggestions(
    session_id: String,
    app: AppHandle,
) -> CommandResult<ChallengeSuggestionResponse> {
    on_state(app, move |state| {
        challenge_suggestions_for(state, &session_id).map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn start_vehicle_challenge(
    session_id: String,
    suggestion_id: String,
    app: AppHandle,
) -> CommandResult<ChallengePresentation> {
    on_state(app, move |state| {
        let response = challenge_suggestions_for(state, &session_id)
            .map_err(|error| challenge_command_error(state, error))?;
        let suggestion = response
            .suggestions
            .into_iter()
            .find(|suggestion| suggestion.suggestion_id == suggestion_id)
            .ok_or_else(|| challenge_command_error(state, AppError::StaleChallengeSuggestion))?;

        // Resolve again at the mutation boundary. The optional vehicle key is only a resolver
        // cross-check; it is never a class fallback.
        let vehicle_key = state
            .database
            .session_battles(&session_id)
            .map_err(|error| challenge_command_error(state, error))?
            .into_iter()
            .filter(|battle| battle.wg_tank_id == Some(suggestion.vehicle_id))
            .max_by_key(|battle| (battle.battle_time, battle.arena_id.clone()))
            .map(|battle| battle.vehicle_key);
        let resolution = state.vehicle_classes.snapshot().map_or_else(
            || unavailable_resolution(suggestion.vehicle_id),
            |snapshot| snapshot.resolve(suggestion.vehicle_id, vehicle_key.as_deref()),
        );
        if resolution.state != VehicleClassResolutionState::Resolved
            || resolution.vehicle_class != suggestion.vehicle_class
            || resolution.source != suggestion.vehicle_class_source
            || resolution.catalog_schema_version
                != Some(suggestion.vehicle_class_catalog_schema_version)
            || resolution.protocol_source != suggestion.compact_descriptor_protocol_source
            || !challenges::allowed_challenge(suggestion.vehicle_class, suggestion.challenge_type)
        {
            return Err(challenge_command_error(
                state,
                AppError::StaleChallengeSuggestion,
            ));
        }
        state
            .database
            .start_challenge(&session_id, &suggestion, Utc::now().timestamp_millis())
            .map_err(|error| challenge_command_error(state, error))
    })
    .await
}

#[tauri::command]
async fn get_active_challenge(app: AppHandle) -> CommandResult<Option<ChallengePresentation>> {
    on_state(app, move |state| {
        state.database.active_challenge().map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn get_challenge_history(
    offset: usize,
    page_size: usize,
    app: AppHandle,
) -> CommandResult<ChallengeHistoryPage> {
    on_state(app, move |state| {
        state
            .database
            .challenge_history_page(offset, page_size)
            .map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn abandon_active_challenge(app: AppHandle) -> CommandResult<ChallengePresentation> {
    on_state(app, move |state| {
        state
            .database
            .abandon_active_challenge()
            .map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn update_preferences(
    language: String,
    inactivity_minutes: u32,
    app: AppHandle,
) -> CommandResult<AppSnapshot> {
    on_state(app, move |state| {
        state
            .database
            .update_preferences(&language, inactivity_minutes)
            .map_err(CommandError::from)?;
        snapshot(state).map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn complete_onboarding(app: AppHandle) -> CommandResult<()> {
    on_state(app, move |state| {
        state
            .database
            .set_setting("onboarding_complete", "1")
            .map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn reset_onboarding(app: AppHandle) -> CommandResult<()> {
    on_state(app, move |state| {
        state
            .database
            .remove_setting("onboarding_complete")
            .map_err(Into::into)
    })
    .await
}

#[tauri::command]
async fn get_diagnostics(language: String, app: AppHandle) -> CommandResult<String> {
    on_state(app, move |state| {
        let (_, expected) = state.expected_values.snapshot_and_status();
        let vehicle_classes = vehicle_class_catalog_status(state).map_err(CommandError::from)?;
        let challenge_last_error = state
            .challenge_last_error_code
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut report = build_report(
            &state.database,
            &state.tracker.status(),
            &expected,
            &vehicle_classes,
            challenge_last_error.as_deref(),
            &language,
            Utc::now().timestamp_millis(),
        )
        .map_err(CommandError::from)?;
        for code in state
            .startup_error_code
            .iter()
            .chain(state.startup_notice_code.iter())
        {
            report.recent_error_codes.push(code.clone());
            report.recent_error_codes.sort();
            report.recent_error_codes.dedup();
        }
        serde_json::to_string_pretty(&report)
            .map_err(|_| CommandError::from(AppError::InvalidApiResponse))
    })
    .await
}

#[tauri::command]
async fn export_anonymous_session(session_id: String, app: AppHandle) -> CommandResult<String> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        let s = state
            .database
            .session(&session_id)
            .map_err(CommandError::from)?;
        let sum =
            summary(&state.database, s.clone(), expected.as_ref()).map_err(CommandError::from)?;
        let battles = state
            .database
            .session_battles(&session_id)
            .map_err(CommandError::from)?;
        anonymous_session_export(&s, &sum, &battles, Utc::now().timestamp_millis())
            .map_err(CommandError::from)
    })
    .await
}

fn anonymous_session_export(
    session: &Session,
    summary: &SessionSummary,
    battles: &[Battle],
    now: i64,
) -> Result<String, AppError> {
    let anonymous_battles = battles
        .iter()
        .map(|battle| {
            json!({
                "sourceKind": battle.source_kind,
                "dataQuality": battle.data_quality,
                "outcome": battle.outcome,
                "damage": battle.damage,
                "assist": battle.assist,
                "blocked": battle.blocked,
                "frags": battle.frags,
                "spots": battle.spots,
                "survived": battle.survived,
                "xp": battle.xp
            })
        })
        .collect::<Vec<Value>>();
    let duration = session
        .ended_at
        .unwrap_or(now)
        .saturating_sub(session.started_at)
        / 60_000;
    serde_json::to_string_pretty(&json!({
        "schemaVersion": 3,
        "app": "Afterbattle",
        "appVersion": env!("CARGO_PKG_VERSION"),
        "anonymous": true,
        "session": {
            "durationMinutes": duration,
            "status": session.status,
            "metrics": summary.metrics,
            "feedback": summary.feedback,
            "battles": anonymous_battles
        }
    }))
    .map_err(|_| AppError::InvalidApiResponse)
}
#[tauri::command]
async fn delete_all_data(app: AppHandle) -> CommandResult<()> {
    on_state(app, move |state| {
        ensure_delete_all_data_allowed(state.startup_error_code.as_deref())
            .map_err(CommandError::from)?;
        state
            .tracker
            .clear_configuration()
            .map_err(CommandError::from)?;
        legacy_credentials::purge_legacy_credentials(
            &legacy_credentials::KeyringStore,
            &state.database,
        )
        .map_err(CommandError::from)?;
        state.database.wipe().map_err(CommandError::from)?;
        state.game_images.clear();
        state.vehicle_names.clear();
        remove_backups_and_corrupt_copies(&state.data_dir).map_err(Into::into)
    })
    .await
}

/// The Battle screen: one battle with its feedback and the comparison of both teams from
/// `battle_team_stats` (D17, never roster rows).
#[tauri::command]
async fn get_battle_view(
    app: AppHandle,
    battle_id: String,
) -> CommandResult<models::BattleDetailView> {
    on_state(app, move |state| {
        battle_detail(&state.database, &battle_id).map_err(Into::into)
    })
    .await
}

fn battle_detail(
    database: &Database,
    battle_id: &str,
) -> Result<models::BattleDetailView, AppError> {
    let battle = database
        .battle_by_id(battle_id)?
        .ok_or(AppError::BattleNotFound)?;
    let (health_remaining, max_health, most_damage_in_team) =
        database.battle_own_extras(battle_id)?;
    Ok(models::BattleDetailView {
        health_remaining,
        max_health,
        most_damage_in_team,
        teams: database.team_comparison(battle_id)?,
        view: view(database, battle)?,
    })
}

/// Bytes of a vehicle icon or map image from the game folder, held only in RAM (D19); an empty
/// response means "no image" and the window shows a placeholder. A map asked for with `width`
/// and `height` (device pixels of the box it fills) comes sharp at that size (E5-maps-hd): PNG
/// from the game files or made from the loading screen.
#[tauri::command]
async fn get_game_image(
    app: AppHandle,
    kind: String,
    key: String,
    width: Option<u32>,
    height: Option<u32>,
) -> CommandResult<tauri::ipc::Response> {
    on_state(app, move |state| {
        Ok(tauri::ipc::Response::new(game_image_bytes(
            &state.game_images,
            state.tracker.configured_root().as_deref(),
            &kind,
            &key,
            (width.unwrap_or(0), height.unwrap_or(0)),
        )))
    })
    .await
}

/// The weekly report (spec 20 E7, spec 27): this ISO week in local time against the four weeks
/// before, weekly goals and last week's result. WN8 only with XVM data.
#[tauri::command]
async fn get_weekly_report(app: AppHandle) -> CommandResult<weekly::WeeklyReport> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        weekly_report_view(
            &state.database,
            expected.as_ref(),
            Utc::now().timestamp_millis(),
            &chrono::Local,
        )
        .map_err(Into::into)
    })
    .await
}

fn weekly_report_view<Tz: chrono::TimeZone>(
    database: &Database,
    expected: Option<&LoadedExpectedSnapshot>,
    now: i64,
    zone: &Tz,
) -> Result<weekly::WeeklyReport, AppError> {
    let battles = database.all_history_battles()?;
    let finished = database.challenge_history_page(0, 50)?.items;
    let finished = finished
        .iter()
        .filter_map(|challenge| Some((challenge.status.as_str(), challenge.finished_at?)))
        .collect::<Vec<_>>();
    let mut report = weekly::weekly_report(&battles, &finished, now, zone);
    if let Some(expected) = expected {
        let values = weekly::week_battles(&battles, now, zone)
            .iter()
            .map(|week| session_wn8_presentation(week, Some(expected)).value)
            .collect::<Vec<_>>();
        report.wn8 = Some(weekly::WeeklyWn8 {
            current: values[0],
            previous: values[1..].to_vec(),
        });
    }
    Ok(report)
}

/// The deterministic story of one session (E5-flare §2): headline, best battle, moments. A
/// failed baseline comparison only takes the comparison-based parts away.
#[tauri::command]
async fn get_session_story(
    app: AppHandle,
    session_id: String,
) -> CommandResult<session_story::SessionStory> {
    on_state(app, move |state| {
        let (expected, _) = state.expected_values.snapshot_and_status();
        session_story_view(&state.database, &session_id, expected.as_ref()).map_err(Into::into)
    })
    .await
}

fn session_story_view(
    database: &Database,
    session_id: &str,
    expected: Option<&LoadedExpectedSnapshot>,
) -> Result<session_story::SessionStory, AppError> {
    let battles = database.session_battles(session_id)?;
    let comparison = session_baseline_comparison(database, session_id, expected).ok();
    Ok(session_story::session_story(&battles, comparison.as_ref()))
}

/// Map keys are lowercase letters, digits and underscores (`NN_name`).
fn valid_map_key(map_key: &str) -> bool {
    (1..=128).contains(&map_key.len())
        && map_key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

/// Every counted battle of the player on one map (E5-flare §4, "You on <map>").
#[tauri::command]
async fn get_map_record(
    app: AppHandle,
    map_key: String,
) -> CommandResult<session_story::MapRecord> {
    on_state(app, move |state| {
        map_record_view(&state.database, &map_key).map_err(Into::into)
    })
    .await
}

fn map_record_view(
    database: &Database,
    map_key: &str,
) -> Result<session_story::MapRecord, AppError> {
    // An unknown or malformed key is simply a map without battles.
    let battles = if valid_map_key(map_key) {
        database.battles_on_map(map_key)?
    } else {
        Vec::new()
    };
    Ok(session_story::map_record(map_key, &battles))
}

/// Up to four map keys per session for the session cards in History (E5-flare §5).
#[tauri::command]
async fn get_session_maps(
    app: AppHandle,
    session_ids: Vec<String>,
) -> CommandResult<Vec<session_story::SessionMaps>> {
    on_state(app, move |state| {
        session_maps_view(&state.database, session_ids).map_err(Into::into)
    })
    .await
}

fn session_maps_view(
    database: &Database,
    mut session_ids: Vec<String>,
) -> Result<Vec<session_story::SessionMaps>, AppError> {
    session_ids.truncate(session_story::MAX_SESSION_IDS);
    let battles = database.battles_for_sessions(&session_ids)?;
    Ok(session_story::session_maps(&session_ids, &battles))
}

/// Vehicle names from the game's translation files, in the window language or English (E5-ui
/// §2). Held only in RAM; without a game folder or a usable language the list is empty and the
/// window shows the readable code.
#[tauri::command]
async fn get_vehicle_names(
    app: AppHandle,
    language: String,
) -> CommandResult<vehicle_names::VehicleNamesView> {
    on_state(app, move |state| {
        Ok(vehicle_names_view(
            &state.vehicle_names,
            state.tracker.configured_root().as_deref(),
            &language,
        ))
    })
    .await
}

fn vehicle_names_view(
    names: &vehicle_names::VehicleNames,
    game_root: Option<&std::path::Path>,
    language: &str,
) -> vehicle_names::VehicleNamesView {
    match (game_root, language) {
        (Some(root), "pl" | "en") => names.view(root, language),
        _ => vehicle_names::VehicleNamesView::default(),
    }
}

fn game_image_bytes(
    images: &game_images::GameImages,
    game_root: Option<&std::path::Path>,
    kind: &str,
    key: &str,
    (width, height): (u32, u32),
) -> Vec<u8> {
    let (Some(root), Some(kind)) = (game_root, game_images::ImageKind::parse(kind)) else {
        return Vec::new();
    };
    images
        .get_sized(root, kind, key, width, height)
        .map(|image| image.as_ref().clone())
        .unwrap_or_default()
}

/// "Delete all data" also removes the migration backups (`afterbattle.v{n}.bak`) and the moved
/// corrupt databases (`afterbattle.corrupt-*`) next to the database (D16).
fn remove_backups_and_corrupt_copies(data_dir: &std::path::Path) -> Result<(), AppError> {
    let stem = DATABASE_FILE.trim_end_matches(".sqlite3");
    for entry in fs::read_dir(data_dir)?.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix(&format!("{stem}.")) else {
            continue;
        };
        let is_backup = rest.starts_with('v') && rest.contains(".bak");
        if (is_backup || rest.starts_with("corrupt-"))
            && entry.file_type().is_ok_and(|kind| kind.is_file())
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn show_main<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            fs::create_dir_all(&dir)?;
            let expected_values =
                ExpectedValuesRuntime::load(dir.join("expected-values").join("wn8-wg"));
            let vehicle_classes =
                VehicleClassCatalogRuntime::load(dir.join("vehicle-class-catalog").join("wg"));
            let source = legacy::LegacySource {
                replay_dir: local_tracker::configured_replay_dir(),
            };
            let startup = open_startup_database_with(&dir, &source)
                .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            let tracker_may_start = startup.tracker_may_start();
            let StartupDatabase {
                database: db,
                startup_error_code,
                startup_notice_code,
            } = startup;
            if tracker_may_start {
                // D2: credentials of the removed Tomato.gg and Wargaming integrations.
                let _ = legacy_credentials::purge_legacy_credentials(
                    &legacy_credentials::KeyringStore,
                    &db,
                );
            }
            let challenge_last_error_code = Arc::new(RwLock::new(None));
            let tracker = if tracker_may_start {
                LocalTracker::new(
                    db.clone(),
                    Arc::clone(&challenge_last_error_code),
                    Some(vehicle_classes.clone()),
                )
            } else {
                LocalTracker::disabled(db.clone(), Arc::clone(&challenge_last_error_code))
            };
            tracker.set_events(Arc::new(TauriEvents(app.handle().clone())));
            let vehicle_class_source = tracker.configured_root();
            app.manage(AppState {
                database: db.clone(),
                tracker,
                expected_values: expected_values.clone(),
                vehicle_classes: vehicle_classes.clone(),
                challenge_last_error_code: Arc::clone(&challenge_last_error_code),
                startup_error_code,
                startup_notice_code,
                data_dir: dir.clone(),
                game_images: game_images::GameImages::default(),
                vehicle_names: vehicle_names::VehicleNames::default(),
            });
            let xvm_db = db.clone();
            tauri::async_runtime::spawn(async move {
                let _ = gated_expected_values_update(
                    &xvm_db,
                    &expected_values,
                    ExpectedUpdateMode::Automatic,
                )
                .await;
            });
            // With a running tracker the catalog is refreshed by its worker before the first
            // import (E2), so every imported battle gets its tier.
            if !tracker_may_start {
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = vehicle_classes.refresh(vehicle_class_source.as_deref());
                });
            }
            let challenge_db = db.clone();
            tauri::async_runtime::spawn_blocking(move || {
                if challenge_db.catch_up_active_challenge().is_err() {
                    *challenge_last_error_code
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some("challenge_catch_up_failed".to_owned());
                }
            });
            let open = MenuItem::with_id(app, "open", "Open Afterbattle", true, None::<&str>)?;
            let pause =
                MenuItem::with_id(app, "pause", "Pause / Resume tracker", true, None::<&str>)?;
            let end = MenuItem::with_id(app, "end", "End session", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &pause, &end, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().ok_or("missing icon")?)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_main(app),
                    "pause" => {
                        let s = app.state::<AppState>();
                        if s.tracker.status().paused {
                            s.tracker.resume();
                        } else {
                            s.tracker.pause();
                        }
                    }
                    "end" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let _ = end_session_now(&app.state::<AppState>());
                        });
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        show_main(tray.app_handle())
                    }
                })
                .build(app)?;
            // Sessions end on time in one place: the tracker worker (D6).
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_expected_values_status,
            refresh_expected_values,
            get_vehicle_class_catalog_status,
            refresh_vehicle_class_catalog,
            detect_wot_installations,
            configure_local_source,
            pause_tracker,
            resume_tracker,
            start_import,
            cancel_import,
            get_recent_sessions_wn8,
            set_xvm_enabled,
            start_session,
            end_session,
            get_session_summary,
            get_attempts_conversion_session_report,
            get_attempts_conversion_recent_report,
            get_battle_history_page,
            get_vehicle_history_overview,
            get_vehicle_history_report,
            get_player_baseline,
            get_vehicle_baseline,
            get_session_baseline_comparison,
            get_performance_trends,
            get_challenge_suggestions,
            start_vehicle_challenge,
            get_active_challenge,
            get_challenge_history,
            abandon_active_challenge,
            update_preferences,
            complete_onboarding,
            reset_onboarding,
            get_diagnostics,
            export_anonymous_session,
            delete_all_data,
            get_game_image,
            get_vehicle_names,
            get_session_story,
            get_weekly_report,
            get_map_record,
            get_session_maps,
            get_battle_view
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Afterbattle");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expected_values::activate_snapshot_bytes,
        models::{BattleOutcome, DataQuality, SourceKind},
    };
    use rusqlite::{Connection, OpenFlags, types::ValueRef};
    use sha2::{Digest, Sha256};
    use std::future::ready;
    use tempfile::tempdir;

    fn expected_dataset(version: &str) -> Vec<u8> {
        let rows = (1..=500)
            .map(|id| {
                json!({
                    "IDNum": id,
                    "expDef": 0.5,
                    "expFrag": 0.8,
                    "expSpot": 1.1,
                    "expDamage": 900.0,
                    "expWinRate": 49.5
                })
            })
            .collect::<Vec<_>>();
        serde_json::to_vec(&json!({
            "header": {"source": "XVM", "version": version},
            "data": rows
        }))
        .unwrap()
    }

    fn activate_expected(cache_root: &std::path::Path, version: &str, downloaded_at: i64) {
        activate_snapshot_bytes(cache_root, &expected_dataset(version), downloaded_at).unwrap();
    }

    fn presentation_battle(
        id: &str,
        battle_time: i64,
        vehicle_id: i64,
        quality: DataQuality,
    ) -> Battle {
        Battle {
            arena_id: id.to_owned(),
            arena_unique_id: None,
            battle_time,
            battle_time_source: Some("metadata".to_owned()),
            vehicle_key: format!("vehicle-{vehicle_id}"),
            wg_tank_id: Some(vehicle_id),
            tank_name: None,
            map_name: None,
            map_key: None,
            battle_type: Some("random".to_owned()),
            gameplay_id: None,
            source_kind: models::SourceKind::Replay,
            data_quality: quality,
            parser_version: Some(2),
            feature_schema_version: Some(1),
            game_version: None,
            data_completeness: Some("results".to_owned()),
            replay_source: Some("replay_results".to_owned()),
            parser_error_code: None,
            outcome: BattleOutcome::Victory,
            battle_pace: crate::models::strict_random_pace(),
            player_team: Some(1),
            winner_team_raw: Some(1),
            damage: Some(1_800),
            assist: None,
            assist_spotting: None,
            assist_tracking: None,
            assist_stun: None,
            blocked: None,
            damage_received: None,
            frags: Some(2),
            spots: Some(2),
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
            xp: None,
            original_xp: None,
            duration_seconds: None,
            session_id: None,
        }
    }

    fn logical_database_digest(path: &std::path::Path) -> String {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        connection.execute_batch("PRAGMA query_only=ON;").unwrap();
        let tables = {
            let mut statement = connection
                .prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        let mut digest = Sha256::new();
        for table in tables {
            digest.update(table.as_bytes());
            let escaped_table = table.replace('"', "\"\"");
            let columns = {
                let mut statement = connection
                    .prepare(&format!("PRAGMA table_info(\"{escaped_table}\")"))
                    .unwrap();
                statement
                    .query_map([], |row| row.get::<_, String>(1))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
            };
            let quoted = columns
                .iter()
                .map(|column| format!("\"{}\"", column.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(",");
            let mut statement = connection
                .prepare(&format!(
                    "SELECT {quoted} FROM \"{escaped_table}\" ORDER BY {quoted}"
                ))
                .unwrap();
            let column_count = columns.len();
            let mut rows = statement.query([]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                digest.update([0xff]);
                for index in 0..column_count {
                    match row.get_ref(index).unwrap() {
                        ValueRef::Null => digest.update([0]),
                        ValueRef::Integer(value) => {
                            digest.update([1]);
                            digest.update(value.to_le_bytes());
                        }
                        ValueRef::Real(value) => {
                            digest.update([2]);
                            digest.update(value.to_bits().to_le_bytes());
                        }
                        ValueRef::Text(value) => {
                            digest.update([3]);
                            digest.update((value.len() as u64).to_le_bytes());
                            digest.update(value);
                        }
                        ValueRef::Blob(value) => {
                            digest.update([4]);
                            digest.update((value.len() as u64).to_le_bytes());
                            digest.update(value);
                        }
                    }
                }
            }
        }
        crate::hashing::hex_lower(digest.finalize())
    }

    #[test]
    fn anonymous_export_omits_local_and_account_identifiers() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("export.db")).unwrap();
        let mut battle = Battle {
            arena_id: "SECRET_ARENA_ID".into(),
            arena_unique_id: Some(1_111_222_233_334_444),
            battle_time: Utc::now().timestamp_millis(),
            battle_time_source: None,
            vehicle_key: "SECRET_VEHICLE_KEY".into(),
            wg_tank_id: Some(12345),
            tank_name: Some("SECRET_TANK_NAME".into()),
            map_name: Some("SECRET_MAP_NAME".into()),
            map_key: None,
            battle_type: Some("random".into()),
            gameplay_id: None,
            source_kind: SourceKind::LocalHybrid,
            data_quality: DataQuality::Full,
            parser_version: Some(2),
            feature_schema_version: None,
            game_version: Some("SECRET_GAME_VERSION".into()),
            data_completeness: Some("results".into()),
            replay_source: Some("replay_results".into()),
            parser_error_code: None,
            outcome: BattleOutcome::Victory,
            battle_pace: crate::models::strict_random_pace(),
            player_team: None,
            winner_team_raw: None,
            damage: Some(2345),
            assist: Some(321),
            assist_spotting: None,
            assist_tracking: None,
            assist_stun: None,
            blocked: Some(456),
            damage_received: Some(789),
            frags: Some(2),
            spots: Some(1),
            shots: None,
            hits: None,
            direct_enemy_hits: None,
            direct_team_hits: None,
            penetrations: None,
            piercing_enemy_hits: None,
            explosion_hits: None,
            defense_points: Some(7),
            moving_avg_damage: Some(4_490),
            damage_rating: Some(84),
            survived: Some(true),
            xp: Some(987),
            original_xp: None,
            duration_seconds: None,
            session_id: None,
        };
        database.insert_battle(&mut battle, true).unwrap();
        let ended = database.end_active_session("ended").unwrap();
        let report = summary(&database, ended.clone(), None).unwrap();
        let export = anonymous_session_export(
            &ended,
            &report,
            &database.session_battles(&ended.id).unwrap(),
            ended.ended_at.unwrap(),
        )
        .unwrap();

        for forbidden in [
            "SECRET_ARENA_ID",
            "1111222233334444",
            "SECRET_GAME_VERSION",
            "SECRET_VEHICLE_KEY",
            "SECRET_TANK_NAME",
            "SECRET_MAP_NAME",
            &ended.id,
            "12345",
        ] {
            assert!(!export.contains(forbidden), "export leaked {forbidden}");
        }
        let value: Value = serde_json::from_str(&export).unwrap();
        assert_eq!(value["schemaVersion"], 3);
        assert_eq!(value["anonymous"], true);
        assert_eq!(value["session"]["battles"][0]["damage"], 2345);
        assert_eq!(value["session"]["battles"][0]["outcome"], "victory");
        assert!(value["session"]["battles"][0].get("won").is_none());
    }

    /// D17: the session export never contains anything from `battle_roster` or
    /// `battle_team_stats`, and "delete all data" empties both tables.
    #[test]
    fn export_has_no_roster_data_and_wipe_clears_v10_tables() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("roster.db");
        let database = Database::initialize(path.clone()).unwrap();
        let (metadata, results) = crate::foundations::tests::synthetic_battle();
        let roster =
            crate::foundations::extract_battle_roster(&metadata, &results, &|_| None).unwrap();
        let extras = crate::foundations::ImportExtras {
            own: Some(crate::foundations::extract_own_player_facts(&results)),
            roster: Some(roster),
            packet_stream_bytes: Some(10),
            ..Default::default()
        };
        let mut battle = crate::storage::tests::full_replay_battle("roster-battle");
        let file = crate::storage::ReplayFileState {
            source_path_key: "roster-key".into(),
            file_size: 100,
            file_mtime_ms: 1,
            content_fingerprint: "f".into(),
            block_count: 2,
        };
        database
            .upsert_imported(&mut battle, &file, true, None, &extras)
            .unwrap();
        let count = |table: &str| -> i64 {
            rusqlite::Connection::open(&path)
                .unwrap()
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap()
        };
        assert_eq!(count("battle_roster"), 30);
        assert_eq!(count("battle_team_stats"), 2);
        let ended = database.end_active_session("ended").unwrap();
        let report = summary(&database, ended.clone(), None).unwrap();
        let export = anonymous_session_export(
            &ended,
            &report,
            &database.session_battles(&ended.id).unwrap(),
            ended.ended_at.unwrap(),
        )
        .unwrap()
        .to_ascii_lowercase();
        for forbidden in [
            "slot",
            "killer",
            "platoon",
            "roster",
            "typecompdescr",
            "tier",
            "lifetime",
            "mileage",
            "packet",
        ] {
            assert!(!export.contains(forbidden), "export contains {forbidden}");
        }
        database.wipe().unwrap();
        for table in [
            "battle_roster",
            "battle_team_stats",
            "battles",
            "replay_files",
            "replay_file_skips",
            "legacy_battles",
        ] {
            assert_eq!(count(table), 0, "{table}");
        }
    }

    fn counting_updater(
        calls: &Arc<AtomicUsize>,
    ) -> impl FnOnce(
        PathBuf,
        ExpectedUpdateMode,
    ) -> std::future::Ready<Result<ExpectedUpdateOutcome, ExpectedValuesError>> {
        let calls = Arc::clone(calls);
        move |_, _| {
            calls.fetch_add(1, Ordering::SeqCst);
            ready(Ok(ExpectedUpdateOutcome::Unchanged))
        }
    }

    fn xvm_fixture(enabled: Option<&str>) -> (tempfile::TempDir, Database, ExpectedValuesRuntime) {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("db.sqlite3")).unwrap();
        if let Some(value) = enabled {
            database.set_setting("xvm_enabled", value).unwrap();
        }
        let runtime = ExpectedValuesRuntime::load(directory.path().join("expected"));
        (directory, database, runtime)
    }

    // M9: the HTTP client is replaced by a counting fake; a disabled XVM must not see a request.
    #[test]
    fn xvm_update_sends_no_request_when_xvm_disabled_or_unset() {
        let cases = [None, Some("0"), Some("true"), Some("")]
            .into_iter()
            .flat_map(|setting| {
                [
                    ExpectedUpdateMode::Automatic,
                    ExpectedUpdateMode::ManualForce,
                ]
                .map(|mode| (setting, mode))
            });
        for (setting, mode) in cases {
            let (_directory, database, runtime) = xvm_fixture(setting);
            let calls = Arc::new(AtomicUsize::new(0));
            let result = tauri::async_runtime::block_on(gated_expected_values_update_with(
                &database,
                &runtime,
                mode,
                counting_updater(&calls),
            ))
            .unwrap();
            assert_eq!(
                calls.load(Ordering::SeqCst),
                0,
                "setting {setting:?}, {mode:?}"
            );
            assert!(result.is_none());
        }
    }

    #[test]
    fn xvm_update_runs_when_enabled() {
        let (_directory, database, runtime) = xvm_fixture(Some("1"));
        let calls = Arc::new(AtomicUsize::new(0));
        let result = tauri::async_runtime::block_on(gated_expected_values_update_with(
            &database,
            &runtime,
            ExpectedUpdateMode::Automatic,
            counting_updater(&calls),
        ))
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            result.unwrap().outcome,
            ExpectedValuesUpdateOutcome::Unchanged
        );
    }

    /// A database that cannot be opened but is not corrupt (written by a newer version), so it
    /// must stay in place and the app runs in recovery mode.
    fn corrupt_database_dir() -> tempfile::TempDir {
        let directory = tempdir().unwrap();
        let path = directory.path().join(DATABASE_FILE);
        drop(Database::initialize(path.clone()).unwrap());
        rusqlite::Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        directory
    }

    fn garbage_database_dir() -> tempfile::TempDir {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join(DATABASE_FILE),
            b"this is not an sqlite database, but it is large enough to be read as one; "
                .repeat(64),
        )
        .unwrap();
        directory
    }

    // D16 step 2: a corrupt file is moved aside, a new database is created and the tracker may
    // import into it; "delete all data" later removes the moved copy and backups.
    #[test]
    fn corrupt_database_is_moved_aside_and_a_new_database_is_created() {
        let data = garbage_database_dir();
        let before = fs::read(data.path().join(DATABASE_FILE)).unwrap();
        let startup = open_startup_database(data.path()).unwrap();
        assert!(startup.startup_error_code.is_none());
        assert!(startup.tracker_may_start());
        assert_eq!(
            startup.startup_notice_code.as_deref(),
            Some("database_recovered")
        );
        let moved = fs::read_dir(data.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("afterbattle.corrupt-") && name.ends_with(".sqlite3"))
            .collect::<Vec<_>>();
        assert_eq!(moved.len(), 1);
        assert_eq!(fs::read(data.path().join(&moved[0])).unwrap(), before);
        let mut battle = crate::storage::tests::full_replay_battle("after-recovery");
        assert!(startup.database.insert_battle(&mut battle, false).unwrap());
        drop(startup);
        let reopened = open_startup_database(data.path()).unwrap();
        assert!(reopened.startup_notice_code.is_none());
        assert_eq!(reopened.database.recent_battles(5).unwrap().len(), 1);

        fs::write(data.path().join("afterbattle.v9.bak"), b"backup").unwrap();
        fs::write(data.path().join("unrelated.v9.bak"), b"keep").unwrap();
        remove_backups_and_corrupt_copies(data.path()).unwrap();
        let left = fs::read_dir(data.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            left.iter()
                .all(|name| !name.contains("corrupt") && name != "afterbattle.v9.bak")
        );
        assert!(left.contains(&"unrelated.v9.bak".to_owned()));
        assert!(left.contains(&DATABASE_FILE.to_owned()));
    }

    #[test]
    fn only_unreadable_or_failed_integrity_counts_as_corrupt() {
        let failure = |code| {
            AppError::Database(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error {
                    code,
                    extended_code: 0,
                },
                None,
            ))
        };
        assert!(is_corrupt_database(&AppError::DatabaseIntegrity));
        assert!(is_corrupt_database(&failure(
            rusqlite::ErrorCode::NotADatabase
        )));
        assert!(is_corrupt_database(&failure(
            rusqlite::ErrorCode::DatabaseCorrupt
        )));
        assert!(!is_corrupt_database(&failure(
            rusqlite::ErrorCode::DatabaseBusy
        )));
        assert!(!is_corrupt_database(&AppError::DatabaseTooNew));
        assert!(!is_corrupt_database(&AppError::LegacyRebuildBlocked {
            missing: 1,
            total: 2
        }));
    }

    #[test]
    fn moving_a_corrupt_database_reports_errors_other_than_missing_files() {
        let data = garbage_database_dir();
        let path = data.path().join(DATABASE_FILE);
        fs::create_dir(data.path().join("afterbattle.corrupt-7.sqlite3")).unwrap();
        assert!(move_corrupt_database(&path, 7).is_err());
        assert!(path.exists());
        move_corrupt_database(&path, 8).unwrap();
        assert!(!path.exists());
        assert!(data.path().join("afterbattle.corrupt-8.sqlite3").exists());
    }

    #[test]
    fn deleting_copies_keeps_other_files_and_directories() {
        let data = tempdir().unwrap();
        for name in [
            "afterbattle.sqlite3",
            "afterbattle.v9.bak",
            "afterbattle.v10.bak",
            "afterbattle.corrupt-1.sqlite3",
            "afterbattle.corrupt-1.sqlite3-wal",
            "afterbattle.version-note.txt",
            "afterbattle.backup.bak",
            "other.corrupt-1.sqlite3",
        ] {
            fs::write(data.path().join(name), b"x").unwrap();
        }
        fs::create_dir(data.path().join("afterbattle.corrupt-dir")).unwrap();
        remove_backups_and_corrupt_copies(data.path()).unwrap();
        let mut left = fs::read_dir(data.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        left.sort();
        assert_eq!(
            left,
            [
                "afterbattle.backup.bak",
                "afterbattle.corrupt-dir",
                "afterbattle.sqlite3",
                "afterbattle.version-note.txt",
                "other.corrupt-1.sqlite3"
            ]
        );
    }

    #[test]
    fn read_only_database_does_not_lose_data() {
        let data = tempdir().unwrap();
        let path = data.path().join(DATABASE_FILE);
        drop(Database::initialize(path.clone()).unwrap());
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions.clone()).unwrap();
        let before = fs::read(&path).unwrap();
        let startup = open_startup_database(data.path());
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&path, permissions).unwrap();
        let startup = startup.unwrap();
        assert!(startup.startup_notice_code.is_none(), "never moved aside");
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    fn recovery_files(dir: &std::path::Path) -> Vec<String> {
        let mut names = fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("recovery"))
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    // N1: a corrupt database must not leave a new recovery database in %TEMP% on every start,
    // and the tracker must not write battles into the recovery database.
    #[test]
    fn corrupt_database_uses_one_fixed_recovery_path_outside_temp_and_blocks_tracker() {
        let data = corrupt_database_dir();
        let temp_before = recovery_files(&std::env::temp_dir());
        for _ in 0..3 {
            let startup = open_startup_database(data.path()).unwrap();
            assert!(startup.startup_error_code.is_some());
            assert!(!startup.tracker_may_start());
        }
        assert_eq!(
            recovery_files(&std::env::temp_dir()),
            temp_before,
            "no new files in temp"
        );
        assert!(
            recovery_files(data.path()).len() <= 3,
            "one database and its WAL files"
        );
        assert!(
            recovery_files(data.path())
                .iter()
                .all(|name| name.starts_with(RECOVERY_DATABASE_FILE))
        );
    }

    #[test]
    fn successful_start_removes_the_recovery_database() {
        let data = corrupt_database_dir();
        drop(open_startup_database(data.path()).unwrap());
        assert!(!recovery_files(data.path()).is_empty());
        fs::remove_file(data.path().join(DATABASE_FILE)).unwrap();
        let startup = open_startup_database(data.path()).unwrap();
        assert!(startup.startup_error_code.is_none());
        assert!(startup.tracker_may_start());
        assert!(recovery_files(data.path()).is_empty());
    }

    #[test]
    fn recovery_database_removal_reports_errors_other_than_missing_files() {
        let data = tempdir().unwrap();
        let recovery = data.path().join(RECOVERY_DATABASE_FILE);
        assert!(
            remove_recovery_database(&recovery).is_ok(),
            "missing files are fine"
        );
        fs::create_dir(&recovery).unwrap();
        assert!(remove_recovery_database(&recovery).is_err());
    }

    // N2: "delete all data" in recovery mode must refuse instead of reporting success, and the
    // real database file must stay untouched.
    #[test]
    fn battle_detail_of_a_missing_battle_is_battle_not_found() {
        let folder = tempfile::tempdir().unwrap();
        let database = Database::initialize(folder.path().join("detail.sqlite3")).unwrap();
        let error = battle_detail(&database, "missing").unwrap_err();
        assert_eq!(CommandError::from(error).code, "battle_not_found");
    }

    #[test]
    fn weekly_report_reads_the_database_and_adds_wn8_only_with_xvm() {
        let folder = tempdir().unwrap();
        let database = Database::initialize(folder.path().join("weekly.db")).unwrap();
        let mut battle = crate::storage::tests::full_replay_battle("synthetic-weekly-1");
        battle.battle_time = Utc::now().timestamp_millis() - 60_000;
        database.insert_battle(&mut battle, false).unwrap();
        let zone = chrono::FixedOffset::east_opt(0).unwrap();
        let report =
            weekly_report_view(&database, None, Utc::now().timestamp_millis(), &zone).unwrap();
        assert_eq!(report.battles, 1);
        assert_eq!(report.confidence, weekly::WeekConfidence::TooFew);
        assert!(report.wn8.is_none());
        assert!(report.goals.is_empty());
    }

    #[test]
    fn session_story_map_record_and_session_maps_read_the_database() {
        let folder = tempdir().unwrap();
        let database = Database::initialize(folder.path().join("story.db")).unwrap();
        let mut first = crate::storage::tests::full_replay_battle("synthetic-story-1");
        first.map_key = Some("01_synth".into());
        first.damage = Some(4_000);
        first.outcome = models::BattleOutcome::Victory;
        let mut second = crate::storage::tests::full_replay_battle("synthetic-story-2");
        second.map_key = Some("01_synth".into());
        second.damage = Some(2_000);
        second.battle_time = first.battle_time + 60_000;
        second.outcome = models::BattleOutcome::Defeat;
        database.insert_battle(&mut first, true).unwrap();
        database.insert_battle(&mut second, true).unwrap();
        let session = database.active_session().unwrap().unwrap();
        let story = session_story_view(&database, &session.id, None).unwrap();
        assert_eq!(story.headline.battles, 2);
        assert_eq!(story.best_battle.unwrap().damage, 4_000);
        let record = map_record_view(&database, "01_synth").unwrap();
        assert_eq!(
            (record.battles, record.wins, record.average_damage),
            (2, 1, Some(3_000.0))
        );
        for bad in ["01-SYNTH", "", "../x"] {
            assert_eq!(map_record_view(&database, bad).unwrap().battles, 0, "{bad}");
        }
        assert!(valid_map_key(&"a".repeat(128)) && !valid_map_key(&"a".repeat(129)));
        let maps = session_maps_view(&database, vec![session.id.clone(); 60]).unwrap();
        assert_eq!(maps.len(), session_story::MAX_SESSION_IDS);
        assert_eq!(maps[0].map_keys, ["01_synth"]);
    }

    #[test]
    fn vehicle_names_without_a_game_folder_or_with_an_unknown_language_are_empty() {
        let names = vehicle_names::VehicleNames::default();
        let folder = tempfile::tempdir().unwrap();
        let empty = vehicle_names::VehicleNamesView::default();
        assert_eq!(vehicle_names_view(&names, None, "pl"), empty);
        assert_eq!(vehicle_names_view(&names, Some(folder.path()), "de"), empty);
        assert_eq!(vehicle_names_view(&names, Some(folder.path()), "pl"), empty);
    }

    #[test]
    fn game_image_without_a_game_folder_or_with_an_unknown_kind_is_empty() {
        let images = game_images::GameImages::default();
        let folder = tempfile::tempdir().unwrap();
        assert!(game_image_bytes(&images, None, "vehicle", "a-b", (0, 0)).is_empty());
        assert!(game_image_bytes(&images, Some(folder.path()), "tank", "a-b", (0, 0)).is_empty());
        assert!(
            game_image_bytes(&images, Some(folder.path()), "vehicle", "a-b", (0, 0)).is_empty()
        );
        assert!(game_image_bytes(&images, Some(folder.path()), "map", "a-b", (64, 32)).is_empty());
    }

    #[test]
    fn delete_all_data_is_refused_in_recovery_mode_and_real_file_is_untouched() {
        let data = corrupt_database_dir();
        let before = fs::read(data.path().join(DATABASE_FILE)).unwrap();
        let startup = open_startup_database(data.path()).unwrap();
        let refused = ensure_delete_all_data_allowed(startup.startup_error_code.as_deref());
        assert!(matches!(refused, Err(AppError::RecoveryModeDeleteRefused)));
        assert_eq!(
            CommandError::from(refused.unwrap_err()).code,
            "recovery_mode_delete_refused"
        );
        assert_eq!(fs::read(data.path().join(DATABASE_FILE)).unwrap(), before);
        assert!(ensure_delete_all_data_allowed(None).is_ok());
    }

    #[test]
    fn expected_runtime_loads_local_snapshot_before_any_update() {
        let directory = tempdir().unwrap();
        activate_expected(directory.path(), "2026-07-10", 1_000);
        let runtime = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        let status = runtime.status();
        assert_eq!(status.availability, ExpectedValuesAvailability::Available);
        assert_eq!(status.source.as_deref(), Some("xvm_wargaming"));
        assert_eq!(status.source_version.as_deref(), Some("2026-07-10"));
        assert_eq!(status.vehicle_count, Some(500));
        assert!(!status.update_in_progress);
    }

    #[test]
    fn expected_runtime_without_snapshot_is_immediately_unavailable() {
        let directory = tempdir().unwrap();
        let runtime = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        let status = runtime.status();
        assert_eq!(status.availability, ExpectedValuesAvailability::Unavailable);
        assert_eq!(status.sha256, None);
        assert!(!status.update_in_progress);
    }

    #[test]
    fn automatic_activated_reloads_snapshot_and_unchanged_preserves_it() {
        let directory = tempdir().unwrap();
        activate_expected(directory.path(), "2026-07-10", 1_000);
        let runtime = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        let activated = tauri::async_runtime::block_on(run_expected_values_update_with(
            &runtime,
            ExpectedUpdateMode::Automatic,
            |cache_root, _| {
                activate_expected(&cache_root, "2026-07-11", 2_000);
                ready(Ok(ExpectedUpdateOutcome::Activated))
            },
        ))
        .unwrap();
        assert_eq!(activated.outcome, ExpectedValuesUpdateOutcome::Activated);
        assert_eq!(
            activated.status.source_version.as_deref(),
            Some("2026-07-11")
        );
        let sha = activated.status.sha256.clone();

        let unchanged = tauri::async_runtime::block_on(run_expected_values_update_with(
            &runtime,
            ExpectedUpdateMode::Automatic,
            |_, _| ready(Ok(ExpectedUpdateOutcome::Unchanged)),
        ))
        .unwrap();
        assert_eq!(unchanged.outcome, ExpectedValuesUpdateOutcome::Unchanged);
        assert_eq!(unchanged.status.sha256, sha);
        assert_eq!(
            unchanged.status.availability,
            ExpectedValuesAvailability::Available
        );
    }

    #[test]
    fn not_due_and_offline_outcomes_preserve_current_snapshot() {
        let directory = tempdir().unwrap();
        activate_expected(directory.path(), "2026-07-10", 1_000);
        let runtime = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        let original_sha = runtime.status().sha256;

        for outcome in [
            ExpectedUpdateOutcome::NotDue,
            ExpectedUpdateOutcome::OfflineUsingCurrent,
            ExpectedUpdateOutcome::RejectedUsingCurrent,
        ] {
            let result = tauri::async_runtime::block_on(run_expected_values_update_with(
                &runtime,
                ExpectedUpdateMode::Automatic,
                |_, _| ready(Ok(outcome)),
            ))
            .unwrap();
            assert_eq!(result.status.sha256, original_sha);
            assert_eq!(
                result.status.availability,
                ExpectedValuesAvailability::Available
            );
        }
    }

    #[test]
    fn offline_without_snapshot_remains_unavailable() {
        let directory = tempdir().unwrap();
        let runtime = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        let result = tauri::async_runtime::block_on(run_expected_values_update_with(
            &runtime,
            ExpectedUpdateMode::Automatic,
            |_, _| ready(Ok(ExpectedUpdateOutcome::Unavailable)),
        ))
        .unwrap();
        assert_eq!(
            result.status.availability,
            ExpectedValuesAvailability::Unavailable
        );
        assert_eq!(result.status.vehicle_count, None);
    }

    #[test]
    fn manual_force_uses_runtime_cache_root_only() {
        let directory = tempdir().unwrap();
        let expected_root = directory.path().to_path_buf();
        let runtime = ExpectedValuesRuntime::load(expected_root.clone());
        let result = tauri::async_runtime::block_on(run_expected_values_update_with(
            &runtime,
            ExpectedUpdateMode::ManualForce,
            |actual_root, mode| {
                assert_eq!(actual_root, expected_root);
                assert_eq!(mode, ExpectedUpdateMode::ManualForce);
                ready(Ok(ExpectedUpdateOutcome::Unavailable))
            },
        ))
        .unwrap();
        assert_eq!(result.outcome, ExpectedValuesUpdateOutcome::Unavailable);
    }

    #[test]
    fn busy_refresh_does_not_clear_another_updates_progress() {
        let directory = tempdir().unwrap();
        let runtime = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        let first_progress = runtime.begin_update();
        assert!(runtime.status().update_in_progress);

        let busy = tauri::async_runtime::block_on(run_expected_values_update_with(
            &runtime,
            ExpectedUpdateMode::ManualForce,
            |_, _| ready(Ok(ExpectedUpdateOutcome::Busy)),
        ))
        .unwrap();
        assert_eq!(busy.outcome, ExpectedValuesUpdateOutcome::Busy);
        assert!(busy.status.update_in_progress);
        drop(first_progress);
        assert!(!runtime.status().update_in_progress);
    }

    #[test]
    fn update_progress_resets_after_success_and_error() {
        let directory = tempdir().unwrap();
        let runtime = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        let success = tauri::async_runtime::block_on(run_expected_values_update_with(
            &runtime,
            ExpectedUpdateMode::Automatic,
            |_, _| ready(Ok(ExpectedUpdateOutcome::NotDue)),
        ));
        assert!(success.is_ok());
        assert!(!runtime.status().update_in_progress);

        let failure = tauri::async_runtime::block_on(run_expected_values_update_with(
            &runtime,
            ExpectedUpdateMode::Automatic,
            |_, _| ready(Err(ExpectedValuesError::InvalidCacheEntry)),
        ));
        assert!(failure.is_err());
        assert!(!runtime.status().update_in_progress);
    }

    #[test]
    fn update_progress_resets_during_panic_unwind() {
        let directory = tempdir().unwrap();
        let runtime = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tauri::async_runtime::block_on(run_expected_values_update_with(
                &runtime,
                ExpectedUpdateMode::Automatic,
                |_, _| -> std::future::Ready<Result<ExpectedUpdateOutcome, ExpectedValuesError>> {
                    panic!("synthetic updater panic")
                },
            ))
        }));
        assert!(panic.is_err());
        assert!(!runtime.status().update_in_progress);
    }

    #[test]
    fn expected_status_is_safe_and_restart_loads_same_local_snapshot() {
        let directory = tempdir().unwrap();
        activate_expected(directory.path(), "2026-07-10", 1_000);
        let first = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        first.data.write().unwrap().last_error_code = Some("offline".to_owned());
        let serialized = serde_json::to_string(&first.status()).unwrap();
        assert!(!serialized.contains(&directory.path().to_string_lossy().to_string()));
        assert!(!serialized.contains("expected-values"));
        assert!(serialized.contains("offline"));

        let restarted = ExpectedValuesRuntime::load(directory.path().to_path_buf());
        assert_eq!(first.status().sha256, restarted.status().sha256);
        assert_eq!(
            restarted.status().availability,
            ExpectedValuesAvailability::Available
        );
    }

    #[test]
    fn presentation_uses_engine_for_session_and_battles_and_preserves_visible_order() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("presentation.db")).unwrap();
        database.start_session().unwrap();
        let now = Utc::now().timestamp_millis();
        for (id, time, vehicle) in [
            ("first", now, 1),
            ("second", now + 1, 2),
            ("third", now + 2, 3),
        ] {
            let mut battle = presentation_battle(id, time, vehicle, DataQuality::Full);
            database.insert_battle(&mut battle, true).unwrap();
        }
        let session = database.active_session().unwrap().unwrap();
        let cache = tempdir().unwrap();
        activate_expected(cache.path(), "2026-07-10", 4_000);
        let runtime = ExpectedValuesRuntime::load(cache.path().to_path_buf());
        let (expected, status) = runtime.snapshot_and_status();
        assert!(runtime.data.try_write().is_ok(), "runtime read lock leaked");
        let visible = database.recent_battles(30).unwrap();
        let before = database.session_battles(&session.id).unwrap();
        let presentation = wn8_presentation(Some(&before), &visible, expected.as_ref(), status);

        let engine = calculate_wn8(&before, expected.as_ref().unwrap());
        let Wn8Calculation::Available(engine) = engine else {
            panic!("expected available WN8: {engine:?}")
        };
        let shown = presentation.session.unwrap();
        assert_eq!(shown.state, Wn8PresentationState::Available, "{shown:?}");
        assert_eq!(shown.value, Some(engine.value));
        assert_eq!(shown.eligible_battles, 3);
        assert_eq!(shown.total_battles, 3);
        assert_eq!(shown.formula_version, WN8_FORMULA_VERSION);
        assert_eq!(shown.snapshot_source_version.as_deref(), Some("2026-07-10"));
        assert_eq!(
            presentation
                .battles
                .iter()
                .map(|battle| battle.battle_id.as_str())
                .collect::<Vec<_>>(),
            vec!["third", "second", "first"]
        );
        for (shown, battle) in presentation.battles.iter().zip(visible.iter()) {
            let Wn8Calculation::Available(calculated) =
                calculate_battle_wn8(battle, expected.as_ref().unwrap())
            else {
                panic!("expected battle WN8")
            };
            assert_eq!(shown.value, Some(calculated.value));
        }
        assert_eq!(
            database.session_battles(&session.id).unwrap(),
            before,
            "presentation wrote SQLite"
        );
    }

    #[test]
    fn presentation_reports_partial_coverage_and_safe_battle_reasons() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("coverage.db")).unwrap();
        database.start_session().unwrap();
        let now = Utc::now().timestamp_millis();
        let mut full = presentation_battle("full", now, 1, DataQuality::Full);
        let mut partial = presentation_battle("partial", now + 1, 2, DataQuality::Partial);
        let mut unknown = presentation_battle("unknown", now + 2, 9_999, DataQuality::Full);
        database.insert_battle(&mut full, true).unwrap();
        database.insert_battle(&mut partial, true).unwrap();
        database.insert_battle(&mut unknown, true).unwrap();
        let session = database.active_session().unwrap().unwrap();
        let cache = tempdir().unwrap();
        activate_expected(cache.path(), "2026-07-10", 4_000);
        let runtime = ExpectedValuesRuntime::load(cache.path().to_path_buf());
        let (expected, status) = runtime.snapshot_and_status();
        let visible = database.recent_battles(30).unwrap();
        let active_battles = database.session_battles(&session.id).unwrap();
        let presentation =
            wn8_presentation(Some(&active_battles), &visible, expected.as_ref(), status);
        let shown = presentation.session.unwrap();
        assert_eq!(shown.state, Wn8PresentationState::Available);
        assert_eq!(shown.eligible_battles, 1);
        assert_eq!(shown.total_battles, 3);
        assert_eq!(shown.coverage_ratio, 1.0 / 3.0);
        assert_eq!(shown.exclusions.partial_or_incomplete, 1);
        assert_eq!(shown.exclusions.vehicle_not_in_expected_values, 1);
        assert_eq!(
            presentation.battles[0].exclusion_reason,
            Some(Wn8UnavailableReason::VehicleNotInExpectedValues)
        );
        assert_eq!(
            presentation.battles[1].exclusion_reason,
            Some(Wn8UnavailableReason::PartialOrIncomplete)
        );
        assert_eq!(
            presentation.battles[2].state,
            Wn8PresentationState::Available
        );
    }

    #[test]
    fn presentation_without_snapshot_or_eligible_battles_is_explicitly_unavailable() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("unavailable.db")).unwrap();
        database.start_session().unwrap();
        let mut partial = presentation_battle(
            "partial",
            Utc::now().timestamp_millis(),
            1,
            DataQuality::Partial,
        );
        database.insert_battle(&mut partial, true).unwrap();
        let session = database.active_session().unwrap().unwrap();
        let runtime = ExpectedValuesRuntime::load(directory.path().join("empty-cache"));
        let (none, status) = runtime.snapshot_and_status();
        let visible = database.recent_battles(30).unwrap();
        let active_battles = database.session_battles(&session.id).unwrap();
        let without_snapshot =
            wn8_presentation(Some(&active_battles), &visible, none.as_ref(), status);
        assert_eq!(without_snapshot.session.unwrap().value, None);
        assert_eq!(
            without_snapshot.battles[0].exclusion_reason,
            Some(Wn8UnavailableReason::ExpectedValuesUnavailable)
        );

        let cache = tempdir().unwrap();
        activate_expected(cache.path(), "2026-07-10", 4_000);
        let runtime = ExpectedValuesRuntime::load(cache.path().to_path_buf());
        let (expected, status) = runtime.snapshot_and_status();
        let no_eligible =
            wn8_presentation(Some(&active_battles), &visible, expected.as_ref(), status);
        let shown = no_eligible.session.unwrap();
        assert_eq!(shown.state, Wn8PresentationState::Unavailable);
        assert_eq!(shown.value, None);
        assert_eq!(
            shown.unavailable_reason,
            Some(Wn8UnavailableReason::NoEligibleBattles)
        );
    }

    #[test]
    fn historical_sessions_are_grouped_in_one_bulk_read_and_keep_session_order() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("history.db")).unwrap();
        let now = Utc::now().timestamp_millis();
        database.start_session().unwrap();
        for (id, offset, quality) in [
            ("old-full-a", 0, DataQuality::Full),
            ("old-full-b", 1, DataQuality::Full),
            ("old-partial", 2, DataQuality::Partial),
        ] {
            let mut battle = presentation_battle(id, now + offset, 1, quality);
            database.insert_battle(&mut battle, true).unwrap();
        }
        let older = database.end_active_session("ended").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        database.start_session().unwrap();
        let mut newer_battle = presentation_battle("new-full", now + 3, 2, DataQuality::Full);
        database.insert_battle(&mut newer_battle, true).unwrap();
        let newer = database.end_active_session("ended").unwrap();

        let sessions = database.recent_sessions(30).unwrap();
        assert_eq!(
            sessions
                .iter()
                .map(|session| &session.id)
                .collect::<Vec<_>>(),
            vec![&newer.id, &older.id]
        );
        let session_ids = sessions
            .iter()
            .map(|session| session.id.clone())
            .collect::<Vec<_>>();
        let all_history_battles = database.battles_for_sessions(&session_ids).unwrap();
        assert_eq!(all_history_battles.len(), 4);
        let cache = tempdir().unwrap();
        activate_expected(cache.path(), "2026-07-10", now);
        let runtime = ExpectedValuesRuntime::load(cache.path().to_path_buf());
        let (expected, status) = runtime.snapshot_and_status();
        let _ = status;
        let historical_sessions = sessions_wn8(&sessions, all_history_battles, expected.as_ref());
        assert_eq!(
            historical_sessions
                .iter()
                .map(|session| &session.session_id)
                .collect::<Vec<_>>(),
            vec![&newer.id, &older.id]
        );
        assert_eq!(historical_sessions[0].wn8.eligible_battles, 1);
        assert_eq!(historical_sessions[0].wn8.total_battles, 1);
        assert_eq!(historical_sessions[1].wn8.eligible_battles, 2);
        assert_eq!(historical_sessions[1].wn8.total_battles, 3);
        assert_eq!(historical_sessions[1].wn8.coverage_ratio, 2.0 / 3.0);

        let older_battles = database.session_battles(&older.id).unwrap();
        let Wn8Calculation::Available(expected_older) =
            calculate_wn8(&older_battles, expected.as_ref().unwrap())
        else {
            panic!("historical WN8 should be available")
        };
        assert_eq!(historical_sessions[1].wn8.value, Some(expected_older.value));
        let modal = summary(&database, older.clone(), expected.as_ref()).unwrap();
        assert_eq!(modal.wn8.value, Some(expected_older.value));
        assert_eq!(modal.wn8.eligible_battles, 2);
        assert_eq!(modal.wn8.total_battles, 3);
        assert_eq!(
            modal
                .battles
                .iter()
                .map(|view| view.battle.arena_id.as_str())
                .collect::<Vec<_>>(),
            vec!["old-full-a", "old-full-b", "old-partial"]
        );
        assert_eq!(
            modal
                .battle_wn8
                .iter()
                .map(|battle| battle.battle_id.as_str())
                .collect::<Vec<_>>(),
            vec!["old-full-a", "old-full-b", "old-partial"]
        );
        assert!(
            modal
                .battles
                .iter()
                .all(|view| view.battle.session_id.as_deref() == Some(older.id.as_str()))
        );
        assert!(
            !modal
                .battles
                .iter()
                .any(|view| view.battle.arena_id == "new-full")
        );
    }

    #[test]
    fn history_without_snapshot_and_empty_active_session_are_na_not_zero() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("history-na.db")).unwrap();
        let now = Utc::now().timestamp_millis();
        database.start_session().unwrap();
        let mut battle = presentation_battle("history-full", now, 1, DataQuality::Full);
        database.insert_battle(&mut battle, true).unwrap();
        database.end_active_session("ended").unwrap();
        database.start_session().unwrap();
        let active = database.active_session().unwrap().unwrap();
        let active_battles = database.session_battles(&active.id).unwrap();
        let sessions = database.recent_sessions(30).unwrap();
        let history = database
            .battles_for_sessions(
                &sessions
                    .iter()
                    .map(|session| session.id.clone())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let runtime = ExpectedValuesRuntime::load(directory.path().join("empty-cache"));
        let (expected, status) = runtime.snapshot_and_status();
        let presentation = wn8_presentation(Some(&active_battles), &[], expected.as_ref(), status);
        let historical_sessions = sessions_wn8(&sessions, history, expected.as_ref());
        let active_wn8 = presentation.session.unwrap();
        assert_eq!(active_wn8.total_battles, 0);
        assert_eq!(active_wn8.value, None);
        assert_eq!(
            active_wn8.unavailable_reason,
            Some(Wn8UnavailableReason::ExpectedValuesUnavailable)
        );
        assert!(
            historical_sessions
                .iter()
                .all(|session| session.wn8.value.is_none())
        );
    }

    #[test]
    fn snapshot_history_and_summary_reads_preserve_logical_sqlite_content() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("logical-read.db");
        let database = Database::initialize(path.clone()).unwrap();
        let now = Utc::now().timestamp_millis();
        database.start_session().unwrap();
        let mut battle = presentation_battle("logical-full", now, 1, DataQuality::Full);
        database.insert_battle(&mut battle, true).unwrap();
        let ended = database.end_active_session("ended").unwrap();
        database.start_session().unwrap();
        let cache = tempdir().unwrap();
        activate_expected(cache.path(), "2026-07-10", now);
        let runtime = ExpectedValuesRuntime::load(cache.path().to_path_buf());
        let (expected, status) = runtime.snapshot_and_status();
        let before = logical_database_digest(&path);

        let active = database.active_session().unwrap();
        let active_battles = active
            .as_ref()
            .map(|session| database.session_battles(&session.id))
            .transpose()
            .unwrap();
        let recent_battles = database.recent_battles(30).unwrap();
        for battle in recent_battles.iter().cloned() {
            view(&database, battle).unwrap();
        }
        let sessions = database.recent_sessions(30).unwrap();
        let history = database
            .battles_for_sessions(
                &sessions
                    .iter()
                    .map(|session| session.id.clone())
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let _snapshot_presentation = wn8_presentation(
            active_battles.as_deref(),
            &recent_battles,
            expected.as_ref(),
            status,
        );
        let _sessions_wn8 = sessions_wn8(&sessions, history, expected.as_ref());
        let _modal = summary(&database, ended, expected.as_ref()).unwrap();

        assert_eq!(logical_database_digest(&path), before);
    }

    /// E3 checkpoint: a command waiting on SQLite or files holds a blocking-pool thread, not the
    /// async runtime, so another command still answers (the window stays responsive).
    #[test]
    fn blocking_command_work_does_not_stall_other_commands() {
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let slow = tauri::async_runtime::spawn(off_thread(move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(1)
        }));
        started_rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .unwrap();
        let quick = tauri::async_runtime::block_on(off_thread(|| Ok(2)));
        assert_eq!(quick.unwrap(), 2);
        release_tx.send(()).unwrap();
        assert_eq!(tauri::async_runtime::block_on(slow).unwrap().unwrap(), 1);
    }

    #[test]
    fn a_panicking_command_job_becomes_an_error() {
        let result = tauri::async_runtime::block_on(off_thread::<(), _>(|| panic!("boom")));
        assert_eq!(
            result.unwrap_err().code,
            CommandError::from(AppError::Tracker).code
        );
    }

    #[test]
    fn events_have_the_names_and_payloads_of_spec_20() {
        use events::AppEvent;
        let progress = AppEvent::ImportProgress {
            done: 3,
            total: 7,
            finished: false,
            cancelled: false,
        };
        assert_eq!(progress.name(), "import-progress");
        assert_eq!(
            progress.payload(),
            json!({"done": 3, "total": 7, "finished": false, "cancelled": false})
        );
        assert_eq!(AppEvent::DataChanged.name(), "data-changed");
        assert_eq!(AppEvent::DataChanged.payload(), json!({}));
        let ended = AppEvent::SessionEnded {
            session_id: "s".to_owned(),
        };
        assert_eq!(ended.name(), "session-ended");
        assert_eq!(ended.payload(), json!({"sessionId": "s"}));
    }

    fn attempts_conversion_battle(id: &str, battle_time: i64, quality: DataQuality) -> Battle {
        let mut battle = presentation_battle(id, battle_time, 1, quality);
        battle.parser_version = Some(6);
        battle.feature_schema_version = Some(2);
        battle.gameplay_id = Some("ctf".to_owned());
        battle.shots = Some(10);
        battle.hits = Some(7);
        battle.direct_enemy_hits = Some(7);
        battle.direct_team_hits = Some(0);
        battle.penetrations = Some(5);
        battle.piercing_enemy_hits = Some(5);
        battle.explosion_hits = Some(0);
        battle.battle_pace.battle_type_raw = Some(1);
        battle.battle_pace.bonus_type_raw = Some(1);
        battle.battle_pace.initial_team1_size = Some(15);
        battle.battle_pace.initial_team2_size = Some(15);
        battle
    }

    #[test]
    fn attempts_conversion_session_command_path_is_read_only_and_safe_without_catalog() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("attempts-session-command.db");
        let database = Database::initialize(path.clone()).unwrap();
        let session = database.start_session().unwrap();
        let now = Utc::now().timestamp_millis();
        let mut full = attempts_conversion_battle("attempts-full", now, DataQuality::Full);
        database.insert_battle(&mut full, true).unwrap();
        let mut partial =
            attempts_conversion_battle("attempts-partial", now + 1, DataQuality::Partial);
        database.insert_battle(&mut partial, true).unwrap();
        let before = logical_database_digest(&path);

        let first = attempts_conversion_session_report(&database, None, &session.id).unwrap();
        let second = attempts_conversion_session_report(&database, None, &session.id).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.selected_battles, 2);
        assert_eq!(first.common_context.eligible_battles, 0);
        assert_eq!(first.common_context.ineligible_battles, 2);
        assert_eq!(first.hit_rate.value, None);
        assert!(matches!(
            first.scope,
            AttemptsConversionScopeDto::Session { ref session_id } if session_id == &session.id
        ));
        let json = serde_json::to_value(&first).unwrap();
        assert_eq!(json["contractVersion"], "attempts-conversion-v2");
        assert_eq!(json["scope"]["type"], "session");
        assert_eq!(json["scope"]["sessionId"], session.id);
        assert!(json["scope"].get("session_id").is_none());
        assert_eq!(
            json["commonContext"],
            serde_json::to_value(&first.common_context).unwrap()
        );
        assert_eq!(
            json["hitRate"],
            serde_json::to_value(&first.hit_rate).unwrap()
        );
        assert_eq!(
            json["penetrationRate"],
            serde_json::to_value(&first.penetration_rate).unwrap()
        );
        assert_eq!(json["facts"], serde_json::to_value(&first.facts).unwrap());
        assert_eq!(
            first
                .common_context
                .rejection_reasons
                .iter()
                .find(|reason| reason.code == "partial_replay")
                .unwrap()
                .count,
            1
        );
        assert_eq!(
            first
                .common_context
                .rejection_reasons
                .iter()
                .find(|reason| reason.code == "vehicle_class_catalog_unavailable")
                .unwrap()
                .count,
            1
        );
        assert_eq!(logical_database_digest(&path), before);
    }

    #[test]
    fn attempts_conversion_recent_command_path_validates_windows_before_reading() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("attempts-recent-command.db");
        let database = Database::initialize(path.clone()).unwrap();
        for index in 0..12 {
            let mut battle = attempts_conversion_battle(
                &format!("attempts-recent-{index}"),
                1_000 - index,
                DataQuality::Full,
            );
            database.insert_battle(&mut battle, false).unwrap();
        }
        let before = logical_database_digest(&path);
        for window in [0, 1, 11, 99, 101, u32::MAX] {
            assert!(matches!(
                attempts_conversion_recent_report(&database, None, window),
                Err(AppError::InvalidAttemptsConversionWindow)
            ));
        }
        let recent = attempts_conversion_recent_report(&database, None, 10).unwrap();
        assert_eq!(recent.selected_battles, 10);
        assert!(matches!(
            recent.scope,
            AttemptsConversionScopeDto::Recent { window: 10 }
        ));
        assert_eq!(logical_database_digest(&path), before);
    }

    #[test]
    fn attempts_conversion_session_command_path_handles_empty_and_missing_sessions() {
        let directory = tempdir().unwrap();
        let database =
            Database::initialize(directory.path().join("attempts-empty-command.db")).unwrap();
        let session = database.start_session().unwrap();
        let empty = attempts_conversion_session_report(&database, None, &session.id).unwrap();
        assert_eq!(empty.selected_battles, 0);
        assert_eq!(empty.common_context.eligible_battles, 0);
        assert_eq!(empty.common_context.ineligible_battles, 0);
        assert_eq!(empty.facts.damage_dealt.value_sum, None);
        assert_eq!(empty.hit_rate.value, None);
        assert_eq!(empty.penetration_rate.value, None);
        assert!(
            empty
                .common_context
                .rejection_reasons
                .iter()
                .all(|reason| reason.count == 0)
        );
        assert!(matches!(
            attempts_conversion_session_report(&database, None, "missing-session"),
            Err(AppError::SessionNotFound)
        ));
        let invalid = CommandError::from(AppError::InvalidAttemptsConversionWindow);
        assert_eq!(invalid.code, "invalid_window");
        assert!(!invalid.message.contains(':'));
    }
}
