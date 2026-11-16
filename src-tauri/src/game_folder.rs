//! The game folder (hotfix 1.0.0-beta.3): checking a folder the player chose, turning a chosen
//! `replays` folder or `WorldOfTanks.exe` into the game folder, and finding installations.
//!
//! Detection never scans a drive recursively: it probes a fixed list of places (uninstall entries
//! of the registry, a few folders on every fixed drive, Steam libraries) and stops at a deadline.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::AppError;

const GAME_EXECUTABLE: &str = "WorldOfTanks.exe";
const REPLAYS: &str = "replays";

/// Folders probed on every fixed drive (relative to the drive root).
const DRIVE_GAME_FOLDERS: [&[&str]; 7] = [
    &["Games", "World_of_Tanks_EU"],
    &["Games", "World_of_Tanks_NA"],
    &["Games", "World_of_Tanks_ASIA"],
    &["Games", "World_of_Tanks"],
    &["World_of_Tanks"],
    &["Program Files (x86)", "World_of_Tanks_EU"],
    &["Program Files", "World_of_Tanks_EU"],
];

/// Usual Steam folders on every fixed drive (relative to the drive root).
const DRIVE_STEAM_FOLDERS: [&[&str]; 4] = [
    &["Program Files (x86)", "Steam"],
    &["Program Files", "Steam"],
    &["Steam"],
    &["SteamLibrary"],
];

/// The game inside a Steam library.
const STEAM_GAME: [&str; 3] = ["steamapps", "common", "World of Tanks"];

/// How long detection may take before it returns what it found so far.
pub(crate) const DETECTION_LIMIT: Duration = Duration::from_secs(4);

/// Where detection looks, read from the system by [`system_sources`] (tests pass their own).
#[derive(Debug, Default, Clone)]
pub(crate) struct DetectionSources {
    /// `InstallLocation` values of the uninstall entries in the registry.
    pub install_locations: Vec<PathBuf>,
    /// Roots of the fixed drives (`C:\`, `D:\`, ...).
    pub fixed_drives: Vec<PathBuf>,
    /// Steam folders named by the registry.
    pub steam_roots: Vec<PathBuf>,
}

/// A read error of the game folder as the message the player sees.
pub(crate) fn access_error(error: &io::Error) -> AppError {
    if error.kind() == io::ErrorKind::PermissionDenied {
        AppError::GameFolderUnreadable
    } else {
        AppError::InvalidGameDirectory
    }
}

/// Checks that `path` is a game folder: `WorldOfTanks.exe` (in the folder or in `win64`) and a
/// readable `replays` folder inside it. Returns the canonical folder.
pub(crate) fn validate_root(path: &Path) -> Result<PathBuf, AppError> {
    let root = fs::canonicalize(path).map_err(|error| access_error(&error))?;
    if !root.is_dir() {
        return Err(AppError::InvalidGameDirectory);
    }
    fs::read_dir(&root).map_err(|error| access_error(&error))?;
    let has_executable =
        root.join(GAME_EXECUTABLE).is_file() || root.join("win64").join(GAME_EXECUTABLE).is_file();
    if !has_executable {
        return Err(AppError::GameExecutableMissing);
    }
    let replays = root.join(REPLAYS);
    if !replays.is_dir() {
        return Err(AppError::ReplaysFolderMissing);
    }
    let replays = fs::canonicalize(replays).map_err(|error| access_error(&error))?;
    if !replays.starts_with(&root) {
        return Err(AppError::InvalidGameDirectory);
    }
    fs::read_dir(&replays).map_err(|error| access_error(&error))?;
    Ok(root)
}

fn named(path: &Path, name: &str) -> bool {
    path.file_name()
        .is_some_and(|file| file.to_string_lossy().eq_ignore_ascii_case(name))
}

/// The folder the player chose, as the game folder: a chosen `replays` folder becomes its parent,
/// a chosen `WorldOfTanks.exe` its folder (out of `win64`/`win32` too). Nothing is read.
pub(crate) fn normalize_selection(path: &Path) -> PathBuf {
    let parent = |path: &Path| path.parent().unwrap_or(path).to_path_buf();
    if named(path, REPLAYS) {
        return parent(path);
    }
    if named(path, GAME_EXECUTABLE) {
        let folder = parent(path);
        if named(&folder, "win64") || named(&folder, "win32") {
            return parent(&folder);
        }
        return folder;
    }
    path.to_path_buf()
}

