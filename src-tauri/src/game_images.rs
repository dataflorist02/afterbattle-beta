//! Vehicle icons and map images read on the fly from the game installation (spec 20, D19;
//! spec 25 §6). Nothing is extracted to disk: the package central directories are indexed in
//! memory, a requested PNG is read from its `Stored` entry with a size limit checked before
//! allocation, and kept only in a RAM LRU keyed by a fingerprint of the client's GUI packages.
//! A missing image is `None`; the window then shows a neutral placeholder.

use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::UNIX_EPOCH,
};

use sha2::{Digest, Sha256};

use crate::map_picture;

const VEHICLE_DIR: &str = "gui/maps/icons/vehicle/";
/// E5-flare §0: map pictures as PNG (500×235, 58 maps on the author's client); maps missing
/// there fall back to the square 300×300 PNG of `map/` (67 maps).
const MAP_DIR: &str = "gui/maps/icons/map/stats/";
const MAP_FALLBACK_DIR: &str = "gui/maps/icons/map/";
/// E5-maps-hd: loading screens, BC1 DDS 2560×1600 (64 files, 57 of the 58 `stats` keys), used
/// when the PNG would have to be scaled up more than [`MAX_UPSCALE`].
const MAP_SCREEN_DIR: &str = "gui/maps/icons/map/screen/";
/// A picture may be shown at most this much larger than its pixels (UI rule, E5-maps-hd §4).
pub(crate) const MAX_UPSCALE: f64 = 1.25;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
/// Largest image accepted (real files: at most 0.33 MB).
pub(crate) const MAX_IMAGE_BYTES: u64 = 1024 * 1024;
/// Upper bound on indexed entries across all GUI packages (real: about 1 300).
const MAX_INDEX_ENTRIES: usize = 20_000;
/// Upper bound on GUI packages considered.
const MAX_PACKAGES: usize = 16;
pub(crate) const DEFAULT_MAX_CACHED_IMAGES: usize = 512;
/// E5-maps-hd: sized pictures (a header at 2560 px is about 4 MB as BMP) share this budget.
pub(crate) const DEFAULT_MAX_CACHED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ImageKind {
    Vehicle,
    Map,
    /// A map's loading screen (DDS); internal, the window asks for `Map` with a size.
    MapScreen,
}

impl ImageKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "vehicle" => Some(Self::Vehicle),
            "map" => Some(Self::Map),
            _ => None,
        }
    }
}

/// Keys are the lowercase file stem: `nation-tag` for vehicles (the stored `vehicle_key`),
/// `NN_name` for maps (the stored `map_key`).
fn valid_key(key: &str) -> bool {
    (1..=128).contains(&key.len())
        && key.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexEntry {
    package: PathBuf,
    entry_name: String,
    /// 0 for the preferred folder, 1 for the map fallback; a lower rank replaces a higher one.
    rank: u8,
}

/// Kind, key and the requested size in device pixels (0×0: the file as it is).
type CacheKey = (ImageKind, String, u32, u32);

#[derive(Default)]
struct Lru {
    images: HashMap<CacheKey, Arc<Vec<u8>>>,
    order: VecDeque<CacheKey>,
    bytes: usize,
    max_images: usize,
    max_bytes: usize,
}

impl Lru {
    fn get(&mut self, key: &CacheKey) -> Option<Arc<Vec<u8>>> {
        let image = self.images.get(key)?.clone();
        self.order.retain(|item| item != key);
        self.order.push_back(key.clone());
        Some(image)
    }

    fn insert(&mut self, key: CacheKey, image: Arc<Vec<u8>>) {
        if image.len() > self.max_bytes || self.max_images == 0 {
            return;
        }
        if let Some(old) = self.images.remove(&key) {
            self.bytes -= old.len();
            self.order.retain(|item| item != &key);
        }
        while self.images.len() >= self.max_images || self.bytes + image.len() > self.max_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(evicted) = self.images.remove(&oldest) {
                self.bytes -= evicted.len();
            }
        }
        self.bytes += image.len();
        self.order.push_back(key.clone());
        self.images.insert(key, image);
    }

    fn clear(&mut self) {
        self.images.clear();
        self.order.clear();
        self.bytes = 0;
    }
}

#[derive(Default)]
struct State {
    root: Option<PathBuf>,
    fingerprint: Option<String>,
    index: HashMap<(ImageKind, String), IndexEntry>,
    cache: Lru,
}

/// Process-wide image store; cheap to clone.
#[derive(Clone)]
pub(crate) struct GameImages {
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for GameImages {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GameImages")
    }
}

impl Default for GameImages {
    fn default() -> Self {
        Self::with_limits(DEFAULT_MAX_CACHED_IMAGES, DEFAULT_MAX_CACHED_BYTES)
    }
}

fn gui_packages(game_root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(game_root.join("res").join("packages")) else {
        return Vec::new();
    };
    let mut packages = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("gui") && name.ends_with(".pkg"))
        })
        .collect::<Vec<_>>();
    packages.sort();
    packages.truncate(MAX_PACKAGES);
    packages
}

/// Client fingerprint: names, sizes and modification times of the GUI packages. A game update
/// changes it, which empties the cache and the index.
fn client_fingerprint(packages: &[PathBuf]) -> String {
    let mut hasher = Sha256::new();
    for package in packages {
        hasher.update(
            package
                .file_name()
                .map(|name| name.as_encoded_bytes())
                .unwrap_or_default(),
        );
        hasher.update([0]);
        if let Ok(metadata) = std::fs::metadata(package) {
            hasher.update(metadata.len().to_le_bytes());
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis())
                .unwrap_or_default();
            hasher.update(modified.to_le_bytes());
        }
    }
    crate::hashing::hex_lower(hasher.finalize())
}

