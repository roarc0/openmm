use openmm_data::{LodManager, odm::Odm};

fn main() {
    let lod_manager = LodManager::new(openmm_data::get_data_path()).unwrap();
    let map = Odm::load(&lod_manager, "oute3.odm").unwrap();

    println!("Spawn points: {}", map.spawn_points.len());
    for (i, sp) in map.spawn_points.iter().enumerate() {
        println!(
            "  [{:2}] pos=({},{},{}) radius={} type={} idx={} attrs={}",
            i,
            sp.position[0],
            sp.position[1],
            sp.position[2],
            sp.radius,
            sp.spawn_type,
            sp.monster_index,
            sp.attributes
        );
    }
}
