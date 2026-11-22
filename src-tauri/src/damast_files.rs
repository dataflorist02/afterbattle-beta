//! Damast files (spec 34 §3–§4): the plates of History kept as images in the app's data folder,
//! and the 1200×630 card the player saves. The window sends bytes it drew; this module checks
//! them, keeps tiles only under `<data>/damast/<version>/<key>.jpg`, and writes a card only to
//! the path the player chose in the save dialog, as a bare PNG without text or metadata chunks.

use crate::error::AppError;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Folder of the tile cache inside the app's data folder.
pub(crate) const DAMAST_DIR: &str = "damast";
/// A tile is a small JPEG (752×352); anything bigger is not one of ours.
pub(crate) const MAX_TILE_BYTES: usize = 2 * 1024 * 1024;
/// The card is 1200×630; PNG of that size stays far below this.
pub(crate) const MAX_CARD_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const CARD_WIDTH: u32 = 1200;
pub(crate) const CARD_HEIGHT: u32 = 630;

/// `damast-1`, `damast-1.2`: lower-case letters, digits, dots and dashes, at most 16.
fn valid_version(version: &str) -> bool {
    (1..=16).contains(&version.len())
        && version.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-'
        })
        && version
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && !version.contains("..")
}

/// A hash in lower-case hex, 16 to 64 characters.
fn valid_key(key: &str) -> bool {
    (16..=64).contains(&key.len())
        && key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn tile_path(root: &Path, version: &str, key: &str) -> Result<PathBuf, AppError> {
    if !valid_version(version) || !valid_key(key) {
        return Err(AppError::InvalidApiResponse);
    }
    Ok(root.join(version).join(format!("{key}.jpg")))
}

fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.len() >= 6 && bytes.starts_with(&[0xFF, 0xD8, 0xFF]) && bytes.ends_with(&[0xFF, 0xD9])
}

