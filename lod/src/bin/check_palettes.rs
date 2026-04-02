fn main() {
    let lod = lod::LodManager::new(lod::get_lod_path()).unwrap();
    let raw = lod.try_get_bytes("games/oute3.ddm").unwrap();
    let data = lod::lod_data::LodData::try_from(raw).unwrap();
    let data = &data.data;

    let mut actor_start = 0;
    for offset in (0..data.len().saturating_sub(40)).step_by(2) {
        let val = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]) as usize;
        if val > 0
            && val < 500
            && offset + 4 + val * 548 <= data.len()
            && data[offset + 4] >= b'A'
            && data[offset + 4] <= b'Z'
        {
            actor_start = offset;
            break;
        }
    }
    let base = actor_start + 4;
    let monlist = lod::monlist::MonsterList::new(&lod).unwrap();

    // Try offsets around 0x58, 0x5A, 0x5C, 0x60 for monsterInfo.id as u16
    println!("Checking candidate offsets for monsterInfo.id (u16):\n");
    for off in [
        0x54u16, 0x56, 0x58, 0x5A, 0x5C, 0x5E, 0x60, 0x62, 0x64, 0x66, 0x68, 0x6A, 0x6C, 0x6E, 0x70, 0x72, 0x74, 0x76,
    ] {
        let mut vals: Vec<u16> = Vec::new();
        for i in 0..5 {
            let actor_off = base + i * 548 + off as usize;
            vals.push(u16::from_le_bytes([data[actor_off], data[actor_off + 1]]));
        }
        // Only show if values are in a reasonable monlist range AND vary between actors
        let in_range = vals.iter().all(|v| *v < 200);
        let varies = vals.iter().collect::<std::collections::HashSet<_>>().len() > 1;
        if in_range && varies {
            let names: Vec<&str> = vals
                .iter()
                .map(|v| {
                    monlist
                        .get(*v as usize)
                        .map(|m| m.internal_name.as_str())
                        .unwrap_or("???")
                })
                .collect();
            println!("  offset 0x{:02X}: {:?} → {:?}", off, vals, names);
        }
    }
}
