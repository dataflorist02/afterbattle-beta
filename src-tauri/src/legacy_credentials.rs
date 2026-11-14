//! One-time cleanup of credentials left by removed integrations (spec 20, D2).
//!
//! Before v2.0 the app could store a Tomato.gg profile and API key and a Wargaming standalone
//! `application_id` in Windows Credential Manager. Those integrations are gone; every start (and
//! "delete all data") removes their entries. A missing entry is not an error.

use crate::{error::AppError, storage::Database};

const TOMATO_PROFILE: (&str, &str) = ("app.afterbattle.desktop.tomato-profile", "configuration");
const TOMATO_KEY_SERVICE: &str = "app.afterbattle.desktop";
const WG_APPLICATION_ID: (&str, &str) = (
    "app.afterbattle.desktop.wargaming",
    "standalone-application-id",
);
/// Settings written by the old profile migration path.
const PROFILE_SETTINGS: [&str; 5] = [
    "profile_url",
    "nickname",
    "account_id",
    "region",
    "polling_seconds",
];

/// Minimal credential store interface, so tests never touch the real Credential Manager.
pub(crate) trait CredentialStore {
    fn read(&self, service: &str, user: &str) -> Result<Option<String>, AppError>;
    fn delete(&self, service: &str, user: &str) -> Result<(), AppError>;
}

pub(crate) struct KeyringStore;

impl CredentialStore for KeyringStore {
    fn read(&self, service: &str, user: &str) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(service, user).map_err(|_| AppError::Credential)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppError::Credential),
        }
    }

    fn delete(&self, service: &str, user: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(service, user).map_err(|_| AppError::Credential)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppError::Credential),
        }
    }
}

fn tomato_key_user(account_id: i64) -> String {
    format!("account-{account_id}")
}

/// Account ids are only needed to find the per-account Tomato.gg key entry; they are never
/// stored or returned.
fn legacy_account_ids(
    store: &dyn CredentialStore,
    database: &Database,
) -> Result<Vec<i64>, AppError> {
    let mut ids = Vec::new();
    if let Some(profile) = store.read(TOMATO_PROFILE.0, TOMATO_PROFILE.1)?
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&profile)
        && let Some(id) = ["accountId", "account_id"]
            .iter()
            .find_map(|key| value.get(*key).and_then(serde_json::Value::as_i64))
    {
        ids.push(id);
    }
    if let Some(id) = database
        .get_setting("account_id")?
        .and_then(|value| value.parse::<i64>().ok())
    {
        ids.push(id);
    }
    ids.retain(|id| *id > 0);
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

