use std::error::Error;

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use crate::blv::{BlvDoor, DoorState};
use crate::ddm::{Ddm, DdmActor};
use crate::lod_data::LodData;
use crate::LodManager;

/// Parsed DLV indoor delta file. The indoor equivalent of DDM.
/// Contains actors and doors for an indoor (BLV) map.
pub struct Dlv {
    pub actors: Vec<DdmActor>,
    pub doors: Vec<BlvDoor>,
}

/// MM6 chest size: 36-byte header + 140 items x 36 bytes = 5076 bytes.
const CHEST_SIZE_MM6: usize = 5076;
/// MM6 SpriteObject size: 112 bytes.
const SPRITE_OBJECT_SIZE_MM6: usize = 112;
/// MM6 Actor (MapMonster) size: 548 bytes.
const ACTOR_SIZE_MM6: usize = 548;
/// Door header size: 80 bytes.
const DOOR_HEADER_SIZE: usize = 80;

impl Dlv {
    /// Parse a DLV file from a LOD archive.
    ///
    /// `door_count` and `doors_data_size` come from the BLV header, since
    /// the DLV door sections have NO count prefix — they are "presized"
    /// using metadata from the BLV file.
    ///
    /// Similarly, the faceAttributes and decorationFlags sections are presized
    /// (no count prefix) using the BLV's face count and decoration count.
    pub fn new(
        lod_manager: &LodManager,
        map_name: &str,
        door_count: u32,
        doors_data_size: i32,
    ) -> Result<Self, Box<dyn Error>> {
        let dlv_name = map_name
            .rsplit_once('.')
            .map(|(base, _)| format!("{}.dlv", base))
            .unwrap_or_else(|| format!("{}.dlv", map_name));

        let blv_name = map_name
            .rsplit_once('.')
            .map(|(base, _)| format!("{}.blv", base))
            .unwrap_or_else(|| format!("{}.blv", map_name));

        // We need face_count and decoration_count from BLV to know the presized
        // section lengths. Parse the BLV to get them.
        let blv = crate::blv::Blv::new(lod_manager, &blv_name)?;
        let face_count = blv.faces.len();
        let decoration_count = blv.decorations.len();

        let raw = lod_manager.try_get_bytes(&format!("games/{}", dlv_name))?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data, door_count, doors_data_size, face_count, decoration_count)
    }

    fn parse(
        data: &[u8],
        door_count: u32,
        doors_data_size: i32,
        face_count: usize,
        decoration_count: usize,
    ) -> Result<Self, Box<dyn Error>> {
        let mut offset: usize = 0;

        // 1. LocationHeader (40 bytes)
        offset += 40;

        // 2. visibleOutlines (875 bytes)
        offset += 875;

        // 3. faceAttributes: presized (NO count prefix), face_count x u32
        offset += face_count * 4;

        // 4. decorationFlags: NO count prefix, decoration_count x u16 (presized from BLV)
        offset += decoration_count * 2;

        // 5. actors: u32 count + count x 548 bytes
        if offset + 4 > data.len() {
            return Err(format!("DLV too short at actors count (offset={}, len={})", offset, data.len()).into());
        }
        let actor_count = read_u32(data, offset) as usize;
        offset += 4;
        let actors = Ddm::parse_actors_at(data, offset, actor_count);
        offset += actor_count * ACTOR_SIZE_MM6;

        // 6. spriteObjects: u32 count + count x 112 bytes
        if offset + 4 > data.len() {
            return Err(format!("DLV too short at spriteObjects count (offset={}, len={})", offset, data.len()).into());
        }
        let sprite_count = read_u32(data, offset) as usize;
        offset += 4 + sprite_count * SPRITE_OBJECT_SIZE_MM6;

        // 7. chests: u32 count + count x 5076 bytes
        if offset + 4 > data.len() {
            return Err(format!("DLV too short at chests count (offset={}, len={})", offset, data.len()).into());
        }
        let chest_count = read_u32(data, offset) as usize;
        offset += 4 + chest_count * CHEST_SIZE_MM6;

        // 8. doors: door_count x 80 bytes (NO count prefix — count from BLV)
        let dc = door_count as usize;
        let mut door_headers = Vec::with_capacity(dc);
        for _ in 0..dc {
            if offset + DOOR_HEADER_SIZE > data.len() {
                return Err("DLV too short at door headers".into());
            }
            door_headers.push(parse_door_header(data, offset)?);
            offset += DOOR_HEADER_SIZE;
        }

        // 9. doorsData: (doors_data_size / 2) x i16 flat blob (NO count prefix)
        let doors_data_i16_count = (doors_data_size / 2) as usize;
        if offset + doors_data_i16_count * 2 > data.len() {
            return Err("DLV too short at doorsData blob".into());
        }
        let mut doors_data = Vec::with_capacity(doors_data_i16_count);
        let mut cursor = Cursor::new(&data[offset..offset + doors_data_i16_count * 2]);
        for _ in 0..doors_data_i16_count {
            doors_data.push(cursor.read_i16::<LittleEndian>()?);
        }

        // Partition doorsData per door
        let doors = partition_door_data(door_headers, &doors_data);

        Ok(Dlv { actors, doors })
    }
}

