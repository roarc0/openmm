use std::io::Cursor;
use byteorder::{LittleEndian, ReadBytesExt};
use lod::{LodManager, lod_data::LodData, dsft::DSFT};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let raw = lod_manager.try_get_bytes("games/oute3.ddm").unwrap();
    let data = LodData::try_from(raw).unwrap().data;
    let dsft = DSFT::new(&lod_manager).unwrap();

    let actor_offset = 1948;
    let actor_stride = 548;
    let actor_count = 38;

    // For each actor, show all unique sprite ID sets and what they resolve to
    let mut seen = std::collections::HashSet::new();
    for i in 0..actor_count {
        let base = actor_offset + actor_stride * i;
        let actor_data = &data[base..base + actor_stride];

        let name_end = actor_data[..32].iter().position(|&b| b == 0).unwrap_or(32);
        let name = String::from_utf8_lossy(&actor_data[..name_end]);

        // Read sprite IDs at 0xBC
        let mut cursor = Cursor::new(&actor_data[0xBC..]);
        let mut ids = [0u16; 5];
        for id in &mut ids {
            *id = cursor.read_u16::<LittleEndian>().unwrap_or(0);
        }

        let key = format!("{:?}", ids);
        if !seen.contains(&key) {
            seen.insert(key.clone());

            // Resolve standing sprite (slot 2) via group lookup
            let standing_id = ids[2];
            let standing_name = if (standing_id as usize) < dsft.groups.len() {
                let fidx = dsft.groups[standing_id as usize] as usize;
                if fidx < dsft.frames.len() {
                    dsft.frames[fidx].sprite_name().unwrap_or_default()
                } else { String::new() }
            } else { String::new() };

            // Also try direct frame lookup
            let direct_name = if (standing_id as usize) < dsft.frames.len() {
                dsft.frames[standing_id as usize].sprite_name().unwrap_or_default()
            } else { String::new() };

            println!("Actor {} '{}': ids={:?}", i, name, ids);
            println!("  standing(group): {} -> '{}'", standing_id, standing_name);
            println!("  standing(frame): {} -> '{}'", standing_id, direct_name);
        }
    }

    // Also: are there peasant-related groups in the DSFT?
    println!("\n--- Looking for peasant DSFT groups ---");
    for (gid, &frame_offset) in dsft.groups.iter().enumerate() {
        if (frame_offset as usize) < dsft.frames.len() {
            let f = &dsft.frames[frame_offset as usize];
            if let Some(name) = f.sprite_name() {
                if name.contains("pman") || name.contains("pfem") || name.contains("pmn2") {
                    println!("  group {}: offset {} -> '{}'", gid, frame_offset, name);
                }
            }
        }
    }
}
