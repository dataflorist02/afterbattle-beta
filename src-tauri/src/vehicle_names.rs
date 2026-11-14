//! Vehicle names from the game's own translation files (E5-ui §2). The client keeps one gettext
//! catalog per nation, `res/text/lc_messages/<nation>_vehicles.mo`; `list.xml` names the keys
//! (`userString`, `shortUserString`, e.g. `#nation_vehicles:Tag`). The files are read on the fly,
//! parsed with limits checked before any allocation and kept only in RAM. Names never reach the
//! database or an export; the window asks for them and falls back to the readable code.

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::UNIX_EPOCH,
};

use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    local_tracker::canonicalize_vehicle_key,
    vehicle_catalog_probe::{NATION_KEYS, VehicleCatalogProbeEntry, probe_package},
};

/// Largest translation file accepted (real vehicle catalogs are far smaller).
pub(crate) const MAX_MO_BYTES: u64 = 4 * 1024 * 1024;
/// Upper bound on entries in one catalog, checked before the tables are read.
pub(crate) const MAX_MO_ENTRIES: u32 = 50_000;
/// Longest single string (key or translation) accepted.
pub(crate) const MAX_MO_STRING_BYTES: u32 = 8 * 1024;
/// Longest name shown; longer translations are cut at a character boundary.
const MAX_NAME_CHARS: usize = 80;
const MAGIC_LE: u32 = 0x9504_12de;
const MAGIC_BE: u32 = 0xde12_0495;
const HEADER_BYTES: usize = 28;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum MoError {
    #[error("translation file is larger than the limit")]
    TooLarge,
    #[error("translation file is truncated")]
    Truncated,
    #[error("translation file has an unknown signature")]
    BadMagic,
    #[error("translation file revision is unsupported")]
    UnsupportedRevision,
    #[error("translation file declares too many entries")]
    TooManyEntries,
    #[error("translation table lies outside the file")]
    TableOutOfBounds,
    #[error("translation string lies outside the file or exceeds the limit")]
    StringOutOfBounds,
}

/// One parsed gettext catalog: the `Language` of its header and `msgid → msgstr` for entries
/// that are valid UTF-8 and have a translation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct MoCatalog {
    pub language: Option<String>,
    pub entries: HashMap<String, String>,
    /// Entries left out because a key or translation is not valid UTF-8.
    pub invalid_entries: usize,
}

fn read_u32(bytes: &[u8], offset: usize, big_endian: bool) -> Result<u32, MoError> {
    let end = offset.checked_add(4).ok_or(MoError::Truncated)?;
    let chunk: [u8; 4] = bytes
        .get(offset..end)
        .ok_or(MoError::Truncated)?
        .try_into()
        .map_err(|_| MoError::Truncated)?;
    Ok(if big_endian {
        u32::from_be_bytes(chunk)
    } else {
        u32::from_le_bytes(chunk)
    })
}

/// The `(length, offset)` descriptor `index` of a table, its string checked against the file
/// and the string limit before it is sliced. The terminating NUL must be inside the file.
fn string_at(bytes: &[u8], table: usize, index: usize, big_endian: bool) -> Result<&[u8], MoError> {
    let descriptor = index
        .checked_mul(8)
        .and_then(|delta| table.checked_add(delta))
        .ok_or(MoError::TableOutOfBounds)?;
    let length = read_u32(bytes, descriptor, big_endian)?;
    let offset = read_u32(bytes, descriptor + 4, big_endian)?;
    if length > MAX_MO_STRING_BYTES {
        return Err(MoError::StringOutOfBounds);
    }
    let start = offset as usize;
    let end = start
        .checked_add(length as usize)
        .ok_or(MoError::StringOutOfBounds)?;
    if end >= bytes.len() || bytes[end] != 0 {
        return Err(MoError::StringOutOfBounds);
    }
    Ok(&bytes[start..end])
}

fn header_language(header: &str) -> Option<String> {
    header.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if !name.trim().eq_ignore_ascii_case("language") {
            return None;
        }
        let code = value
            .trim()
            .split(['_', '-', '.', '@'])
            .next()?
            .to_ascii_lowercase();
        ((2..=3).contains(&code.len()) && code.bytes().all(|byte| byte.is_ascii_lowercase()))
            .then_some(code)
    })
}

