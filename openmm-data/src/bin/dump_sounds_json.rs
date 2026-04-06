use openmm_data::snd::SndArchive;
use openmm_data::{Archive, Assets, dsounds::DSounds, find_path_case_insensitive};
use serde_json::json;
use std::fs::File;
use std::io::Write;

fn main() {
    let data_path = openmm_data::get_data_path();
    let assets = Assets::new(&data_path).expect("failed to load LODs");
    let dsounds = match DSounds::load(&assets) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error loading dsounds.bin: {}", e);
            std::process::exit(1);
        }
    };

    let base = std::path::Path::new(&data_path);
    let parent = base.parent().unwrap_or(base);
    let snd_path = find_path_case_insensitive(parent, "Sounds/Audio.snd")
        .or_else(|| find_path_case_insensitive(base, "Audio.snd"));

    let snd_archive = snd_path.and_then(|p| {
        println!("Opening Audio.snd at {:?}", p);
        SndArchive::open(p).ok()
    });

    if snd_archive.is_none() {
        println!("Warning: Audio.snd not found. Sizes will be 0.");
    }

    let mut results = Vec::new();
    for item in &dsounds.items {
        let name = item.name().unwrap_or_default();
        let mut size = 0;

        if name.is_empty() {
            println!(
                "Warning: Sound ID {} has an empty name entry in dsounds.bin",
                item.sound_id
            );
        } else if let Some(ref archive) = snd_archive {
            let lower_name = name.to_lowercase();
            if let Some(entry) = archive
                .list_files()
                .iter()
                .find(|e| e.name.to_lowercase() == lower_name)
            {
                size = entry.size;
            }
        }

        results.push(json!({
            "name": name,
            "id": item.sound_id,
            "size": size,
            "type": item.sound_type,
            "attrs": format!("0x{:04x}", item.attributes)
        }));
    }

    // Ensure data directory exists
    std::fs::create_dir_all("data").expect("failed to create data directory");

    let json_output = serde_json::to_string_pretty(&results).expect("failed to serialize JSON");
    let mut file = File::create("data/dsounds.json").expect("failed to create data/dsounds.json");
    file.write_all(json_output.as_bytes()).expect("failed to write JSON");

    println!("Successfully dumped {} sounds to data/dsounds.json", results.len());
}
