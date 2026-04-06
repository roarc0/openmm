use openmm_data::Assets;

fn main() {
    let lod_path = openmm_data::get_data_path();
    let assets = Assets::new(&lod_path).expect("failed to open LOD files");

    if let Some(files) = assets.files_in("sprites") {
        let mut sprite_files: Vec<_> = files
            .iter()
            .filter(|f| f.to_lowercase().contains("gob") || f.to_lowercase().contains("bar"))
            .collect();
        sprite_files.sort();

        println!("Goblin and Barbarian sprites found:");
        for file in sprite_files {
            println!("  {}", file);
        }
    }
}
