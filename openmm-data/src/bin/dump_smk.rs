/// Dump all embedded SMK files from MM6 Smacker archives (usually .vid) to data/dump/smk/.
///
/// Output:
///   data/dump/smk/{name}.smk        — raw Smacker video bytes
///   data/dump/smk/index.txt         — human-readable index with SMK metadata
use std::fs;
use std::path::Path;

use openmm_data::assets::provider::archive::Archive;
use openmm_data::assets::{SmkArchive, parse_smk_info};

fn main() {
    let data_path = openmm_data::get_data_path();
    let anims_dir = Path::new(&data_path).join("Anims");

    let out_dir = Path::new("data/dump/smk");
    fs::create_dir_all(out_dir).expect("failed to create data/dump/smk");

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
            eprintln!("skipping {archive_name}: not found at {}", archive_path.display());
            continue;
        }

        let archive = SmkArchive::open(&archive_path).unwrap_or_else(|e| panic!("failed to open {archive_name}: {e}"));
        let entries = archive.list_files();
        println!("{archive_name}: {} entries", entries.len());
        index_lines.push(format!("\n# {archive_name}"));

        for entry in entries {
            let smk_bytes = match archive.get_file(&entry.name) {
                Some(b) => b,
                None => {
                    eprintln!("  skipping {}: no data", entry.name);
                    continue;
                }
            };
            let out_path = out_dir.join(format!("{}.smk", entry.name));
            fs::write(&out_path, &smk_bytes).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));

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

            println!("  {info_str}");
            index_lines.push(info_str);
            total += 1;
        }
    }

    let index_path = out_dir.join("index.txt");
    fs::write(&index_path, index_lines.join("\n") + "\n").expect("write index.txt");
    println!("\nDumped {total} SMK files to {}/", out_dir.display());
    println!("Index: {}", index_path.display());
}
