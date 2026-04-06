/// Dump all oute3 NPCs to data/dump/npc2.json with ids, names, portraits, and types.
fn main() {
    let lod_path = openmm_data::get_data_path();
    let mgr = openmm_data::Assets::new(&lod_path).unwrap();
    let monlist = openmm_data::monlist::MonsterList::load(&mgr).unwrap();
    let npc_table = mgr.game().npc_table().expect("npcdata.txt");

    let map = "oute3.odm";
    let ddm = openmm_data::ddm::Ddm::load(&mgr, map).expect("failed to load oute3 DDM");

    let mut entries = Vec::new();

    for (i, a) in ddm.actors.iter().enumerate() {
        let monster = monlist.get(a.monlist_id as usize);
        let internal_name = monster.map(|m| m.internal_name.as_str()).unwrap_or("?");
        let is_peasant = monlist.is_peasant(a.monlist_id);
        let is_female = monlist.is_female_peasant(a.monlist_id);

        let (display_name, portrait, npc_type) = if a.npc_id > 0 && !is_peasant {
            // Quest/named NPC from npcdata.txt
            let name = npc_table.npc_name(a.npc_id as i32).unwrap_or(&a.name).to_string();
            let portrait = npc_table
                .portrait_name(a.npc_id as i32)
                .unwrap_or_else(|| "none".into());
            (name, portrait, "quest")
        } else if is_peasant {
            // Peasant NPC: identity from npcdata.txt peasant-profession entries
            let (name, portrait_num, _prof_id) = npc_table.peasant_identity(is_female, i).unwrap_or(("Peasant", 1, 52));
            let portrait = format!("NPC{:03}", portrait_num);
            let npc_type = if is_female { "peasant_female" } else { "peasant_male" };
            (name.to_string(), portrait, npc_type)
        } else {
            // Monster
            (a.name.clone(), "none".into(), "monster")
        };

        entries.push(format!(
            r#"    {{
      "index": {},
      "ddm_name": "{}",
      "monlist_id": {},
      "internal_name": "{}",
      "npc_id": {},
      "npc_id_note": "{}",
      "type": "{}",
      "display_name": "{}",
      "portrait": "{}",
      "position": [{}, {}, {}]
    }}"#,
            i,
            a.name.replace('"', r#"\""#),
            a.monlist_id,
            internal_name.replace('"', r#"\""#),
            a.npc_id,
            if is_peasant {
                if a.npc_id == 1 {
                    "type_flag: female peasant"
                } else {
                    "type_flag: male peasant"
                }
            } else if a.npc_id > 0 {
                "npcdata.txt index"
            } else {
                "none (monster)"
            },
            npc_type,
            display_name.replace('"', r#"\""#),
            portrait,
            a.position[0],
            a.position[1],
            a.position[2],
        ));
    }

    let json = format!(
        "{{\n  \"map\": \"{}\",\n  \"actor_count\": {},\n  \"actors\": [\n{}\n  ]\n}}\n",
        map,
        entries.len(),
        entries.join(",\n"),
    );

    let out_path = std::path::Path::new("data/dump/npc2.json");
    std::fs::write(out_path, &json).expect("failed to write data/dump/npc2.json");
    println!("Wrote {} actors to {}", entries.len(), out_path.display());
}
