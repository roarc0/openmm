use lod::{LodManager, odm::Odm, billboard::BillboardManager};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let map = Odm::new(&lod_manager, "oute3.odm").unwrap();
    let bb_mgr = BillboardManager::new(&lod_manager).unwrap();

    println!("Map has {} billboards", map.billboards.len());

    for (i, bb) in map.billboards.iter().enumerate().take(30) {
        let pos = bb.data.position;
        let name = &bb.declist_name;
        let invisible = bb.data.is_invisible();

        if let Some(sprite) = bb_mgr.get(&lod_manager, name, bb.data.declist_id) {
            let (w, h) = sprite.dimensions();
            let dec_name = sprite.d_declist_item.name().unwrap_or_default();
            let sft_name = sprite.d_sft_frame.sprite_name().unwrap_or_default();
            println!(
                "  [{:3}] '{}' dec='{}' sft='{}' pos=({},{},{}) size={:.0}x{:.0} inv={}",
                i, name, dec_name, sft_name, pos[0], pos[1], pos[2], w, h, invisible
            );
        } else {
            println!(
                "  [{:3}] '{}' pos=({},{},{}) FAILED TO LOAD inv={}",
                i, name, pos[0], pos[1], pos[2], invisible
            );
        }
    }

    // Count by type
    let mut loaded = 0;
    let mut failed = 0;
    let mut invisible_count = 0;
    for bb in &map.billboards {
        if bb.data.is_invisible() {
            invisible_count += 1;
            continue;
        }
        if bb_mgr.get(&lod_manager, &bb.declist_name, bb.data.declist_id).is_some() {
            loaded += 1;
        } else {
            failed += 1;
        }
    }
    println!("\nSummary: {} loaded, {} failed, {} invisible", loaded, failed, invisible_count);
}
