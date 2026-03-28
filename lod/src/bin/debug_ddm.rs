use lod::{LodManager, ddm::Ddm};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();

    let maps = ["outa1", "outa2", "outa3", "outb1", "outb2", "outb3",
                "outc1", "outc2", "outc3", "outd1", "outd2", "outd3",
                "oute1", "oute2", "oute3"];

    for map in &maps {
        let odm_name = format!("{}.odm", map);
        match Ddm::new(&lod_manager, &odm_name) {
            Ok(ddm) => {
                let names: Vec<_> = ddm.actors.iter()
                    .map(|a| a.name.as_str())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter().collect();
                println!("{}: {} actors, types: {:?}", map, ddm.actors.len(), names);
            }
            Err(e) => println!("{}: error: {}", map, e),
        }
    }

    // Detailed view of oute3 (starting map)
    println!("\n--- oute3 detail ---");
    let ddm = Ddm::new(&lod_manager, "oute3.odm").unwrap();
    for (i, a) in ddm.actors.iter().enumerate() {
        println!("  [{:2}] '{}' hp={} pos=({},{},{}) speed={} tether={}",
            i, a.name, a.hp, a.position[0], a.position[1], a.position[2],
            a.move_speed, a.tether_distance);
    }
}
