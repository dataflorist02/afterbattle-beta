use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("application is not configured")]
    NotConfigured,
    #[error("an internal response could not be prepared")]
    InvalidApiResponse,
    #[error("no active session")]
    NoActiveSession,
    #[error("session was not found")]
    SessionNotFound,
    #[error("battle was not found")]
    BattleNotFound,
    #[error("database operation failed")]
    Database(#[from] rusqlite::Error),
    #[error("secure credential storage failed")]
    Credential,
    #[error("local filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("the selected directory is not a valid World of Tanks installation")]
    InvalidGameDirectory,
    #[error("the selected folder has no WorldOfTanks.exe")]
    GameExecutableMissing,
    #[error("the selected game folder has no replays folder")]
    ReplaysFolderMissing,
    #[error("the replays folder of the game has no battle replays")]
    ReplaysFolderEmpty,
    #[error("the game folder cannot be read with the current permissions")]
    GameFolderUnreadable,
    #[error("the replay is still being written")]
    ReplayIncomplete,
    #[error("the replay format is invalid or unsupported")]
    InvalidReplay,
    #[error("the replay exceeds the 64 MiB safety limit")]
    ReplayTooLarge,
    #[error("the local tracker is temporarily unavailable")]
    Tracker,
    #[error("the local database schema is unsupported or incomplete")]
    UnsupportedDatabaseSchema,
    #[error("the local database failed an integrity check")]
    DatabaseIntegrity,
    #[error("a vehicle challenge is already active")]
    ChallengeAlreadyActive,
    #[error("there is no active vehicle challenge")]
    NoActiveChallenge,
    #[error("the selected vehicle challenge suggestion is no longer current")]
    StaleChallengeSuggestion,
    #[error("the vehicle challenge data is invalid")]
    InvalidChallenge,
    #[error("attempts/conversion window must be 10, 30, or 100")]
    InvalidAttemptsConversionWindow,
    #[error("attempts/conversion report invariants were violated")]
    AttemptsConversionReportInvariant,
    #[error("network request failed")]
    Network(#[from] reqwest::Error),
    #[error("the local database is in recovery mode; deleting data is refused")]
    RecoveryModeDeleteRefused,
    #[error("the local database is in recovery mode; battles are not imported")]
    RecoveryModeTrackerDisabled,
    #[error("XVM expected values are disabled")]
    XvmDisabled,
    #[error("the local database was created by a newer version of the application")]
    DatabaseTooNew,
    #[error(
        "{missing} of {total} battles in the old database have no replay file; the database was not rebuilt"
    )]
    LegacyRebuildBlocked { missing: i64, total: i64 },
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<AppError> for CommandError {
    fn from(error: AppError) -> Self {
        let code = match error {
            AppError::NotConfigured => "not_configured",
            AppError::InvalidApiResponse => "invalid_api_response",
            AppError::NoActiveSession => "no_active_session",
            AppError::SessionNotFound => "session_not_found",
            AppError::BattleNotFound => "battle_not_found",
            AppError::Database(_) => "database_error",
            AppError::Credential => "credential_error",
            AppError::Io(_) => "filesystem_error",
            AppError::InvalidGameDirectory => "invalid_game_directory",
            AppError::GameExecutableMissing => "game_executable_missing",
            AppError::ReplaysFolderMissing => "replays_folder_missing",
            AppError::ReplaysFolderEmpty => "replays_folder_empty",
            AppError::GameFolderUnreadable => "game_folder_unreadable",
            AppError::ReplayIncomplete => "replay_incomplete",
            AppError::InvalidReplay => "invalid_replay",
            AppError::ReplayTooLarge => "replay_too_large",
            AppError::Tracker => "tracker_error",
            AppError::UnsupportedDatabaseSchema => "unsupported_database_schema",
            AppError::DatabaseIntegrity => "database_integrity_error",
            AppError::ChallengeAlreadyActive => "challenge_already_active",
            AppError::NoActiveChallenge => "no_active_challenge",
            AppError::StaleChallengeSuggestion => "stale_suggestion",
            AppError::InvalidChallenge => "invalid_challenge",
            AppError::InvalidAttemptsConversionWindow => "invalid_window",
            AppError::AttemptsConversionReportInvariant => "report_invariant_violation",
            AppError::Network(_) => "network_error",
            AppError::RecoveryModeDeleteRefused => "recovery_mode_delete_refused",
            AppError::RecoveryModeTrackerDisabled => "recovery_mode_tracker_disabled",
            AppError::XvmDisabled => "xvm_disabled",
            AppError::DatabaseTooNew => "database_too_new",
            AppError::LegacyRebuildBlocked { .. } => "legacy_rebuild_blocked",
        };

        Self {
            code,
            message: error.to_string(),
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