/// The cached tile, or `None` when there is none yet.
pub(crate) fn read_tile(
    root: &Path,
    version: &str,
    key: &str,
) -> Result<Option<Vec<u8>>, AppError> {
    let path = tile_path(root, version, key)?;
    match fs::read(path) {
        Ok(bytes) if is_jpeg(&bytes) && bytes.len() <= MAX_TILE_BYTES => Ok(Some(bytes)),
        // A damaged file is as good as none: the window draws the plate again.
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Keeps a tile of the current algorithm version; the folders of other versions are removed
/// (a new `DAMAST_VERSION` invalidates every plate). Written through a temporary file, so a
/// half-written tile is never read.
pub(crate) fn write_tile(
    root: &Path,
    version: &str,
    key: &str,
    bytes: &[u8],
) -> Result<(), AppError> {
    let path = tile_path(root, version, key)?;
    if !is_jpeg(bytes) || bytes.len() > MAX_TILE_BYTES {
        return Err(AppError::InvalidApiResponse);
    }
    remove_other_versions(root, version)?;
    let folder = root.join(version);
    fs::create_dir_all(&folder)?;
    let temporary = folder.join(format!("{key}.tmp"));
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, &path)?;
    Ok(())
}

fn remove_other_versions(root: &Path, version: &str) -> Result<(), AppError> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name != version
            && valid_version(&name)
            && entry.file_type().is_ok_and(|kind| kind.is_dir())
        {
            fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

/// "Delete all data": the cached plates go with it.
pub(crate) fn clear_tiles(root: &Path) -> Result<(), AppError> {
    match fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
/// The chunks a picture needs; every other chunk (text, time, profiles, Exif) is dropped.
const IMAGE_CHUNKS: [&[u8; 4]; 5] = [b"IHDR", b"PLTE", b"tRNS", b"IDAT", b"IEND"];

/// The card as it is written: a PNG of exactly 1200×630 with only the image chunks (no text,
/// no metadata), so nothing but the picture leaves the app. The result is decoded once to make
/// sure it is a whole, undamaged image of that size.
pub(crate) fn clean_card_png(bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    if bytes.len() > MAX_CARD_BYTES || !bytes.starts_with(PNG_SIGNATURE) {
        return Err(AppError::InvalidApiResponse);
    }
    let mut clean = PNG_SIGNATURE.to_vec();
    let mut at = PNG_SIGNATURE.len();
    let mut ended = false;
    while at < bytes.len() {
        let header = bytes.get(at..at + 8).ok_or(AppError::InvalidApiResponse)?;
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let end = at
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .filter(|end| *end <= bytes.len())
            .ok_or(AppError::InvalidApiResponse)?;
        let name = &header[4..8];
        if IMAGE_CHUNKS.iter().any(|kept| kept.as_slice() == name) {
            clean.extend_from_slice(&bytes[at..end]);
        }
        at = end;
        if name == b"IEND" {
            ended = true;
            break;
        }
    }
    if !ended {
        return Err(AppError::InvalidApiResponse);
    }
    let decoder = png::Decoder::new(std::io::Cursor::new(&clean));
    let mut reader = decoder
        .read_info()
        .map_err(|_| AppError::InvalidApiResponse)?;
    let (width, height) = (reader.info().width, reader.info().height);
    if (width, height) != (CARD_WIDTH, CARD_HEIGHT) {
        return Err(AppError::InvalidApiResponse);
    }
    let mut pixels = vec![0; reader.output_buffer_size()];
    reader
        .next_frame(&mut pixels)
        .map_err(|_| AppError::InvalidApiResponse)?;
    Ok(clean)
}

/// The file name offered in the save dialog: the session's date (`YYYY-MM-DD`) only.
pub(crate) fn card_file_name(date: &str) -> String {
    let bytes = date.as_bytes();
    let is_date = bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                *byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        });
    if is_date {
        format!("afterbattle-damast-{date}.png")
    } else {
        "afterbattle-damast.png".to_owned()
    }
}

/// The chosen path, with `.png` added when the player typed a name without it.
pub(crate) fn card_path(chosen: PathBuf) -> PathBuf {
    let is_png = chosen
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
    if is_png {
        chosen
    } else {
        let mut name = chosen.into_os_string();
        name.push(".png");
        PathBuf::from(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const VERSION: &str = "damast-1";
    const KEY: &str = "0123456789abcdef0123456789abcdef";

    fn jpeg(size: usize) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
        bytes.resize(size.max(6) - 2, 0x11);
        bytes.extend([0xFF, 0xD9]);
        bytes
    }

    #[test]
    fn a_tile_comes_back_as_it_was_kept() {
        let temp = tempdir().unwrap();
        let root = temp.path().join(DAMAST_DIR);
        assert_eq!(read_tile(&root, VERSION, KEY).unwrap(), None);
        write_tile(&root, VERSION, KEY, &jpeg(4_000)).unwrap();
        assert_eq!(read_tile(&root, VERSION, KEY).unwrap(), Some(jpeg(4_000)));
        assert!(root.join(VERSION).join(format!("{KEY}.jpg")).is_file());
    }

    #[test]
    fn keys_and_versions_never_leave_the_tile_folder() {
        let temp = tempdir().unwrap();
        let root = temp.path().join(DAMAST_DIR);
        for key in [
            "../outside",
            "..",
            "ABCDEF0123456789",
            "0123456789abcde",
            "0123456789abcdef/0",
            "0123456789abcdef.jpg",
            &"a".repeat(65),
            "",
        ] {
            assert!(read_tile(&root, VERSION, key).is_err(), "key {key:?}");
            assert!(
                write_tile(&root, VERSION, key, &jpeg(100)).is_err(),
                "key {key:?}"
            );
        }
        for version in ["..", "../x", "a/b", "A", "", "-x", &"v".repeat(17), "x\\y"] {
            assert!(
                read_tile(&root, version, KEY).is_err(),
                "version {version:?}"
            );
            assert!(
                write_tile(&root, version, KEY, &jpeg(100)).is_err(),
                "version {version:?}"
            );
        }
        assert!(!temp.path().join("outside.jpg").exists());
        assert!(read_tile(&root, "damast-1.2", &"0".repeat(16)).is_ok());
        assert!(read_tile(&root, VERSION, &"f".repeat(64)).is_ok());
    }

    #[test]
    fn only_a_jpeg_of_a_sensible_size_is_kept() {
        let temp = tempdir().unwrap();
        let root = temp.path().join(DAMAST_DIR);
        let png = b"\x89PNG\r\n\x1a\n0000".to_vec();
        assert!(write_tile(&root, VERSION, KEY, &png).is_err());
        assert!(write_tile(&root, VERSION, KEY, &[]).is_err());
        assert!(write_tile(&root, VERSION, KEY, &jpeg(MAX_TILE_BYTES + 1)).is_err());
        let mut unfinished = jpeg(1_000);
        unfinished.truncate(900);
        assert!(write_tile(&root, VERSION, KEY, &unfinished).is_err());
        assert_eq!(read_tile(&root, VERSION, KEY).unwrap(), None);
        write_tile(&root, VERSION, KEY, &jpeg(MAX_TILE_BYTES)).unwrap();
    }

    /// rc.3 mutations (the read guard → `true`, `&&` → `||`): a file under a tile's path that is
    /// not a JPEG, or one too big, is not handed out as a tile.
    #[test]
    fn a_damaged_file_under_the_tile_path_reads_as_no_tile() {
        let temp = tempdir().unwrap();
        let root = temp.path().join(DAMAST_DIR);
        let path = root.join(VERSION).join(format!("{KEY}.jpg"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not a jpeg, only a few bytes").unwrap();
        assert_eq!(read_tile(&root, VERSION, KEY).unwrap(), None);
        fs::write(&path, jpeg(MAX_TILE_BYTES + 1)).unwrap();
        assert_eq!(read_tile(&root, VERSION, KEY).unwrap(), None);
    }

    /// rc.3 mutation (`NotFound` guard → `true` in `read_tile`): only a missing file means "no
    /// tile yet"; any other read error is reported.
    #[test]
    fn a_folder_in_place_of_the_tile_is_an_error() {
        let temp = tempdir().unwrap();
        let root = temp.path().join(DAMAST_DIR);
        fs::create_dir_all(root.join(VERSION).join(format!("{KEY}.jpg"))).unwrap();
        assert!(read_tile(&root, VERSION, KEY).is_err());
    }

    /// rc.3 mutations (`NotFound` guard → `true` in `remove_other_versions` and `clear_tiles`): a
    /// missing tile folder is fine, a file in its place is an error and stays where it is.
    #[test]
    fn a_file_in_place_of_the_tile_folder_is_an_error() {
        let temp = tempdir().unwrap();
        let root = temp.path().join(DAMAST_DIR);
        remove_other_versions(&root, VERSION).unwrap();
        fs::write(&root, b"x").unwrap();
        assert!(remove_other_versions(&root, VERSION).is_err());
        assert!(clear_tiles(&root).is_err());
        assert!(root.is_file());
    }

    #[test]
    fn a_new_algorithm_version_removes_the_old_tiles() {
        let temp = tempdir().unwrap();
        let root = temp.path().join(DAMAST_DIR);
        write_tile(&root, "damast-1", KEY, &jpeg(100)).unwrap();
        // Something else in the folder is not ours to remove.
        fs::write(root.join("note.txt"), b"x").unwrap();
        write_tile(&root, "damast-2", KEY, &jpeg(200)).unwrap();
        assert!(!root.join("damast-1").exists());
        assert_eq!(read_tile(&root, "damast-1", KEY).unwrap(), None);
        assert_eq!(read_tile(&root, "damast-2", KEY).unwrap(), Some(jpeg(200)));
        assert!(root.join("note.txt").exists());
    }

    #[test]
    fn deleting_all_data_removes_the_tiles() {
        let temp = tempdir().unwrap();
        let root = temp.path().join(DAMAST_DIR);
        clear_tiles(&root).unwrap();
        write_tile(&root, VERSION, KEY, &jpeg(100)).unwrap();
        clear_tiles(&root).unwrap();
        assert!(!root.exists());
        assert!(temp.path().exists());
    }

    /// A PNG of the given size whose text chunks carry a synthetic nickname and account id, as a
    /// careless encoder might write them.
    fn card_with_text(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .add_text_chunk("Author".to_owned(), "SyntheticNick_42".to_owned())
                .unwrap();
            encoder
                .add_itxt_chunk("Comment".to_owned(), "account 500123456".to_owned())
                .unwrap();
            encoder
                .add_ztxt_chunk("Source".to_owned(), "SyntheticNick_42".to_owned())
                .unwrap();
            let mut writer = encoder.write_header().unwrap();
            let pixels = (0..width * height * 4)
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>();
            writer.write_image_data(&pixels).unwrap();
        }
        bytes
    }

    fn chunk_names(png: &[u8]) -> Vec<String> {
        let mut names = Vec::new();
        let mut at = 8;
        while at + 8 <= png.len() {
            let length = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            names.push(String::from_utf8_lossy(&png[at + 4..at + 8]).into_owned());
            at += 12 + length;
        }
        names
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn the_card_file_has_no_nickname_no_account_id_and_no_metadata() {
        let source = card_with_text(CARD_WIDTH, CARD_HEIGHT);
        assert!(contains(&source, b"SyntheticNick_42"));
        let clean = clean_card_png(&source).unwrap();
        assert!(!contains(&clean, b"SyntheticNick_42"));
        assert!(!contains(&clean, b"500123456"));
        let names = chunk_names(&clean);
        assert_eq!(names.first().map(String::as_str), Some("IHDR"));
        assert_eq!(names.last().map(String::as_str), Some("IEND"));
        assert!(
            names
                .iter()
                .all(|name| ["IHDR", "IDAT", "IEND"].contains(&name.as_str())),
            "{names:?}"
        );
        // Still the same picture.
        let decoder = png::Decoder::new(std::io::Cursor::new(&clean));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (CARD_WIDTH, CARD_HEIGHT));
        assert_eq!(pixels[0..4], [0, 1, 2, 3]);
    }

    #[test]
    fn only_a_whole_png_of_the_card_size_is_written() {
        assert!(clean_card_png(&card_with_text(1199, 630)).is_err());
        assert!(clean_card_png(&card_with_text(1200, 631)).is_err());
        assert!(clean_card_png(b"not a png").is_err());
        assert!(clean_card_png(&[]).is_err());
        let whole = card_with_text(CARD_WIDTH, CARD_HEIGHT);
        assert!(clean_card_png(&whole[..whole.len() / 2]).is_err());
        let mut broken = whole.clone();
        let middle = broken.len() / 2;
        broken[middle] ^= 0xFF;
        assert!(
            clean_card_png(&broken).is_err(),
            "a damaged image is refused"
        );
        assert!(clean_card_png(&vec![0; MAX_CARD_BYTES + 1]).is_err());
    }

    /// A whole card padded with a private chunk (dropped by the cleaning) to `total` bytes.
    fn padded_card(total: usize) -> Vec<u8> {
        let card = card_with_text(CARD_WIDTH, CARD_HEIGHT);
        // The signature (8) and IHDR (12 + 13) come first; the padding chunk goes after them.
        let (head, tail) = card.split_at(8 + 12 + 13);
        let length = total - card.len() - 12;
        let mut padded = head.to_vec();
        padded.extend((length as u32).to_be_bytes());
        padded.extend(b"zzPd");
        padded.resize(padded.len() + length + 4, 0);
        padded.extend(tail);
        assert_eq!(padded.len(), total);
        padded
    }

    /// rc.3 mutations (`>` → `==` and `>=`, `||` → `&&` in the entry check of `clean_card_png`):
    /// the size limit is inclusive, and a card without the PNG signature is refused even when
    /// its chunks are whole.
    #[test]
    fn the_card_size_limit_is_inclusive_and_the_signature_is_required() {
        let at_limit = clean_card_png(&padded_card(MAX_CARD_BYTES)).unwrap();
        assert!(!contains(&at_limit, b"zzPd"));
        assert!(clean_card_png(&padded_card(MAX_CARD_BYTES + 1)).is_err());
        let mut unsigned = card_with_text(CARD_WIDTH, CARD_HEIGHT);
        unsigned[..8].copy_from_slice(b"NOTAPNG!");
        assert!(clean_card_png(&unsigned).is_err());
    }

    #[test]
    fn the_offered_name_carries_only_the_date_and_png_is_added() {
        assert_eq!(
            card_file_name("2026-11-21"),
            "afterbattle-damast-2026-11-21.png"
        );
        assert_eq!(card_file_name("../../x"), "afterbattle-damast.png");
        assert_eq!(card_file_name("SyntheticNick"), "afterbattle-damast.png");
        // rc.3 mutation (`&&` → `||`): ten characters that are not a date, and a date's shape
        // that is too short, both fall back to the plain name.
        assert_eq!(card_file_name("2026/11/21"), "afterbattle-damast.png");
        assert_eq!(card_file_name("2026-11-2"), "afterbattle-damast.png");
        assert_eq!(
            card_path(PathBuf::from("C:/cards/mine")),
            PathBuf::from("C:/cards/mine.png")
        );
        assert_eq!(
            card_path(PathBuf::from("C:/cards/mine.PNG")),
            PathBuf::from("C:/cards/mine.PNG")
        );
        assert_eq!(
            card_path(PathBuf::from("C:/cards/mine.jpg")),
            PathBuf::from("C:/cards/mine.jpg.png")
        );
    }
}
