//! Dump billboard events and EVT file contents for investigation.
//! Usage: cargo run --example dump_events [map_name]

fn main() {
    let map = std::env::args().nth(1).unwrap_or_else(|| "oute3".to_string());
    let data_path = std::env::var("OPENMM_6_PATH").unwrap_or_else(|_| "./data/mm6/data".to_string());
    let lod = openmm_data::Assets::new(&data_path).expect("Failed to load LOD");

    // Dump billboard events
    println!("=== {map} BILLBOARD EVENTS ===");
    let odm_name = format!("{}.odm", map);
    match openmm_data::odm::Odm::load(&lod, &odm_name) {
        Ok(odm) => {
            let bb_mgr = openmm_data::billboard::BillboardManager::load(&lod).ok();
            for (i, bb) in odm.billboards.iter().enumerate() {
                let has_event = bb.data.event != 0;
                let has_var = bb.data.event_variable != 0;
                let has_attrs = bb.data.attributes != 0;
                if has_event || has_var || has_attrs {
                    let display_name = bb_mgr
                        .as_ref()
                        .and_then(|mgr: &openmm_data::billboard::BillboardManager| {
                            mgr.get_declist_item(bb.data.declist_id)
                        })
                        .and_then(|item| item.display_name())
                        .unwrap_or_default();
                    println!(
                        "  [{i:3}] '{}' (game='{}') declist={} event={} var={} trigger_r={} attrs=0x{:04X}",
                        bb.declist_name,
                        display_name,
                        bb.data.declist_id,
                        bb.data.event,
                        bb.data.event_variable,
                        bb.data.trigger_radius,
                        bb.data.attributes,
                    );
                    let mut flags = Vec::new();
                    if bb.data.is_triggered_by_touch() {
                        flags.push("TOUCH");
                    }
                    if bb.data.is_triggered_by_monster() {
                        flags.push("MONSTER");
                    }
                    if bb.data.is_triggered_by_object() {
                        flags.push("OBJECT");
                    }
                    if bb.data.is_visible_on_map() {
                        flags.push("VISIBLE_MAP");
                    }
                    if bb.data.is_chest() {
                        flags.push("CHEST");
                    }
                    if bb.data.is_original_invisible() {
                        flags.push("INVISIBLE");
                    }
                    if bb.data.is_obelisk_chest() {
                        flags.push("OBELISK");
                    }
                    if !flags.is_empty() {
                        println!("         flags: {}", flags.join(" | "));
                    }
                }
            }
            println!(
                "  Total: {} billboards, {} with events",
                odm.billboards.len(),
                odm.billboards.iter().filter(|b| b.data.event != 0).count()
            );
        }
        Err(e) => println!("  Not an outdoor map or error: {e}"),
    }

    // Dump map EVT events
    println!("\n=== {map}.EVT EVENTS ===");
    match openmm_data::evt::EvtFile::parse(&lod, &map) {
        Ok(evt) => {
            let mut ids: Vec<_> = evt.events.keys().collect();
            ids.sort();
            for id in &ids {
                let steps = &evt.events[id];
                println!("  Event {id}:");
                for s in steps {
                    println!("    step {}: {}", s.step, s.event);
                }
            }
            println!("  Total: {} events", ids.len());
        }
        Err(e) => println!("  Error: {e}"),
    }

    // Dump global.evt events
    println!("\n=== GLOBAL.EVT EVENTS ===");
    match openmm_data::evt::EvtFile::parse(&lod, "global") {
        Ok(evt) => {
            let mut ids: Vec<_> = evt.events.keys().collect();
            ids.sort();
            for id in &ids {
                let steps = &evt.events[id];
                println!("  Event {id}:");
                for s in steps {
                    println!("    step {}: {}", s.step, s.event);
                }
            }
            println!("  Total: {} events", ids.len());
        }
        Err(e) => println!("  Error: {e}"),
    }
}
