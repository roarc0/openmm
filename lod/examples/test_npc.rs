/// Debug: show what npc_ids and sprite names the DDM actors have in oute3
fn main() {
    let lod_path = lod::get_lod_path();
    let mgr = lod::LodManager::new(lod_path).unwrap();

    let ddm = lod::ddm::Ddm::new(&mgr, "oute3.odm").expect("oute3 DDM");
    let monlist = lod::monlist::MonsterList::new(&mgr).expect("monlist");
    let npc_table = mgr.game().npc_table().expect("npc_table");

    println!("DDM unique NPC types (by monlist_id + npc_id):");
    let mut seen = std::collections::HashSet::new();
    for a in &ddm.actors {
        let key = (a.monlist_id, a.npc_id);
        if seen.insert(key) {
            let sprite = monlist.get(a.monlist_id as usize).map(|m| m.sprite_names[0].clone()).unwrap_or_default();
            let npc_entry = npc_table.get(a.npc_id as i32);
            println!("  monlist_id={} sprite='{}' npc_id={} -> NPC name={:?} portrait={:?}",
                a.monlist_id, sprite, a.npc_id,
                npc_entry.map(|e| e.name.as_str()),
                npc_entry.map(|e| format!("NPC{:03}", e.portrait)));
        }
    }
}
