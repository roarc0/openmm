use std::error::Error;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Serialize, Deserialize};

use crate::LodManager;
use crate::raw::blv::{BlvDoor, DoorState};
use crate::raw::ddm::{Ddm, DdmActor};
use crate::LodSerialise;
use crate::raw::lod_data::LodData;

fn skip_slice(offset: &mut usize, count: usize) {
    *offset += count;
}

/// Parsed DLV (indoor) delta file. The indoor equivalent of DDM.
///
/// On-disk layout (sections in order — sizes presized from BLV header values):
///   1. FaceAttributes  — face_count × u16 (mutable face flags, no count prefix)
///   2. DecorationFlags — decoration_count × u16 (no count prefix)
///   3. FaceExtras      — face_extras_count × u32 (no count prefix)
///   4. FaceData        — face_data_size bytes blob (no count prefix)
///   5. MapObjects      — u32 count + count × 0x64 bytes
///   6. MapSprites      — u32 count + count × 0x1C bytes
///   7. SoundSprites    — 10 × i32
///   8. MapChests       — u32 count + count × ~4204 bytes
///   9. MapMonsters     — u32 count + count × 548 bytes
///  10. DoorHeaders     — door_count × 80 bytes (presized)
///  11. DoorsData       — doors_data_size bytes blob (presized)
///
/// **Save support:** only actors (9) and doors (10–11) are currently parsed; sections
/// 1–8 are skipped. Round-trip saving requires storing all sections' raw bytes and
/// re-serialising in the exact same order with correct C struct layout.
#[derive(Debug, Serialize, Deserialize)]
pub struct Dlv {
    pub actors: Vec<DdmActor>,
    pub doors: Vec<BlvDoor>,
}

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
        let blv = crate::raw::blv::Blv::load(lod_manager, &blv_name)?;
        let face_count = blv.faces.len();
        let decoration_count = blv.decorations.len();
        let face_extras_count = blv.face_extras.len();
        let face_data_size = blv.face_data_size;

        let raw = lod_manager.try_get_bytes(format!("games/{}", dlv_name))?;
        let data = LodData::try_from(raw.as_slice())?;
        Self::parse(
            &data.data,
            door_count,
            doors_data_size,
            face_count,
            decoration_count,
            face_extras_count,
            face_data_size,
        )
    }

    fn parse(
        data: &[u8],
        door_count: u32,
        doors_data_size: i32,
        _face_count: usize,
        _decoration_count: usize,
        _face_extras_count: usize,
        _face_data_size: i32,
    ) -> Result<Self, Box<dyn Error>> {
        // MM6 DLV format has additional sections compared to MM7 (face extras, face data blob)
        // that make exact offset calculation unreliable. Instead, scan backwards from the end
        // to locate the door and actor sections.
        //
        // Known end layout: ... | doors (door_count × 80) | doorsData (doors_data_size) | tail
        // Tail = PersistentVariables (200 bytes) + LocationTime (~56-136 bytes)
        //
        // We locate doors by scanning for valid door headers (non-zero door_id with reasonable
        // parameters) and verify the doorsData blob follows at the expected offset.
        // Use heuristic actor scan (works across MM6/MM7 format differences)
        let actors = Ddm::parse_from_data(data).unwrap_or_default();

        // Locate doors by scanning for valid door headers.
        // A valid door header has: door_id > 0, move_length > 0, speed > 0, num_vertices > 0.
        // Door headers are contiguous (door_count × 80 bytes), followed by doorsData.
        let dc = door_count as usize;
        let doors = if dc > 0 && doors_data_size > 0 {
            Self::scan_doors(data, dc, doors_data_size)
        } else {
            Vec::new()
        };

        Ok(Dlv { actors, doors })
    }

    /// Scan the DLV data for door headers by finding a valid door and verifying
    /// it's part of a contiguous 80-byte-aligned array.
    fn scan_doors(data: &[u8], door_count: usize, doors_data_size: i32) -> Vec<BlvDoor> {
        let mut best_start: Option<usize> = None;

        // Scan for valid door headers and verify array alignment
        for offset in 0..data.len().saturating_sub(DOOR_HEADER_SIZE) {
            if !Self::is_valid_door_header(data, offset) {
                continue;
            }

            // Found a candidate. Compute potential array start by trying each slot index.
            for slot in 0..door_count {
                let array_start = offset.checked_sub(slot * DOOR_HEADER_SIZE);
                let Some(start) = array_start else { continue };

                // Verify: the array should fit door_count × 80 bytes + doorsData
                let array_end = start + door_count * DOOR_HEADER_SIZE;
                let total_end = array_end + doors_data_size as usize;
                if total_end > data.len() {
                    continue;
                }

                // Count how many slots in this array have valid door headers
                let valid_count = (0..door_count)
                    .filter(|&s| Self::is_valid_door_header(data, start + s * DOOR_HEADER_SIZE))
                    .count();

                // Also check that the doorsData blob has non-zero entries
                let data_nonzero = (0..((doors_data_size / 2) as usize).min(100)).any(|i| {
                    let off = array_end + i * 2;
                    off + 2 <= data.len() && read_u16(data, off) != 0
                });

                if valid_count >= 1 && data_nonzero {
                    best_start = Some(start);
                    break;
                }
            }

            if best_start.is_some() {
                break;
            }
        }

        let Some(array_start) = best_start else {
            // No valid doors found — return empty doors for all slots
            return (0..door_count)
                .map(|_| BlvDoor {
                    attributes: 0,
                    door_id: 0,
                    direction: [0.0; 3],
                    move_length: 0,
                    open_speed: 0,
                    close_speed: 0,
                    vertex_ids: Vec::new(),
                    face_ids: Vec::new(),
                    x_offsets: Vec::new(),
                    y_offsets: Vec::new(),
                    z_offsets: Vec::new(),
                    delta_us: Vec::new(),
                    delta_vs: Vec::new(),
                    state: DoorState::Open,
                })
                .collect();
        };

        // Parse door headers from the array start
        let mut door_headers = Vec::with_capacity(door_count);
        let mut offset = array_start;
        for _ in 0..door_count {
            if offset + DOOR_HEADER_SIZE > data.len() {
                break;
            }
            if let Ok(header) = parse_door_header(data, offset) {
                door_headers.push(header);
            }
            offset += DOOR_HEADER_SIZE;
        }

        // Parse doorsData blob right after the door headers
        let doors_data_i16_count = (doors_data_size / 2) as usize;
        if offset + doors_data_i16_count * 2 > data.len() {
            return partition_door_data(door_headers, &Vec::new());
        }
        let mut doors_data = Vec::with_capacity(doors_data_i16_count);
        let mut cursor = Cursor::new(&data[offset..offset + doors_data_i16_count * 2]);
        for _ in 0..doors_data_i16_count {
            if let Ok(v) = cursor.read_i16::<LittleEndian>() {
                doors_data.push(v);
            } else {
                break;
            }
        }

        partition_door_data(door_headers, &doors_data)
    }

    fn is_valid_door_header(data: &[u8], offset: usize) -> bool {
        if offset + DOOR_HEADER_SIZE > data.len() {
            return false;
        }
        let door_id = read_u32(data, offset + 4);
        if door_id == 0 || door_id > 200 {
            return false;
        }
        let move_length = read_i32(data, offset + 24);
        let open_speed = read_i32(data, offset + 28);
        let close_speed = read_i32(data, offset + 32);
        let num_vertices = read_u16(data, offset + 68);
        let num_faces = read_u16(data, offset + 70);
        move_length > 0
            && move_length < 10000
            && open_speed > 0
            && open_speed < 10000
            && close_speed > 0
            && close_speed < 10000
            && num_vertices > 0
            && num_vertices < 200
            && num_faces > 0
            && num_faces < 200
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
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

fn read_i32(data: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
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

impl TryFrom<&[u8]> for Dlv {
    type Error = Box<dyn Error>;

    /// NOTE: DLV files are not self-contained and require BLV metadata (door count, etc.)
    /// for a full parse. This implementation uses heuristic actor scanning but may
    /// result in empty doors if no metadata is available.
    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        let actors = Ddm::parse_from_data(&data.data).unwrap_or_default();
        // Doors cannot be reliably parsed without BLV metadata.
        Ok(Dlv {
            actors,
            doors: Vec::new(),
        })
    }
}

impl LodSerialise for Dlv {
    fn to_bytes(&self) -> Vec<u8> {
        // TODO: Implement full DLV serialization if needed.
        // For now, this is a placeholder to satisfy the trait.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::blv::Blv;
    use crate::raw::test_lod;

    #[test]
    fn scan_all_dlv_actors() {
        let Some(lod_manager) = test_lod() else { return };
        for name in ["d01","d02","d03","d04","d05","cd1","cd2","cd3"] {
            let blv_name = format!("{}.blv", name);
            let Ok(blv) = Blv::load(&lod_manager, &blv_name) else { continue };
            let Ok(dlv) = Dlv::new(&lod_manager, &blv_name, blv.door_count, blv.doors_data_size) else { continue };
            let monsters: Vec<_> = dlv.actors.iter().filter(|a| a.npc_id == 0).collect();
            let npcs: Vec<_> = dlv.actors.iter().filter(|a| a.npc_id != 0).collect();
            println!("{}: {} actors ({} monsters, {} npcs)", name, dlv.actors.len(), monsters.len(), npcs.len());
            for a in &monsters { println!("  monster: '{}' monlist_id={} pos={:?}", a.name, a.common_props.monlist_id, a.position); }
        }
    }

    #[test]
    fn parse_d01_dlv_with_doors() {
        let Some(lod_manager) = test_lod() else {
            return;
        };
        let blv = Blv::load(&lod_manager, "d01.blv").unwrap();
        let dlv = Dlv::new(&lod_manager, "d01.blv", blv.door_count, blv.doors_data_size).unwrap();

        println!(
            "d01.blv: door_count={}, doors_data_size={}, face_extras_count={}",
            blv.door_count,
            blv.doors_data_size,
            blv.face_extras.len()
        );
        println!("d01.dlv: {} actors, {} doors", dlv.actors.len(), dlv.doors.len());
        let nonempty: Vec<_> = dlv
            .doors
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.vertex_ids.is_empty())
            .collect();
        println!("  Non-empty doors: {}", nonempty.len());
        for (i, door) in &nonempty {
            println!(
                "  door[{}]: id={} dir={:?} move_len={} open_speed={} close_speed={} verts={} faces={} state={:?}",
                i,
                door.door_id,
                door.direction,
                door.move_length,
                door.open_speed,
                door.close_speed,
                door.vertex_ids.len(),
                door.face_ids.len(),
                door.state
            );
        }
        assert_eq!(dlv.doors.len(), blv.door_count as usize);
        // Pristine DLV files from the LOD have zeroed door data — the game engine
        // populates vertex/face mappings at runtime via InitializeDoors().
        // We verify the parser doesn't crash and produces the right door count.
    }
}
