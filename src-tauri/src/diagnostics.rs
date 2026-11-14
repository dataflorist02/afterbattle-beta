use crate::{
    error::AppError,
    models::{
        DiagnosticExpectedValuesStatus, DiagnosticReport, DiagnosticTrackerStatus,
        ExpectedValuesAvailability, ExpectedValuesStatus, ParserState, SourceStatus,
        VehicleClassCatalogStatus,
    },
    storage::Database,
};

pub const RELEASE_CHANNEL: &str = "public_beta";
pub const DIAGNOSTIC_SCHEMA_VERSION: u32 = 3;

fn parser_state(value: &ParserState) -> String {
    match value {
        ParserState::Ready => "ready",
        ParserState::Waiting => "waiting",
        ParserState::ReplayOnly => "replay_only",
        ParserState::Unsupported => "unsupported",
        ParserState::Error => "error",
    }
    .to_owned()
}

fn availability(value: &ExpectedValuesAvailability) -> String {
    match value {
        ExpectedValuesAvailability::Unavailable => "unavailable",
        ExpectedValuesAvailability::Available => "available",
        ExpectedValuesAvailability::Updating => "updating",
    }
    .to_owned()
}

fn client_region(display: Option<&str>) -> Option<String> {
    let normalized = display?.to_ascii_uppercase();
    ["EU", "NA", "ASIA", "CN"]
        .into_iter()
        .find(|region| normalized.ends_with(region))
        .map(str::to_owned)
}

fn safe_error_codes(
    source: &SourceStatus,
    expected: &ExpectedValuesStatus,
    vehicle_classes: &VehicleClassCatalogStatus,
) -> Vec<String> {
    let mut codes = Vec::new();
    if let Some(message) = source.message.as_deref()
        && matches!(
            message,
            "replay_parse_failed" | "unsupported_session_stats_format"
        )
    {
        codes.push(message.to_owned());
    }
    if let Some(code) = expected.last_error_code.as_deref()
        && matches!(
            code,
            "offline" | "http_status" | "too_large" | "invalid_dataset"
        )
    {
        codes.push(code.to_owned());
    }
    if let Some(code) = safe_vehicle_class_error(vehicle_classes.last_error_code.as_deref()) {
        codes.push(code);
    }
    codes.sort();
    codes.dedup();
    codes
}

fn safe_vehicle_class_error(code: Option<&str>) -> Option<String> {
    code.filter(|value| {
        matches!(
            *value,
            "no_client" | "invalid_source" | "invalid_catalog" | "io"
        )
    })
    .map(str::to_owned)
}

fn safe_expected_error(code: Option<&str>) -> Option<String> {
    code.filter(|value| {
        matches!(
            *value,
            "offline" | "http_status" | "too_large" | "invalid_dataset"
        )
    })
    .map(str::to_owned)
}