/// Maps a package entry name to its image key, or `None` when it is not a direct child of one
/// of the two image folders.
fn key_and_rank(name: &str) -> Option<((ImageKind, String), u8)> {
    let (kind, rest, rank) = if let Some(rest) = name.strip_prefix(VEHICLE_DIR) {
        (ImageKind::Vehicle, rest, 0)
    } else if let Some(rest) = name.strip_prefix(MAP_DIR) {
        (ImageKind::Map, rest, 0)
    } else if let Some(rest) = name.strip_prefix(MAP_SCREEN_DIR) {
        let key = rest.strip_suffix(".dds")?.to_ascii_lowercase();
        return valid_key(&key).then_some(((ImageKind::MapScreen, key), 0));
    } else if let Some(rest) = name.strip_prefix(MAP_FALLBACK_DIR) {
        (ImageKind::Map, rest, 1)
    } else {
        return None;
    };
    let stem = rest.strip_suffix(".png")?;
    let key = stem.to_ascii_lowercase();
    (!stem.contains('/') && valid_key(&key)).then_some(((kind, key), rank))
}

fn build_index(packages: &[PathBuf]) -> HashMap<(ImageKind, String), IndexEntry> {
    let mut index = HashMap::new();
    for package in packages {
        let Ok(file) = File::open(package) else {
            continue;
        };
        let Ok(archive) = zip::ZipArchive::new(file) else {
            continue;
        };
        for name in archive.file_names() {
            if index.len() >= MAX_INDEX_ENTRIES {
                return index;
            }
            let Ok(name) = name else {
                continue;
            };
            let name = name.into_owned();
            if let Some((key, rank)) = key_and_rank(&name) {
                let better = index
                    .get(&key)
                    .is_none_or(|known: &IndexEntry| rank < known.rank);
                if better {
                    index.insert(
                        key,
                        IndexEntry {
                            package: package.clone(),
                            entry_name: name,
                            rank,
                        },
                    );
                }
            }
        }
    }
    index
}

/// Reads one `Stored` entry of at most `limit` bytes that starts with `signature`; any other
/// method, a declared size above the limit, a size mismatch or another start yields `None`.
fn read_entry(entry: &IndexEntry, limit: u64, signature: &[u8]) -> Option<Vec<u8>> {
    let file = File::open(&entry.package).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    let mut zipped = archive.by_name(&entry.entry_name).ok()?;
    if zipped.compression() != zip::CompressionMethod::Stored {
        return None;
    }
    let declared = zipped.size();
    if declared > limit || declared < signature.len() as u64 {
        return None;
    }
    let mut bytes = Vec::with_capacity(usize::try_from(declared).ok()?);
    zipped
        .by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 == declared && bytes.starts_with(signature)).then_some(bytes)
}

fn read_image(entry: &IndexEntry) -> Option<Vec<u8>> {
    read_entry(entry, MAX_IMAGE_BYTES, &PNG_SIGNATURE)
}

