//! Automatic updates (spec 32 §5): the player's consent to the update check and what the check
//! found. The check is a new network connection, so it runs only after an explicit "yes" (first
//! start, or once for players coming from a beta) or the switch in Settings; without consent no
//! request is made. Installing always needs the player's click.

use std::time::Duration;

use serde::Serialize;

use crate::error::AppError;
use crate::storage::Database;

/// The check asks for one small file; the plugin has no timeout of its own.
pub(crate) const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
/// Checking again and downloading the installer (about 10 MB) after the player's click.
pub(crate) const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);

/// Settings key of the consent: "on" or "off"; absent until the player answers.
pub(crate) const CONSENT_KEY: &str = "update_check";

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateConsent {
    /// Not answered yet: the app asks once and makes no request meanwhile.
    Unasked,
    On,
    Off,
}

/// What the check found: the version and its notes; nothing else leaves the update file.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub version: String,
    pub notes: Option<String>,
}

pub(crate) fn consent(database: &Database) -> Result<UpdateConsent, AppError> {
    Ok(match database.get_setting(CONSENT_KEY)?.as_deref() {
        Some("on") => UpdateConsent::On,
        Some("off") => UpdateConsent::Off,
        _ => UpdateConsent::Unasked,
    })
}

pub(crate) fn set_consent(database: &Database, enabled: bool) -> Result<UpdateConsent, AppError> {
    database.set_setting(CONSENT_KEY, if enabled { "on" } else { "off" })?;
    consent(database)
}

/// The gate before any update request: only an explicit "on" lets it through.
pub(crate) fn require_consent(database: &Database) -> Result<(), AppError> {
    match consent(database)? {
        UpdateConsent::On => Ok(()),
        UpdateConsent::Unasked | UpdateConsent::Off => Err(AppError::UpdateCheckOff),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn database() -> (tempfile::TempDir, Database) {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("updates.db")).unwrap();
        (directory, database)
    }

    #[test]
    fn consent_is_unasked_until_answered_and_then_kept() {
        let (_directory, database) = database();
        assert_eq!(consent(&database).unwrap(), UpdateConsent::Unasked);
        assert_eq!(set_consent(&database, true).unwrap(), UpdateConsent::On);
        assert_eq!(consent(&database).unwrap(), UpdateConsent::On);
        assert_eq!(set_consent(&database, false).unwrap(), UpdateConsent::Off);
        assert_eq!(consent(&database).unwrap(), UpdateConsent::Off);
    }

    #[test]
    fn no_request_without_an_explicit_yes() {
        let (_directory, database) = database();
        assert!(matches!(
            require_consent(&database),
            Err(AppError::UpdateCheckOff)
        ));
        set_consent(&database, false).unwrap();
        assert!(matches!(
            require_consent(&database),
            Err(AppError::UpdateCheckOff)
        ));
        database.set_setting(CONSENT_KEY, "yes please").unwrap();
        assert!(require_consent(&database).is_err());
        set_consent(&database, true).unwrap();
        assert!(require_consent(&database).is_ok());
    }

    /// Every row of every table, in a stable order, to compare a database before and after.
    fn dump(path: &std::path::Path) -> Vec<String> {
        let connection = rusqlite::Connection::open(path).unwrap();
        let tables: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let mut rows = Vec::new();
        for table in tables {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM \"{table}\" ORDER BY 1"))
                .unwrap();
            let columns = statement.column_count();
            let mut query = statement.query([]).unwrap();
            while let Some(row) = query.next().unwrap() {
                let values: Vec<String> = (0..columns)
                    .map(|index| match row.get_ref(index).unwrap() {
                        rusqlite::types::ValueRef::Text(text) => {
                            String::from_utf8_lossy(text).into_owned()
                        }
                        other => format!("{other:?}"),
                    })
                    .collect();
                rows.push(format!("{table}: {}", values.join(" | ")));
            }
        }
        rows
    }

    /// Spec 32 §5.2: an update does not touch the database beyond migrations. 1.0.0-beta.4
    /// shipped schema 10 with no migration steps; the RC opens such a database as it is (no
    /// migration, no backup, same rows) and asks about updates once, as for a new player.
    #[test]
    fn a_beta_4_database_opens_in_the_rc_unchanged_and_asks_about_updates_once() {
        assert_eq!(
            crate::schema::SCHEMA_VERSION,
            10,
            "a new schema step needs its own beta.4 → RC migration test"
        );
        assert!(crate::schema::MIGRATIONS.is_empty());
        let directory = tempdir().unwrap();
        let path = directory.path().join("afterbattle.sqlite3");
        {
            // The beta.4 player's database: settings answered there (language, address, XVM).
            let database = Database::initialize(path.clone()).unwrap();
            database.set_setting("language", "pl").unwrap();
            database.set_setting("recap_address", "feminine").unwrap();
            database.set_setting("xvm_enabled", "true").unwrap();
        }
        let before = dump(&path);
        let backups = || {
            std::fs::read_dir(directory.path())
                .unwrap()
                .filter(|entry| {
                    let name = entry.as_ref().unwrap().file_name();
                    name.to_string_lossy().ends_with(".bak")
                })
                .count()
        };
        assert_eq!(backups(), 0);
        let user_version = |path: &std::path::Path| -> i64 {
            rusqlite::Connection::open(path)
                .unwrap()
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap()
        };
        assert_eq!(user_version(&path), 10);

        let database = Database::initialize(path.clone()).unwrap();
        assert_eq!(consent(&database).unwrap(), UpdateConsent::Unasked);
        assert_eq!(user_version(&path), 10);
        assert_eq!(dump(&path), before, "reading the consent writes nothing");
        assert_eq!(backups(), 0, "no migration backup");

        // The answer adds its one setting and nothing else.
        set_consent(&database, false).unwrap();
        let after = dump(&path);
        let added: Vec<&String> = after.iter().filter(|row| !before.contains(row)).collect();
        assert_eq!(added.len(), 1);
        assert!(added[0].contains("update_check") && added[0].contains("off"));
        assert!(before.iter().all(|row| after.contains(row)));
    }
}
