fn main() {
    let lod_path = lod::get_data_path();
    let mgr = lod::LodManager::new(&lod_path).unwrap();
    let monlist = lod::monlist::MonsterList::load(&mgr).unwrap();
    for d in &monlist.monsters { println!("{} {} {} {} {}", d.internal_name, d.radius, d.sprite_names[2], d.sprite_names[5], d.move_speed); }
}
