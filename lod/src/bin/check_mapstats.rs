use lod::LodManager;

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let mapstats = lod::mapstats::MapStats::new(&lod_manager).unwrap();

    println!("All maps with music tracks:");
    for (name, cfg) in &mapstats.maps {
        println!("  {:20} track={}", name, cfg.music_track);
    }
}
