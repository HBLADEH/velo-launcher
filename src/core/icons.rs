//! Disk PNGs are reconstructible; decoded pixels and estimated textures share a
//! separate 8 MiB UI budget. Loading/extraction never happens during search.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::Path;

use image::{ImageDecoder, ImageEncoder};

use super::indexer::{Ticket, modified_ns};
use super::model::{AppItem, cached_icon_path, identity};
use super::{DataError, Result};

pub const BUDGET: usize = 8 * 1024 * 1024;
// Reserve for one 96-icon result batch, PNG decoder and Shell scratch buffers.
pub const TRANSFER_BUDGET: usize = 4 * 1024 * 1024;
pub const SIZE: u32 = 64;

#[derive(Clone)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Pixels {
    pub fn valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= 256
            && self.height <= 256
            && self.rgba.len() == self.width as usize * self.height as usize * 4
    }
    pub fn cost(&self) -> usize {
        // One straight RGBA buffer plus a conservative RGBA texture estimate.
        // Renderer-private caches are outside application ownership, measured later.
        self.rgba.len() * 2
    }
}

pub fn signature(item: &AppItem) -> String {
    if item.is_folder() {
        return String::new();
    }
    let icon = item
        .icon_path
        .split(',')
        .next()
        .unwrap_or("")
        .trim_matches('"');
    let mut stamp = String::new();
    for part in ["2", &item.path, &item.exec_path, icon] {
        stamp.push_str(part);
        if let Ok(metadata) = fs::metadata(part) {
            stamp.push_str(&format!("|{}|{}", metadata.len(), modified_ns(&metadata)));
        }
    }
    format!("/icons/{}.png", identity(&stamp, ""))
}

pub fn assign(items: &mut [AppItem], ticket: &Ticket) -> Result<()> {
    for item in items {
        ticket.check()?;
        item.icon_url = signature(item);
    }
    Ok(())
}

fn decode(path: &Path) -> Result<Pixels> {
    let file = fs::File::open(path).map_err(|e| DataError::new("open icon cache", e))?;
    let mut decoder = image::codecs::png::PngDecoder::new(std::io::BufReader::new(file))
        .map_err(|e| DataError::new("decode icon cache", e))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(256);
    limits.max_image_height = Some(256);
    limits.max_alloc = Some(1024 * 1024);
    decoder
        .set_limits(limits)
        .map_err(|e| DataError::new("limit icon dimensions", e))?;
    let image = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| DataError::new("read icon pixels", e))?
        .thumbnail(SIZE, SIZE)
        .into_rgba8();
    Ok(Pixels {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    })
}

pub fn load(
    directory: &Path,
    item: &AppItem,
    ticket: &Ticket,
    extract: impl FnOnce(&AppItem) -> Result<Pixels>,
) -> Result<Pixels> {
    ticket.check()?;
    let url = if item.icon_url.is_empty() {
        signature(item)
    } else {
        item.icon_url.clone()
    };
    let path = cached_icon_path(directory, &url)
        .ok_or_else(|| DataError::new("load icon", "invalid cache URL"))?;
    if let Ok(pixels) = decode(&path) {
        ticket.check()?;
        return Ok(pixels);
    }
    let mut pixels = extract(item)?;
    if !pixels.valid() {
        return Err(DataError::new("extract icon", "invalid dimensions"));
    }
    if pixels.width > SIZE || pixels.height > SIZE {
        let image = image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.rgba)
            .ok_or_else(|| DataError::new("resize icon", "invalid pixels"))?;
        let image = image::imageops::thumbnail(&image, SIZE, SIZE);
        pixels = Pixels {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        };
    }
    ticket.check()?;
    let parent = path
        .parent()
        .ok_or_else(|| DataError::new("write icon", "missing directory"))?;
    fs::create_dir_all(parent).map_err(|e| DataError::new("create icon directory", e))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".icon-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|e| DataError::new("create icon file", e))?;
    image::codecs::png::PngEncoder::new_with_quality(
        &mut temporary,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    )
    .write_image(
        &pixels.rgba,
        pixels.width,
        pixels.height,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| DataError::new("encode icon", e))?;
    temporary
        .flush()
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|e| DataError::new("flush icon", e))?;
    ticket.check()?;
    temporary
        .persist(&path)
        .map_err(|e| DataError::new("replace icon", e.error))?;
    Ok(pixels)
}

/// Call only after index persistence succeeds. Retain current/previous indexes,
/// custom apps and pins; never remove files outside the generated namespace.
pub fn prune(directory: &Path, retained: impl Iterator<Item = String>) -> Result<()> {
    let used: BTreeSet<_> = retained.collect();
    let root = directory.join("cache/icons");
    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(DataError::new("read icon directory", error)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| DataError::new("read icon entry", e))?;
        let url = format!("/icons/{}", entry.file_name().to_string_lossy());
        if cached_icon_path(directory, &url).is_some()
            && !used.contains(&url)
            && entry.file_type().is_ok_and(|t| t.is_file())
        {
            fs::remove_file(entry.path()).map_err(|e| DataError::new("prune icon", e))?;
        }
    }
    Ok(())
}

/// Values must not be copied into offscreen models: removing a cache entry
/// alone cannot release pixels/textures still referenced by the UI.
pub struct Lru<T> {
    entries: BTreeMap<String, (T, usize, u64)>,
    bytes: usize,
    clock: u64,
    budget: usize,
    protected: BTreeSet<String>,
}

impl<T: Clone> Lru<T> {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            bytes: 0,
            clock: 0,
            budget,
            protected: BTreeSet::new(),
        }
    }
    pub fn protect(&mut self, keys: BTreeSet<String>) {
        self.protected = keys;
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get(&mut self, key: &str) -> Option<T> {
        self.clock += 1;
        let (value, _, used) = self.entries.get_mut(key)?;
        *used = self.clock;
        Some(value.clone())
    }
    pub fn insert(&mut self, key: String, value: T, cost: usize) -> bool {
        if cost > self.budget {
            return false;
        }
        let old_cost = self.entries.get(&key).map_or(0, |(_, cost, _)| *cost);
        let evictable: usize = self
            .entries
            .iter()
            .filter(|(current, _)| *current != &key && !self.protected.contains(*current))
            .map(|(_, (_, cost, _))| *cost)
            .sum();
        if self.bytes - old_cost + cost > self.budget + evictable {
            return false;
        }
        if let Some((_, old, _)) = self.entries.remove(&key) {
            self.bytes -= old;
        }
        while self.bytes + cost > self.budget {
            let Some(oldest) = self
                .entries
                .iter()
                .filter(|(key, _)| !self.protected.contains(*key))
                .min_by_key(|(_, (_, _, used))| used)
                .map(|(key, _)| key.clone())
            else {
                return false;
            };
            if let Some((_, bytes, _)) = self.entries.remove(&oldest) {
                self.bytes -= bytes;
            }
        }
        self.clock += 1;
        self.bytes += cost;
        self.entries.insert(key, (value, cost, self.clock));
        true
    }
}
