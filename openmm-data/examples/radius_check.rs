fn main() {
    let lod_path = openmm_data::get_data_path();
    let mgr = openmm_data::Assets::new(&lod_path).unwrap();
    let monlist = openmm_data::monlist::MonsterList::load(&mgr).unwrap();
    for d in &monlist.monsters {
        println!(
            "{} {} {} {} {}",
            d.internal_name, d.radius, d.sprite_names[2], d.sprite_names[5], d.move_speed
        );
    }
}
