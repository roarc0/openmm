/// Dump MM6 save files (.mm6 LOD archives) to data/dump/saves/{slot}/.
///
/// Scans two locations:
///   - OPENMM_PATH_MM6/../Saves/  (original MM6 saves, default: data/mm6/Saves/)
///   - ./saves/                  (OpenMM saves)
///
/// For each save, outputs:
///   data/dump/saves/{slot}/info.json       — save header (name, map)
///   data/dump/saves/{slot}/screenshot.png  — decoded screenshot
///   data/dump/saves/{slot}/{file}          — raw bytes of every file in the LOD
use std::fs;
use std::path::{Path, PathBuf};

use log::{error, info, warn};
use openmm_data::save::{SaveFile, list_saves};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let out_root = Path::new("data/dump/Saves");
    fs::create_dir_all(out_root).expect("failed to create data/dump/Saves");

    let scan_dirs = candidate_save_dirs();
    if scan_dirs.is_empty() {
        error!("No save directories found.");
        return;
    }

    let mut total = 0usize;
    for dir in &scan_dirs {
        let saves = list_saves(dir);
        if saves.is_empty() {
            info!("  (no .mm6 saves in {})", dir.display());
            continue;
        }
        for save in saves {
            dump_save(&save, out_root);
            total += 1;
        }
    }
    info!("Dumped {} save(s) to {}/", total, out_root.display());
}

fn dump_save(save: &SaveFile, out_root: &Path) {
    let slot_dir = out_root.join(&save.slot);
    fs::create_dir_all(&slot_dir).expect("failed to create slot dir");

    // --- info.json ---
    let header = save.header();
    let raw_hex = header.raw.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let info = format!(
        "{{\n  \"slot\": \"{}\",\n  \"save_name\": \"{}\",\n  \"map_name\": \"{}\",\n  \"playing_time\": {},\n  \"raw_hex\": \"{}\"\n}}\n",
        save.slot,
        header.save_name.replace('"', "\\\""),
        header.map_name.replace('"', "\\\""),
        header.playing_time,
        raw_hex,
    );
    let info_path = slot_dir.join("info.json");
    if let Err(e) = fs::write(&info_path, info) {
        error!("failed to write {}: {}", info_path.display(), e);
    }

    // --- screenshot.png ---
    if let Some(img) = save.screenshot() {
        let png_path = slot_dir.join("screenshot.png");
        if let Err(e) = img.save(&png_path) {
            error!("screenshot save failed for {}: {}", save.slot, e);
        }
    } else {
        warn!("no screenshot in {}", save.slot);
    }

    // --- raw files ---
    for name in save.list_files() {
        if let Some(data) = save.get_file(&name) {
            let out = slot_dir.join(&name);
            // Create subdirs if the name contains path separators (shouldn't, but be safe)
            if let Some(parent) = out.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                error!("failed to create parent dir for {}: {}", out.display(), e);
            }
            if let Err(e) = fs::write(&out, data) {
                error!("failed to write file {}: {}", out.display(), e);
            } else {
                info!("    extracted {}", name);
            }
        }
    }

    let total_minutes = (header.playing_time / 7680) as u64;
    let formatted_time = openmm_data::utils::time::format(total_minutes);

    info!(
        "  {} — map: {:?}  name: {:?}  time: {}  ({} files)",
        save.slot,
        header.map_name,
        header.save_name,
        formatted_time,
        save.list_files().len(),
    );
}

/// Collect directories to scan for .mm6 files.
fn candidate_save_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Original MM6 saves: derive from OPENMM_PATH_MM6 or default data path
    let data_path = openmm_data::get_data_path(); // e.g. "data/mm6/data"
    let data_path = PathBuf::from(&data_path);
    // data/mm6/data -> data/mm6/Saves
    if let Some(game_root) = data_path.parent() {
        let saves = game_root.join("Saves");
        if saves.is_dir() {
            info!("Scanning original MM6 saves: {}", saves.display());
            dirs.push(saves);
        }
    }

    // OpenMM saves in ./Saves/
    let openmm_saves = PathBuf::from("Saves");
    if openmm_saves.is_dir() {
        info!("Scanning OpenMM saves: {}", openmm_saves.display());
        dirs.push(openmm_saves);
    }

    dirs
}
