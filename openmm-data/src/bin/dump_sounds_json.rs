use openmm_data::{Archive, Assets};
use serde_json::json;
use std::fs::File;
use std::io::Write;

fn main() {
    let data_path = openmm_data::get_data_path();
    let assets = Assets::new(&data_path).expect("failed to load game assets");

    let dsounds = assets.dsounds().expect("dsounds.bin not loaded");
    let snd_archive = assets.get_snd("audio");

    if snd_archive.is_none() {
        println!("Warning: Audio.snd not found in loaded archives. Sizes will be 0.");
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
        } else if let Some(archive) = snd_archive {
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
