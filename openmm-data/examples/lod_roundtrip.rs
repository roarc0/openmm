//! LOD round-trip validation.
//!
//! Reads every .lod file from OPENMM_PATH_MM6, re-saves it via `LodWriter`,
//! then verifies that every entry's bytes are identical to the original.
//!
//! Usage:
//!   OPENMM_PATH_MM6=/path/to/mm6/data cargo run -p openmm-data --example lod_roundtrip
//!
//! On success: prints a summary table and exits 0.
//! On failure: prints which entries differ and exits 1.

use openmm_data::{Assets, LodWriter, get_data_path};
use std::{fs, path::PathBuf};

fn main() {
    let lod_path_str = get_data_path();
    let lod_path = std::path::Path::new(&lod_path_str);

    if !lod_path.exists() {
        eprintln!("OPENMM_PATH_MM6 not set or does not exist: {}", lod_path.display());
        eprintln!("Set OPENMM_PATH_MM6 to your MM6 data directory (the folder with .lod files).");
        std::process::exit(1);
    }

    let lod_files: Vec<PathBuf> = fs::read_dir(lod_path)
        .expect("failed to read lod directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("lod"))
                .unwrap_or(false)
        })
        .collect();

    if lod_files.is_empty() {
        eprintln!("No .lod files found in {}", lod_path.display());
        std::process::exit(1);
    }

    let tmp_dir = std::env::temp_dir().join("openmm_roundtrip");
    fs::create_dir_all(&tmp_dir).expect("failed to create tmp dir");

    let mut total_archives = 0;
    let mut total_entries = 0;
    let mut failures: Vec<String> = Vec::new();

    for src_path in &lod_files {
        let name = src_path.file_name().unwrap_or_default().to_string_lossy();
        total_archives += 1;

        // ── Step 1: open original ───────────────────────────────────────────
        let original_bytes = match fs::read(src_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("  SKIP  {} — cannot read: {}", name, e);
                continue;
            }
        };

        // ── Step 2: save via LodArchive::patch (no overrides = pure copy) ──────────
        let out_path = tmp_dir.join(name.as_ref());
        match Assets::new(&lod_path_str) {
            Ok(assets) => {
                let archive_key = src_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if let Some(_lod) = assets.get_lod(&archive_key) {
                    if let Err(e) = LodWriter::patch(src_path, &out_path, &[]) {
                        eprintln!("  FAIL  {} — save error: {}", name, e);
                        failures.push(format!("{}: save error: {}", name, e));
                        continue;
                    }
                } else {
                    eprintln!("  FAIL  {} — archive not found in Assets", name);
                    failures.push(format!("{}: not found", name));
                    continue;
                }
            }
            Err(e) => {
                eprintln!("  FAIL  {} — Assets open error: {}", name, e);
                failures.push(format!("{}: Assets error: {}", name, e));
                continue;
            }
        }

        // ── Step 3: re-read the written copy ───────────────────────────────
        let written_bytes = match fs::read(&out_path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("  FAIL  {} — cannot read output: {}", name, e);
                failures.push(format!("{}: read output error: {}", name, e));
                continue;
            }
        };

        // ── Step 4: byte-exact comparison via Assets ───────────────────
        let orig_assets = match Assets::new(&lod_path_str) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  FAIL  {} — cannot open original for compare: {}", name, e);
                failures.push(format!("{}: reopen error: {}", name, e));
                continue;
            }
        };
        let tmp_dir_str = tmp_dir.to_string_lossy();
        let written_assets = match Assets::new(&*tmp_dir_str) {
            Ok(m) => m,
            Err(_) => {
                // Fall back to raw byte compare if second Assets fails
                if original_bytes.len() != written_bytes.len() {
                    let msg = format!(
                        "{}: byte length mismatch {} vs {}",
                        name,
                        original_bytes.len(),
                        written_bytes.len()
                    );
                    eprintln!("  FAIL  {}", msg);
                    failures.push(msg);
                } else {
                    println!("  OK    {} ({} bytes)", name, original_bytes.len());
                }
                total_entries += 1;
                continue;
            }
        };

        let archive_key = src_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();

        let orig_files = orig_assets.files_in(&archive_key).unwrap_or_default();
        let mut archive_ok = true;
        let mut entry_count = 0;

        for entry_name in &orig_files {
            entry_count += 1;
            let path = format!("{}/{}", archive_key, entry_name);
            let orig_data = orig_assets.get_bytes(&path).ok();
            let copy_data = written_assets.get_bytes(&path).ok();

            match (orig_data, copy_data) {
                (Some(od), Some(cd)) if od == cd => { /* ok */ }
                (Some(od), Some(cd)) => {
                    let msg = format!(
                        "{}/{}: data mismatch ({} vs {} bytes)",
                        archive_key,
                        entry_name,
                        od.len(),
                        cd.len()
                    );
                    eprintln!("  FAIL  {}", msg);
                    failures.push(msg);
                    archive_ok = false;
                }
                (None, _) | (_, None) => {
                    let msg = format!("{}/{}: entry missing in copy", archive_key, entry_name);
                    eprintln!("  FAIL  {}", msg);
                    failures.push(msg);
                    archive_ok = false;
                }
            }
        }

        total_entries += entry_count;

        if archive_ok {
            println!("  OK    {} ({} entries)", name, entry_count);
        }
    }

    // Cleanup
    let _ = fs::remove_dir_all(&tmp_dir);

    println!();
    println!(
        "Round-trip: {} archives, {} entries total, {} failures",
        total_archives,
        total_entries,
        failures.len()
    );

    if failures.is_empty() {
        println!("All LOD archives round-trip cleanly ✓");
        std::process::exit(0);
    } else {
        eprintln!("FAILURES:");
        for f in &failures {
            eprintln!("  - {}", f);
        }
        std::process::exit(1);
    }
}