/// A folder the player chose, checked for use: normalized, validated, with at least one replay.
pub(crate) fn prepare_selection(path: &Path) -> Result<PathBuf, AppError> {
    let root = validate_root(&normalize_selection(path))?;
    let has_replay = fs::read_dir(root.join(REPLAYS))
        .map_err(|error| access_error(&error))?
        .filter_map(Result::ok)
        .any(|entry| {
            let path = entry.path();
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("wotreplay"))
        });
    if has_replay {
        Ok(root)
    } else {
        Err(AppError::ReplaysFolderEmpty)
    }
}

/// Library folders listed in Steam's `libraryfolders.vdf`.
pub(crate) fn steam_library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|line| {
            let mut tokens = line.split('"').skip(1).step_by(2);
            let key = tokens.next()?;
            let value = tokens.next()?;
            (key.eq_ignore_ascii_case("path") && !value.is_empty())
                .then(|| PathBuf::from(value.replace(r"\\", r"\")))
        })
        .collect()
}

/// An `InstallLocation` value as a path (quotes and blanks removed); `None` when empty.
pub(crate) fn clean_install_location(raw: &str) -> Option<PathBuf> {
    let value = raw.trim().trim_matches('"').trim();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

/// Roots of the drives set in `mask` (bit 0 = `A:`) for which `is_fixed` holds.
pub(crate) fn fixed_drive_roots(mask: u32, is_fixed: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    (b'A'..=b'Z')
        .zip(0..26)
        .filter(|(_, bit)| mask & (1 << bit) != 0)
        .map(|(letter, _)| format!(r"{}:\", letter as char))
        .filter(|root| is_fixed(root))
        .map(PathBuf::from)
        .collect()
}

/// A canonical Windows path without the `\\?\` prefix of a drive path, for showing and storing.
pub(crate) fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest.to_owned(),
        _ => text.into_owned(),
    }
}

fn joined(base: &Path, folders: &[&str]) -> PathBuf {
    folders
        .iter()
        .fold(base.to_path_buf(), |path, folder| path.join(folder))
}

/// Game folders found from `sources`, canonical, without duplicates, in the order probed:
/// registry entries, the usual folders of every fixed drive, Steam libraries. Probing stops at
/// `deadline`.
pub(crate) fn detect_candidates(sources: &DetectionSources, deadline: Instant) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut probe = |path: PathBuf| -> bool {
        if Instant::now() >= deadline {
            return false;
        }
        if let Ok(root) = validate_root(&path)
            && seen.insert(root.clone())
        {
            found.push(root);
        }
        true
    };
    let drive_games = sources.fixed_drives.iter().flat_map(|drive| {
        DRIVE_GAME_FOLDERS
            .iter()
            .map(move |folders| joined(drive, folders))
    });
    for path in sources.install_locations.iter().cloned().chain(drive_games) {
        if !probe(path) {
            return found;
        }
    }
    let drive_steam = sources.fixed_drives.iter().flat_map(|drive| {
        DRIVE_STEAM_FOLDERS
            .iter()
            .map(move |folders| joined(drive, folders))
    });
    for steam in sources.steam_roots.iter().cloned().chain(drive_steam) {
        let listed = fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf"))
            .map(|vdf| steam_library_paths(&vdf))
            .unwrap_or_default();
        for library in std::iter::once(steam).chain(listed) {
            if !probe(joined(&library, &STEAM_GAME)) {
                return found;
            }
        }
    }
    found
}

/// Where detection looks on this computer: uninstall entries of the registry, Steam folders named
/// by the registry and the fixed drives. Reads the system, so it is not unit-tested.
#[cfg(windows)]
pub(crate) fn system_sources() -> DetectionSources {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    const UNINSTALL_32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
    let mut install_locations = Vec::new();
    for (hive, path) in [
        (HKEY_LOCAL_MACHINE, UNINSTALL),
        (HKEY_LOCAL_MACHINE, UNINSTALL_32),
        (HKEY_CURRENT_USER, UNINSTALL),
    ] {
        let Ok(uninstall) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ) else {
            continue;
        };
        for name in uninstall.enum_keys().filter_map(Result::ok) {
            let location = uninstall
                .open_subkey_with_flags(&name, KEY_READ)
                .and_then(|entry| entry.get_value::<String, _>("InstallLocation"));
            if let Some(location) = location.ok().as_deref().and_then(clean_install_location)
                && location.join(GAME_EXECUTABLE).is_file()
            {
                install_locations.push(location);
            }
        }
    }
    let mut steam_roots = Vec::new();
    for (hive, path, value) in [
        (HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Valve\Steam",
            "InstallPath",
        ),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Valve\Steam", "InstallPath"),
    ] {
        let root = RegKey::predef(hive)
            .open_subkey_with_flags(path, KEY_READ)
            .and_then(|key| key.get_value::<String, _>(value));
        if let Some(root) = root.ok().as_deref().and_then(clean_install_location) {
            steam_roots.push(root);
        }
    }
    DetectionSources {
        install_locations,
        fixed_drives: system_fixed_drives(),
        steam_roots,
    }
}

