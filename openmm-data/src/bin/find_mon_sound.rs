use openmm_data::{Assets, monlist::MonsterList};

fn main() {
    let data_path = openmm_data::get_data_path();
    let assets = Assets::new(&data_path).expect("failed to load LODs");
    let monlist = MonsterList::load(&assets).expect("failed to load dmonlist.bin");

    let bad_sids = [1403, 1413, 1423, 1433, 1443, 1453, 1463, 1473];

    println!("Searching for bad sound_ids in monster list...");
    for (i, mon) in monlist.monsters.iter().enumerate() {
        for (sid_idx, &sid) in mon.sound_ids.iter().enumerate() {
            if bad_sids.contains(&sid) {
                println!(
                    "Monster {} ('{}') uses empty sound_id {} for slot {}",
                    i, mon.internal_name, sid, sid_idx
                );
            }
        }
    }
}