/// Intermediate door header parsed from the 80-byte block.
struct DoorHeader {
    attributes: u32,
    door_id: u32,
    direction: [f32; 3],
    move_length: i32,
    open_speed: i32,
    close_speed: i32,
    num_vertices: u16,
    num_faces: u16,
    num_sectors: u16,
    num_offsets: u16,
    state: u16,
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_i32(data: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn parse_door_header(data: &[u8], base: usize) -> Result<DoorHeader, Box<dyn Error>> {
    let attributes = read_u32(data, base);
    let door_id = read_u32(data, base + 4);
    // base + 8: time_since_triggered (u32, runtime — skip)
    // base + 12: direction[3] as i32 fixed-point 16.16
    let dir_x = read_i32(data, base + 12) as f32 / 65536.0;
    let dir_y = read_i32(data, base + 16) as f32 / 65536.0;
    let dir_z = read_i32(data, base + 20) as f32 / 65536.0;
    let move_length = read_i32(data, base + 24);
    let open_speed = read_i32(data, base + 28);
    let close_speed = read_i32(data, base + 32);
    // base + 36: 8 runtime pointers (32 bytes) — skip
    // base + 68: num_vertices(u16), num_faces(u16), num_sectors(u16), num_offsets(u16)
    let num_vertices = read_u16(data, base + 68);
    let num_faces = read_u16(data, base + 70);
    let num_sectors = read_u16(data, base + 72);
    let num_offsets = read_u16(data, base + 74);
    // base + 76: state(u16), padding(i16)
    let state = read_u16(data, base + 76);

    Ok(DoorHeader {
        attributes,
        door_id,
        direction: [dir_x, dir_y, dir_z],
        move_length,
        open_speed,
        close_speed,
        num_vertices,
        num_faces,
        num_sectors,
        num_offsets,
        state,
    })
}

fn partition_door_data(headers: Vec<DoorHeader>, blob: &[i16]) -> Vec<BlvDoor> {
    let mut offset = 0;
    let mut doors = Vec::with_capacity(headers.len());

    for h in headers {
        let nv = h.num_vertices as usize;
        let nf = h.num_faces as usize;
        let ns = h.num_sectors as usize;
        let no = h.num_offsets as usize;

        // Per-door data layout in the flat blob:
        // vertex_ids: nv x i16
        // face_ids: nf x i16
        // sector_ids: ns x i16 (skip)
        // delta_us: nf x i16
        // delta_vs: nf x i16
        // x_offsets: no x i16
        // y_offsets: no x i16
        // z_offsets: no x i16
        let total = nv + nf + ns + nf + nf + no + no + no;

        // Skip empty/unused door slots — either all dimensions zero, or
        // unreasonable sizes that would exceed the blob (garbage/uninitialized data).
        if total == 0 || offset + total > blob.len() {
            doors.push(BlvDoor {
                attributes: h.attributes,
                door_id: h.door_id,
                direction: h.direction,
                move_length: h.move_length,
                open_speed: h.open_speed,
                close_speed: h.close_speed,
                vertex_ids: Vec::new(),
                face_ids: Vec::new(),
                x_offsets: Vec::new(),
                y_offsets: Vec::new(),
                z_offsets: Vec::new(),
                delta_us: Vec::new(),
                delta_vs: Vec::new(),
                state: DoorState::Open,
            });
            continue;
        }

        let end = offset + total;

        let vertex_ids = read_slice_u16(blob, &mut offset, nv, end);
        let face_ids = read_slice_u16(blob, &mut offset, nf, end);
        // sector_ids: skip
        skip_slice(&mut offset, ns);
        let delta_us = read_slice_i16(blob, &mut offset, nf, end);
        let delta_vs = read_slice_i16(blob, &mut offset, nf, end);
        let x_offsets = read_slice_i16(blob, &mut offset, no, end);
        let y_offsets = read_slice_i16(blob, &mut offset, no, end);
        let z_offsets = read_slice_i16(blob, &mut offset, no, end);

        let state = match h.state {
            0 => DoorState::Open,
            1 => DoorState::Closing,
            2 => DoorState::Closed,
            3 => DoorState::Opening,
            _ => DoorState::Closed,
        };

        doors.push(BlvDoor {
            attributes: h.attributes,
            door_id: h.door_id,
            direction: h.direction,
            move_length: h.move_length,
            open_speed: h.open_speed,
            close_speed: h.close_speed,
            vertex_ids,
            face_ids,
            x_offsets,
            y_offsets,
            z_offsets,
            delta_us,
            delta_vs,
            state,
        });
    }

    doors
}

fn read_slice_u16(blob: &[i16], offset: &mut usize, count: usize, end: usize) -> Vec<u16> {
    let start = *offset;
    let actual = count.min(end.saturating_sub(start));
    *offset += count;
    blob[start..start + actual].iter().map(|&v| v as u16).collect()
}

fn read_slice_i16(blob: &[i16], offset: &mut usize, count: usize, end: usize) -> Vec<i16> {
    let start = *offset;
    let actual = count.min(end.saturating_sub(start));
    *offset += count;
    blob[start..start + actual].to_vec()
}

fn skip_slice(offset: &mut usize, count: usize) {
    *offset += count;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blv::Blv;
    use crate::get_lod_path;

    #[test]
    fn parse_d01_dlv_with_doors() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let blv = Blv::new(&lod_manager, "d01.blv").unwrap();
        let dlv = Dlv::new(&lod_manager, "d01.blv", blv.door_count, blv.doors_data_size).unwrap();

        println!("d01.blv: door_count={}, doors_data_size={}", blv.door_count, blv.doors_data_size);
        println!("d01.dlv: {} actors, {} doors", dlv.actors.len(), dlv.doors.len());
        for (i, door) in dlv.doors.iter().enumerate() {
            if !door.vertex_ids.is_empty() {
                println!(
                    "  door[{}]: id={} dir={:?} move_len={} open_speed={} close_speed={} verts={} faces={} state={:?}",
                    i, door.door_id, door.direction, door.move_length,
                    door.open_speed, door.close_speed,
                    door.vertex_ids.len(), door.face_ids.len(), door.state
                );
            }
        }
        assert_eq!(dlv.doors.len(), blv.door_count as usize);
        // Pristine DLV files from the LOD have zeroed door data — the game engine
        // populates vertex/face mappings at runtime via InitializeDoors().
        // We verify the parser doesn't crash and produces the right door count.
    }
}