/// Parses a GNU gettext `.mo` file. Every size is checked before it is used: the file length,
/// the entry count, both tables and each string (with its NUL) must fit the file and the limits.
/// Plural and context forms keep their first part only (vehicle names have neither).
pub(crate) fn parse_mo(bytes: &[u8]) -> Result<MoCatalog, MoError> {
    if bytes.len() as u64 > MAX_MO_BYTES {
        return Err(MoError::TooLarge);
    }
    if bytes.len() < HEADER_BYTES {
        return Err(MoError::Truncated);
    }
    let big_endian = match read_u32(bytes, 0, false)? {
        MAGIC_LE => false,
        MAGIC_BE => true,
        _ => return Err(MoError::BadMagic),
    };
    if read_u32(bytes, 4, big_endian)? >> 16 > 1 {
        return Err(MoError::UnsupportedRevision);
    }
    let count = read_u32(bytes, 8, big_endian)?;
    if count > MAX_MO_ENTRIES {
        return Err(MoError::TooManyEntries);
    }
    let originals = read_u32(bytes, 12, big_endian)? as usize;
    let translations = read_u32(bytes, 16, big_endian)? as usize;
    let table_bytes = (count as usize) * 8;
    for table in [originals, translations] {
        let end = table
            .checked_add(table_bytes)
            .ok_or(MoError::TableOutOfBounds)?;
        if end > bytes.len() {
            return Err(MoError::TableOutOfBounds);
        }
    }
    let mut catalog = MoCatalog {
        entries: HashMap::with_capacity(count as usize),
        ..MoCatalog::default()
    };
    for index in 0..count as usize {
        let key = string_at(bytes, originals, index, big_endian)?;
        let value = string_at(bytes, translations, index, big_endian)?;
        let (Ok(key), Ok(value)) = (std::str::from_utf8(key), std::str::from_utf8(value)) else {
            catalog.invalid_entries += 1;
            continue;
        };
        if key.is_empty() {
            catalog.language = header_language(value);
            continue;
        }
        let key = key.split('\0').next().unwrap_or_default();
        let key = key.rsplit('\u{4}').next().unwrap_or_default();
        let value = value.split('\0').next().unwrap_or_default();
        if !key.is_empty() && !value.trim().is_empty() {
            catalog.entries.insert(key.to_owned(), value.to_owned());
        }
    }
    Ok(catalog)
}

/// Reads one translation file with the size limit checked on the metadata and on the read.
fn read_mo(path: &Path) -> Option<MoCatalog> {
    let file = File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_MO_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_MO_BYTES + 1).read_to_end(&mut bytes).ok()?;
    parse_mo(&bytes).ok()
}

/// `#file:id` → (`file`, `id`), with the file limited to `<nation>_vehicles`-like names.
fn translation_key(value: &str) -> Option<(&str, &str)> {
    let (file, id) = value.strip_prefix('#')?.split_once(':')?;
    let valid_file = (1..=64).contains(&file.len())
        && file
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    let valid_id = (1..=128).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-.".contains(&byte));
    (valid_file && valid_id).then_some((file, id))
}

fn shown(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && !value.chars().any(char::is_control))
        .then(|| value.chars().take(MAX_NAME_CHARS).collect())
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VehicleNameView {
    pub name: String,
    pub short_name: Option<String>,
}

/// Names for the window, keyed by the stored lowercase `vehicle_key` (`nation-tag`).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VehicleNamesView {
    /// Language declared by the game's translation files; `None` with names present means the
    /// language of the game client, not specified in the file ("język klienta, nieokreślony w
    /// pliku").
    pub language: Option<String>,
    pub names: BTreeMap<String, VehicleNameView>,
}

/// Everything read from one client: the parsed vehicle catalogs by file stem and the vehicle
/// records. Names are built per window language at request time (cheap: about 1 300 lookups).
#[derive(Debug, Default)]
struct NameTable {
    catalogs: HashMap<String, MoCatalog>,
    records: Vec<Record>,
}

/// (nation, tag, `userString`, `shortUserString`) of one vehicle from `list.xml`.
type Record = (String, String, Option<String>, Option<String>);

/// Most catalogs are `<nation>_vehicles.mo`; the UK nation uses the game's `gb_vehicles.mo`.
fn nation_catalog(nation: &str) -> String {
    match nation {
        "uk" => "gb_vehicles".to_owned(),
        other => format!("{other}_vehicles"),
    }
}

/// Stems of the vehicle catalogs a client may reference (`gb_vehicles`, `igr_vehicles`,
/// `multinational_vehicles`, ...). Event files with a dot in the name are not vehicle names.
fn catalog_stem(file_name: &str) -> Option<&str> {
    let stem = file_name.strip_suffix(".mo")?;
    (stem.ends_with("_vehicles")
        && (1..=64).contains(&stem.len())
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'))
    .then_some(stem)
}

/// Upper bound on vehicle catalogs read from one client (real: 14).
const MAX_CATALOG_FILES: usize = 64;

fn catalog_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut files = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    catalog_stem(&name).map(|stem| (stem.to_owned(), entry.path()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    files.sort();
    files.truncate(MAX_CATALOG_FILES);
    files
}

fn text_dir(game_root: &Path) -> Option<PathBuf> {
    let text = game_root.join("res").join("text");
    std::fs::read_dir(&text)
        .ok()?
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("lc_messages")
                && entry.file_type().is_ok_and(|kind| kind.is_dir())
        })
        .map(|entry| entry.path())
}

fn scripts_package(game_root: &Path) -> PathBuf {
    game_root.join("res").join("packages").join("scripts.pkg")
}

/// Size and modification time of every file the table depends on: a new client version or
/// language changes it and the table is rebuilt.
fn fingerprint(game_root: &Path) -> String {
    let mut hash = Sha256::new();
    let mut files = vec![scripts_package(game_root)];
    if let Some(dir) = text_dir(game_root) {
        files.extend(catalog_files(&dir).into_iter().map(|(_, path)| path));
    }
    for file in files {
        hash.update(file.to_string_lossy().as_bytes());
        if let Ok(metadata) = std::fs::metadata(&file) {
            hash.update(metadata.len().to_le_bytes());
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |time| time.as_millis());
            hash.update(modified.to_le_bytes());
        }
    }
    crate::hashing::hex_lower(hash.finalize())
}

