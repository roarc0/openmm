/// Dump all embedded SMK files from MM6 Smacker archives (usually .vid) to data/dump/smk/.
///
/// Output:
///   data/dump/smk/{name}.smk        — raw Smacker video bytes
///   data/dump/smk/index.txt         — human-readable index with SMK metadata
use std::fs;
use std::path::{Path, PathBuf};

use log::{error, info};
use openmm_data::assets::provider::archive::Archive;
use openmm_data::assets::{SmkArchive, parse_smk_info};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let data_path = PathBuf::from(openmm_data::get_data_path());
    
    // Anims directory is usually a sibling to Data (where .lod files are)
    let anims_dir = if data_path.join("Anims").is_dir() {
        data_path.join("Anims")
    } else if let Some(parent) = data_path.parent() && parent.join("Anims").is_dir() {
        parent.join("Anims")
    } else if Path::new("data/mm6/Anims").is_dir() {
        PathBuf::from("data/mm6/Anims")
    } else {
        data_path.join("Anims") // Fallback to original behavior for error reporting
    };

    info!("Searching for Smacker archives in {}", anims_dir.display());

    let out_root = Path::new("data/dump/anims");
    fs::create_dir_all(out_root).expect("failed to create data/dump/anims");

    let smk_archives = ["Anims1.vid", "Anims2.vid"];
    let mut index_lines: Vec<String> = Vec::new();
    index_lines.push(format!(
        "{:<20} {:>6} {:>5} {:>8}  {}",
        "name", "frames", "fps", "size", "resolution"
    ));
    index_lines.push("-".repeat(60));

    let mut total = 0usize;

    for archive_name in &smk_archives {
        let archive_path = anims_dir.join(archive_name);
        if !archive_path.exists() {
            error!("skipping {archive_name}: not found at {}", archive_path.display());
            continue;
        }

        let archive = SmkArchive::open(&archive_path).unwrap_or_else(|e| panic!("failed to open {archive_name}: {e}"));
        let entries = archive.list_files();
        info!("{archive_name}: {} entries", entries.len());
        index_lines.push(format!("\n# {archive_name}"));

        let arch_out_dir = out_root.join(archive_name.strip_suffix(".vid").unwrap_or(archive_name).to_lowercase());
        let _ = fs::create_dir_all(&arch_out_dir);

        for entry in entries {
            let smk_bytes = match archive.get_file(&entry.name) {
                Some(b) => b,
                None => {
                    error!("  skipping {}: no data", entry.name);
                    continue;
                }
            };
            let out_path = arch_out_dir.join(format!("{}.smk", entry.name));
            if let Err(e) = fs::write(&out_path, &smk_bytes) {
                error!("write {}: {e}", out_path.display());
                continue;
            }

            let info_str = if let Some(info) = parse_smk_info(&smk_bytes) {
                format!(
                    "{:<20} {:>6} {:>5.1} {:>8}  {}x{}",
                    entry.name,
                    info.frames,
                    info.fps(),
                    entry.size,
                    info.width,
                    info.height,
                )
            } else {
                format!(
                    "{:<20} {:>6} {:>5} {:>8}  (no SMK header)",
                    entry.name, "-", "-", entry.size
                )
            };

            info!("  {info_str}");
            index_lines.push(info_str);

            total += 1;
        }
    }

    let index_path = out_root.join("index.txt");
    fs::write(&index_path, index_lines.join("\n") + "\n").expect("write index.txt");
    info!("Dumped {total} video archives to {}/", out_root.display());
    info!("Index: {}", index_path.display());
}
