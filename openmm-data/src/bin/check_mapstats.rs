use openmm_data::LodManager;

fn main() {
    let lod_manager = LodManager::new(openmm_data::get_data_path()).unwrap();
    let mapstats = openmm_data::mapstats::MapStats::load(&lod_manager).unwrap();

    println!("All maps with music tracks:");
    for map in &mapstats.maps {
        println!(
            "  {:20} {:20} track={} lock={} trap={} treasure={} encounter={}%",
            map.filename,
            map.name,
            map.music_track,
            map.lock,
            map.trap_d20_count,
            map.treasure_level,
            map.encounter_chance
        );
    }
}
