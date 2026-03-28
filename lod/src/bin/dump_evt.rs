use std::io::Read;

fn main() {
    let lod = lod::LodManager::new(lod::get_lod_path()).unwrap();

    let evt_data = lod.try_get_bytes("icons/oute3.evt")
        .or_else(|_| lod.try_get_bytes("games/oute3.evt"))
        .or_else(|_| lod.try_get_bytes("new/oute3.evt"))
        .expect("Could not find oute3.evt");

    // Decompress (LOD data may have header before zlib)
    let zlib_pos = evt_data.windows(2).position(|w| w[0] == 0x78 && w[1] == 0x9c).unwrap();
    let mut decoder = flate2::read::ZlibDecoder::new(&evt_data[zlib_pos..]);
    let mut data = Vec::new();
    decoder.read_to_end(&mut data).unwrap();
    println!("Decompressed oute3.evt: {} bytes\n", data.len());

    // EVT binary format (from OpenEnroth EvtProgram::load + EvtInstruction::parse):
    // Each record:
    //   pos[0]: size_byte — number of bytes AFTER this byte (total = size_byte + 1)
    //   pos[1..3]: event_id (u16 LE)
    //   pos[3]: step (u8)
    //   pos[4]: opcode (u8)
    //   pos[5..size_byte+1]: params (opcode-dependent)

    let mut pos = 0;
    let mut count = 0;
    while pos < data.len() {
        let size_byte = data[pos] as usize;
        let total = size_byte + 1;
        if total < 5 || pos + total > data.len() {
            println!("[{:04x}] Bad record: size_byte={} total={} remaining={}",
                pos, size_byte, total, data.len() - pos);
            break;
        }

        let event_id = u16::from_le_bytes([data[pos + 1], data[pos + 2]]);
        let step = data[pos + 3];
        let opcode = data[pos + 4];
        let params = &data[pos + 5..pos + total];

        let desc = decode_opcode(opcode, params);
        println!("[{:04x}] evt={:3} step={:2} op=0x{:02x} {}",
            pos, event_id, step, opcode, desc);

        pos += total;
        count += 1;
    }
    println!("\nTotal: {} instructions", count);
}

fn u32_param(params: &[u8], offset: usize) -> Option<u32> {
    if params.len() >= offset + 4 {
        Some(u32::from_le_bytes([params[offset], params[offset+1], params[offset+2], params[offset+3]]))
    } else {
        None
    }
}

fn i32_param(params: &[u8], offset: usize) -> Option<i32> {
    u32_param(params, offset).map(|v| v as i32)
}

fn decode_opcode(opcode: u8, params: &[u8]) -> String {
    match opcode {
        0x01 => "Exit".into(),
        0x02 => {
            let house_id = u32_param(params, 0).unwrap_or(0);
            format!("SpeakInHouse(house_id={})", house_id)
        }
        0x03 => {
            let id = i32_param(params, 0).unwrap_or(0);
            format!("PlaySound(id={})", id)
        }
        0x04 => {
            let str_id = params.first().copied().unwrap_or(0);
            format!("Hint(str={})", str_id)
        }
        0x05 => {
            let str_id = params.first().copied().unwrap_or(0);
            format!("MazeInfo(str={})", str_id)
        }
        0x06 => {
            let x = i32_param(params, 0).unwrap_or(0);
            let y = i32_param(params, 4).unwrap_or(0);
            let z = i32_param(params, 8).unwrap_or(0);
            let dir = i32_param(params, 12).unwrap_or(0);
            let house_id = params.get(24).copied().unwrap_or(0);
            let icon = params.get(25).copied().unwrap_or(0);
            // Map name starts at offset 26
            let map_name = if params.len() > 26 {
                let name_bytes = &params[26..];
                let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(name_bytes.len());
                String::from_utf8_lossy(&name_bytes[..end]).to_string()
            } else {
                String::new()
            };
            format!("MoveToMap(x={} y={} z={} dir={} house={} icon={} map='{}')",
                x, y, z, dir, house_id, icon, map_name)
        }
        0x07 => format!("OpenChest(id={})", params.first().copied().unwrap_or(0)),
        0x0E => format!("Compare(params={:02x?})", &params[..params.len().min(12)]),
        0x0F => {
            let door_id = u32_param(params, 0).unwrap_or(0);
            format!("SetDoorState(door_id={})", door_id)
        }
        0x10 => format!("Add(params={:02x?})", &params[..params.len().min(12)]),
        0x11 => format!("Subtract(params={:02x?})", &params[..params.len().min(12)]),
        0x12 => format!("Set(params={:02x?})", &params[..params.len().min(12)]),
        0x1D => {
            let str_id = u32_param(params, 0).unwrap_or(0);
            format!("StatusText(str={})", str_id)
        }
        0x24 => {
            let target = u32_param(params, 0).unwrap_or(0);
            format!("GoTo(step={})", target)
        }
        0x25 => "OnLoadMap".into(),
        0x35 => "OnLeaveMap".into(),
        _ => format!("op_0x{:02x}(params={:02x?})", opcode, &params[..params.len().min(16)]),
    }
}