/// Removes every credential and setting of the removed Tomato.gg and Wargaming integrations.
pub(crate) fn purge_legacy_credentials(
    store: &dyn CredentialStore,
    database: &Database,
) -> Result<(), AppError> {
    for account_id in legacy_account_ids(store, database)? {
        store.delete(TOMATO_KEY_SERVICE, &tomato_key_user(account_id))?;
    }
    store.delete(TOMATO_PROFILE.0, TOMATO_PROFILE.1)?;
    store.delete(WG_APPLICATION_ID.0, WG_APPLICATION_ID.1)?;
    for key in PROFILE_SETTINGS {
        database.remove_setting(key)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
    };
    use tempfile::tempdir;

    #[derive(Default)]
    struct FakeStore {
        entries: RefCell<BTreeMap<(String, String), String>>,
        deletes: RefCell<Vec<(String, String)>>,
        fail_delete: bool,
    }

    impl FakeStore {
        fn with(entries: &[(&str, &str, &str)]) -> Self {
            let store = Self::default();
            for (service, user, value) in entries {
                store
                    .entries
                    .borrow_mut()
                    .insert(((*service).into(), (*user).into()), (*value).into());
            }
            store
        }
        fn keys(&self) -> BTreeSet<(String, String)> {
            self.entries.borrow().keys().cloned().collect()
        }
    }

    impl CredentialStore for FakeStore {
        fn read(&self, service: &str, user: &str) -> Result<Option<String>, AppError> {
            Ok(self
                .entries
                .borrow()
                .get(&(service.into(), user.into()))
                .cloned())
        }
        fn delete(&self, service: &str, user: &str) -> Result<(), AppError> {
            if self.fail_delete {
                return Err(AppError::Credential);
            }
            self.deletes
                .borrow_mut()
                .push((service.into(), user.into()));
            self.entries
                .borrow_mut()
                .remove(&(service.into(), user.into()));
            Ok(())
        }
    }

    fn database() -> (tempfile::TempDir, Database) {
        let directory = tempdir().unwrap();
        let database = Database::initialize(directory.path().join("db.sqlite3")).unwrap();
        (directory, database)
    }

    // Synthetic account ids only.
    const ACCOUNT: i64 = 900_000_001;

    #[test]
    fn removes_profile_tomato_key_and_wg_entries_and_keeps_unrelated_ones() {
        let (_directory, database) = database();
        let unrelated = (
            "app.afterbattle.desktop.local-source",
            "wot-root",
            "C:/Games/WoT",
        );
        let store = FakeStore::with(&[
            (
                TOMATO_PROFILE.0,
                TOMATO_PROFILE.1,
                &format!(r#"{{"accountId":{ACCOUNT},"nickname":"synthetic"}}"#),
            ),
            (
                TOMATO_KEY_SERVICE,
                &tomato_key_user(ACCOUNT),
                "tmgg_synthetic",
            ),
            (WG_APPLICATION_ID.0, WG_APPLICATION_ID.1, "0123456789abcdef"),
            unrelated,
        ]);
        purge_legacy_credentials(&store, &database).unwrap();
        assert_eq!(
            store.keys(),
            BTreeSet::from([(unrelated.0.to_owned(), unrelated.1.to_owned())])
        );
    }

    #[test]
    fn missing_entries_are_not_an_error() {
        let (_directory, database) = database();
        let store = FakeStore::default();
        purge_legacy_credentials(&store, &database).unwrap();
        purge_legacy_credentials(&store, &database).unwrap();
        // The profile and WG entries are always asked to be deleted; no account key is guessed.
        assert_eq!(store.deletes.borrow().len(), 4);
        assert!(
            store
                .deletes
                .borrow()
                .iter()
                .all(|(service, _)| service != TOMATO_KEY_SERVICE)
        );
    }

    #[test]
    fn account_id_from_old_settings_finds_the_key_and_profile_settings_are_removed() {
        let (_directory, database) = database();
        database
            .set_setting("account_id", &ACCOUNT.to_string())
            .unwrap();
        database.set_setting("nickname", "synthetic").unwrap();
        database
            .set_setting("profile_url", "https://example.invalid/p")
            .unwrap();
        database.set_setting("region", "EU").unwrap();
        database.set_setting("polling_seconds", "30").unwrap();
        database.set_setting("language", "pl").unwrap();
        let store = FakeStore::with(&[(TOMATO_KEY_SERVICE, &tomato_key_user(ACCOUNT), "tmgg_x")]);
        purge_legacy_credentials(&store, &database).unwrap();
        assert!(store.keys().is_empty());
        for key in PROFILE_SETTINGS {
            assert_eq!(database.get_setting(key).unwrap(), None, "{key}");
        }
        assert_eq!(
            database.get_setting("language").unwrap().as_deref(),
            Some("pl")
        );
    }

    #[test]
    fn snake_case_profile_and_invalid_ids_are_handled() {
        let (_directory, database) = database();
        database.set_setting("account_id", "not-a-number").unwrap();
        let store = FakeStore::with(&[
            (
                TOMATO_PROFILE.0,
                TOMATO_PROFILE.1,
                &format!(r#"{{"account_id":{ACCOUNT}}}"#),
            ),
            (TOMATO_KEY_SERVICE, &tomato_key_user(ACCOUNT), "tmgg_x"),
            (TOMATO_KEY_SERVICE, "account-0", "never-guessed"),
        ]);
        purge_legacy_credentials(&store, &database).unwrap();
        assert_eq!(
            store.keys(),
            BTreeSet::from([(TOMATO_KEY_SERVICE.to_owned(), "account-0".to_owned())])
        );
    }

    #[test]
    fn duplicate_account_ids_delete_the_key_once() {
        let (_directory, database) = database();
        database
            .set_setting("account_id", &ACCOUNT.to_string())
            .unwrap();
        let store = FakeStore::with(&[
            (
                TOMATO_PROFILE.0,
                TOMATO_PROFILE.1,
                &format!(r#"{{"accountId":{ACCOUNT}}}"#),
            ),
            (TOMATO_KEY_SERVICE, &tomato_key_user(ACCOUNT), "tmgg_x"),
        ]);
        purge_legacy_credentials(&store, &database).unwrap();
        let key_deletes = store
            .deletes
            .borrow()
            .iter()
            .filter(|(service, _)| service == TOMATO_KEY_SERVICE)
            .count();
        assert_eq!(key_deletes, 1);
    }

    #[test]
    fn credential_failure_is_reported() {
        let (_directory, database) = database();
        let store = FakeStore {
            fail_delete: true,
            ..FakeStore::default()
        };
        assert!(matches!(
            purge_legacy_credentials(&store, &database),
            Err(AppError::Credential)
        ));
    }
}