pub fn build_report(
    database: &Database,
    source: &SourceStatus,
    expected: &ExpectedValuesStatus,
    vehicle_classes: &VehicleClassCatalogStatus,
    challenge_last_error_code: Option<&str>,
    language: &str,
    generated_at: i64,
) -> Result<DiagnosticReport, AppError> {
    let safe_language = if language == "en" { "en" } else { "pl" };
    let challenges = database.challenge_diagnostic_state()?;
    Ok(DiagnosticReport {
        schema_version: DIAGNOSTIC_SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        release_channel: RELEASE_CHANNEL.to_owned(),
        generated_at,
        language: safe_language.to_owned(),
        operating_system: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
        tracker: DiagnosticTrackerStatus {
            configured: source.configured,
            active: source.active,
            paused: source.paused,
            parser_state: parser_state(&source.parser_state),
            client_version: source.client_version.clone(),
            partial_mode: source.partial_mode,
            client_region: client_region(source.game_root_display.as_deref()),
        },
        expected_values: DiagnosticExpectedValuesStatus {
            availability: availability(&expected.availability),
            source: expected.source.clone(),
            source_version: expected.source_version.clone(),
            sha256: expected.sha256.clone(),
            vehicle_count: expected.vehicle_count,
            skipped_row_count: expected.skipped_row_count,
            last_attempt_at: expected.last_attempt_at,
            last_success_at: expected.last_success_at,
            last_error_code: safe_expected_error(expected.last_error_code.as_deref()),
            update_in_progress: expected.update_in_progress,
        },
        vehicle_class_catalog_available: vehicle_classes.vehicle_count > 0,
        vehicle_class_catalog_schema_version:
            crate::vehicle_class_catalog::VEHICLE_CLASS_CATALOG_SCHEMA_VERSION,
        vehicle_class_resolver_version:
            crate::vehicle_class_catalog::VEHICLE_CLASS_RESOLVER_VERSION.to_owned(),
        vehicle_class_protocol_source: vehicle_classes.protocol_source,
        vehicle_class_catalog_vehicle_count: vehicle_classes.vehicle_count,
        vehicle_class_invalid_record_count: vehicle_classes.invalid_record_count,
        vehicle_class_unresolved_current_vehicle_count: vehicle_classes
            .unresolved_current_vehicle_count,
        vehicle_class_conflict_current_vehicle_count: vehicle_classes
            .conflict_current_vehicle_count,
        vehicle_class_update_in_progress: vehicle_classes.update_in_progress,
        vehicle_class_last_error_code: safe_vehicle_class_error(
            vehicle_classes.last_error_code.as_deref(),
        ),
        active_challenge: challenges.active_challenge,
        challenge_schema_version: crate::challenges::CHALLENGE_SCHEMA_VERSION,
        challenge_engine_version: crate::challenges::CHALLENGE_ENGINE_VERSION.to_owned(),
        active_challenge_type: challenges.active_challenge_type,
        active_challenge_vehicle_class: challenges.active_challenge_vehicle_class,
        active_challenge_completed_battles: challenges.active_challenge_completed_battles,
        active_challenge_target_battles: challenges.active_challenge_target_battles,
        active_challenge_successful_battles: challenges.active_challenge_successful_battles,
        active_challenge_required_successes: challenges.active_challenge_required_successes,
        challenge_counts_by_status: challenges.challenge_counts_by_status,
        challenge_last_error_code: challenge_last_error_code
            .filter(|code| {
                matches!(
                    *code,
                    "challenge_progress_failed"
                        | "challenge_catch_up_failed"
                        | "stale_suggestion"
                        | "challenge_already_active"
                )
            })
            .map(str::to_owned),
        database: database.diagnostics_read_only()?,
        recent_error_codes: safe_error_codes(source, expected, vehicle_classes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        CompactDescriptorProtocolSource, ExpectedValuesAvailability,
        VehicleClassCatalogAvailability,
    };
    use std::fs;
    use tempfile::tempdir;

    fn expected(last_error_code: Option<&str>) -> ExpectedValuesStatus {
        ExpectedValuesStatus {
            availability: ExpectedValuesAvailability::Unavailable,
            source: None,
            source_version: None,
            sha256: None,
            downloaded_at: None,
            vehicle_count: None,
            skipped_row_count: None,
            last_attempt_at: None,
            last_success_at: None,
            last_error_code: last_error_code.map(str::to_owned),
            update_in_progress: false,
        }
    }

    fn vehicle_classes(last_error_code: Option<&str>) -> VehicleClassCatalogStatus {
        VehicleClassCatalogStatus {
            availability: VehicleClassCatalogAvailability::Unavailable,
            source_version: None,
            vehicle_count: 0,
            invalid_record_count: 0,
            unresolved_current_vehicle_count: 0,
            conflict_current_vehicle_count: 0,
            last_attempt_at: None,
            last_success_at: None,
            last_error_code: last_error_code.map(str::to_owned),
            update_in_progress: false,
            protocol_source: CompactDescriptorProtocolSource::Unconfirmed,
            schema_version: None,
            vehicles_with_tier: 0,
            missing_nations: Vec::new(),
            unknown_nations: Vec::new(),
            stale_for_client_version: None,
        }
    }

    #[test]
    fn report_is_read_only_deterministic_and_omits_private_values() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("diagnostics.sqlite3");
        let database = Database::initialize(path.clone()).unwrap();
        database.set_setting("nickname", "SECRET_NICKNAME").unwrap();
        database.set_setting("account_id", "123456789").unwrap();
        database
            .set_setting("profile_url", "C:\\Users\\private\\profile")
            .unwrap();
        let source = SourceStatus {
            game_root_display: Some("C:\\Users\\private\\World_of_Tanks".into()),
            message: Some("C:\\Users\\private\\raw_error".into()),
            ..SourceStatus::default()
        };
        let expected = expected(Some("C:\\Users\\private\\network_error"));
        let before = fs::read(&path).unwrap();
        let classes = vehicle_classes(Some("C:\\Users\\private\\catalog_error"));
        let first = build_report(
            &database,
            &source,
            &expected,
            &classes,
            Some("C:\\Users\\private\\challenge_error"),
            "en",
            123,
        )
        .unwrap();
        let second = build_report(
            &database,
            &source,
            &expected,
            &classes,
            Some("C:\\Users\\private\\challenge_error"),
            "en",
            123,
        )
        .unwrap();
        let serialized = serde_json::to_string(&first).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.schema_version, DIAGNOSTIC_SCHEMA_VERSION);
        assert_eq!(first.database.schema_version, 10);
        assert_eq!(first.database.integrity_check, vec!["ok"]);
        assert_eq!(first.database.foreign_key_violation_count, 0);
        assert_eq!(first.database.battles, 0);
        assert_eq!(first.expected_values.last_error_code, None);
        assert_eq!(first.challenge_last_error_code, None);
        assert!(first.recent_error_codes.is_empty());
        for forbidden in [
            "SECRET_NICKNAME",
            "123456789",
            "profile",
            "C:\\\\Users",
            "raw_error",
            "network_error",
            "afterbattle.sqlite3",
            "wotreplay",
            "arenaUniqueID",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
        assert_eq!(fs::read(&path).unwrap(), before);
    }

    #[test]
    fn report_accepts_empty_database_and_only_whitelisted_error_codes() {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("empty.sqlite3")).unwrap();
        let source = SourceStatus {
            message: Some("replay_parse_failed".into()),
            ..SourceStatus::default()
        };
        let report = build_report(
            &database,
            &source,
            &expected(Some("offline")),
            &vehicle_classes(Some("no_client")),
            None,
            "xx",
            456,
        )
        .unwrap();
        assert_eq!(report.language, "pl");
        assert_eq!(
            report.expected_values.last_error_code.as_deref(),
            Some("offline")
        );
        assert_eq!(
            report.recent_error_codes,
            vec!["no_client", "offline", "replay_parse_failed"]
        );
        assert_eq!(report.database.sessions, 0);
        assert_eq!(report.database.replay_files, 0);
    }
}
