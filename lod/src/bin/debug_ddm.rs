use lod::{LodManager, ddm::Ddm};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let ddm = Ddm::new(&lod_manager, "oute3.odm").unwrap();

    println!("Actors: {}", ddm.actors.len());
    for (i, actor) in ddm.actors.iter().enumerate() {
        let pos = actor.position;
        println!(
            "  [{:2}] '{}' hp={} pos=({},{},{}) yaw={} ai={} anim={} speed={} tether={} sprites={:?}",
            i, actor.name, actor.hp, pos[0], pos[1], pos[2],
            actor.yaw, actor.ai_state, actor.current_animation,
            actor.move_speed, actor.tether_distance,
            &actor.sprite_ids[..3],
        );
    }
}
