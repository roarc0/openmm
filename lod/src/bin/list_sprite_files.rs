use lod::LodManager;

fn main() {
    let lod_path = lod::get_lod_path();
    let lod_manager = LodManager::new(&lod_path).expect("failed to open LOD files");
    
    if let Some(files) = lod_manager.files_in("sprites") {
        let mut sprite_files: Vec<_> = files.iter()
            .filter(|f| f.to_lowercase().contains("gob") || f.to_lowercase().contains("bar"))
            .collect();
        sprite_files.sort();
        
        println!("Goblin and Barbarian sprites found:");
        for file in sprite_files {
            println!("  {}", file);
        }
    }
}
