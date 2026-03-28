use std::io::Cursor;
use byteorder::{LittleEndian, ReadBytesExt};
use lod::{LodManager, lod_data::LodData, dsft::DSFT};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();
    let raw = lod_manager.try_get_bytes("games/oute3.ddm").unwrap();
    let data = LodData::try_from(raw).unwrap().data;
    let dsft = DSFT::new(&lod_manager).unwrap();

    println!("DSFT: {} frames, {} groups", dsft.frames.len(), dsft.groups.len());

    let actor_offset = 1948;
    let actor_stride = 548;
    let actor_count = 38;

    // Check group lookups for the sprite IDs we found
    let test_ids: &[u16] = &[1400, 1401, 1402, 1403, 1404, 1440, 1441, 1442, 1443, 1444];
    println!("\n--- Group lookup for test IDs ---");
    for &gid in test_ids {
        if (gid as usize) < dsft.groups.len() {
            let frame_idx = dsft.groups[gid as usize];
            print!("  group {} -> frame {}", gid, frame_idx);
            if (frame_idx as usize) < dsft.frames.len() {
                let f = &dsft.frames[frame_idx as usize];
                print!(" sprite={:?} group={:?}", f.sprite_name(), f.group_name());
            }
            println!();
        }
    }

    // Direct frame lookup
    println!("\n--- Direct frame lookup ---");
    for &fid in test_ids {
        if (fid as usize) < dsft.frames.len() {
            let f = &dsft.frames[fid as usize];
            println!("  frame {} sprite={:?} group={:?}", fid, f.sprite_name(), f.group_name());
        }
    }

    // For each unique set of sprite IDs, resolve the group->frame->sprite chain
    println!("\n--- All unique actor sprite sets ---");
    let mut seen = std::collections::HashSet::new();
    for i in 0..actor_count {
        let base = actor_offset + actor_stride * i;
        let actor_data = &data[base..base + actor_stride];
        let mut cursor = Cursor::new(&actor_data[0xBC..]);
        let mut ids = [0u16; 5];
        for id in &mut ids {
            *id = cursor.read_u16::<LittleEndian>().unwrap_or(0);
        }
        let key = format!("{:?}", ids);
        if seen.contains(&key) { continue; }
        seen.insert(key);

        println!("  Actor {}: ids={:?}", i, ids);
        for &gid in &ids {
            if gid == 0 { continue; }
            if (gid as usize) < dsft.groups.len() {
                let fidx = dsft.groups[gid as usize] as usize;
                if fidx < dsft.frames.len() {
                    let f = &dsft.frames[fidx];
                    println!("    {} -> group_name={:?} sprite_name={:?}",
                        gid, f.group_name(), f.sprite_name());
                }
            }
        }
    }
}
