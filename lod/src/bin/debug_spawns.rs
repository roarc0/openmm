use std::io::{Cursor, Read, Seek};
use byteorder::{LittleEndian, ReadBytesExt};
use lod::{LodManager, odm::Odm, lod_data::LodData};

fn main() {
    let lod_manager = LodManager::new(lod::get_lod_path()).unwrap();

    // Raw parse to find spawn points
    let data = LodData::try_from(lod_manager.try_get_bytes("games/oute3.odm").unwrap()).unwrap();
    let data = data.data.as_slice();
    let mut cursor = Cursor::new(data);

    // Skip to after billboards by parsing the same way as Odm::new
    cursor.seek(std::io::SeekFrom::Start(176)).unwrap(); // HEIGHT_MAP_OFFSET
    cursor.seek(std::io::SeekFrom::Current(128*128 + 128*128 + 128*128)).unwrap(); // height+tile+attr maps

    let bsp_count = cursor.read_u32::<LittleEndian>().unwrap();
    println!("BSP models: {}", bsp_count);

    // Skip BSP models (complex, use Odm parser)
    let map = Odm::new(&lod_manager, "oute3.odm").unwrap();
    println!("Billboards: {}", map.billboards.len());
    println!("Spawn points from Odm: {}", map.spawn_points.len());

    // Let's look at raw data after billboard names
    // Billboard data: billboard_count * sizeof(BillboardData) + billboard_count * 32 (names)
    // Each BillboardData is 20 bytes (C repr: u16 + u16 + i32*3 + i32 + i16 + i16 + i16 + i16 = 2+2+12+4+2+2+2+2 = 28 bytes)
    println!("\nTotal data size: {} bytes", data.len());

    // Scan for plausible spawn counts in the last 10% of the file
    let scan_start = data.len() * 80 / 100;
    println!("Scanning from byte {} for spawn count...", scan_start);
    for offset in (scan_start..data.len()-100).step_by(4) {
        let val = u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
        if val > 5 && val < 200 {
            // Check if next bytes look like coordinates
            let x = i32::from_le_bytes([data[offset+4], data[offset+5], data[offset+6], data[offset+7]]);
            let y = i32::from_le_bytes([data[offset+8], data[offset+9], data[offset+10], data[offset+11]]);
            let z = i32::from_le_bytes([data[offset+12], data[offset+13], data[offset+14], data[offset+15]]);
            if x.abs() < 50000 && y.abs() < 50000 && z.abs() < 50000 && z.abs() < 5000 {
                // Check if spawn_count * 20 bytes fit
                if offset + 4 + (val as usize) * 20 <= data.len() {
                    println!("  Candidate at offset {}: count={} first_pos=({},{},{})", offset, val, x, y, z);
                }
            }
        }
    }
}