/// Roots of the fixed drives of this computer. The only `unsafe` code of the application: two
/// Windows calls without Rust bindings in the standard library.
#[cfg(windows)]
fn system_fixed_drives() -> Vec<PathBuf> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    const DRIVE_FIXED: u32 = 3;

    // SAFETY: `GetLogicalDrives` takes no arguments, reads no memory of ours and only returns a
    // bit mask (0 on failure, which simply yields no drives).
    let mask = unsafe { GetLogicalDrives() };
    fixed_drive_roots(mask, |root| {
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string owned by this closure and alive for
        // the whole call; `GetDriveTypeW` only reads it up to the NUL and keeps no pointer.
        unsafe { GetDriveTypeW(wide.as_ptr()) == DRIVE_FIXED }
    })
}

#[cfg(not(windows))]
pub(crate) fn system_sources() -> DetectionSources {
    DetectionSources::default()
}

/// Game folders found on this computer within [`DETECTION_LIMIT`], as shown to the player.
pub(crate) fn detect_installations() -> Vec<String> {
    let deadline = Instant::now() + DETECTION_LIMIT;
    detect_candidates(&system_sources(), deadline)
        .iter()
        .map(|root| display_path(root))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn game(root: &Path) -> PathBuf {
        fs::create_dir_all(root.join(REPLAYS)).unwrap();
        fs::write(root.join(GAME_EXECUTABLE), []).unwrap();
        root.to_path_buf()
    }

    fn with_replay(root: &Path) {
        fs::write(root.join(REPLAYS).join("battle.wotreplay"), [1]).unwrap();
    }

    fn far() -> Instant {
        Instant::now() + Duration::from_secs(60)
    }

    // ---- validation ----

    #[test]
    fn a_game_folder_is_valid_and_canonical() {
        let directory = tempdir().unwrap();
        let root = game(directory.path());
        assert_eq!(
            validate_root(&root).unwrap(),
            fs::canonicalize(&root).unwrap()
        );
    }

    #[test]
    fn the_executable_may_be_in_win64() {
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path().join(REPLAYS)).unwrap();
        fs::create_dir_all(directory.path().join("win64")).unwrap();
        fs::write(directory.path().join("win64").join(GAME_EXECUTABLE), []).unwrap();
        assert!(validate_root(directory.path()).is_ok());
    }

    #[test]
    fn a_folder_without_the_executable_says_so() {
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path().join(REPLAYS)).unwrap();
        assert!(matches!(
            validate_root(directory.path()),
            Err(AppError::GameExecutableMissing)
        ));
    }

    #[test]
    fn an_executable_that_is_a_folder_does_not_count() {
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path().join(REPLAYS)).unwrap();
        fs::create_dir_all(directory.path().join(GAME_EXECUTABLE)).unwrap();
        fs::create_dir_all(directory.path().join("win64").join(GAME_EXECUTABLE)).unwrap();
        assert!(matches!(
            validate_root(directory.path()),
            Err(AppError::GameExecutableMissing)
        ));
    }

    #[test]
    fn a_folder_without_replays_says_so() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join(GAME_EXECUTABLE), []).unwrap();
        assert!(matches!(
            validate_root(directory.path()),
            Err(AppError::ReplaysFolderMissing)
        ));
    }

    #[test]
    fn a_replays_file_is_not_a_replays_folder() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join(GAME_EXECUTABLE), []).unwrap();
        fs::write(directory.path().join(REPLAYS), []).unwrap();
        assert!(matches!(
            validate_root(directory.path()),
            Err(AppError::ReplaysFolderMissing)
        ));
    }

    #[test]
    fn a_missing_folder_or_a_file_is_not_a_game_folder() {
        let directory = tempdir().unwrap();
        assert!(matches!(
            validate_root(&directory.path().join("absent")),
            Err(AppError::InvalidGameDirectory)
        ));
        let file = directory.path().join("file.txt");
        fs::write(&file, []).unwrap();
        assert!(matches!(
            validate_root(&file),
            Err(AppError::InvalidGameDirectory)
        ));
    }

    #[test]
    fn read_errors_name_the_missing_permission() {
        assert!(matches!(
            access_error(&io::Error::from(io::ErrorKind::PermissionDenied)),
            AppError::GameFolderUnreadable
        ));
        assert!(matches!(
            access_error(&io::Error::from(io::ErrorKind::NotFound)),
            AppError::InvalidGameDirectory
        ));
    }

    // ---- normalization ----

    #[test]
    fn a_chosen_replays_folder_becomes_the_game_folder() {
        let root = Path::new(r"D:\Games\World_of_Tanks_EU");
        assert_eq!(normalize_selection(&root.join("replays")), root);
        assert_eq!(normalize_selection(&root.join("REPLAYS")), root);
    }

    #[test]
    fn a_chosen_executable_becomes_its_folder() {
        let root = Path::new(r"D:\Games\World_of_Tanks_EU");
        assert_eq!(normalize_selection(&root.join("WorldOfTanks.exe")), root);
        assert_eq!(normalize_selection(&root.join("worldoftanks.EXE")), root);
        assert_eq!(
            normalize_selection(&root.join("win64").join("WorldOfTanks.exe")),
            root
        );
        assert_eq!(
            normalize_selection(&root.join("Win32").join("WorldOfTanks.exe")),
            root
        );
    }

    #[test]
    fn other_folders_stay_as_chosen() {
        for path in [
            r"D:\Games\World_of_Tanks_EU",
            r"D:\Games\World_of_Tanks_EU\win64",
            r"D:\replays_old",
            r"D:\Games\other.exe",
        ] {
            assert_eq!(normalize_selection(Path::new(path)), Path::new(path));
        }
    }

    #[test]
    fn a_drive_root_named_like_nothing_stays() {
        assert_eq!(normalize_selection(Path::new(r"D:\")), Path::new(r"D:\"));
    }

    #[test]
    fn preparing_a_chosen_replays_folder_with_a_replay_gives_the_game_folder() {
        let directory = tempdir().unwrap();
        let root = game(directory.path());
        with_replay(&root);
        assert_eq!(
            prepare_selection(&root.join(REPLAYS)).unwrap(),
            fs::canonicalize(&root).unwrap()
        );
        assert_eq!(
            prepare_selection(&root.join(GAME_EXECUTABLE)).unwrap(),
            fs::canonicalize(&root).unwrap()
        );
    }

    #[test]
    fn preparing_a_folder_without_replay_files_says_the_folder_is_empty() {
        let directory = tempdir().unwrap();
        let root = game(directory.path());
        fs::write(root.join(REPLAYS).join("notes.txt"), [1]).unwrap();
        fs::create_dir_all(root.join(REPLAYS).join("folder.wotreplay")).unwrap();
        assert!(matches!(
            prepare_selection(&root),
            Err(AppError::ReplaysFolderEmpty)
        ));
    }

    #[test]
    fn preparing_accepts_any_case_of_the_replay_extension() {
        let directory = tempdir().unwrap();
        let root = game(directory.path());
        fs::write(root.join(REPLAYS).join("battle.WOTREPLAY"), [1]).unwrap();
        assert!(prepare_selection(&root).is_ok());
    }

    #[test]
    fn preparing_a_wrong_folder_keeps_the_specific_error() {
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path().join(REPLAYS)).unwrap();
        assert!(matches!(
            prepare_selection(&directory.path().join(REPLAYS)),
            Err(AppError::GameExecutableMissing)
        ));
    }

    // ---- sources ----

    #[test]
    fn steam_library_paths_are_read_from_libraryfolders_vdf() {
        let vdf = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"label"		""
		"apps"
		{
			"1407200"		"123"
		}
	}
	"1"
	{
		"Path"		"E:\\SteamLibrary"
	}
}
"#;
        assert_eq!(
            steam_library_paths(vdf),
            vec![
                PathBuf::from(r"C:\Program Files (x86)\Steam"),
                PathBuf::from(r"E:\SteamLibrary"),
            ]
        );
    }

    #[test]
    fn steam_library_paths_ignore_other_and_broken_lines() {
        assert!(steam_library_paths("\"label\"\t\"path\"\n\"path\"\n\"path\" \"\"\n").is_empty());
        assert!(steam_library_paths("").is_empty());
    }

    #[test]
    fn install_locations_lose_quotes_and_blanks() {
        assert_eq!(
            clean_install_location("  \"D:\\Games\\World_of_Tanks_EU\\\" "),
            Some(PathBuf::from(r"D:\Games\World_of_Tanks_EU\"))
        );
        assert_eq!(
            clean_install_location(r"D:\WoT"),
            Some(PathBuf::from(r"D:\WoT"))
        );
        assert_eq!(clean_install_location("  "), None);
        assert_eq!(clean_install_location("\"\""), None);
    }

    #[test]
    fn fixed_drive_roots_follow_the_mask_and_the_drive_type() {
        // A:, C:, D:, Z: present; D: is not fixed.
        let mask = 0b1 | 0b100 | 0b1000 | (1 << 25);
        assert_eq!(
            fixed_drive_roots(mask, |root| root != r"D:\"),
            vec![
                PathBuf::from(r"A:\"),
                PathBuf::from(r"C:\"),
                PathBuf::from(r"Z:\")
            ]
        );
        assert!(fixed_drive_roots(0, |_| true).is_empty());
        assert!(fixed_drive_roots(u32::MAX << 26, |_| true).is_empty());
    }

    #[test]
    fn display_paths_drop_the_verbatim_drive_prefix_only() {
        assert_eq!(
            display_path(Path::new(r"\\?\D:\Games\World_of_Tanks_EU")),
            r"D:\Games\World_of_Tanks_EU"
        );
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share\WoT")),
            r"\\?\UNC\server\share\WoT"
        );
        assert_eq!(display_path(Path::new(r"D:\WoT")), r"D:\WoT");
    }

    // ---- detection ----

    fn drive(directory: &Path, folders: &[&str]) -> PathBuf {
        let path = folders
            .iter()
            .fold(directory.to_path_buf(), |p, f| p.join(f));
        game(&path)
    }

    #[test]
    fn detection_finds_the_game_on_any_drive() {
        let c = tempdir().unwrap();
        let d = tempdir().unwrap();
        let e = tempdir().unwrap();
        let na = drive(d.path(), &["Games", "World_of_Tanks_NA"]);
        let asia = drive(e.path(), &["Games", "World_of_Tanks_ASIA"]);
        let sources = DetectionSources {
            fixed_drives: vec![c.path().into(), d.path().into(), e.path().into()],
            ..Default::default()
        };
        assert_eq!(
            detect_candidates(&sources, far()),
            vec![
                fs::canonicalize(na).unwrap(),
                fs::canonicalize(asia).unwrap()
            ]
        );
    }

    #[test]
    fn detection_probes_every_usual_folder() {
        for folders in DRIVE_GAME_FOLDERS {
            let d = tempdir().unwrap();
            let root = drive(d.path(), folders);
            let sources = DetectionSources {
                fixed_drives: vec![d.path().into()],
                ..Default::default()
            };
            assert_eq!(
                detect_candidates(&sources, far()),
                vec![fs::canonicalize(root).unwrap()],
                "{folders:?}"
            );
        }
    }

    #[test]
    fn detection_takes_registry_locations_with_the_game_only() {
        let game_dir = tempdir().unwrap();
        let other = tempdir().unwrap();
        let root = game(game_dir.path());
        let sources = DetectionSources {
            install_locations: vec![other.path().into(), root.clone()],
            ..Default::default()
        };
        assert_eq!(
            detect_candidates(&sources, far()),
            vec![fs::canonicalize(root).unwrap()]
        );
    }

    #[test]
    fn detection_finds_steam_games_in_every_library() {
        let c = tempdir().unwrap();
        let library = tempdir().unwrap();
        let steam = c.path().join("Program Files (x86)").join("Steam");
        let in_steam = drive(&steam, &STEAM_GAME);
        let in_library = drive(library.path(), &STEAM_GAME);
        fs::write(
            steam.join("steamapps").join("libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n",
                steam.display().to_string().replace('\\', "\\\\"),
                library.path().display().to_string().replace('\\', "\\\\"),
            ),
        )
        .unwrap();
        let sources = DetectionSources {
            fixed_drives: vec![c.path().into()],
            ..Default::default()
        };
        assert_eq!(
            detect_candidates(&sources, far()),
            vec![
                fs::canonicalize(in_steam).unwrap(),
                fs::canonicalize(in_library).unwrap()
            ]
        );
    }

    #[test]
    fn detection_uses_steam_roots_from_the_registry_and_every_drive_folder() {
        let registry_steam = tempdir().unwrap();
        let from_registry = drive(registry_steam.path(), &STEAM_GAME);
        let mut expected = vec![fs::canonicalize(from_registry).unwrap()];
        let mut drives = Vec::new();
        let mut keep = Vec::new();
        for folders in DRIVE_STEAM_FOLDERS {
            let d = tempdir().unwrap();
            let root = drive(
                &folders
                    .iter()
                    .fold(d.path().to_path_buf(), |p, f| p.join(f)),
                &STEAM_GAME,
            );
            expected.push(fs::canonicalize(root).unwrap());
            drives.push(d.path().to_path_buf());
            keep.push(d);
        }
        let sources = DetectionSources {
            steam_roots: vec![registry_steam.path().into()],
            fixed_drives: drives,
            ..Default::default()
        };
        assert_eq!(detect_candidates(&sources, far()), expected);
    }

    #[test]
    fn detection_reports_one_folder_once() {
        let d = tempdir().unwrap();
        let root = drive(d.path(), &["Games", "World_of_Tanks_EU"]);
        let sources = DetectionSources {
            install_locations: vec![
                root.clone(),
                root.join(".").join("..").join("World_of_Tanks_EU"),
            ],
            fixed_drives: vec![d.path().into(), d.path().into()],
            ..Default::default()
        };
        assert_eq!(
            detect_candidates(&sources, far()),
            vec![fs::canonicalize(root).unwrap()]
        );
    }

    #[test]
    fn detection_skips_folders_that_are_not_games() {
        let d = tempdir().unwrap();
        fs::create_dir_all(
            d.path()
                .join("Games")
                .join("World_of_Tanks_EU")
                .join(REPLAYS),
        )
        .unwrap();
        let sources = DetectionSources {
            fixed_drives: vec![d.path().into()],
            ..Default::default()
        };
        assert!(detect_candidates(&sources, far()).is_empty());
    }

    #[test]
    fn detection_stops_at_the_deadline() {
        let d = tempdir().unwrap();
        let root = drive(d.path(), &["Games", "World_of_Tanks_EU"]);
        let sources = DetectionSources {
            install_locations: vec![root],
            ..Default::default()
        };
        assert!(detect_candidates(&sources, Instant::now()).is_empty());
    }
}