/// Width and height from a PNG's IHDR chunk.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let header = bytes.get(16..24)?;
    let width = u32::from_be_bytes(header[..4].try_into().ok()?);
    let height = u32::from_be_bytes(header[4..].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

/// True when `source` covers a `width`×`height` box scaled up at most [`MAX_UPSCALE`].
fn sharp_enough(source: (u32, u32), width: u32, height: u32) -> bool {
    let scale =
        (f64::from(width) / f64::from(source.0)).max(f64::from(height) / f64::from(source.1));
    scale <= MAX_UPSCALE
}

impl GameImages {
    pub(crate) fn with_limits(max_images: usize, max_bytes: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                cache: Lru {
                    max_images,
                    max_bytes,
                    ..Lru::default()
                },
                ..State::default()
            })),
        }
    }

    /// The state with its index and cache valid for `game_root` (rebuilt when the folder or the
    /// client's GUI packages change).
    fn current(&self, game_root: &Path) -> std::sync::MutexGuard<'_, State> {
        let packages = gui_packages(game_root);
        let fingerprint = client_fingerprint(&packages);
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.root.as_deref() != Some(game_root)
            || state.fingerprint.as_deref() != Some(&fingerprint)
        {
            state.cache.clear();
            state.index = build_index(&packages);
            state.root = Some(game_root.to_path_buf());
            state.fingerprint = Some(fingerprint);
        }
        state
    }

    /// The PNG bytes of one image, or `None` when the folder, package, entry or a valid PNG is
    /// missing.
    pub(crate) fn get(&self, game_root: &Path, kind: ImageKind, key: &str) -> Option<Arc<Vec<u8>>> {
        if !valid_key(key) || kind == ImageKind::MapScreen {
            return None;
        }
        let mut state = self.current(game_root);
        let cache_key = (kind, key.to_owned(), 0, 0);
        if let Some(image) = state.cache.get(&cache_key) {
            return Some(image);
        }
        let entry = state.index.get(&(kind, key.to_owned()))?.clone();
        let image = Arc::new(read_image(&entry)?);
        state.cache.insert(cache_key, image.clone());
        Some(image)
    }

    /// A map picture for a box of `width`×`height` device pixels (E5-maps-hd): the PNG when it
    /// covers the box scaled up at most [`MAX_UPSCALE`], otherwise the loading screen cropped
    /// and scaled down to the box as PNG, otherwise the PNG anyway (the window then shows it
    /// without stretching). A zero size or a vehicle: [`GameImages::get`].
    pub(crate) fn get_sized(
        &self,
        game_root: &Path,
        kind: ImageKind,
        key: &str,
        width: u32,
        height: u32,
    ) -> Option<Arc<Vec<u8>>> {
        if kind != ImageKind::Map || width == 0 || height == 0 {
            return self.get(game_root, kind, key);
        }
        if !valid_key(key) {
            return None;
        }
        let cache_key = (kind, key.to_owned(), width, height);
        let screen = {
            let mut state = self.current(game_root);
            if let Some(image) = state.cache.get(&cache_key) {
                return Some(image);
            }
            state
                .index
                .get(&(ImageKind::MapScreen, key.to_owned()))
                .cloned()
        };
        let png = self.get(game_root, kind, key);
        if let Some(png) = &png
            && png_size(png).is_some_and(|size| sharp_enough(size, width, height))
        {
            return Some(png.clone());
        }
        // Decoding runs without the lock; only the result goes into the cache.
        let picture = screen
            .and_then(|entry| read_entry(&entry, map_picture::MAX_DDS_BYTES, b"DDS "))
            .and_then(|bytes| map_picture::dds_to_png(&bytes, width, height))
            .map(Arc::new);
        match picture {
            Some(picture) => {
                self.current(game_root)
                    .cache
                    .insert(cache_key, picture.clone());
                Some(picture)
            }
            None => png,
        }
    }

    /// Empties the cache and the index ("Delete all local data", a new game folder).
    pub(crate) fn clear(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *state = State {
            cache: Lru {
                max_images: state.cache.max_images,
                max_bytes: state.cache.max_bytes,
                ..Lru::default()
            },
            ..State::default()
        };
    }

    #[cfg(test)]
    fn cached(&self) -> (usize, usize) {
        let state = self.state.lock().unwrap();
        (state.cache.images.len(), state.cache.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_for_entry(name: &str) -> Option<(ImageKind, String)> {
        key_and_rank(name).map(|(key, _)| key)
    }
    use std::io::{Cursor, Write};

    /// A synthetic "PNG": the signature followed by filler. No real game graphics anywhere.
    fn png(size: usize, fill: u8) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.resize(size.max(PNG_SIGNATURE.len()), fill);
        bytes
    }

    fn package(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn game(packages: &[(&str, Vec<u8>)]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("res").join("packages");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes) in packages {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        root
    }

    fn files_under(path: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                out.extend(files_under(&path));
            } else {
                out.push(path);
            }
        }
        out.sort();
        out
    }

    #[test]
    fn vehicle_and_map_images_are_found_case_insensitively_across_packages() {
        let vehicle = png(64, 1);
        let map = png(80, 2);
        let root = game(&[
            (
                "gui-part1.pkg",
                package(&[("gui/maps/icons/vehicle/synth-S01_Test_Tank.png", &vehicle)]),
            ),
            (
                "gui-part2.pkg",
                package(&[("gui/maps/icons/map/stats/99_synthetic_map.png", &map)]),
            ),
        ]);
        let images = GameImages::default();
        assert_eq!(
            images
                .get(root.path(), ImageKind::Vehicle, "synth-s01_test_tank")
                .as_deref(),
            Some(&vehicle)
        );
        assert_eq!(
            images
                .get(root.path(), ImageKind::Map, "99_synthetic_map")
                .as_deref(),
            Some(&map)
        );
        assert_eq!(
            images.get(root.path(), ImageKind::Map, "synth-s01_test_tank"),
            None
        );
    }

    #[test]
    fn missing_folder_entry_or_invalid_key_is_none() {
        let root = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/vehicle/a-b.png", &png(32, 0))]),
        )]);
        let images = GameImages::default();
        assert_eq!(
            images.get(root.path(), ImageKind::Vehicle, "a-missing"),
            None
        );
        assert_eq!(
            images.get(Path::new("Z:/no/such/game"), ImageKind::Vehicle, "a-b"),
            None
        );
        for key in ["", "A-B", "a/b", "..", "a b", &"x".repeat(129)] {
            assert_eq!(
                images.get(root.path(), ImageKind::Vehicle, key),
                None,
                "{key:?}"
            );
        }
        assert!(images.get(root.path(), ImageKind::Vehicle, "a-b").is_some());
    }

    #[test]
    fn map_stats_win_over_the_square_fallback_in_any_package_order() {
        let wide = png(90, 7);
        let square = png(70, 8);
        let only_square = png(60, 9);
        // The square picture comes first, the wide one in a later package: the wide one wins.
        let root = game(&[
            (
                "gui-part1.pkg",
                package(&[("gui/maps/icons/map/01_synth.png", &square)]),
            ),
            (
                "gui-part2.pkg",
                package(&[
                    ("gui/maps/icons/map/stats/01_synth.png", &wide),
                    ("gui/maps/icons/map/02_only.png", &only_square),
                ]),
            ),
        ]);
        let images = GameImages::default();
        assert_eq!(
            images
                .get(root.path(), ImageKind::Map, "01_synth")
                .as_deref(),
            Some(&wide)
        );
        assert_eq!(
            images
                .get(root.path(), ImageKind::Map, "02_only")
                .as_deref(),
            Some(&only_square)
        );
        let reversed = game(&[
            (
                "gui-part1.pkg",
                package(&[("gui/maps/icons/map/stats/01_synth.png", &wide)]),
            ),
            (
                "gui-part2.pkg",
                package(&[("gui/maps/icons/map/01_synth.png", &square)]),
            ),
        ]);
        assert_eq!(
            GameImages::default()
                .get(reversed.path(), ImageKind::Map, "01_synth")
                .as_deref(),
            Some(&wide)
        );
        assert_eq!(
            key_and_rank("gui/maps/icons/map/01_synth.png"),
            Some(((ImageKind::Map, "01_synth".to_owned()), 1))
        );
        assert_eq!(key_and_rank("gui/maps/icons/map/small/01_synth.png"), None);
    }

    #[test]
    fn only_direct_children_of_the_two_folders_are_indexed() {
        let image = png(32, 0);
        let root = game(&[(
            "gui-part1.pkg",
            package(&[
                ("gui/maps/icons/vehicle/contour/a-b.png", &image),
                ("gui/maps/icons/vehicle/a-c.dds", &image),
                ("gui/maps/icons/map/screen/01_map.png", &image),
                ("other/gui/maps/icons/vehicle/a-d.png", &image),
            ]),
        )]);
        let images = GameImages::default();
        for (kind, key) in [
            (ImageKind::Vehicle, "a-b"),
            (ImageKind::Vehicle, "a-c"),
            (ImageKind::Map, "01_map"),
            (ImageKind::Vehicle, "a-d"),
        ] {
            assert_eq!(images.get(root.path(), kind, key), None, "{key}");
        }
    }

    #[test]
    fn non_png_and_oversized_entries_are_none() {
        let too_big = png(usize::try_from(MAX_IMAGE_BYTES).unwrap() + 1, 0);
        let at_limit = png(usize::try_from(MAX_IMAGE_BYTES).unwrap(), 0);
        let root = game(&[(
            "gui-part1.pkg",
            package(&[
                ("gui/maps/icons/vehicle/a-text.png", b"not an image at all"),
                ("gui/maps/icons/vehicle/a-short.png", &PNG_SIGNATURE[..7]),
                ("gui/maps/icons/vehicle/a-big.png", &too_big),
                ("gui/maps/icons/vehicle/a-limit.png", &at_limit),
            ]),
        )]);
        let images = GameImages::default();
        assert_eq!(images.get(root.path(), ImageKind::Vehicle, "a-text"), None);
        assert_eq!(images.get(root.path(), ImageKind::Vehicle, "a-short"), None);
        assert_eq!(images.get(root.path(), ImageKind::Vehicle, "a-big"), None);
        assert_eq!(
            images
                .get(root.path(), ImageKind::Vehicle, "a-limit")
                .map(|image| image.len()),
            Some(at_limit.len())
        );
    }

    #[test]
    fn deflated_entry_is_refused_before_reading() {
        // The zip crate is built without deflate; mark a stored entry as method 8 in both headers.
        let mut bytes = package(&[("gui/maps/icons/vehicle/a-b.png", &png(64, 7))]);
        let mut patched = 0;
        for start in 0..bytes.len().saturating_sub(12) {
            match &bytes[start..start + 4] {
                b"PK\x03\x04" => {
                    bytes[start + 8] = 8;
                    patched += 1;
                }
                b"PK\x01\x02" => {
                    bytes[start + 10] = 8;
                    patched += 1;
                }
                _ => {}
            }
        }
        assert_eq!(patched, 2);
        let root = game(&[("gui-part1.pkg", bytes)]);
        assert_eq!(
            GameImages::default().get(root.path(), ImageKind::Vehicle, "a-b"),
            None
        );
    }

    #[test]
    fn truncated_package_at_every_length_never_panics() {
        let full = package(&[
            ("gui/maps/icons/vehicle/a-b.png", &png(48, 3)),
            ("gui/maps/icons/map/stats/01_m.png", &png(40, 4)),
        ]);
        for length in 0..full.len() {
            let root = game(&[("gui-part1.pkg", full[..length].to_vec())]);
            let images = GameImages::default();
            let _ = images.get(root.path(), ImageKind::Vehicle, "a-b");
            let _ = images.get(root.path(), ImageKind::Map, "01_m");
        }
    }

    #[test]
    fn lru_evicts_the_least_recently_used_by_count_and_by_bytes() {
        let names = ["a-1", "a-2", "a-3"];
        let bytes = names.map(|_| png(100, 9));
        let entries = names
            .iter()
            .zip(&bytes)
            .map(|(name, bytes)| {
                (
                    format!("gui/maps/icons/vehicle/{name}.png"),
                    bytes.as_slice(),
                )
            })
            .collect::<Vec<_>>();
        let refs = entries
            .iter()
            .map(|(name, bytes)| (name.as_str(), *bytes))
            .collect::<Vec<_>>();
        let root = game(&[("gui-part1.pkg", package(&refs))]);

        let by_count = GameImages::with_limits(2, 10_000);
        by_count
            .get(root.path(), ImageKind::Vehicle, "a-1")
            .unwrap();
        by_count
            .get(root.path(), ImageKind::Vehicle, "a-2")
            .unwrap();
        by_count
            .get(root.path(), ImageKind::Vehicle, "a-1")
            .unwrap(); // a-1 is now newest
        by_count
            .get(root.path(), ImageKind::Vehicle, "a-3")
            .unwrap(); // evicts a-2
        assert_eq!(by_count.cached(), (2, 200));
        {
            let state = by_count.state.lock().unwrap();
            assert!(
                state
                    .cache
                    .images
                    .contains_key(&(ImageKind::Vehicle, "a-1".into(), 0, 0))
            );
            assert!(
                !state
                    .cache
                    .images
                    .contains_key(&(ImageKind::Vehicle, "a-2".into(), 0, 0))
            );
        }

        let by_bytes = GameImages::with_limits(10, 250);
        for name in names {
            by_bytes.get(root.path(), ImageKind::Vehicle, name).unwrap();
        }
        assert_eq!(by_bytes.cached(), (2, 200));

        let too_small = GameImages::with_limits(10, 50);
        assert!(
            too_small
                .get(root.path(), ImageKind::Vehicle, "a-1")
                .is_some()
        );
        assert_eq!(too_small.cached(), (0, 0));
    }

    #[test]
    fn a_new_client_fingerprint_empties_the_cache_and_reindexes() {
        let first = png(40, 1);
        let root = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/vehicle/a-b.png", &first)]),
        )]);
        let images = GameImages::default();
        assert_eq!(
            images
                .get(root.path(), ImageKind::Vehicle, "a-b")
                .as_deref(),
            Some(&first)
        );
        // A game update rewrites the package with a different size.
        let second = png(90, 2);
        std::fs::write(
            root.path().join("res/packages/gui-part1.pkg"),
            package(&[("gui/maps/icons/vehicle/a-b.png", &second)]),
        )
        .unwrap();
        assert_eq!(
            images
                .get(root.path(), ImageKind::Vehicle, "a-b")
                .as_deref(),
            Some(&second)
        );
        assert_eq!(images.cached(), (1, 90));
        // An empty game folder: placeholders, and nothing stale is served.
        std::fs::remove_file(root.path().join("res/packages/gui-part1.pkg")).unwrap();
        assert_eq!(images.get(root.path(), ImageKind::Vehicle, "a-b"), None);
        assert_eq!(images.cached(), (0, 0));
    }

    #[test]
    fn clear_empties_memory_and_reading_writes_no_file() {
        let root = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/vehicle/a-b.png", &png(40, 1))]),
        )]);
        let before = files_under(root.path());
        let images = GameImages::default();
        images.get(root.path(), ImageKind::Vehicle, "a-b").unwrap();
        assert_eq!(files_under(root.path()), before);
        images.clear();
        assert_eq!(images.cached(), (0, 0));
        assert!(images.get(root.path(), ImageKind::Vehicle, "a-b").is_some());
        // The limits survive `clear`: the image is cached again.
        assert_eq!(images.cached(), (1, 40));
    }

    #[test]
    fn clear_keeps_the_configured_limits() {
        let names = ["a-1", "a-2", "a-3"];
        let image = png(100, 9);
        let entries = names.map(|name| format!("gui/maps/icons/vehicle/{name}.png"));
        let refs = entries
            .iter()
            .map(|name| (name.as_str(), image.as_slice()))
            .collect::<Vec<_>>();
        let root = game(&[("gui-part1.pkg", package(&refs))]);
        let by_count = GameImages::with_limits(2, 10_000);
        let by_bytes = GameImages::with_limits(10, 250);
        for images in [&by_count, &by_bytes] {
            images.clear();
            for name in names {
                images.get(root.path(), ImageKind::Vehicle, name).unwrap();
            }
            assert_eq!(images.cached(), (2, 200));
        }
    }

    #[test]
    fn size_limits_have_their_documented_values() {
        assert_eq!(MAX_IMAGE_BYTES, 1_048_576);
        assert_eq!(DEFAULT_MAX_CACHED_BYTES, 67_108_864);
        assert_eq!(DEFAULT_MAX_CACHED_IMAGES, 512);
        assert_eq!(MAX_UPSCALE, 1.25);
    }

    /// A synthetic PNG whose IHDR says `width`×`height`.
    fn png_sized(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = png(64, 3);
        bytes[16..20].copy_from_slice(&width.to_be_bytes());
        bytes[20..24].copy_from_slice(&height.to_be_bytes());
        bytes
    }

    fn sized_png(bytes: &[u8]) -> (u32, u32) {
        let picture = crate::map_picture::tests::read_png(bytes);
        (picture.width, picture.height)
    }

    #[test]
    fn loading_screens_are_indexed_as_their_own_kind() {
        assert_eq!(
            key_and_rank("gui/maps/icons/map/screen/01_Synth.dds"),
            Some(((ImageKind::MapScreen, "01_synth".to_owned()), 0))
        );
        for name in [
            "gui/maps/icons/map/screen/01_synth.png",
            "gui/maps/icons/map/screen/sub/01_synth.dds",
            "gui/maps/icons/map/screen/.dds",
            "gui/maps/icons/map/screen/a b.dds",
        ] {
            assert_eq!(key_and_rank(name), None, "{name}");
        }
        assert_eq!(png_size(&png_sized(500, 235)), Some((500, 235)));
        assert_eq!(png_size(&png_sized(0, 235)), None);
        assert_eq!(png_size(&png_sized(500, 0)), None);
        assert_eq!(png_size(&png(23, 0)), None);
        assert!(sharp_enough((500, 235), 625, 100));
        assert!(!sharp_enough((500, 235), 626, 100));
        assert!(sharp_enough((500, 235), 100, 293));
        assert!(!sharp_enough((500, 235), 100, 294));
    }

    #[test]
    fn a_sized_map_comes_from_the_png_when_sharp_enough_otherwise_from_the_loading_screen() {
        use crate::map_picture::tests::flat_dds;
        let stats = png_sized(500, 235);
        let screen = flat_dds(64, 40, 0xf800);
        let mut wrong = flat_dds(64, 40, 0xf800);
        wrong[84..88].copy_from_slice(b"DXT5");
        let root = game(&[(
            "gui-part1.pkg",
            package(&[
                ("gui/maps/icons/map/stats/01_synth.png", &stats),
                ("gui/maps/icons/map/screen/01_synth.dds", &screen),
                ("gui/maps/icons/map/stats/02_only.png", &stats),
                ("gui/maps/icons/map/stats/03_wrong.png", &stats),
                ("gui/maps/icons/map/screen/03_wrong.dds", &wrong),
                ("gui/maps/icons/map/screen/04_screen.dds", &screen),
                ("gui/maps/icons/vehicle/a-b.png", &stats),
            ]),
        )]);
        let images = GameImages::default();
        let get = |key: &str, width: u32, height: u32| {
            images
                .get_sized(root.path(), ImageKind::Map, key, width, height)
                .map(|bytes| bytes.as_ref().clone())
        };
        // Small enough for the PNG (up to 1.25×).
        assert_eq!(get("01_synth", 625, 100), Some(stats.clone()));
        // Larger: the loading screen, cropped and scaled to the box.
        let sharp = get("01_synth", 1_000, 200).unwrap();
        assert_eq!(sized_png(&sharp), (64, 13), "never larger than the source");
        assert_eq!(
            &crate::map_picture::tests::read_png(&sharp).pixels[..3],
            &[255, 0, 0]
        );
        // Second request from the cache.
        let before = images.cached();
        assert_eq!(get("01_synth", 1_000, 200), Some(sharp));
        assert_eq!(images.cached(), before);
        // Without a usable screen: the PNG anyway.
        assert_eq!(get("02_only", 1_000, 200), Some(stats.clone()));
        assert_eq!(get("03_wrong", 1_000, 200), Some(stats.clone()));
        // Only a screen: used for any box.
        assert_eq!(sized_png(&get("04_screen", 1_000, 200).unwrap()), (64, 13));
        assert_eq!(get("05_none", 1_000, 200), None);
        assert_eq!(get("Bad Key", 1_000, 200), None);
        // Zero size or a vehicle: the file as it is.
        assert_eq!(get("01_synth", 0, 200), Some(stats.clone()));
        assert_eq!(get("01_synth", 1_000, 0), Some(stats.clone()));
        assert_eq!(
            images
                .get_sized(root.path(), ImageKind::Vehicle, "a-b", 1_000, 200)
                .as_deref(),
            Some(&stats)
        );
        // The screen itself is never handed out raw.
        assert_eq!(
            images.get(root.path(), ImageKind::MapScreen, "01_synth"),
            None
        );
    }

    #[test]
    fn an_oversized_loading_screen_is_refused_before_reading() {
        use crate::map_picture::tests::flat_dds;
        let mut screen = flat_dds(8, 8, 0xf800);
        screen.resize(
            usize::try_from(crate::map_picture::MAX_DDS_BYTES).unwrap() + 1,
            0,
        );
        let root = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/map/screen/01_big.dds", &screen)]),
        )]);
        let images = GameImages::default();
        assert_eq!(
            images.get_sized(root.path(), ImageKind::Map, "01_big", 8, 8),
            None
        );
        screen.truncate(usize::try_from(crate::map_picture::MAX_DDS_BYTES).unwrap());
        let root = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/map/screen/01_big.dds", &screen)]),
        )]);
        assert!(
            images
                .get_sized(root.path(), ImageKind::Map, "01_big", 8, 8)
                .is_some()
        );
    }

    #[test]
    fn key_pattern_is_checked_on_length_and_on_every_byte() {
        for key in ["a", "a-b_1", "99_synthetic_map", &"x".repeat(128)] {
            assert!(valid_key(key), "{key:?}");
        }
        for key in ["", "A", "a b", "a.b", "a/b", "ż", &"x".repeat(129)] {
            assert!(!valid_key(key), "{key:?}");
        }
    }

    #[test]
    fn entry_present_in_a_package_but_outside_the_key_pattern_is_never_served() {
        let image = png(32, 5);
        let long = "x".repeat(129);
        let long_entry = format!("gui/maps/icons/vehicle/{long}.png");
        let root = game(&[(
            "gui-part1.pkg",
            package(&[
                ("gui/maps/icons/vehicle/a b.png", &image),
                ("gui/maps/icons/vehicle/a.b.png", &image),
                (long_entry.as_str(), &image),
                ("gui/maps/icons/vehicle/a-ok.png", &image),
            ]),
        )]);
        let images = GameImages::default();
        for key in ["a b", "a.b", long.as_str()] {
            assert_eq!(
                images.get(root.path(), ImageKind::Vehicle, key),
                None,
                "{key:?}"
            );
        }
        assert!(
            images
                .get(root.path(), ImageKind::Vehicle, "a-ok")
                .is_some()
        );
    }

    #[test]
    fn second_read_is_served_from_memory_without_touching_the_package() {
        let root = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/vehicle/a-b.png", &png(40, 1))]),
        )]);
        let images = GameImages::default();
        let first = images.get(root.path(), ImageKind::Vehicle, "a-b").unwrap();
        let second = images.get(root.path(), ImageKind::Vehicle, "a-b").unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(images.cached(), (1, 40));
    }

    fn lru(max_images: usize, max_bytes: usize) -> Lru {
        Lru {
            max_images,
            max_bytes,
            ..Lru::default()
        }
    }

    fn lru_key(name: &str) -> CacheKey {
        (ImageKind::Vehicle, name.to_owned(), 0, 0)
    }

    #[test]
    fn lru_reinserting_a_key_replaces_the_image_and_keeps_the_byte_count_exact() {
        let mut cache = lru(10, 1_000);
        cache.insert(lru_key("a-1"), Arc::new(vec![1; 100]));
        cache.insert(lru_key("a-2"), Arc::new(vec![2; 50]));
        cache.insert(lru_key("a-1"), Arc::new(vec![3; 30]));
        assert_eq!(cache.images.len(), 2);
        assert_eq!(cache.bytes, 80);
        assert_eq!(
            cache.order.iter().cloned().collect::<Vec<_>>(),
            vec![lru_key("a-2"), lru_key("a-1")]
        );
        assert_eq!(cache.get(&lru_key("a-1")).as_deref(), Some(&vec![3_u8; 30]));
    }

    #[test]
    fn lru_limits_are_inclusive_and_a_zero_image_limit_caches_nothing() {
        let mut exact = lru(10, 200);
        exact.insert(lru_key("a-1"), Arc::new(vec![1; 200]));
        assert_eq!((exact.images.len(), exact.bytes), (1, 200));
        exact.clear();
        exact.insert(lru_key("a-1"), Arc::new(vec![1; 100]));
        exact.insert(lru_key("a-2"), Arc::new(vec![2; 100]));
        assert_eq!((exact.images.len(), exact.bytes), (2, 200));
        exact.insert(lru_key("a-3"), Arc::new(vec![3; 100]));
        assert_eq!((exact.images.len(), exact.bytes), (2, 200));
        assert!(!exact.images.contains_key(&lru_key("a-1")));
        exact.insert(lru_key("a-4"), Arc::new(vec![4; 201]));
        assert_eq!((exact.images.len(), exact.bytes), (2, 200));
        assert!(!exact.images.contains_key(&lru_key("a-4")));

        let mut none = lru(0, 1_000);
        none.insert(lru_key("a-1"), Arc::new(vec![1; 10]));
        assert_eq!((none.images.len(), none.bytes, none.order.len()), (0, 0, 0));
    }

    #[test]
    fn only_gui_packages_are_read() {
        let image = png(32, 6);
        let root = game(&[
            (
                "scripts.pkg",
                package(&[("gui/maps/icons/vehicle/a-other.png", &image)]),
            ),
            (
                "gui-part1.bak",
                package(&[("gui/maps/icons/vehicle/a-backup.png", &image)]),
            ),
            (
                "gui-part1.pkg",
                package(&[("gui/maps/icons/vehicle/a-gui.png", &image)]),
            ),
        ]);
        let images = GameImages::default();
        assert_eq!(images.get(root.path(), ImageKind::Vehicle, "a-other"), None);
        assert_eq!(
            images.get(root.path(), ImageKind::Vehicle, "a-backup"),
            None
        );
        assert!(
            images
                .get(root.path(), ImageKind::Vehicle, "a-gui")
                .is_some()
        );
    }

    #[test]
    fn image_of_exactly_the_signature_length_is_accepted() {
        let root = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/vehicle/a-min.png", &PNG_SIGNATURE)]),
        )]);
        assert_eq!(
            GameImages::default()
                .get(root.path(), ImageKind::Vehicle, "a-min")
                .as_deref(),
            Some(&PNG_SIGNATURE.to_vec())
        );
    }

    #[test]
    fn index_stops_at_the_entry_limit() {
        let image = png(8, 0);
        let names = (0..MAX_INDEX_ENTRIES)
            .map(|number| format!("gui/maps/icons/vehicle/a-{number}.png"))
            .chain(["gui/maps/icons/vehicle/a-over.png".to_owned()])
            .collect::<Vec<_>>();
        let entries = names
            .iter()
            .map(|name| (name.as_str(), image.as_slice()))
            .collect::<Vec<_>>();
        let root = game(&[("gui-part1.pkg", package(&entries))]);
        let images = GameImages::default();
        let last = format!("a-{}", MAX_INDEX_ENTRIES - 1);
        assert!(images.get(root.path(), ImageKind::Vehicle, &last).is_some());
        assert_eq!(images.get(root.path(), ImageKind::Vehicle, "a-over"), None);
    }

    #[test]
    fn another_game_folder_with_the_same_fingerprint_is_not_served_from_the_old_cache() {
        let first_image = png(40, 1);
        let second_image = png(40, 2);
        let first = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/vehicle/a-b.png", &first_image)]),
        )]);
        let second = game(&[(
            "gui-part1.pkg",
            package(&[("gui/maps/icons/vehicle/a-b.png", &second_image)]),
        )]);
        // Same name, size and modification time: the fingerprints are equal.
        let stamp = UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        for root in [&first, &second] {
            File::options()
                .write(true)
                .open(root.path().join("res/packages/gui-part1.pkg"))
                .unwrap()
                .set_modified(stamp)
                .unwrap();
        }
        assert_eq!(
            client_fingerprint(&gui_packages(first.path())),
            client_fingerprint(&gui_packages(second.path()))
        );
        let images = GameImages::default();
        assert_eq!(
            images
                .get(first.path(), ImageKind::Vehicle, "a-b")
                .as_deref(),
            Some(&first_image)
        );
        assert_eq!(
            images
                .get(second.path(), ImageKind::Vehicle, "a-b")
                .as_deref(),
            Some(&second_image)
        );
    }

    #[test]
    fn entry_names_map_to_keys_only_for_direct_children_with_a_valid_key() {
        assert_eq!(
            key_for_entry("gui/maps/icons/vehicle/Synth-S01_Tank.png"),
            Some((ImageKind::Vehicle, "synth-s01_tank".to_owned()))
        );
        assert_eq!(
            key_for_entry("gui/maps/icons/map/stats/01_map.png"),
            Some((ImageKind::Map, "01_map".to_owned()))
        );
        for name in [
            "gui/maps/icons/vehicle/a b.png",
            "gui/maps/icons/vehicle/a.b.png",
            "gui/maps/icons/vehicle/contour/a-b.png",
            "gui/maps/icons/vehicle/.png",
            "gui/maps/icons/vehicle/a-b.dds",
            "other/a-b.png",
        ] {
            assert_eq!(key_for_entry(name), None, "{name}");
        }
    }

    #[test]
    fn debug_output_names_the_store_without_its_contents() {
        assert_eq!(format!("{:?}", GameImages::default()), "GameImages");
    }

    #[test]
    fn kind_parsing_is_exact() {
        assert_eq!(ImageKind::parse("vehicle"), Some(ImageKind::Vehicle));
        assert_eq!(ImageKind::parse("map"), Some(ImageKind::Map));
        for value in ["", "Vehicle", "maps", "x"] {
            assert_eq!(ImageKind::parse(value), None);
        }
    }

    /// E5-flare §0 reconnaissance on the author's client: map image variants in the GUI
    /// packages, numbers only (directories, counts, formats, compression, sizes, PNG dimensions
    /// from the header). Read-only; nothing is extracted.
    #[test]
    #[ignore = "reads the author's game folder; set AFTERBATTLE_GOLDEN_DIR and run with --ignored --nocapture"]
    fn game_folder_map_images_report() {
        use std::collections::BTreeMap;
        let client = std::env::var_os("AFTERBATTLE_GOLDEN_CLIENT_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("AFTERBATTLE_GOLDEN_DIR")
                    .map(PathBuf::from)
                    .and_then(|replays| replays.parent().map(Path::to_path_buf))
            })
            .expect("set AFTERBATTLE_GOLDEN_DIR or AFTERBATTLE_GOLDEN_CLIENT_DIR");
        #[derive(Default, Debug)]
        struct Folder {
            files: usize,
            by_extension: BTreeMap<String, usize>,
            stored: usize,
            deflated: usize,
            min_bytes: u64,
            max_bytes: u64,
            dimensions: BTreeMap<(u32, u32), usize>,
            dds_formats: BTreeMap<(String, u32), usize>,
            stems: std::collections::BTreeSet<String>,
            samples: Vec<String>,
        }
        let mut folders = BTreeMap::<String, Folder>::new();
        // E5-maps-hd: loading screens decoded and encoded for real boxes (device pixels at
        // 2560 px, DPR 1 and 2): PNG sizes per box, refused files, slowest ms.
        let boxes: [(&str, u32, u32); 8] = [
            ("header", 1792, 448),
            ("moment", 896, 144),
            ("tile", 192, 224),
            ("row", 1792, 64),
            ("header@2", 3520, 880),
            ("moment@2", 1792, 288),
            ("tile@2", 320, 448),
            ("row@2", 3520, 128),
        ];
        let mut sizes = BTreeMap::<&str, Vec<usize>>::new();
        let (mut refused, mut slowest_ms) = (0usize, 0u128);
        let packages = gui_packages(&client);
        for package in &packages {
            let Ok(file) = File::open(package) else {
                continue;
            };
            let Ok(mut archive) = zip::ZipArchive::new(file) else {
                continue;
            };
            for index in 0..archive.len() {
                let Ok(mut entry) = archive.by_index(index) else {
                    continue;
                };
                let Ok(name) = entry.name().map(|name| name.to_ascii_lowercase()) else {
                    continue;
                };
                if !name.starts_with("gui/maps/")
                    || !name.contains("map")
                    || name.contains("/vehicle/")
                {
                    continue;
                }
                let Some((folder, file_name)) = name.rsplit_once('/') else {
                    continue;
                };
                let Some((stem, extension)) = file_name.rsplit_once('.') else {
                    continue;
                };
                if !matches!(extension, "png" | "dds" | "jpg" | "webp") {
                    continue;
                }
                let item = folders.entry(folder.to_owned()).or_insert_with(|| Folder {
                    min_bytes: u64::MAX,
                    ..Folder::default()
                });
                item.files += 1;
                *item.by_extension.entry(extension.to_owned()).or_default() += 1;
                let size = entry.size();
                item.min_bytes = item.min_bytes.min(size);
                item.max_bytes = item.max_bytes.max(size);
                if item.samples.len() < 3 {
                    item.samples.push(stem.to_owned());
                }
                item.stems.insert(stem.to_owned());
                match entry.compression() {
                    zip::CompressionMethod::Stored => item.stored += 1,
                    _ => item.deflated += 1,
                }
                if extension == "png" && entry.compression() == zip::CompressionMethod::Stored {
                    let mut header = [0u8; 24];
                    if entry.read_exact(&mut header).is_ok() && header.starts_with(&PNG_SIGNATURE) {
                        let width = u32::from_be_bytes(header[16..20].try_into().unwrap());
                        let height = u32::from_be_bytes(header[20..24].try_into().unwrap());
                        *item.dimensions.entry((width, height)).or_default() += 1;
                    }
                }
                // E5-maps-hd: DDS header (magic, DDS_HEADER, optional DX10 header), stored only.
                if extension == "dds" && entry.compression() == zip::CompressionMethod::Stored {
                    let mut header = [0u8; 148];
                    let read = entry.read(&mut header).unwrap_or(0);
                    if read >= 128 && header.starts_with(b"DDS ") {
                        let word =
                            |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
                        let (height, width, mips) = (word(12), word(16), word(28));
                        let flags = word(80);
                        let four_cc = &header[84..88];
                        let format = if four_cc == b"DX10" && read >= 148 {
                            format!("DX10/dxgi {}", word(128))
                        } else if flags & 0x4 != 0 {
                            String::from_utf8_lossy(four_cc).into_owned()
                        } else {
                            format!("uncompressed {} bpp", word(88))
                        };
                        *item.dimensions.entry((width, height)).or_default() += 1;
                        *item.dds_formats.entry((format, mips)).or_default() += 1;
                    }
                    if folder == "gui/maps/icons/map/screen" {
                        let mut bytes = header[..read].to_vec();
                        let _ = entry.read_to_end(&mut bytes);
                        for (name, width, height) in boxes {
                            let started = std::time::Instant::now();
                            match crate::map_picture::dds_to_png(&bytes, width, height) {
                                Some(png) => sizes.entry(name).or_default().push(png.len()),
                                None => refused += 1,
                            }
                            slowest_ms = slowest_ms.max(started.elapsed().as_millis());
                        }
                    }
                }
            }
        }
        println!("=== map images in the GUI packages (numbers only) ===");
        println!("gui packages: {}", packages.len());
        for (folder, item) in &folders {
            if item.files < 5 {
                continue;
            }
            let mut dims = item.dimensions.iter().collect::<Vec<_>>();
            dims.sort_by(|a, b| b.1.cmp(a.1));
            dims.truncate(4);
            println!(
                "{folder}: files {}, {:?}, stored {}, other {}, bytes {}..{}, top dims {:?}, sample keys {:?}",
                item.files,
                item.by_extension,
                item.stored,
                item.deflated,
                item.min_bytes,
                item.max_bytes,
                dims,
                item.samples
            );
        }
        let stats = folders
            .get("gui/maps/icons/map/stats")
            .map(|item| item.stems.clone())
            .unwrap_or_default();
        for (folder, item) in &folders {
            if !item.dds_formats.is_empty() {
                println!("{folder}: dds (format, mipmaps) {:?}", item.dds_formats);
            }
            if folder.ends_with("/screen") {
                println!(
                    "{folder}: stems {}, matching map/stats keys {}, map/stats keys without a screen {}",
                    item.stems.len(),
                    item.stems.intersection(&stats).count(),
                    stats.difference(&item.stems).count()
                );
            }
        }
        for (name, width, height) in boxes {
            let mut list = sizes.remove(name).unwrap_or_default();
            list.sort_unstable();
            if let (Some(min), Some(max)) = (list.first(), list.last()) {
                println!(
                    "map/screen as PNG for {name} {width}×{height}: {} files, bytes min {min}, median {}, max {max}",
                    list.len(),
                    list[list.len() / 2]
                );
            }
        }
        println!("map/screen refused (box × file) {refused}, slowest {slowest_ms} ms (test build)");
        println!("=== end ===");
    }
}
