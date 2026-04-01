use lod::LodManager;

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let mapstats = lod::mapstats::MapStats::new(&lod_manager).unwrap();

    println!("All maps with music tracks:");
    for map in &mapstats.maps {
        println!("  {:20} {:20} track={} lock={} trap={} treasure={} encounter={}%",
            map.filename, map.name, map.music_track, map.lock, map.trap_d20_count,
            map.treasure_level, map.encounter_chance);
    }
}
