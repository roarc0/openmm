fn main() {
    let lod_path = lod::get_lod_path();
    let mgr = lod::LodManager::new(lod_path).unwrap();

    if let Some(table) = mgr.npc_table() {
        println!("NPC table loaded: {} entries", table.npcs.len());
        for id in 1i32..=8 {
            if let Some(entry) = table.get(id) {
                println!("  NPC#{}: name='{}' portrait={} -> image='{}'",
                    id, entry.name, entry.portrait,
                    table.portrait_name(id).unwrap_or_default());
            }
        }
    } else {
        println!("Failed to load npc_table");
    }
}
