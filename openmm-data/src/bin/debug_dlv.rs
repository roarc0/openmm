use openmm_data::{Assets, assets::blv::Blv, assets::lod_data::LodData};

fn read_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn scan_for_name(data: &[u8], name: &[u8]) -> Vec<usize> {
    let mut results = Vec::new();
    for i in 0..data.len().saturating_sub(name.len()) {
        if &data[i..i + name.len()] == name {
            results.push(i);
        }
    }
    results
}

fn main() {
    let assets = Assets::new(openmm_data::get_data_path()).unwrap();

    // First: check what actors are in the DDM for outdoor maps to confirm format
    println!("=== Checking oute3.ddm actors (outdoor, known-working) ===");
    {
        let raw = assets.get_bytes("games/oute3.ddm").unwrap();
        let data = openmm_data::assets::lod_data::LodData::try_from(raw.as_slice())
            .unwrap()
            .data;
        // First few bytes of DDM = MapVars (200 bytes), then MapObjects count
        let obj_count = read_u32(&data, 200) as usize;
        println!("oute3.ddm: MapObjects count={}", obj_count);
        let spr_off = 200 + 4 + obj_count * 0x64;
        let spr_count = read_u32(&data, spr_off) as usize;
        println!("oute3.ddm: MapSprites count={}", spr_count);
        let sound_off = spr_off + 4 + spr_count * 0x1C;
        let chest_off = sound_off + 40;
        let chest_count = read_u32(&data, chest_off) as usize;
        println!("oute3.ddm: MapChests count={}", chest_count);
        // Monster size = 4204 bytes? Or try different sizes
        for chest_sz in [4204_usize, 148, 4096] {
            let mon_off = chest_off + 4 + chest_count * chest_sz;
            if mon_off + 4 > data.len() {
                continue;
            }
            let mon_count = read_u32(&data, mon_off) as usize;
            if mon_count > 500 {
                continue;
            }
            let first_name_off = mon_off + 4;
            if first_name_off + 32 > data.len() {
                continue;
            }
            let nb = &data[first_name_off..first_name_off + 32];
            let nend = nb.iter().position(|&b| b == 0).unwrap_or(32);
            let name_str = String::from_utf8_lossy(&nb[..nend]);
            println!(
                "  chest_sz={}: monsters at off={}, count={}, first='{}'",
                chest_sz, mon_off, mon_count, name_str
            );
        }
    }

    println!();
    println!("=== Scanning d01.dlv for monster names ===");
    {
        let Ok(blv) = Blv::load(&assets, "d01.blv") else {
            return;
        };
        let raw = assets.get_bytes("games/d01.dlv").unwrap();
        let data = LodData::try_from(raw.as_slice()).unwrap().data;
        println!("d01.dlv size={}", data.len());

        // Scan for known monster names likely in d01 (early dungeon)
        for name in [b"Goblin" as &[u8], b"Skeleton", b"Orc", b"Rat", b"Peasant", b"Guard"] {
            let positions = scan_for_name(&data, name);
            if !positions.is_empty() {
                println!(
                    "  Found '{}' at offsets: {:?}",
                    String::from_utf8_lossy(name),
                    &positions[..positions.len().min(5)]
                );
                // Check if 4 bytes before could be a count (try offset-4 as start of actor record)
                // or 32 bytes back from name for start of actor record at offset-0 = name field
                for &pos in positions.iter().take(3) {
                    // Actor name is at offset 0 in the 548-byte actor struct
                    // So pos should be the start of actor name = start of actor struct
                    // Then pos-4 should be actor_count in the section header
                    // Or: pos is somewhere inside; let's check if pos is actor-aligned
                    let n = data.len();
                    let count_guess = if pos >= 4 { read_u32(&data, pos - 4) as usize } else { 0 };
                    println!(
                        "    pos={}: bytes[-4..+4]={:?} count_if_header={}",
                        pos,
                        &data[pos.saturating_sub(4)..pos.min(n - 1) + 1],
                        count_guess
                    );
                    // Also check if pos-548 boundary alignment works
                    // Monster section offset = pos - (index_in_section * 548) - 4
                    // Just print context
                    println!("    u32 at pos={}: {}", pos, read_u32(&data, pos));
                }
            }
        }

        // Also: print 16 bytes around the computed monster end offset
        let tail = 256_usize;
        let mon_end = data
            .len()
            .saturating_sub(tail)
            .saturating_sub(blv.doors_data_size as usize)
            .saturating_sub(blv.door_count as usize * 80);
        println!("\n  Computed monster section end at off={}", mon_end);
        if mon_end >= 4 {
            println!(
                "  bytes [mon_end-8..mon_end+8]: {:?}",
                &data[mon_end.saturating_sub(8)..mon_end.min(data.len() - 1) + 8.min(data.len() - mon_end)]
            );
            println!("  u32 at mon_end-4: {}", read_u32(&data, mon_end.saturating_sub(4)));
            println!(
                "  u32 at mon_end: {}",
                if mon_end + 4 <= data.len() {
                    read_u32(&data, mon_end)
                } else {
                    0
                }
            );
        }
    }
}
