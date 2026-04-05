use lod::{LodManager, raw::lod_data::LodData};

fn main() {
    let lod_manager = LodManager::new(lod::get_data_path()).unwrap();
    let raw = lod_manager.try_get_bytes("games/oute3.ddm").unwrap();
    let data = LodData::try_from(raw.as_slice()).unwrap().data;

    let actor_offset = 1948;
    let actor_stride = 548;

    // For each of first 5 actors, print ALL u16 values in peasant range (115-170)
    println!("Looking for peasant monster IDs (115-170) in actors...\n");
    for i in 0..5 {
        let base = actor_offset + actor_stride * i;
        let actor_data = &data[base..base + actor_stride];
        let name_end = actor_data[..32].iter().position(|&b| b == 0).unwrap_or(32);
        let name = String::from_utf8_lossy(&actor_data[..name_end]);
        print!("Actor {} '{}': ", i, name);
        for off in (0..actor_stride - 1).step_by(2) {
            let val = u16::from_le_bytes([actor_data[off], actor_data[off + 1]]);
            if (115..=170).contains(&val) {
                print!("@0x{:02x}={} ", off, val);
            }
        }
        println!();
    }

    // Also try single byte values (in case it's a u8 field)
    println!("\nLooking for peasant IDs as u8 values (115-170)...\n");
    for i in 0..5 {
        let base = actor_offset + actor_stride * i;
        let actor_data = &data[base..base + actor_stride];
        let name_end = actor_data[..32].iter().position(|&b| b == 0).unwrap_or(32);
        let name = String::from_utf8_lossy(&actor_data[..name_end]);
        print!("Actor {} '{}': ", i, name);
        for off in 0x2C..0x80 {
            if actor_data[off] >= 115 && actor_data[off] <= 170 {
                print!("@0x{:02x}={} ", off, actor_data[off]);
            }
        }
        println!();
    }

    // Print the full MonsterInfo area as u16 values for manual inspection
    println!("\nFull MonsterInfo area (0x2C-0x74) as u16:");
    let actor_data = &data[actor_offset..actor_offset + actor_stride];
    for off in (0x2C..0x74).step_by(2) {
        let val = u16::from_le_bytes([actor_data[off], actor_data[off + 1]]);
        print!("  0x{:02x}: {:5}", off, val);
        if (off - 0x2C) % 16 == 14 {
            println!();
        }
    }
    println!();
}
