//! Offline native icon cold-disk/decode/LRU observations. No UI or app launch.
use std::cell::Cell;
use std::hint::black_box;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use serde_json::json;
use sha2::{Digest, Sha256};
use velo_slint::core::{icons, indexer::Generation, model::AppItem};
use velo_slint::platform::windows::{icons as native, shell::Apartment};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("RS-12 icon timings require Release".into());
    }
    let output = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("missing new evidence directory")?,
    );
    std::fs::create_dir(&output)?;
    let apartment = Apartment::new()?;
    let windows = PathBuf::from(std::env::var_os("WINDIR").ok_or("missing WINDIR")?);
    let mut items: Vec<_> = ["notepad.exe", "calc.exe", "cmd.exe", "control.exe"]
        .into_iter()
        .map(|name| {
            let path = windows
                .join("System32")
                .join(name)
                .to_string_lossy()
                .into_owned();
            AppItem {
                name: name.into(),
                path: path.clone(),
                exec_path: path,
                source: "System".into(),
                ..Default::default()
            }
        })
        .collect();
    let ticket = Generation::default().ticket();
    icons::assign(&mut items, &ticket)?;
    let mut rows = Vec::new();
    for round in 1..=3 {
        let directory = output.join(format!("round-{round}"));
        let calls = Cell::new(0);
        let start = Instant::now();
        let pixels: Vec<_> = items
            .iter()
            .map(|item| {
                icons::load(&directory, item, &ticket, |item| {
                    calls.set(calls.get() + 1);
                    native::extract(&apartment, item)
                })
            })
            .collect::<Result<_, _>>()?;
        let cold = start.elapsed().as_nanos() as u64;
        if calls.get() != items.len() {
            return Err("native extraction count mismatch".into());
        }
        let hashes: Vec<_> = pixels
            .iter()
            .map(|pixels| format!("{:x}", Sha256::digest(&pixels.rgba)))
            .collect();
        let urls: Vec<_> = items.iter().map(|item| item.icon_url.clone()).collect();
        if urls.iter().collect::<std::collections::BTreeSet<_>>().len() != items.len() {
            return Err("icon cache keys are not unique".into());
        }
        let decoded: usize = pixels.iter().map(|pixels| pixels.rgba.len()).sum();
        let disk: u64 = std::fs::read_dir(directory.join("cache/icons"))?
            .map(|entry| Ok(entry?.metadata()?.len()))
            .sum::<std::io::Result<_>>()?;
        rows.push(json!({"round":round,"phase":"empty-application-disk-cache-extract-encode-flush","os_file_cache_cold":false,"items":items.len(),"elapsed_nanoseconds":cold,"extractions":calls.get(),"disk_png_bytes":disk,"decoded_rgba_bytes":decoded,"pixel_hashes":hashes,"icon_urls":urls}));
        let start = Instant::now();
        let hit: Vec<_> = items
            .iter()
            .map(|item| {
                icons::load(&directory, item, &ticket, |item| {
                    calls.set(calls.get() + 1);
                    native::extract(&apartment, item)
                })
            })
            .collect::<Result<_, _>>()?;
        let disk_hit = start.elapsed().as_nanos() as u64;
        if calls.get() != items.len() {
            return Err("cache hit unexpectedly extracted an icon".into());
        }
        if hit
            .iter()
            .zip(&pixels)
            .any(|(a, b)| a.rgba != b.rgba || a.width != b.width || a.height != b.height)
        {
            return Err("cached PNG pixels differ from first extraction".into());
        }
        rows.push(json!({"round":round,"phase":"existing-disk-cache-decode","items":items.len(),"elapsed_nanoseconds":disk_hit,"extractions":0,"disk_png_bytes":disk,"decoded_rgba_bytes":decoded,"pixel_hashes":hashes,"icon_urls":urls}));
        let mut lru = icons::Lru::new(icons::BUDGET - icons::TRANSFER_BUDGET);
        for (item, pixels) in items.iter().zip(hit) {
            let cost = pixels.cost();
            if !lru.insert(item.icon_url.clone(), Rc::new(pixels), cost) {
                return Err("bounded icon LRU rejected fixture".into());
            }
        }
        let start = Instant::now();
        for _ in 0..256 {
            for item in &items {
                black_box(lru.get(&item.icon_url).ok_or("LRU cache miss")?);
            }
        }
        rows.push(json!({"round":round,"phase":"decoded-lru-clone","items":items.len(),"iterations":256,"elapsed_nanoseconds":start.elapsed().as_nanos() as u64,"extractions":0,"decoded_rgba_bytes":decoded,"application_cpu_texture_estimate_bytes":lru.bytes(),"renderer_internal_bytes":null}));
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("rust-icons.json"))?;
    serde_json::to_writer_pretty(file, &rows)?;
    println!("RS-12 Rust icons: three empty-disk, disk-hit and bounded-LRU rounds passed.");
    Ok(())
}
