use lod::{LodManager, monlist::MonsterList};

fn main() {
    let lod_path = lod::get_data_path();
    let lod_manager = LodManager::new(&lod_path).expect("failed to open LOD files");
    let monlist = MonsterList::load(&lod_manager).expect("failed to load monlist");

    println!("Monsters containing 'Goblin' or 'Barbar' in name:");
    println!();

    for (idx, monster) in monlist.monsters.iter().enumerate() {
        if monster.internal_name.to_lowercase().contains("goblin")
            || monster.internal_name.to_lowercase().contains("barbar")
        {
            println!("Index: {}", idx);
            println!("  Internal Name: {}", monster.internal_name);
            println!("  Sprites: {:?}", monster.sprite_names);
            println!();
        }
    }

    println!("\n=== All Monsters ===\n");
    for (idx, monster) in monlist.monsters.iter().enumerate() {
        println!("{}: {} -> {:?}", idx, monster.internal_name, monster.sprite_names);
    }
}