/// Builds the table from already parsed catalogs (`<file stem>` → catalog) and the vehicle
/// records. Without a record the key is the tag itself (`Tag`, `Tag_short`).
fn build_names(
    catalogs: &HashMap<String, &MoCatalog>,
    records: &[Record],
) -> BTreeMap<String, VehicleNameView> {
    let lookup = |nation: &str, key: Option<&str>, fallback: String| -> Option<String> {
        let (file, id) = match key.and_then(translation_key) {
            Some((file, id)) => (file.to_owned(), id.to_owned()),
            None => (nation_catalog(nation), fallback),
        };
        catalogs
            .get(&file)?
            .entries
            .get(&id)
            .and_then(|value| shown(value))
    };
    let mut names = BTreeMap::new();
    for (nation, tag, user_string, short_user_string) in records {
        let Some(name) = lookup(nation, user_string.as_deref(), tag.clone()) else {
            continue;
        };
        let short_name = lookup(nation, short_user_string.as_deref(), format!("{tag}_short"))
            .filter(|short| short != &name);
        let Ok(key) = canonicalize_vehicle_key(&format!("{nation}-{tag}")) else {
            continue;
        };
        names.insert(
            key.as_str().to_owned(),
            VehicleNameView { name, short_name },
        );
    }
    names
}

/// Without the vehicle list (no `scripts.pkg`), every key of `<nation>_vehicles` that is not a
/// short name or a description stands for the tag of that nation.
fn records_from_catalogs(catalogs: &HashMap<String, MoCatalog>) -> Vec<Record> {
    let mut records = NATION_KEYS
        .iter()
        .filter_map(|nation| Some((*nation, catalogs.get(&nation_catalog(nation))?)))
        .flat_map(|(nation, catalog)| {
            catalog
                .entries
                .keys()
                .filter(|key| !key.ends_with("_short") && !key.ends_with("_descr"))
                .map(move |key| (nation.to_owned(), key.clone(), None, None))
        })
        .collect::<Vec<_>>();
    records.sort();
    records
}

fn load_table(game_root: &Path) -> NameTable {
    let Some(dir) = text_dir(game_root) else {
        return NameTable::default();
    };
    let catalogs = catalog_files(&dir)
        .into_iter()
        .filter_map(|(stem, path)| read_mo(&path).map(|catalog| (stem, catalog)))
        .collect::<HashMap<_, _>>();
    if catalogs.is_empty() {
        return NameTable::default();
    }
    let mut records = probe_package(&scripts_package(game_root))
        .map(|report| report.entries)
        .unwrap_or_default()
        .into_iter()
        .map(|entry: VehicleCatalogProbeEntry| {
            (
                entry.nation_key,
                entry.vehicle_key,
                entry.user_string,
                entry.short_user_string,
            )
        })
        .collect::<Vec<_>>();
    if records.is_empty() {
        records = records_from_catalogs(&catalogs);
    }
    NameTable { catalogs, records }
}

/// E5-ui §2: names follow the window language; otherwise English; otherwise none (the window
/// then shows the readable code). A catalog without a `Language` header is not assumed to match.
/// E5-ui §2 (decision of the author, 2026-11-14, variant A): a catalog that declares its
/// `Language` is used for the window language or English. A catalog without the field is in the
/// **language of the game client, not specified in the file** ("język klienta, nieokreślony w
/// pliku") and is used for both window languages: tank names rarely differ between them.
fn usable(catalog_language: Option<&str>, window_language: &str) -> bool {
    match catalog_language {
        None => true,
        Some(language) => language == window_language || language == "en",
    }
}

impl NameTable {
    fn view(&self, window_language: &str) -> VehicleNamesView {
        let used = self
            .catalogs
            .iter()
            .filter(|(_, catalog)| usable(catalog.language.as_deref(), window_language))
            .map(|(stem, catalog)| (stem.clone(), catalog))
            .collect::<HashMap<_, _>>();
        let names = build_names(&used, &self.records);
        if names.is_empty() {
            return VehicleNamesView::default();
        }
        VehicleNamesView {
            language: used.values().find_map(|catalog| catalog.language.clone()),
            names,
        }
    }
}

#[derive(Default)]
struct State {
    key: Option<(PathBuf, String)>,
    table: Arc<NameTable>,
}

/// Process-wide name store; cheap to clone. Holds at most one table, only in RAM.
#[derive(Clone, Default)]
pub(crate) struct VehicleNames {
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for VehicleNames {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("VehicleNames")
    }
}

