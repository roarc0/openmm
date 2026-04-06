/// Dump all embedded SMK files from MM6 VID archives to data/dump/vid/.
///
/// Output:
///   data/dump/vid/{name}.smk        — raw Smacker video bytes
///   data/dump/vid/index.txt         — human-readable index with SMK metadata
use std::fs;
use std::path::Path;

use lodcratecrate::raw::vid::{Vid, parse_smk_info};

fn main() {
    let data_path = openmm_data::get_data_path();
    let anims_dir = Path::new(&data_path).join("Anims");

    let out_dir = Path::new("data/dump/vid");
    fs::create_dir_all(out_dir).expect("failed to create data/dump/vid");

    let vid_files = ["Anims1.vid", "Anims2.vid"];
    let mut index_lines: Vec<String> = Vec::new();
    index_lines.push(format!(
        "{:<20} {:>6} {:>5} {:>8}  {}",
        "name", "frames", "fps", "size", "resolution"
    ));
    index_lines.push("-".repeat(60));

    let mut total = 0usize;

    for vid_name in &vid_files {
        let vid_path = anims_dir.join(vid_name);
        if !vid_path.exists() {
            eprintln!("skipping {vid_name}: not found at {}", vid_path.display());
            continue;
        }

        let vid = Vid::open(&vid_path).unwrap_or_else(|e| panic!("failed to open {vid_name}: {e}"));
        println!("{vid_name}: {} entries", vid.entries.len());
        index_lines.push(format!("\n# {vid_name}"));

        for (i, entry) in vid.entries.iter().enumerate() {
            let smk = vid.smk_bytes(i);
            let out_path = out_dir.join(format!("{}.smk", entry.name));
            fs::write(&out_path, smk).unwrap_or_else(|e| panic!("write {}: {e}", out_path.display()));

            let info_str = if let Some(info) = parse_smk_info(smk) {
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
