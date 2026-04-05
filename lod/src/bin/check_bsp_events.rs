fn main() {
    let lod = lod::LodManager::new(lod::get_data_path()).unwrap();
    let odm = lod::odm::Odm::load(&lod, "oute3.odm").unwrap();
    let mut ids: std::collections::BTreeMap<u16, (u16, u16)> = Default::default();
    for model in &odm.bsp_models {
        for face in &model.faces {
            if face.cog_trigger_id != 0 {
                ids.entry(face.cog_trigger_id)
                    .and_modify(|v| v.1 += 1)
                    .or_insert((face.cog_trigger_type, 1));
            }
        }
    }
    for (id, (ttype, count)) in &ids {
        println!("event_id={:3}  trigger_type={}  face_count={}", id, ttype, count);
    }
    println!("total distinct ids: {}", ids.len());
}
