//! Thumbnails: decoding with limits, a disk cache keyed by file identity, and pruning of the cache.
//!
//! The UI calls these on worker threads only. A thumbnail is made once per file (path, size, time and edge) and kept as
//! a small JPEG in the cache folder, so the next start reads a few KB instead of decoding a full screenshot.

use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageEncoder, ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};

use super::Shot;
use crate::{Error, Result, json_file};

/// The longest side of a grid thumbnail, in pixels (about 2x the cell of a 175 % screen).
pub const THUMB_EDGE: u32 = 384;
/// The longest side of the picture in the large view; larger screenshots are scaled down to it.
pub const VIEW_EDGE: u32 = 2560;
/// Files above this size are not decoded.
pub const MAX_FILE: u64 = 128 << 20;
/// Widest or tallest image that is decoded.
const MAX_SIDE: u32 = 12_000;
/// Memory the decoder may allocate for one image.
const MAX_ALLOC: u64 = 512 << 20;
/// The thumbnail cache keeps at most this many bytes (about 8,000 thumbnails).
pub const CACHE_LIMIT: u64 = 96 << 20;
const JPEG_QUALITY: u8 = 85;
const CACHE_EXT: &str = "jpg";

/// Decoded pixels, RGBA, rows top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Pixels {
    fn from_image(image: &DynamicImage) -> Self {
        let rgba = image.to_rgba8();
        Self { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() }
    }
}

/// The cache key of a thumbnail: a hash of the file's path, size and time and the edge, so a changed or renamed file
/// gets a new thumbnail and the old one is pruned later.
pub fn cache_key(shot: &Shot, edge: u32) -> String {
    let mut hash = Sha256::new();
    hash.update(format!("v1|{}|{}|{}|{edge}", shot.path.display(), shot.size, shot.modified_ms));
    hash.finalize().iter().take(16).map(|b| format!("{b:02x}")).collect()
}

fn cache_file(cache: &Path, key: &str) -> PathBuf {
    cache.join(format!("{key}.{CACHE_EXT}"))
}

/// Decodes an image file within the limits; a file that is too big, not an image or damaged is an error.
fn decode(path: &Path) -> Result<DynamicImage> {
    let size = fs::metadata(path).map_err(|e| Error::io(path, e))?.len();
    if size > MAX_FILE {
        return Err(Error::refused(format!("{}: the file is larger than {} MB", path.display(), MAX_FILE >> 20)));
    }
    if size == 0 {
        return Err(Error::invalid(format!("{}: the file is empty", path.display())));
    }
    let file = fs::File::open(path).map_err(|e| Error::io(path, e))?;
    let mut reader = ImageReader::new(BufReader::new(file)).with_guessed_format().map_err(|e| Error::io(path, e))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    reader.decode().map_err(|e| Error::invalid(format!("{}: {e}", path.display())))
}

/// `image` scaled down to fit `edge` on its longest side; a smaller one is kept as it is.
fn fit(image: DynamicImage, edge: u32) -> DynamicImage {
    if image.width() <= edge && image.height() <= edge { image } else { image.thumbnail(edge, edge) }
}

fn encode_jpeg(image: &DynamicImage) -> Result<Vec<u8>> {
    let rgb = image.to_rgb8();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| Error::invalid(format!("thumbnail: {e}")))?;
    Ok(bytes)
}

/// The grid thumbnail of `shot`: read from the cache when it is there, else decoded from the file, scaled and cached.
/// A failure to write the cache is not an error. Call it from a worker thread.
pub fn load_thumbnail(cache: &Path, shot: &Shot) -> Result<Pixels> {
    let file = cache_file(cache, &cache_key(shot, THUMB_EDGE));
    if let Ok(bytes) = fs::read(&file)
        && let Ok(image) = image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg)
    {
        return Ok(Pixels::from_image(&image));
    }
    let thumb = fit(decode(&shot.path)?, THUMB_EDGE);
    if let Ok(bytes) = encode_jpeg(&thumb) {
        let _ = json_file::write_atomic(&file, &bytes);
    }
    Ok(Pixels::from_image(&thumb))
}

/// The picture of a file for the large view, scaled down to `VIEW_EDGE` on its longest side.
pub fn load_view(path: &Path) -> Result<Pixels> {
    Ok(Pixels::from_image(&fit(decode(path)?, VIEW_EDGE)))
}

/// The picture of a file at its real size, for the clipboard.
pub fn load_full(path: &Path) -> Result<Pixels> {
    Ok(Pixels::from_image(&decode(path)?))
}

/// Cuts the cache down: removes every cached thumbnail whose key is not in `live` (a deleted, renamed or changed file),
/// then the oldest ones while the folder is over `limit` bytes. Returns how many files were removed.
pub fn prune_cache(cache: &Path, live: &HashSet<String>, limit: u64) -> usize {
    let Ok(entries) = fs::read_dir(cache) else { return 0 };
    let mut kept = Vec::new();
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned();
        if path.extension().and_then(|e| e.to_str()) != Some(CACHE_EXT) {
            continue;
        }
        if !live.contains(&stem) {
            removed += usize::from(fs::remove_file(&path).is_ok());
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            kept.push((meta.modified().ok(), meta.len(), path));
        }
    }
    let mut total: u64 = kept.iter().map(|(_, len, _)| len).sum();
    kept.sort_by_key(|(time, _, _)| *time);
    for (_, len, path) in kept {
        if total <= limit {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total -= len;
            removed += 1;
        }
    }
    removed
}

/// The keys of the cached thumbnails of these shots.
pub fn live_keys(shots: &[Shot]) -> HashSet<String> {
    shots.iter().map(|s| cache_key(s, THUMB_EDGE)).collect()
}

/// Whether a thumbnail of `shot` is already in the cache.
pub fn is_cached(cache: &Path, shot: &Shot) -> bool {
    cache_file(cache, &cache_key(shot, THUMB_EDGE)).is_file()
}

/// The decoder for a cached or fresh JPEG is the same code; this reads a cache file back (for tests and diagnostics).
pub fn read_cached(cache: &Path, shot: &Shot) -> Option<Pixels> {
    let bytes = fs::read(cache_file(cache, &cache_key(shot, THUMB_EDGE))).ok()?;
    let image = image::ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg).decode().ok()?;
    Some(Pixels::from_image(&image))
}
