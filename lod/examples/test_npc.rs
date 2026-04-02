/// Debug: verify generated NPC names/portraits for peasant actors across maps
fn main() {
    let lod_path = lod::get_lod_path();
    let mgr = lod::LodManager::new(&lod_path).unwrap();
    let monlist = lod::monlist::MonsterList::new(&mgr).unwrap();

    let npc_table = mgr.game().npc_table().expect("npcdata.txt");

    for map in &["oute3.odm", "oute2.odm"] {
        println!("=== {} ===", map);
        let Ok(ddm) = lod::ddm::Ddm::new(&mgr, map) else {
            continue;
        };
        for (i, a) in ddm.actors.iter().enumerate() {
            if a.npc_id <= 0 {
                continue;
            }
            if !monlist.is_peasant(a.monlist_id) {
                continue;
            }
            let is_female = monlist.is_female_peasant(a.monlist_id);
            let (name, portrait) = npc_table.peasant_identity(is_female, i).unwrap_or(("Peasant", 1));
            println!(
                "  [{}] monlist_id={} {} → name='{}' portrait=NPC{:03}",
                i,
                a.monlist_id,
                if is_female { "F" } else { "M" },
                name,
                portrait
            );
        }
    }
}