impl VehicleNames {
    pub(crate) fn view(&self, game_root: &Path, window_language: &str) -> VehicleNamesView {
        let key = (game_root.to_path_buf(), fingerprint(game_root));
        let table = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if state.key.as_ref() != Some(&key) {
                state.table = Arc::new(load_table(game_root));
                state.key = Some(key);
            }
            state.table.clone()
        };
        table.view(window_language)
    }

    pub(crate) fn clear(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *state = State::default();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A little-endian `.mo` file from `(msgid, msgstr)` pairs, sorted as gettext writes them.
    pub(crate) fn mo_file(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut entries = entries.to_vec();
        entries.sort();
        let count = entries.len() as u32;
        let originals = HEADER_BYTES as u32;
        let translations = originals + count * 8;
        let mut strings_at = translations + count * 8;
        let mut tables = Vec::new();
        let mut strings = Vec::new();
        for column in [0, 1] {
            for (key, value) in &entries {
                let text = if column == 0 { key } else { value };
                tables.extend((text.len() as u32).to_le_bytes());
                tables.extend(strings_at.to_le_bytes());
                strings.extend(text.as_bytes());
                strings.push(0);
                strings_at += text.len() as u32 + 1;
            }
        }
        let mut bytes = Vec::new();
        for value in [MAGIC_LE, 0, count, originals, translations, 0, strings_at] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(tables);
        bytes.extend(strings);
        bytes
    }

    fn header(language: &str) -> String {
        format!("Content-Type: text/plain; charset=utf-8\nLanguage: {language}\n")
    }

    #[test]
    fn parses_keys_translations_and_the_header_language() {
        let bytes = mo_file(&[
            ("", &header("pl")),
            ("Synth_Alpha", "Synth Alpha Pełna"),
            ("Synth_Alpha_short", "Alpha"),
        ]);
        let catalog = parse_mo(&bytes).unwrap();
        assert_eq!(catalog.language.as_deref(), Some("pl"));
        assert_eq!(catalog.entries["Synth_Alpha"], "Synth Alpha Pełna");
        assert_eq!(catalog.entries["Synth_Alpha_short"], "Alpha");
        assert_eq!(catalog.entries.len(), 2);
    }

    #[test]
    fn reads_big_endian_files_and_language_variants() {
        let little = mo_file(&[("", &header("en_GB")), ("A", "B")]);
        let mut big = little.clone();
        for word in big.chunks_mut(4).take(HEADER_BYTES / 4 + 8) {
            word.reverse();
        }
        let catalog = parse_mo(&big).unwrap();
        assert_eq!(catalog.language.as_deref(), Some("en"));
        assert_eq!(catalog.entries["A"], "B");
    }

    #[test]
    fn keeps_the_first_form_of_plural_and_context_entries() {
        let bytes = mo_file(&[("ctx\u{4}Key\0Keys", "Value\0Values"), ("Empty", "")]);
        let catalog = parse_mo(&bytes).unwrap();
        assert_eq!(catalog.entries["Key"], "Value");
        assert!(!catalog.entries.contains_key("Empty"));
        assert_eq!(catalog.language, None);
    }

    #[test]
    fn skips_invalid_utf8_and_counts_it() {
        let mut bytes = mo_file(&[("Bad", "xx"), ("Good", "yes")]);
        let position = bytes
            .windows(3)
            .position(|window| window == b"xx\0")
            .unwrap();
        bytes[position] = 0xff;
        let catalog = parse_mo(&bytes).unwrap();
        assert_eq!(catalog.invalid_entries, 1);
        assert_eq!(catalog.entries.len(), 1);
    }

    #[test]
    fn rejects_bad_magic_revision_and_limits_before_allocating() {
        let good = mo_file(&[("A", "B")]);
        let mut bad = good.clone();
        bad[0] ^= 1;
        assert_eq!(parse_mo(&bad), Err(MoError::BadMagic));
        let mut revision = good.clone();
        revision[4..8].copy_from_slice(&(2u32 << 16).to_le_bytes());
        assert_eq!(parse_mo(&revision), Err(MoError::UnsupportedRevision));
        let mut minor = good.clone();
        minor[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(parse_mo(&minor).is_ok());
        let mut many = good.clone();
        many[8..12].copy_from_slice(&(MAX_MO_ENTRIES + 1).to_le_bytes());
        assert_eq!(parse_mo(&many), Err(MoError::TooManyEntries));
        let mut at_limit = good.clone();
        at_limit[8..12].copy_from_slice(&MAX_MO_ENTRIES.to_le_bytes());
        assert_eq!(parse_mo(&at_limit), Err(MoError::TableOutOfBounds));
        let mut table = good.clone();
        table[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_mo(&table), Err(MoError::TableOutOfBounds));
        let mut table = good.clone();
        table[16..20].copy_from_slice(&(good.len() as u32 - 7).to_le_bytes());
        assert_eq!(parse_mo(&table), Err(MoError::TableOutOfBounds));
        let huge = vec![0u8; MAX_MO_BYTES as usize + 1];
        assert_eq!(parse_mo(&huge), Err(MoError::TooLarge));
    }

    #[test]
    fn rejects_strings_outside_the_file_over_the_limit_or_without_nul() {
        let good = mo_file(&[("A", "B")]);
        let descriptor = HEADER_BYTES;
        let mut long = good.clone();
        long[descriptor..descriptor + 4].copy_from_slice(&(MAX_MO_STRING_BYTES + 1).to_le_bytes());
        assert_eq!(parse_mo(&long), Err(MoError::StringOutOfBounds));
        let mut outside = good.clone();
        outside[descriptor + 4..descriptor + 8].copy_from_slice(&(good.len() as u32).to_le_bytes());
        assert_eq!(parse_mo(&outside), Err(MoError::StringOutOfBounds));
        let mut wrap = good.clone();
        wrap[descriptor + 4..descriptor + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_mo(&wrap), Err(MoError::StringOutOfBounds));
        let mut no_nul = good.clone();
        let last = no_nul.len() - 1;
        no_nul[last] = b'x';
        assert_eq!(parse_mo(&no_nul), Err(MoError::StringOutOfBounds));
        let mut at_limit = mo_file(&[("A", &"x".repeat(MAX_MO_STRING_BYTES as usize))]);
        assert!(parse_mo(&at_limit).is_ok());
        at_limit.truncate(at_limit.len() - 1);
        assert_eq!(parse_mo(&at_limit), Err(MoError::StringOutOfBounds));
    }

    #[test]
    fn language_codes_are_two_or_three_lowercase_letters() {
        for (value, expected) in [
            (
                "Language: pl
",
                Some("pl"),
            ),
            (
                "Language: fil
",
                Some("fil"),
            ),
            (
                "Language: 12
",
                None,
            ),
            (
                "Language: abcd
",
                None,
            ),
            (
                "Language: x
",
                None,
            ),
            (
                "X-Other: en
",
                None,
            ),
        ] {
            assert_eq!(header_language(value).as_deref(), expected, "{value}");
        }
    }

    #[test]
    fn real_sized_names_and_files_fit_the_limits() {
        let long = "x".repeat(2_000);
        let catalog = parse_mo(&mo_file(&[("A", &long)])).unwrap();
        assert_eq!(catalog.entries["A"].len(), 2_000);
        let entries = (0..800)
            .map(|index| (format!("K{index:04}"), "y".repeat(2_000)))
            .collect::<Vec<_>>();
        let pairs = entries
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let bytes = mo_file(&pairs);
        assert!(bytes.len() > 1_500_000);
        assert_eq!(parse_mo(&bytes).unwrap().entries.len(), 800);
    }

    /// A file of exactly `size` bytes: one entry whose strings end at the last byte, padding
    /// between the tables and the strings.
    fn mo_file_of_size(size: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        let key_at = (size - 4) as u32;
        let value_at = (size - 2) as u32;
        let tables = HEADER_BYTES as u32;
        for value in [MAGIC_LE, 0, 1, tables, tables + 8, 0, 0] {
            bytes.extend(value.to_le_bytes());
        }
        for (length, offset) in [(1u32, key_at), (1, value_at)] {
            bytes.extend(length.to_le_bytes());
            bytes.extend(offset.to_le_bytes());
        }
        bytes.resize(size - 4, 0);
        bytes.extend(b"A\0B\0");
        assert_eq!(bytes.len(), size);
        bytes
    }

    #[test]
    fn limits_are_inclusive_and_exact() {
        let at_limit = mo_file_of_size(MAX_MO_BYTES as usize);
        assert_eq!(parse_mo(&at_limit).unwrap().entries["A"], "B");
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("synth_vehicles.mo");
        std::fs::write(&file, &at_limit).unwrap();
        assert_eq!(read_mo(&file).unwrap().entries["A"], "B");
        std::fs::write(&file, mo_file_of_size(MAX_MO_BYTES as usize + 1)).unwrap();
        assert_eq!(read_mo(&file), None);
        // A header with no entries and both tables at the end of the file is a valid catalog.
        let mut empty = Vec::new();
        for value in [
            MAGIC_LE,
            0,
            0,
            HEADER_BYTES as u32,
            HEADER_BYTES as u32,
            0,
            0,
        ] {
            empty.extend(value.to_le_bytes());
        }
        assert_eq!(empty.len(), HEADER_BYTES);
        assert_eq!(parse_mo(&empty), Ok(MoCatalog::default()));
        // Major revision 1 is accepted, 2 is not.
        let mut major = mo_file(&[("A", "B")]);
        major[4..8].copy_from_slice(&(1u32 << 16).to_le_bytes());
        assert!(parse_mo(&major).is_ok());
        // Two entries need 16 table bytes: a table one byte short of them is out of bounds.
        let two = mo_file(&[("A", "B"), ("C", "D")]);
        let mut short = two.clone();
        short[16..20].copy_from_slice(&(two.len() as u32 - 15).to_le_bytes());
        assert_eq!(parse_mo(&short), Err(MoError::TableOutOfBounds));
    }

    #[test]
    fn the_translation_folder_must_be_a_folder() {
        let root = tempfile::tempdir().unwrap();
        let text = root.path().join("res").join("text");
        std::fs::create_dir_all(&text).unwrap();
        std::fs::write(text.join("lc_messages"), b"not a folder").unwrap();
        assert_eq!(text_dir(root.path()), None);
    }

    #[test]
    fn without_list_xml_short_names_and_descriptions_are_not_vehicles() {
        let catalogs = HashMap::from([(
            "czech_vehicles".to_owned(),
            parse_mo(&mo_file(&[
                ("S01_Alpha", "Alpha"),
                ("S01_Alpha_short", "A"),
                ("S01_Alpha_descr", "Text"),
            ]))
            .unwrap(),
        )]);
        let records = records_from_catalogs(&catalogs);
        assert_eq!(
            records,
            vec![("czech".into(), "S01_Alpha".into(), None, None)]
        );
    }

    #[test]
    fn the_vehicle_list_comes_from_scripts_pkg_when_present() {
        use std::io::Write as _;
        let root = tempfile::tempdir().unwrap();
        let packages = root.path().join("res").join("packages");
        std::fs::create_dir_all(&packages).unwrap();
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for nation in NATION_KEYS {
            writer
                .start_file(
                    format!("scripts/item_defs/vehicles/{nation}/list.xml"),
                    options,
                )
                .unwrap();
            writer
                .write_all(&crate::vehicle_catalog_probe::tests::minimal_packed_catalog())
                .unwrap();
        }
        std::fs::write(
            packages.join("scripts.pkg"),
            writer.finish().unwrap().into_inner(),
        )
        .unwrap();
        let text = root.path().join("res").join("text").join("lc_messages");
        std::fs::create_dir_all(&text).unwrap();
        std::fs::write(
            text.join("czech_vehicles.mo"),
            mo_file(&[("vehicle", "Synth Listed"), ("Unlisted", "Synth Unlisted")]),
        )
        .unwrap();
        let view = load_table(root.path()).view("pl");
        assert_eq!(view.names["czech-vehicle"].name, "Synth Listed");
        assert!(
            !view.names.contains_key("czech-unlisted"),
            "only vehicles from list.xml"
        );
    }

    #[test]
    fn debug_output_shows_no_names() {
        assert_eq!(format!("{:?}", VehicleNames::default()), "VehicleNames");
    }

    #[test]
    fn rejects_truncation_at_every_length() {
        let full = mo_file(&[("", &header("pl")), ("Synth_Alpha", "Alpha")]);
        for length in 0..full.len() {
            assert!(parse_mo(&full[..length]).is_err(), "length {length}");
        }
        assert!(parse_mo(&full).is_ok());
    }

    #[test]
    fn deterministic_mutations_never_panic_and_results_are_deterministic() {
        let fixture = mo_file(&[("", &header("pl")), ("Synth_Alpha", "Alpha"), ("B", "C")]);
        let mut inputs = vec![Vec::new(), vec![0; 64], fixture.clone()];
        for index in 0..fixture.len() {
            let mut mutated = fixture.clone();
            mutated[index] ^= (index as u8).wrapping_mul(31).wrapping_add(1);
            inputs.push(mutated);
        }
        let mut state = 0x5EED_0E5A_u32;
        for _ in 0..256 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let length = (state as usize) % 512;
            let mut random = MAGIC_LE.to_le_bytes().to_vec();
            for _ in 0..length {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                random.push((state >> 24) as u8);
            }
            inputs.push(random);
        }
        for input in inputs {
            let first = std::panic::catch_unwind(|| parse_mo(&input));
            assert!(first.is_ok());
            assert_eq!(first.unwrap(), parse_mo(&input));
        }
    }

    #[test]
    fn translation_keys_are_validated() {
        assert_eq!(
            translation_key("#synth_vehicles:S01_Alpha"),
            Some(("synth_vehicles", "S01_Alpha"))
        );
        for bad in [
            "synth_vehicles:A",
            "#:A",
            "#synth_vehicles:",
            "#../x:A",
            "#Synth:A",
            "#a:b c",
        ] {
            assert_eq!(translation_key(bad), None, "{bad}");
        }
    }

    fn catalogs(language: &str) -> HashMap<String, MoCatalog> {
        let czech = parse_mo(&mo_file(&[
            ("", &header(language)),
            ("S01_Alpha", "Synth Alpha Mk. 1"),
            ("S01_Alpha_short", "Alpha"),
            ("Other_Key", "Bravo Full"),
            ("Same", "Same Name"),
            ("Same_short", "Same Name"),
            ("Bad", "line\u{7}break"),
        ]))
        .unwrap();
        HashMap::from([("czech_vehicles".to_owned(), czech)])
    }

    fn record(tag: &str, user: Option<&str>) -> (String, String, Option<String>, Option<String>) {
        (
            "czech".into(),
            tag.into(),
            user.map(str::to_owned),
            user.map(|value| format!("{value}_short")),
        )
    }

    fn table(catalogs: HashMap<String, MoCatalog>, records: Vec<Record>) -> NameTable {
        NameTable { catalogs, records }
    }

    #[test]
    fn names_follow_list_xml_keys_and_fall_back_to_the_tag() {
        let view = table(
            catalogs("pl"),
            vec![
                record("S01_Alpha", None),
                record("S02_Bravo", Some("#czech_vehicles:Other_Key")),
                record("Same", None),
                record("Bad", None),
                record("Missing", None),
                record("Elsewhere", Some("#usa_vehicles:Elsewhere")),
            ],
        )
        .view("pl");
        let table = &view;
        assert_eq!(table.language.as_deref(), Some("pl"));
        assert_eq!(
            table.names["czech-s01_alpha"],
            VehicleNameView {
                name: "Synth Alpha Mk. 1".into(),
                short_name: Some("Alpha".into())
            }
        );
        assert_eq!(table.names["czech-s02_bravo"].name, "Bravo Full");
        assert_eq!(table.names["czech-s02_bravo"].short_name, None);
        assert_eq!(table.names["czech-same"].short_name, None);
        assert!(!table.names.contains_key("czech-bad"));
        assert!(!table.names.contains_key("czech-missing"));
        assert!(!table.names.contains_key("czech-elsewhere"));
    }

    #[test]
    fn a_declared_language_follows_the_window_then_english() {
        assert!(usable(Some("pl"), "pl"));
        assert!(usable(Some("en"), "pl"));
        assert!(usable(Some("en"), "en"));
        assert!(!usable(Some("pl"), "en"));
        assert!(!usable(Some("de"), "pl"));
    }

    /// Variant A (author, 2026-11-14): a file without `Language` is in the client's language, not
    /// specified in the file, and is used for both window languages.
    #[test]
    fn a_file_without_a_language_header_is_the_client_language_for_both_windows() {
        assert!(usable(None, "pl"));
        assert!(usable(None, "en"));
        let unspecified = parse_mo(&mo_file(&[
            ("", "Content-Type: text/plain; charset=utf-8\n"),
            ("S01_Alpha", "Synth Alpha"),
        ]))
        .unwrap();
        assert_eq!(unspecified.language, None);
        let declared_de =
            parse_mo(&mo_file(&[("", &header("de")), ("S02_Bravo", "Bravo DE")])).unwrap();
        let both = table(
            HashMap::from([
                ("czech_vehicles".to_owned(), unspecified),
                ("usa_vehicles".to_owned(), declared_de),
            ]),
            vec![
                record("S01_Alpha", None),
                ("usa".into(), "S02_Bravo".into(), None, None),
            ],
        );
        for window in ["pl", "en"] {
            let view = both.view(window);
            assert_eq!(view.language, None, "{window}");
            assert_eq!(view.names["czech-s01_alpha"].name, "Synth Alpha");
            assert!(
                !view.names.contains_key("usa-s02_bravo"),
                "a declared foreign language stays out"
            );
        }
    }

    #[test]
    fn the_uk_nation_reads_gb_vehicles_and_event_files_are_ignored() {
        assert_eq!(nation_catalog("uk"), "gb_vehicles");
        assert_eq!(nation_catalog("czech"), "czech_vehicles");
        assert_eq!(catalog_stem("gb_vehicles.mo"), Some("gb_vehicles"));
        assert_eq!(
            catalog_stem("multinational_vehicles.mo"),
            Some("multinational_vehicles")
        );
        for ignored in [
            "halloween.gb_vehicles.extend.mo",
            "battle_royale.battle_royale_vehicles.mo",
            "gb_vehicles.po",
            "menu.mo",
            "Gb_vehicles.mo",
        ] {
            assert_eq!(catalog_stem(ignored), None, "{ignored}");
        }
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("res").join("text").join("lc_messages");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("gb_vehicles.mo"),
            mo_file(&[("GB01_Synth", "Synth Gb Name")]),
        )
        .unwrap();
        std::fs::write(
            dir.join("halloween.gb_vehicles.extend.mo"),
            mo_file(&[("GB01_Synth", "Event Name")]),
        )
        .unwrap();
        let table = load_table(root.path());
        assert_eq!(table.catalogs.len(), 1);
        let view = table.view("en");
        assert_eq!(view.names["uk-gb01_synth"].name, "Synth Gb Name");
        let keyed = NameTable {
            records: vec![(
                "uk".into(),
                "GB01_Synth".into(),
                Some("#gb_vehicles:GB01_Synth".into()),
                None,
            )],
            ..table
        };
        assert_eq!(
            keyed.view("pl").names["uk-gb01_synth"].name,
            "Synth Gb Name"
        );
    }

    fn game_folder(language: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("res").join("text").join("LC_MESSAGES");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("czech_vehicles.mo"),
            mo_file(&[("", &header(language)), ("S01_Alpha", "Synth Alpha")]),
        )
        .unwrap();
        root
    }

    fn listing(root: &Path) -> Vec<(PathBuf, u64)> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
                let metadata = entry.metadata().unwrap();
                if metadata.is_dir() {
                    pending.push(entry.path());
                } else {
                    files.push((entry.path(), metadata.len()));
                }
            }
        }
        files.sort();
        files
    }

    #[test]
    fn reads_the_game_folder_without_writing_and_reloads_on_a_new_client() {
        let root = game_folder("pl");
        let before = listing(root.path());
        let names = VehicleNames::default();
        let view = names.view(root.path(), "pl");
        assert_eq!(view.language.as_deref(), Some("pl"));
        assert_eq!(view.names["czech-s01_alpha"].name, "Synth Alpha");
        assert_eq!(names.view(root.path(), "en"), VehicleNamesView::default());
        assert_eq!(
            listing(root.path()),
            before,
            "the game folder is never written"
        );

        let dir = root.path().join("res").join("text").join("LC_MESSAGES");
        std::fs::write(
            dir.join("czech_vehicles.mo"),
            mo_file(&[("", &header("en")), ("S01_Alpha", "Synth Alpha EN version")]),
        )
        .unwrap();
        let view = names.view(root.path(), "pl");
        assert_eq!(view.language.as_deref(), Some("en"));
        assert_eq!(view.names["czech-s01_alpha"].name, "Synth Alpha EN version");
        names.clear();
        assert!(names.state.lock().unwrap().key.is_none());
    }

    #[test]
    fn a_folder_without_translations_or_with_a_broken_file_has_no_names() {
        let empty = tempfile::tempdir().unwrap();
        let names = VehicleNames::default();
        assert_eq!(names.view(empty.path(), "pl"), VehicleNamesView::default());
        let root = game_folder("pl");
        let file = root
            .path()
            .join("res")
            .join("text")
            .join("LC_MESSAGES")
            .join("czech_vehicles.mo");
        std::fs::write(&file, b"not a catalog").unwrap();
        assert_eq!(names.view(root.path(), "pl"), VehicleNamesView::default());
        std::fs::write(&file, vec![0u8; MAX_MO_BYTES as usize + 1]).unwrap();
        assert_eq!(read_mo(&file), None);
    }

    /// E5-ui §2 reconnaissance and acceptance on the author's client: aggregates only, read-only.
    #[test]
    #[ignore = "reads the author's game folder; set AFTERBATTLE_GOLDEN_DIR or AFTERBATTLE_GOLDEN_CLIENT_DIR and run with --ignored --nocapture"]
    fn game_folder_names_report() {
        let client = std::env::var_os("AFTERBATTLE_GOLDEN_CLIENT_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("AFTERBATTLE_GOLDEN_DIR")
                    .map(PathBuf::from)
                    .and_then(|replays| replays.parent().map(Path::to_path_buf))
            })
            .expect("set AFTERBATTLE_GOLDEN_DIR or AFTERBATTLE_GOLDEN_CLIENT_DIR");
        println!("=== vehicle names from the game folder (numbers only) ===");
        let text = client.join("res").join("text");
        let folders = std::fs::read_dir(&text)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| e.path().is_dir())
                    .count()
            })
            .unwrap_or(0);
        println!("folders in res/text: {folders}");
        let Some(dir) = text_dir(&client) else {
            println!("lc_messages: missing");
            println!("=== end ===");
            return;
        };
        let all_mo = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|e| e.path().extension().is_some_and(|x| x == "mo"))
                    .count()
            })
            .unwrap_or(0);
        println!(".mo files in lc_messages: {all_mo}");
        let mut vehicle_files = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|name| name.contains("vehicles") && name.ends_with(".mo"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        vehicle_files.sort();
        println!("*vehicles*.mo files: {vehicle_files:?}");
        let started = std::time::Instant::now();
        let table = load_table(&client);
        let elapsed = started.elapsed().as_millis();
        let mut languages = BTreeMap::<String, usize>::new();
        let (mut entries, mut invalid) = (0, 0);
        for catalog in table.catalogs.values() {
            entries += catalog.entries.len();
            invalid += catalog.invalid_entries;
            *languages
                .entry(
                    catalog
                        .language
                        .clone()
                        .unwrap_or_else(|| "(client language, not specified in the file)".into()),
                )
                .or_default() += 1;
        }
        let mut loaded = table.catalogs.keys().cloned().collect::<Vec<_>>();
        loaded.sort();
        println!("vehicle catalogs loaded: {} {loaded:?}", loaded.len());
        let missing = NATION_KEYS
            .iter()
            .filter(|nation| !table.catalogs.contains_key(&nation_catalog(nation)))
            .collect::<Vec<_>>();
        println!("nations without a loaded catalog: {missing:?}");
        println!("catalog languages: {languages:?}");
        println!("entries: {entries}, invalid utf-8: {invalid}");
        let records = &table.records;
        let with_key = records.iter().filter(|r| r.2.is_some()).count();
        let with_short = records.iter().filter(|r| r.3.is_some()).count();
        println!(
            "list.xml records: {}, with userString: {with_key}, with shortUserString: {with_short}",
            records.len()
        );
        let mut reasons = BTreeMap::<&str, usize>::new();
        let mut bad_keys = Vec::new();
        for (nation, _, user_string, _) in records {
            let reason = match user_string
                .as_deref()
                .map(|key| (key, translation_key(key)))
            {
                None => "no userString in list.xml",
                Some((key, None)) => {
                    bad_keys.push(key.to_owned());
                    "userString key not accepted"
                }
                Some((_, Some((file, id)))) => match table.catalogs.get(file) {
                    None if file == nation_catalog(nation) => "nation file missing",
                    None => "key points to a file that is not loaded",
                    Some(catalog) => match catalog.entries.get(id) {
                        None => "no entry in the .mo file",
                        Some(value) if shown(value).is_none() => "entry not shown",
                        Some(_) => "resolved",
                    },
                },
            };
            *reasons.entry(reason).or_default() += 1;
        }
        println!("records by outcome: {reasons:?}");
        bad_keys.sort();
        println!("keys not accepted (game translation keys): {bad_keys:?}");
        for window in ["pl", "en"] {
            let view = table.view(window);
            let shorts = view
                .names
                .values()
                .filter(|n| n.short_name.is_some())
                .count();
            println!(
                "window {window}: names {} (distinct short {shorts}), language {:?}",
                view.names.len(),
                view.language
            );
        }
        println!("load ms: {elapsed}");
        println!("=== end ===");
    }
}
