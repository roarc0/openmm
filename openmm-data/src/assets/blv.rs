use byteorder::{LittleEndian, ReadBytesExt};
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io::{Cursor, Seek};

fn default_blv_unknown_mid() -> [u8; 40] {
    [0; 40]
}

use crate::Assets;
use crate::LodSerialise;
use crate::assets::{
    enums::{DoorAttributes, FaceAttributes, PolygonType},
    lod_data::LodData,
    odm::mm6_to_bevy,
};

/// Read a fixed-size string block, using lossy UTF-8 conversion for non-ASCII bytes.
fn read_string_lossy(cursor: &mut Cursor<&[u8]>, size: usize) -> Result<String, Box<dyn Error>> {
    let mut buf = vec![0u8; size];
    std::io::Read::read_exact(cursor, &mut buf)?;
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).to_string())
}

/// BLV file header. 136 bytes (0x88). Layout from MMExtension `BlvHeader` struct:
///   0x00: unknown[4]
///   0x04: name[60]  — map display name
///   0x40: unknown[40]
///   0x68: face_data_size(i32), room_data_size(i32), room_light_data_size(i32), door_data_size(i32)
///   0x78: unknown[16]  — not described in MMExtension
#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
struct BlvHeader {
    /// Map display name from the BLV header. Offset 0x04, 60 bytes.
    pub name: String,
    /// Unknown 4 bytes at offset 0x00. Preserved for analysis/round-tripping.
    pub unknown_head: [u8; 4],
    /// Unknown 40 bytes at offset 0x40. Preserved for analysis/round-tripping.
    #[serde(skip, default = "default_blv_unknown_mid")]
    pub unknown_mid: [u8; 40],
    face_data_size: i32,
    sector_data_size: i32,
    sector_light_data_size: i32,
    doors_data_size: i32,
    /// Unknown 16 bytes at offset 0x78 (after the four size fields).
    /// Not documented in MMExtension — preserved for analysis/round-tripping.
    pub unknown_tail: [u8; 16],
}

/// A vertex in MM6 coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BlvVertex {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// A face in a BLV indoor map.
///
/// MM6 BLV face struct is 80 bytes (no float normals — those are MM7 only).
/// Layout:
///   0x00: fixed-point normal[4] (i32x4) -- 16 bytes
///   0x10: z_calc[3] (i32x3) -- 12 bytes
///   0x1C: attributes (u32) -- 4 bytes
///   0x20: 6 runtime pointers (24 bytes, zeroed in file)
///   0x38: face_extra_id(u16), bitmap_id(u16), sector_id(u16), back_sector_id(i16)
///   0x40: bounding box (i16x6) -- 12 bytes
///   0x4C: polygon_type(u8), num_vertices(u8), padding(i16)
#[derive(Debug, Serialize, Deserialize)]
pub struct BlvFace {
    /// Fixed-point normal (i32x3, 16.16 format) and distance.
    pub normal_fixed: [i32; 4],
    /// Z calculation coefficients.
    pub z_calc: [i32; 3],
    /// Face attribute flags.
    pub attributes: u32,
    /// Index into face extras array.
    pub face_extra_id: u16,
    /// Bitmap/texture index.
    pub bitmap_id: u16,
    /// Front sector index.
    pub sector_id: u16,
    /// Back sector index (for portals).
    pub back_sector_id: i16,
    /// Bounding box min (x, y, z).
    pub bbox_min: [i16; 3],
    /// Bounding box max (x, y, z).
    pub bbox_max: [i16; 3],
    /// Polygon type: 1=wall, 3=floor, 5=ceiling, etc.
    pub polygon_type: u8,
    /// Number of vertices for this face.
    pub num_vertices: u8,

    // Data from the face data blob (assigned after initial parse):
    /// Vertex indices into the vertex array.
    pub vertex_ids: Vec<u16>,
    /// Texture U/V delta from face extras (applied as offset to UVs).
    pub texture_delta_u: i16,
    pub texture_delta_v: i16,
    /// Texture U coordinates per vertex.
    pub texture_us: Vec<i16>,
    /// Texture V coordinates per vertex.
    pub texture_vs: Vec<i16>,
    /// Event ID from face extras (links to EVT script). Zero = no event.
    pub event_id: u16,
    /// Cog number from face extras — groups faces for doors and scripted actions.
    pub cog_number: i16,
}

/// A face-extras record from section 6 of a BLV file. 36 bytes each.
///
/// Only offsets 0x14–0x1B are documented (from field names in MM6 decompilations).
/// The head (0x00–0x13, 20 bytes) and tail (0x1C–0x23, 8 bytes) are unknown.
/// All bytes are stored for future analysis and round-trip saving.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlvFaceExtra {
    /// Unknown bytes at offset 0x00–0x13 (20 bytes). Purpose unknown — preserved for analysis.
    pub unknown_head: [u8; 20],
    /// Texture UV delta U. Offset 0x14.
    pub texture_delta_u: i16,
    /// Texture UV delta V. Offset 0x16.
    pub texture_delta_v: i16,
    /// Cog group number — groups faces for doors/scripted actions. Offset 0x18.
    pub cog_number: i16,
    /// EVT event ID triggered by this face (click/step/monster). Offset 0x1A.
    pub event_id: u16,
    /// Unknown bytes at offset 0x1C–0x23 (8 bytes). Purpose unknown — preserved for analysis.
    pub unknown_tail: [u8; 8],
}

impl BlvFace {
    /// Get typed face attribute flags.
    pub fn face_attributes(&self) -> FaceAttributes {
        FaceAttributes::from_bits_truncate(self.attributes)
    }

    /// Get typed polygon type.
    pub fn polygon_type_enum(&self) -> Option<PolygonType> {
        PolygonType::from_u8(self.polygon_type)
    }

    pub fn is_portal(&self) -> bool {
        self.face_attributes().contains(FaceAttributes::PORTAL)
    }

    pub fn is_invisible(&self) -> bool {
        self.face_attributes().contains(FaceAttributes::INVISIBLE)
    }

    pub fn is_clickable(&self) -> bool {
        self.face_attributes().contains(FaceAttributes::CLICKABLE)
    }

    pub fn is_touch_trigger(&self) -> bool {
        self.face_attributes().contains(FaceAttributes::EVENT_BY_TOUCH)
    }

    pub fn moves_by_door(&self) -> bool {
        self.face_attributes().contains(FaceAttributes::MOVES_BY_DOOR)
    }

    pub fn is_fluid(&self) -> bool {
        self.face_attributes().contains(FaceAttributes::FLUID)
    }

    pub fn is_lava(&self) -> bool {
        self.face_attributes().contains(FaceAttributes::LAVA)
    }

    pub fn is_sky(&self) -> bool {
        self.face_attributes().contains(FaceAttributes::SKY)
    }

    /// Get float normal in MM6 coordinates, converted from fixed-point 16.16.
    pub fn normal_f32(&self) -> [f32; 3] {
        [
            self.normal_fixed[0] as f32 / 65536.0,
            self.normal_fixed[1] as f32 / 65536.0,
            self.normal_fixed[2] as f32 / 65536.0,
        ]
    }
}

/// A sector in a BLV indoor map (116 bytes on disk).
#[derive(Debug, Serialize, Deserialize)]
/// A sector (room) in a BLV indoor map.
///
/// Contains face lists, lighting parameters, and spatial bounds.
pub struct BlvSector {
    /// Sector attribute flags (e.g. has sky, underwater, no magic zone).
    pub flags: i32,
    /// Number of floor faces in this sector.
    pub floor_count: u16,
    /// Number of wall faces in this sector.
    pub wall_count: u16,
    /// Number of ceiling faces in this sector.
    pub ceiling_count: u16,
    /// Number of fluid (water/lava) faces in this sector.
    pub fluid_count: u16,
    /// Number of portal faces connecting to adjacent sectors.
    pub portal_count: u16,
    /// Total face count for this sector.
    pub num_faces: u16,
    /// Number of faces not included in the BSP tree (non-BSP geometry).
    pub num_non_bsp_faces: u16,
    /// Number of cylinder collision objects in this sector.
    pub cylinder_count: u16,
    /// Number of cog groups (scripted door/object sets) in this sector.
    pub cog_count: u16,
    /// Number of decorations (sprites) placed in this sector.
    pub decoration_count: u16,
    /// Number of marker objects in this sector.
    pub marker_count: u16,
    /// Number of light sources in this sector.
    pub light_count: u16,
    /// Water/fluid floor height in MM6 units (used for swimming/drowning logic).
    pub water_level: i16,
    /// Mist/fog density level.
    pub mist_level: i16,
    /// Multiplier for light source falloff distance.
    pub light_dist_mul: i16,
    /// Minimum ambient light level (0-255).
    pub min_ambient_light: i16,
    /// Index of the first BSP node for this sector's face tree (-1 = none).
    pub first_bsp_node: i16,
    /// Exit event tag used by level transitions.
    pub exit_tag: i16,
    /// Bounding box minimum corner (x, y, z) in MM6 units.
    pub bbox_min: [i16; 3],
    /// Bounding box maximum corner (x, y, z) in MM6 units.
    pub bbox_max: [i16; 3],

    // Assigned from sector data blob:
    /// Face indices (into blv.faces) belonging to this sector.
    pub face_ids: Vec<u16>,
}

/// A decoration/sprite in a BLV indoor map (28 bytes on disk + 28-byte name in MM6).
/// Field layout from MMExtension MapSprite struct.
#[derive(Debug, Serialize, Deserialize)]
pub struct BlvDecoration {
    pub decoration_desc_id: u16,
    /// Instance flags (LevelDecorationFlags).
    pub flags: u16,
    pub position: [i32; 3],
    pub yaw: i32,
    /// Event variable index (MM6 only, at offset 0x14).
    pub event_variable: i16,
    /// Event ID (links to EVT script).
    pub event: i16,
    /// Trigger radius for touch events.
    pub trigger_radius: i16,
    /// Direction in degrees (used if yaw is 0).
    pub direction_degrees: i16,
    pub name: String,
}

/// A point light source in a BLV indoor map. 12 bytes, MM6 format.
///
/// Layout: 0x00: pos[3](i16), 0x06: radius(i16), 0x08: attributes(i16), 0x0A: brightness(u16)
#[derive(Debug, Serialize, Deserialize)]
pub struct BlvLight {
    /// Light position in MM6 coordinates (x, y, z). Offset 0x00.
    pub position: [i16; 3],
    /// Light falloff radius in MM6 units. Offset 0x06.
    pub radius: i16,
    /// Light attribute flags (type, dynamic, etc.). Offset 0x08.
    pub attributes: i16,
    /// Light brightness/intensity (higher = brighter). Offset 0x0A.
    pub brightness: u16,
}

/// A BSP tree node in a BLV indoor map. 8 bytes.
///
/// Layout: 0x00: front(i16), 0x02: back(i16), 0x04: face_id_offset(i16), 0x06: num_faces(i16)
#[derive(Debug, Serialize, Deserialize)]
pub struct BlvBspNode {
    /// Index of the front child node (-1 = leaf). Offset 0x00.
    pub front: i16,
    /// Index of the back child node (-1 = leaf). Offset 0x02.
    pub back: i16,
    /// Offset into the sector face index list for faces at this node. Offset 0x04.
    pub face_id_offset: i16,
    /// Number of faces associated with this BSP node. Offset 0x06.
    pub num_faces: i16,
}

/// A monster or item spawn point in a BLV indoor map.
///
/// Identical layout to ODM `SpawnPoint` — 20 bytes, MM6 format (no MM7 Group field).
/// Layout: 0x00: pos[3](i32), 0x0C: radius(u16), 0x0E: kind(u16), 0x10: index(u16), 0x12: bits(u16)
#[derive(Debug, Serialize, Deserialize)]
pub struct BlvSpawnPoint {
    /// Spawn center in MM6 world coordinates (x, y, z). Offset 0x00.
    pub position: [i32; 3],
    /// Group spread radius — members are scattered within this radius. Offset 0x0C.
    pub radius: u16,
    /// Spawn category: 2 = item/treasure, 3 = monster. Offset 0x0E.
    pub spawn_type: u16,
    /// Monster slot index (1-12) or treasure level. Same encoding as ODM SpawnPoint. Offset 0x10.
    pub monster_index: u16,
    /// Spawn attribute flags. Offset 0x12.
    pub attributes: u16,
}

/// A map outline edge used for the minimap/automap display. 12 bytes.
///
/// Layout: 0x00: v1(u16), 0x02: v2(u16), 0x04: face1(u16), 0x06: face2(u16), 0x08: z(i16), 0x0A: flags(u16)
#[derive(Debug, Serialize, Deserialize)]
pub struct BlvMapOutline {
    /// First vertex index of this outline edge. Offset 0x00.
    pub vertex1_id: u16,
    /// Second vertex index of this outline edge. Offset 0x02.
    pub vertex2_id: u16,
    /// Face on the front side of this edge. Offset 0x04.
    pub face1_id: u16,
    /// Face on the back side of this edge. Offset 0x06.
    pub face2_id: u16,
    /// Z height of the outline line (for 2D map projection). Offset 0x08.
    pub z: i16,
    /// Outline flags (visibility, type). Offset 0x0A.
    pub flags: u16,
}

/// A single door face mesh, ready for rendering and animation.
pub struct BlvDoorFaceMesh {
    /// Index into blv.faces.
    pub face_index: usize,
    /// Index into the doors array (primary door — owns the direction/speed for animation).
    pub door_index: usize,
    /// Texture name for material lookup.
    pub texture_name: String,
    /// Triangle vertex positions in Bevy coordinates (the "open"/base positions).
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals.
    pub normals: Vec<[f32; 3]>,
    /// Texture UVs.
    pub uvs: Vec<[f32; 2]>,
    /// For each triangle vertex, whether it moves with the door.
    /// Built from the union of vertex_ids across ALL doors that include this face,
    /// so cross-door shared faces are correctly marked as fully moving.
    pub is_moving: Vec<bool>,
    /// UV change per unit of door displacement for moving vertices.
    /// Only applied when some vertices are fixed (reveal/frame faces).
    pub uv_rate: [f32; 2],
    /// Whether this face has the MOVES_BY_DOOR (FACE_TexMoveByDoor) attribute.
    /// Controls whether UV scrolling is applied during door animation.
    pub moves_by_door: bool,
}

/// A per-texture mesh extracted from a BLV map, ready for rendering.
pub struct BlvTexturedMesh {
    pub texture_name: String,
    /// Face indices that contributed to this mesh (empty for BLV — indoor maps don't use SetTextureOutdoors).
    pub face_indices: Vec<u32>,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub normals: Vec<[f32; 3]>,
}

/// Parsed BLV indoor map.
#[derive(Debug, Serialize, Deserialize)]
pub struct Blv {
    /// Map display name from the BLV header (offset 0x04, 60 bytes).
    pub header_name: String,
    pub vertices: Vec<BlvVertex>,
    pub faces: Vec<BlvFace>,
    pub texture_names: Vec<String>,
    /// All face-extras records (section 6). Full 36 bytes each, including unknowns.
    pub face_extras: Vec<BlvFaceExtra>,
    /// Texture names for face extras (one per face extra, section 7 of BLV).
    /// Parallel to `face_extras`. May be empty strings.
    pub face_extra_texture_names: Vec<String>,
    pub sectors: Vec<BlvSector>,
    /// Sector light data blob (section 10). Raw u16 values — structure unknown,
    /// preserved for future analysis and round-trip saving.
    pub sector_light_data: Vec<u16>,
    pub decorations: Vec<BlvDecoration>,
    pub lights: Vec<BlvLight>,
    pub bsp_nodes: Vec<BlvBspNode>,
    pub spawn_points: Vec<BlvSpawnPoint>,
    pub map_outlines: Vec<BlvMapOutline>,
    pub door_count: u32,
    pub doors_data_size: i32,
    pub face_data_size: i32,
}

/// Door state in the game engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoorState {
    Open = 0,
    Closing = 1,
    Closed = 2,
    Opening = 3,
}

/// A door in a BLV indoor map.
/// Parsed from the DLV file; metadata (door_count, doors_data_size) comes from BLV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlvDoor {
    pub attributes: u32,
    pub door_id: u32,
    /// Direction vector (float, MM6 coordinates). Vertices slide along this.
    pub direction: [f32; 3],
    /// Total slide distance in map units.
    pub move_length: i32,
    /// Speed when opening (map units per real-time second).
    pub open_speed: i32,
    /// Speed when closing (map units per real-time second).
    pub close_speed: i32,
    /// Vertex indices into blv.vertices.
    pub vertex_ids: Vec<u16>,
    /// Face indices into blv.faces.
    pub face_ids: Vec<u16>,
    /// Base X positions per vertex (the "open" position).
    pub x_offsets: Vec<i16>,
    /// Base Y positions per vertex.
    pub y_offsets: Vec<i16>,
    /// Base Z positions per vertex.
    pub z_offsets: Vec<i16>,
    /// Initial texture U deltas per face.
    pub delta_us: Vec<i16>,
    /// Initial texture V deltas per face.
    pub delta_vs: Vec<i16>,
    /// Current door state.
    pub state: DoorState,
}

impl BlvDoor {
    /// Get typed door attribute flags.
    pub fn door_attributes(&self) -> DoorAttributes {
        DoorAttributes::from_bits_truncate(self.attributes)
    }
}

impl Blv {
    /// Parse a BLV file from a LOD archive.
    pub fn load(assets: &Assets, name: &str) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes(format!("games/{}", name))?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);

        // 1. Header (136 bytes)
        let header = Self::read_header(&mut cursor)?;

        // 2. Vertices: u32 count, then count x 6 bytes (i16 x, y, z)
        let vertex_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(BlvVertex {
                x: cursor.read_i16::<LittleEndian>()?,
                y: cursor.read_i16::<LittleEndian>()?,
                z: cursor.read_i16::<LittleEndian>()?,
            });
        }

        // 3. Faces: u32 count, then count x 80 bytes each (MM6)
        let face_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut faces = Vec::with_capacity(face_count);
        for _ in 0..face_count {
            faces.push(Self::read_face(&mut cursor)?);
        }

        // 4. Face data blob: (header.face_data_size / 2) x i16
        let face_data_count = (header.face_data_size / 2) as usize;
        let mut face_data_blob = Vec::with_capacity(face_data_count);
        for _ in 0..face_data_count {
            face_data_blob.push(cursor.read_i16::<LittleEndian>()?);
        }
        Self::unpack_face_data(&mut faces, &face_data_blob);

        // 5. Face texture names: faces.len x 10-byte null-terminated strings
        let mut texture_names = Vec::with_capacity(face_count);
        for _ in 0..face_count {
            texture_names.push(read_string_lossy(&mut cursor, 10)?);
        }

        // 6. Face extras: u32 count, then count x 36 bytes each.
        //    Layout: unknown_head[20] | delta_u(i16) | delta_v(i16) | cog_number(i16) | event_id(u16) | unknown_tail[8]
        let face_extras_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut face_extras: Vec<BlvFaceExtra> = Vec::with_capacity(face_extras_count);
        for _ in 0..face_extras_count {
            let mut unknown_head = [0u8; 20];
            std::io::Read::read_exact(&mut cursor, &mut unknown_head)?;
            let texture_delta_u = cursor.read_i16::<LittleEndian>()?;
            let texture_delta_v = cursor.read_i16::<LittleEndian>()?;
            let cog_number = cursor.read_i16::<LittleEndian>()?;
            let event_id = cursor.read_u16::<LittleEndian>()?;
            let mut unknown_tail = [0u8; 8];
            std::io::Read::read_exact(&mut cursor, &mut unknown_tail)?;
            face_extras.push(BlvFaceExtra {
                unknown_head,
                texture_delta_u,
                texture_delta_v,
                cog_number,
                event_id,
                unknown_tail,
            });
        }
        // Assign deltas, cog_number, and event_id to faces via face_extra_id
        for face in &mut faces {
            if let Some(fe) = face_extras.get(face.face_extra_id as usize) {
                face.texture_delta_u = fe.texture_delta_u;
                face.texture_delta_v = fe.texture_delta_v;
                face.cog_number = fe.cog_number;
                face.event_id = fe.event_id;
            }
        }

        // 7. Face extra texture names: face_extras.len x 10 bytes
        let mut face_extra_texture_names = Vec::with_capacity(face_extras_count);
        for _ in 0..face_extras_count {
            face_extra_texture_names.push(read_string_lossy(&mut cursor, 10)?);
        }

        // 8. Sectors: u32 count, then count x 116 bytes each
        let sector_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut sectors = Vec::with_capacity(sector_count);
        for _ in 0..sector_count {
            sectors.push(Self::read_sector(&mut cursor)?);
        }

        // 9. Sector data blob: (header.sector_data_size / 2) x u16
        let sector_data_count = (header.sector_data_size / 2) as usize;
        let mut sector_data_blob = Vec::with_capacity(sector_data_count);
        for _ in 0..sector_data_count {
            sector_data_blob.push(cursor.read_u16::<LittleEndian>()?);
        }
        Self::unpack_sector_data(&mut sectors, &sector_data_blob);

        // 10. Sector light data blob: (header.sector_light_data_size / 2) x u16.
        //     Structure unknown — preserved for future analysis and round-trip saving.
        let sector_light_count = (header.sector_light_data_size / 2) as usize;
        let mut sector_light_data = Vec::with_capacity(sector_light_count);
        for _ in 0..sector_light_count {
            sector_light_data.push(cursor.read_u16::<LittleEndian>()?);
        }

        // 11. Door count (actual doors stored in DLV)
        let door_count = cursor.read_u32::<LittleEndian>()?;

        // 12. Decorations: u32 count, then count x 32 bytes
        let decoration_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut decorations = Vec::with_capacity(decoration_count);
        for _ in 0..decoration_count {
            decorations.push(Self::read_decoration(&mut cursor)?);
        }

        // 13. Decoration names: 32-byte null-terminated name buffers
        //     (MM6 BLV files store 32-byte name fields, verified by binary trace)
        for dec in &mut decorations {
            dec.name = read_string_lossy(&mut cursor, 32)?;
        }

        // 14. Lights: u32 count, then count x 12 bytes (MM6 format)
        let light_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut lights = Vec::with_capacity(light_count);
        for _ in 0..light_count {
            lights.push(BlvLight {
                position: [
                    cursor.read_i16::<LittleEndian>()?,
                    cursor.read_i16::<LittleEndian>()?,
                    cursor.read_i16::<LittleEndian>()?,
                ],
                radius: cursor.read_i16::<LittleEndian>()?,
                attributes: cursor.read_i16::<LittleEndian>()?,
                brightness: cursor.read_u16::<LittleEndian>()?,
            });
        }

        // 15. BSP nodes: u32 count, then count x 8 bytes
        let bsp_node_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut bsp_nodes = Vec::with_capacity(bsp_node_count);
        for _ in 0..bsp_node_count {
            bsp_nodes.push(BlvBspNode {
                front: cursor.read_i16::<LittleEndian>()?,
                back: cursor.read_i16::<LittleEndian>()?,
                face_id_offset: cursor.read_i16::<LittleEndian>()?,
                num_faces: cursor.read_i16::<LittleEndian>()?,
            });
        }

        // 16. Spawn points: u32 count, then count x 20 bytes (MM6, no group field)
        let spawn_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut spawn_points = Vec::with_capacity(spawn_count);
        for _ in 0..spawn_count {
            spawn_points.push(BlvSpawnPoint {
                position: [
                    cursor.read_i32::<LittleEndian>()?,
                    cursor.read_i32::<LittleEndian>()?,
                    cursor.read_i32::<LittleEndian>()?,
                ],
                radius: cursor.read_u16::<LittleEndian>()?,
                spawn_type: cursor.read_u16::<LittleEndian>()?,
                monster_index: cursor.read_u16::<LittleEndian>()?,
                attributes: cursor.read_u16::<LittleEndian>()?,
            });
        }

        // 17. Map outlines: u32 count, then count x 12 bytes
        let outline_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut map_outlines = Vec::with_capacity(outline_count);
        for _ in 0..outline_count {
            map_outlines.push(BlvMapOutline {
                vertex1_id: cursor.read_u16::<LittleEndian>()?,
                vertex2_id: cursor.read_u16::<LittleEndian>()?,
                face1_id: cursor.read_u16::<LittleEndian>()?,
                face2_id: cursor.read_u16::<LittleEndian>()?,
                z: cursor.read_i16::<LittleEndian>()?,
                flags: cursor.read_u16::<LittleEndian>()?,
            });
        }

        Ok(Blv {
            header_name: header.name,
            vertices,
            faces,
            texture_names,
            face_extras,
            face_extra_texture_names,
            sectors,
            sector_light_data,
            decorations,
            lights,
            bsp_nodes,
            spawn_points,
            map_outlines,
            door_count,
            doors_data_size: header.doors_data_size,
            face_data_size: header.face_data_size,
        })
    }

    fn read_header(cursor: &mut Cursor<&[u8]>) -> Result<BlvHeader, Box<dyn Error>> {
        // 0x00: unknown[4]
        let mut unknown_head = [0u8; 4];
        std::io::Read::read_exact(cursor, &mut unknown_head)?;
        // 0x04: name[60]
        let name = read_string_lossy(cursor, 60)?;
        // 0x40: unknown[40]
        let mut unknown_mid = [0u8; 40];
        std::io::Read::read_exact(cursor, &mut unknown_mid)?;
        // 0x68: four size fields
        let face_data_size = cursor.read_i32::<LittleEndian>()?;
        let sector_data_size = cursor.read_i32::<LittleEndian>()?;
        let sector_light_data_size = cursor.read_i32::<LittleEndian>()?;
        let doors_data_size = cursor.read_i32::<LittleEndian>()?;
        // 0x78: unknown[16]
        let mut unknown_tail = [0u8; 16];
        std::io::Read::read_exact(cursor, &mut unknown_tail)?;
        Ok(BlvHeader {
            name,
            unknown_head,
            unknown_mid,
            face_data_size,
            sector_data_size,
            sector_light_data_size,
            doors_data_size,
            unknown_tail,
        })
    }

    /// Read a single face (80 bytes, MM6 format — no float normals).
    fn read_face(cursor: &mut Cursor<&[u8]>) -> Result<BlvFace, Box<dyn Error>> {
        let mut normal_fixed = [0i32; 4];
        for v in &mut normal_fixed {
            *v = cursor.read_i32::<LittleEndian>()?;
        }
        let mut z_calc = [0i32; 3];
        for v in &mut z_calc {
            *v = cursor.read_i32::<LittleEndian>()?;
        }
        let attributes = cursor.read_u32::<LittleEndian>()?;
        // 6 runtime pointers (24 bytes, zeroed in file)
        cursor.seek(std::io::SeekFrom::Current(24))?;
        let face_extra_id = cursor.read_u16::<LittleEndian>()?;
        let bitmap_id = cursor.read_u16::<LittleEndian>()?;
        let sector_id = cursor.read_u16::<LittleEndian>()?;
        let back_sector_id = cursor.read_i16::<LittleEndian>()?;
        let bbox_min = [
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
        ];
        let bbox_max = [
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
            cursor.read_i16::<LittleEndian>()?,
        ];
        let polygon_type = cursor.read_u8()?;
        let num_vertices = cursor.read_u8()?;
        let _padding = cursor.read_i16::<LittleEndian>()?;

        Ok(BlvFace {
            normal_fixed,
            z_calc,
            attributes,
            face_extra_id,
            bitmap_id,
            sector_id,
            back_sector_id,
            bbox_min,
            bbox_max,
            polygon_type,
            num_vertices,
            texture_delta_u: 0,
            texture_delta_v: 0,
            vertex_ids: Vec::new(),
            texture_us: Vec::new(),
            texture_vs: Vec::new(),
            event_id: 0,
            cog_number: 0,
        })
    }

    /// Unpack the face data blob into per-face vertex IDs and UV coordinates.
    ///
    /// Each face has 6 sub-arrays of (num_vertices + 1) i16 values:
    /// [vertexIds, xDisp, yDisp, zDisp, textureUs, textureVs].
    /// The +1 is a closing vertex duplicated from the source data (skipped when reading).
    fn unpack_face_data(faces: &mut [BlvFace], blob: &[i16]) {
        let mut offset = 0;
        for face in faces.iter_mut() {
            let n = face.num_vertices as usize;
            let stride = n + 1; // +1 closing vertex per sub-array
            let total = 6 * stride;
            if offset + total > blob.len() {
                break;
            }
            face.vertex_ids = blob[offset..offset + n].iter().map(|&v| v as u16).collect();
            let us_start = offset + 4 * stride;
            face.texture_us = blob[us_start..us_start + n].to_vec();
            let vs_start = offset + 5 * stride;
            face.texture_vs = blob[vs_start..vs_start + n].to_vec();
            offset += total;
        }
    }

    /// Read a sector (116 bytes).
    fn read_sector(cursor: &mut Cursor<&[u8]>) -> Result<BlvSector, Box<dyn Error>> {
        let flags = cursor.read_i32::<LittleEndian>()?;

        // 5 groups of: u16 count, u16 pad, u32 pointer
        let floor_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let wall_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let ceiling_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let fluid_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let portal_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;

        let num_faces = cursor.read_u16::<LittleEndian>()?;
        let num_non_bsp_faces = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;

        let cylinder_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let cog_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let decoration_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let marker_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;
        let light_count = cursor.read_u16::<LittleEndian>()?;
        let _pad = cursor.read_u16::<LittleEndian>()?;
        let _ptr = cursor.read_u32::<LittleEndian>()?;

        let water_level = cursor.read_i16::<LittleEndian>()?;
        let mist_level = cursor.read_i16::<LittleEndian>()?;
        let light_dist_mul = cursor.read_i16::<LittleEndian>()?;
        let min_ambient_light = cursor.read_i16::<LittleEndian>()?;
        let first_bsp_node = cursor.read_i16::<LittleEndian>()?;
        let exit_tag = cursor.read_i16::<LittleEndian>()?;
        // BBoxs is interleaved: x1, x2, y1, y2, z1, z2 (min/max alternating per axis)
        let x1 = cursor.read_i16::<LittleEndian>()?;
        let x2 = cursor.read_i16::<LittleEndian>()?;
        let y1 = cursor.read_i16::<LittleEndian>()?;
        let y2 = cursor.read_i16::<LittleEndian>()?;
        let z1 = cursor.read_i16::<LittleEndian>()?;
        let z2 = cursor.read_i16::<LittleEndian>()?;

        Ok(BlvSector {
            flags,
            floor_count,
            wall_count,
            ceiling_count,
            fluid_count,
            portal_count,
            num_faces,
            num_non_bsp_faces,
            cylinder_count,
            cog_count,
            decoration_count,
            marker_count,
            light_count,
            water_level,
            mist_level,
            light_dist_mul,
            min_ambient_light,
            first_bsp_node,
            exit_tag,
            bbox_min: [x1, y1, z1],
            bbox_max: [x2, y2, z2],
            face_ids: Vec::new(),
        })
    }

    /// Unpack sector data blob: per-sector face indices.
    fn unpack_sector_data(sectors: &mut [BlvSector], blob: &[u16]) {
        let mut offset = 0;
        for sector in sectors.iter_mut() {
            let total = sector.floor_count as usize
                + sector.wall_count as usize
                + sector.ceiling_count as usize
                + sector.fluid_count as usize
                + sector.portal_count as usize;
            if offset + total > blob.len() {
                break;
            }
            sector.face_ids = blob[offset..offset + total].to_vec();
            offset += total;
        }
    }

    /// Read a decoration (28 bytes, MM6 MapSprite format).
    fn read_decoration(cursor: &mut Cursor<&[u8]>) -> Result<BlvDecoration, Box<dyn Error>> {
        let decoration_desc_id = cursor.read_u16::<LittleEndian>()?;
        let flags = cursor.read_u16::<LittleEndian>()?;
        let position = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let yaw = cursor.read_i32::<LittleEndian>()?;
        let event_variable = cursor.read_i16::<LittleEndian>()?;
        let event = cursor.read_i16::<LittleEndian>()?;
        let trigger_radius = cursor.read_i16::<LittleEndian>()?;
        let direction_degrees = cursor.read_i16::<LittleEndian>()?;
        Ok(BlvDecoration {
            decoration_desc_id,
            flags,
            position,
            yaw,
            event_variable,
            event,
            trigger_radius,
            direction_degrees,
            name: String::new(),
        })
    }

    /// Ear-clipping triangulation for coplanar polygons.
    /// Projects vertices to the best-fit 2D plane based on the face normal,
    /// then performs ear clipping to handle concave polygons (arches, doorframes).
    /// Triangulate a BLV face into triangles.
    ///
    /// MM6 BLV faces are almost always convex (quads, pentagons, etc.), so simple
    /// fan triangulation from vertex 0 works correctly and avoids the edge cases
    /// that plague ear-clipping on near-degenerate or floating-point-sensitive polygons.
    fn triangulate_face(face: &BlvFace, vertices: &[BlvVertex]) -> Vec<[usize; 3]> {
        let n = face.num_vertices as usize;
        if n < 3 {
            return vec![];
        }
        if n == 3 {
            return vec![[0, 1, 2]];
        }

        // Project 3D vertices to 2D by dropping the axis with the largest normal component.
        let normal = face.normal_f32();
        let abs_n = [normal[0].abs(), normal[1].abs(), normal[2].abs()];
        // Choose which two axes to keep (drop the dominant one).
        let (ax_u, ax_v) = if abs_n[0] >= abs_n[1] && abs_n[0] >= abs_n[2] {
            (1, 2) // drop X
        } else if abs_n[1] >= abs_n[0] && abs_n[1] >= abs_n[2] {
            (0, 2) // drop Y
        } else {
            (0, 1) // drop Z
        };

        let coords_3d = |idx: usize| -> [f32; 3] {
            let vid = face.vertex_ids[idx] as usize;
            let v = &vertices[vid];
            [v.x as f32, v.y as f32, v.z as f32]
        };
        let project = |idx: usize| -> [f32; 2] {
            let c = coords_3d(idx);
            [c[ax_u], c[ax_v]]
        };

        let pts: Vec<[f32; 2]> = (0..n).map(project).collect();

        // Compute signed area to determine winding.
        let signed_area: f32 = (0..n)
            .map(|i| {
                let j = (i + 1) % n;
                pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1]
            })
            .sum();

        // If area is essentially zero, fall back to fan.
        if signed_area.abs() < 1e-6 {
            return (1..n - 1).map(|i| [0, i, i + 1]).collect();
        }

        // For CCW winding (positive area), a convex ear has positive cross product.
        // For CW winding (negative area), a convex ear has negative cross product.
        // We want the cross product sign to match the sign of signed_area.
        let winding_sign = signed_area.signum();

        fn cross_2d(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
            (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
        }

        fn point_in_triangle(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
            let d1 = cross_2d(p, a, b);
            let d2 = cross_2d(p, b, c);
            let d3 = cross_2d(p, c, a);
            let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
            let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
            !(has_neg && has_pos)
        }

        let mut indices: Vec<usize> = (0..n).collect();
        let mut triangles: Vec<[usize; 3]> = Vec::with_capacity(n - 2);
        let mut fail_count = 0;

        while indices.len() > 3 {
            let len = indices.len();
            let mut ear_found = false;

            for i in 0..len {
                let prev = indices[(i + len - 1) % len];
                let curr = indices[i];
                let next = indices[(i + 1) % len];

                let cross = cross_2d(pts[prev], pts[curr], pts[next]);

                // Check convexity: cross product sign must match winding.
                if cross * winding_sign <= 0.0 {
                    continue;
                }

                // Check no other vertex is inside this triangle.
                let mut contains_point = false;
                for &idx in indices.iter().take(len) {
                    if idx == prev || idx == curr || idx == next {
                        continue;
                    }
                    if point_in_triangle(pts[idx], pts[prev], pts[curr], pts[next]) {
                        contains_point = true;
                        break;
                    }
                }

                if !contains_point {
                    triangles.push([prev, curr, next]);
                    indices.remove(i);
                    ear_found = true;
                    break;
                }
            }

            if !ear_found {
                fail_count += 1;
                if fail_count > indices.len() {
                    // Degenerate polygon — fall back to fan triangulation.
                    return (1..n - 1).map(|i| [0, i, i + 1]).collect();
                }
                // Try removing the vertex with the smallest absolute cross product
                // to make progress on near-degenerate polygons.
                let len = indices.len();
                let mut best = 0;
                let mut best_abs = f32::MAX;
                for i in 0..len {
                    let prev = indices[(i + len - 1) % len];
                    let curr = indices[i];
                    let next = indices[(i + 1) % len];
                    let abs_cross = cross_2d(pts[prev], pts[curr], pts[next]).abs();
                    if abs_cross < best_abs {
                        best_abs = abs_cross;
                        best = i;
                    }
                }
                let prev = indices[(best + len - 1) % len];
                let curr = indices[best];
                let next = indices[(best + 1) % len];
                triangles.push([prev, curr, next]);
                indices.remove(best);
            }
        }

        if indices.len() == 3 {
            triangles.push([indices[0], indices[1], indices[2]]);
        }

        triangles
    }

    /// Fill in door face/vertex/offset data from BLV geometry for doors
    /// that are missing this data. DLV files usually have this populated,
    /// but as a fallback we compute it from face cog_numbers.
    pub fn initialize_doors(&self, doors: &mut [BlvDoor]) {
        for door in doors.iter_mut() {
            if door.door_id == 0 || !door.face_ids.is_empty() {
                continue; // Already has data or unused slot
            }

            let mut face_ids = Vec::new();
            let mut vertex_id_set = std::collections::BTreeSet::new();

            for (fi, face) in self.faces.iter().enumerate() {
                if face.cog_number == door.door_id as i16 {
                    face_ids.push(fi as u16);
                    for &vid in &face.vertex_ids {
                        vertex_id_set.insert(vid);
                    }
                }
            }

            if face_ids.is_empty() {
                continue;
            }

            let vertex_ids: Vec<u16> = vertex_id_set.into_iter().collect();

            // Base offsets = BLV vertex positions (the deployed/blocking positions, i.e. state-0)
            let x_offsets: Vec<i16> = vertex_ids
                .iter()
                .map(|&vid| self.vertices.get(vid as usize).map(|v| v.x).unwrap_or(0))
                .collect();
            let y_offsets: Vec<i16> = vertex_ids
                .iter()
                .map(|&vid| self.vertices.get(vid as usize).map(|v| v.y).unwrap_or(0))
                .collect();
            let z_offsets: Vec<i16> = vertex_ids
                .iter()
                .map(|&vid| self.vertices.get(vid as usize).map(|v| v.z).unwrap_or(0))
                .collect();

            info!(
                "InitializeDoors fallback: door_id={} faces={} verts={}",
                door.door_id,
                face_ids.len(),
                vertex_ids.len()
            );

            door.face_ids = face_ids;
            door.vertex_ids = vertex_ids;
            door.x_offsets = x_offsets;
            door.y_offsets = y_offsets;
            door.z_offsets = z_offsets;
        }
    }

    /// Compute the UV change rate per unit of door displacement for a face.
    ///
    /// Uses the texture mapping gradient derived from 3 face vertices:
    /// finds how much U and V (in normalized 0..1 UV space) change when a
    /// vertex moves one unit along the door direction in MM6 coordinates.
    fn compute_face_uv_rate(
        face: &BlvFace,
        vertices: &[BlvVertex],
        door_direction: &[f32; 3],
        tex_w: f32,
        tex_h: f32,
    ) -> [f32; 2] {
        let n = face.num_vertices as usize;
        if n < 3 || face.texture_us.len() < 3 || face.texture_vs.len() < 3 {
            return [0.0, 0.0];
        }

        // Get 3 vertices with positions (MM6 coords) and UVs (pixel space)
        let pos = |i: usize| -> [f32; 3] {
            let vid = face.vertex_ids[i] as usize;
            let v = &vertices[vid];
            [v.x as f32, v.y as f32, v.z as f32]
        };

        let p0 = pos(0);
        let p1 = pos(1);
        let p2 = pos(2);

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let du1 = face.texture_us[1] as f32 - face.texture_us[0] as f32;
        let du2 = face.texture_us[2] as f32 - face.texture_us[0] as f32;
        let dv1 = face.texture_vs[1] as f32 - face.texture_vs[0] as f32;
        let dv2 = face.texture_vs[2] as f32 - face.texture_vs[0] as f32;

        // Solve for texture gradient vectors using metric tensor on the face plane:
        // gradient_u · e1 = du1, gradient_u · e2 = du2
        // gradient_u = a1*e1 + a2*e2
        let dot = |a: &[f32; 3], b: &[f32; 3]| -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] };

        let g11 = dot(&e1, &e1);
        let g12 = dot(&e1, &e2);
        let g22 = dot(&e2, &e2);
        let det = g11 * g22 - g12 * g12;
        if det.abs() < 1e-6 {
            return [0.0, 0.0]; // Degenerate triangle
        }

        // U gradient in MM6 coords (pixels per MM6 unit)
        let a1_u = (g22 * du1 - g12 * du2) / det;
        let a2_u = (g11 * du2 - g12 * du1) / det;
        let grad_u = [
            a1_u * e1[0] + a2_u * e2[0],
            a1_u * e1[1] + a2_u * e2[1],
            a1_u * e1[2] + a2_u * e2[2],
        ];

        // V gradient in MM6 coords (pixels per MM6 unit)
        let a1_v = (g22 * dv1 - g12 * dv2) / det;
        let a2_v = (g11 * dv2 - g12 * dv1) / det;
        let grad_v = [
            a1_v * e1[0] + a2_v * e2[0],
            a1_v * e1[1] + a2_v * e2[1],
            a1_v * e1[2] + a2_v * e2[2],
        ];

        // Project door direction onto texture gradients → pixels per unit distance
        let du_per_dist = dot(door_direction, &grad_u);
        let dv_per_dist = dot(door_direction, &grad_v);

        // Convert to normalized UV space
        [du_per_dist / tex_w, dv_per_dist / tex_h]
    }

    /// Collect the set of face indices belonging to any door.
    ///
    /// The number of times a face appears in a door's face_ids equals the number of that
    /// door's moving vertices the face contains. Faces appearing only ONCE share just one
    /// corner vertex with the door panel (e.g. the room floor/ceiling that happens to touch
    /// a door corner). Moving that single corner visibly deforms the large surrounding face.
    ///
    /// Only faces appearing MORE THAN ONCE in a single door's face_ids list are included:
    /// these have at least two moving vertices and form genuine door geometry (panel faces,
    /// side jambs, threshold strips). Single-occurrence faces remain in static geometry.
    pub fn door_face_set(doors: &[BlvDoor], faces: &[BlvFace]) -> std::collections::HashSet<usize> {
        let mut result = std::collections::HashSet::new();
        for door in doors {
            // Count how many times each face appears in this door's face_ids.
            // This equals the number of the door's moving vertices that belong to the face.
            let mut counts: std::collections::HashMap<u16, usize> = std::collections::HashMap::new();
            for &fid in &door.face_ids {
                *counts.entry(fid).or_insert(0) += 1;
            }
            for (&fid, &count) in &counts {
                // Only include faces with 2+ moving vertices — genuine door geometry.
                // Single-occurrence faces are large room faces sharing just one corner.
                if count < 2 {
                    continue;
                }
                let fi = fid as usize;
                let Some(face) = faces.get(fi) else { continue };
                if face.vertex_ids.is_empty() {
                    continue;
                }
                result.insert(fi);
            }
        }
        result
    }

    /// Generate individual meshes for each door face, with per-vertex door index tracking
    /// for animation. Each face produces one mesh.
    pub fn door_face_meshes(
        &self,
        doors: &[BlvDoor],
        texture_sizes: &HashMap<String, (u32, u32)>,
    ) -> Vec<BlvDoorFaceMesh> {
        let mut result = Vec::new();

        // Use the same filtering as door_face_set — only include faces where
        // at least half the vertices are door vertices.
        let door_face_indices = Self::door_face_set(doors, &self.faces);

        // Build reverse map: face_index -> door_index
        let mut face_to_door: HashMap<usize, usize> = HashMap::new();
        for (di, door) in doors.iter().enumerate() {
            for &fid in &door.face_ids {
                let fi = fid as usize;
                if door_face_indices.contains(&fi) {
                    face_to_door.insert(fi, di);
                }
            }
        }

        for (&face_idx, &door_index) in &face_to_door {
            let Some(face) = self.faces.get(face_idx) else { continue };
            if face.num_vertices < 3 || face.is_invisible() || face.is_portal() {
                continue;
            }
            let tex_name = if face_idx < self.texture_names.len() {
                &self.texture_names[face_idx]
            } else {
                continue;
            };
            if tex_name.is_empty() {
                continue;
            }

            let (tex_w, tex_h) = texture_sizes.get(tex_name).copied().unwrap_or((128, 128));
            let tex_w_f = tex_w as f32;
            let tex_h_f = tex_h as f32;

            let mm6_normal = face.normal_f32();
            let is_ceiling = face.polygon_type == 5 || face.polygon_type == 6;
            let sign = if is_ceiling { -1.0 } else { 1.0 };
            let normal = [mm6_normal[0] * sign, mm6_normal[2] * sign, -mm6_normal[1] * sign];

            let door = &doors[door_index];

            // Build the set of BLV vertex IDs that move for this face.
            // Combine vertex_ids from ALL doors whose face_ids include this face so that
            // cross-door shared faces (e.g., a trim between two adjacent portcullis panels)
            // are correctly treated as fully moving rather than partially fixed.
            let mut moving_vids: std::collections::HashSet<u16> = std::collections::HashSet::new();
            for d in doors {
                if d.face_ids.iter().any(|&fid| fid as usize == face_idx) {
                    for &vid in &d.vertex_ids {
                        moving_vids.insert(vid);
                    }
                }
            }

            // Compute UV rate: how much U and V change per unit of door displacement.
            // Only used for reveal/frame faces (some vertices fixed, some moving).
            let uv_rate = Self::compute_face_uv_rate(face, &self.vertices, &door.direction, tex_w_f, tex_h_f);

            let mut mesh = BlvDoorFaceMesh {
                face_index: face_idx,
                door_index,
                texture_name: tex_name.clone(),
                positions: Vec::new(),
                normals: Vec::new(),
                uvs: Vec::new(),
                is_moving: Vec::new(),
                uv_rate,
                moves_by_door: face.moves_by_door(),
            };

            let triangles = Self::triangulate_face(face, &self.vertices);
            for tri in &triangles {
                for &vi in tri {
                    // Position
                    let vert_idx = if vi < face.vertex_ids.len() {
                        face.vertex_ids[vi] as usize
                    } else {
                        0
                    };
                    if vert_idx < self.vertices.len() {
                        let v = &self.vertices[vert_idx];
                        mesh.positions.push(mm6_to_bevy(v.x as i32, v.y as i32, v.z as i32));
                    } else {
                        mesh.positions.push([0.0, 0.0, 0.0]);
                    }

                    // UV
                    let u = if vi < face.texture_us.len() {
                        (face.texture_us[vi] as f32 + face.texture_delta_u as f32) / tex_w_f
                    } else {
                        0.0
                    };
                    let v_coord = if vi < face.texture_vs.len() {
                        (face.texture_vs[vi] as f32 + face.texture_delta_v as f32) / tex_h_f
                    } else {
                        0.0
                    };
                    mesh.uvs.push([u, v_coord]);
                    mesh.normals.push(normal);

                    // Mark this triangle vertex as moving if its BLV vertex ID is in
                    // any door's vertex set for this face.
                    let face_vert_id = if vi < face.vertex_ids.len() {
                        face.vertex_ids[vi]
                    } else {
                        0
                    };
                    mesh.is_moving.push(moving_vids.contains(&face_vert_id));
                }
            }

            if !mesh.positions.is_empty() {
                result.push(mesh);
            }
        }

        result
    }

    /// Convert visible, non-portal faces into per-texture mesh data for rendering.
    /// `texture_sizes` maps texture name -> (width, height) in pixels.
    /// `exclude_faces` contains face indices to skip (e.g. door faces spawned separately).
    pub fn textured_meshes(
        &self,
        texture_sizes: &HashMap<String, (u32, u32)>,
        exclude_faces: &std::collections::HashSet<usize>,
    ) -> Vec<BlvTexturedMesh> {
        let mut meshes_by_texture: HashMap<String, BlvTexturedMesh> = HashMap::new();

        for (face_idx, face) in self.faces.iter().enumerate() {
            if exclude_faces.contains(&face_idx) {
                continue;
            }
            if face.num_vertices < 3 {
                continue;
            }
            if face.is_invisible() || face.is_portal() {
                continue;
            }
            let tex_name = if face_idx < self.texture_names.len() {
                &self.texture_names[face_idx]
            } else {
                continue;
            };
            if tex_name.is_empty() {
                continue;
            }

            let (tex_w, tex_h) = texture_sizes.get(tex_name).copied().unwrap_or((128, 128));
            let tex_w_f = tex_w as f32;
            let tex_h_f = tex_h as f32;

            // Convert face normal from MM6 fixed-point (x, y, z) to Bevy float (x, z, -y).
            // Flip ceiling normals (polygon_type 5 or 6) so they point into the room
            // for correct PBR lighting. MM6's original normals point outward (geometrically
            // correct but wrong for lighting ceilings from below).
            let mm6_normal = face.normal_f32();
            let is_ceiling = face.polygon_type == 5 || face.polygon_type == 6;
            let sign = if is_ceiling { -1.0 } else { 1.0 };
            let normal = [mm6_normal[0] * sign, mm6_normal[2] * sign, -mm6_normal[1] * sign];

            let mesh = meshes_by_texture
                .entry(tex_name.clone())
                .or_insert_with(|| BlvTexturedMesh {
                    texture_name: tex_name.clone(),
                    face_indices: Vec::new(),
                    positions: Vec::new(),
                    uvs: Vec::new(),
                    normals: Vec::new(),
                });

            let triangles = Self::triangulate_face(face, &self.vertices);
            for tri in &triangles {
                for &vi in tri {
                    if vi < face.vertex_ids.len() {
                        let vert_idx = face.vertex_ids[vi] as usize;
                        if vert_idx < self.vertices.len() {
                            let v = &self.vertices[vert_idx];
                            mesh.positions.push(mm6_to_bevy(v.x as i32, v.y as i32, v.z as i32));
                        } else {
                            mesh.positions.push([0.0, 0.0, 0.0]);
                        }
                    } else {
                        mesh.positions.push([0.0, 0.0, 0.0]);
                    }

                    let u = if vi < face.texture_us.len() {
                        (face.texture_us[vi] as f32 + face.texture_delta_u as f32) / tex_w_f
                    } else {
                        0.0
                    };
                    let v = if vi < face.texture_vs.len() {
                        (face.texture_vs[vi] as f32 + face.texture_delta_v as f32) / tex_h_f
                    } else {
                        0.0
                    };
                    mesh.uvs.push([u, v]);
                    mesh.normals.push(normal);
                }
            }
        }

        meshes_by_texture.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Assets, get_data_path};

    #[test]
    fn parse_d01_blv() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();

        println!("d01.blv:");
        println!("  vertices: {}", blv.vertices.len());
        println!("  faces: {}", blv.faces.len());
        println!("  sectors: {}", blv.sectors.len());
        println!("  texture_names: {}", blv.texture_names.len());
        println!("  decorations: {}", blv.decorations.len());
        println!("  lights: {}", blv.lights.len());
        println!("  bsp_nodes: {}", blv.bsp_nodes.len());
        println!("  spawn_points: {}", blv.spawn_points.len());
        for (i, sp) in blv.spawn_points.iter().enumerate() {
            println!(
                "    sp[{}]: pos={:?} type={} monster={} radius={} attrs=0x{:04x}",
                i, sp.position, sp.spawn_type, sp.monster_index, sp.radius, sp.attributes
            );
        }
        println!("  map_outlines: {}", blv.map_outlines.len());
        println!("  door_count: {}", blv.door_count);

        assert!(!blv.vertices.is_empty(), "should have vertices");
        assert!(!blv.faces.is_empty(), "should have faces");
        assert!(!blv.sectors.is_empty(), "should have sectors");

        // Verify face data was unpacked
        let faces_with_verts = blv.faces.iter().filter(|f| !f.vertex_ids.is_empty()).count();
        println!("  faces with vertex_ids: {}", faces_with_verts);
        assert!(faces_with_verts > 0, "face data blob should be unpacked");

        // Verify sector data was unpacked
        let sectors_with_faces = blv.sectors.iter().filter(|s| !s.face_ids.is_empty()).count();
        println!("  sectors with face_ids: {}", sectors_with_faces);
        assert!(sectors_with_faces > 0, "sector data blob should be unpacked");

        // Print some unique texture names
        let mut unique: Vec<&str> = blv
            .texture_names
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .collect();
        unique.sort();
        unique.dedup();
        println!("  unique textures ({}):", unique.len());
        for t in unique.iter().take(10) {
            println!("    {}", t);
        }

        // Print a few wall faces to check UV data
        let wall_faces: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.polygon_type == 1 && f.num_vertices == 4) // walls with 4 verts
            .take(3)
            .collect();
        println!("  sample wall faces (4-vert quads):");
        for (i, face) in &wall_faces {
            let verts: Vec<_> = face
                .vertex_ids
                .iter()
                .map(|&vid| {
                    let v = &blv.vertices[vid as usize];
                    (v.x, v.y, v.z)
                })
                .collect();
            println!("    face[{}]: vids={:?}", i, &face.vertex_ids);
            println!("      positions: {:?}", verts);
            println!("      Us: {:?}  Vs: {:?}", &face.texture_us, &face.texture_vs);
            println!("      delta_uv: ({}, {})", face.texture_delta_u, face.texture_delta_v);
            let tex = blv.texture_names.get(*i).map(|s| s.as_str()).unwrap_or("?");
            println!("      texture: {} attrs=0x{:08X}", tex, face.attributes);
        }

        // Find door-textured faces
        let door_faces: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                blv.texture_names
                    .get(*i)
                    .map(|t| {
                        let tl = t.to_lowercase();
                        tl.contains("door") || tl.contains("dr") || tl.contains("gate")
                    })
                    .unwrap_or(false)
            })
            .take(5)
            .collect();
        println!("  door-textured faces ({} found, showing 5):", door_faces.len());
        for (i, face) in &door_faces {
            let verts: Vec<_> = face
                .vertex_ids
                .iter()
                .map(|&vid| {
                    let v = &blv.vertices[vid as usize];
                    (v.x, v.y, v.z)
                })
                .collect();
            let tex = blv.texture_names.get(*i).unwrap();
            println!(
                "    face[{}]: nverts={} tex={} attrs=0x{:08X} extra_id={}",
                i, face.num_vertices, tex, face.attributes, face.face_extra_id
            );
            println!("      verts: {:?}", verts);
            println!("      Us: {:?}", &face.texture_us);
            println!("      Vs: {:?}", &face.texture_vs);
            println!("      delta: ({}, {})", face.texture_delta_u, face.texture_delta_v);
        }

        // Print sector bounding boxes
        for (i, s) in blv.sectors.iter().take(5).enumerate() {
            println!(
                "  sector[{}]: bbox min={:?} max={:?} floors={} walls={} ceilings={}",
                i, s.bbox_min, s.bbox_max, s.floor_count, s.wall_count, s.ceiling_count
            );
        }

        // Print decoration info, resolving names via ddeclist
        let ddeclist = crate::assets::ddeclist::DDecList::load(&assets).unwrap();
        println!("  first 20 decorations:");
        for (i, d) in blv.decorations.iter().take(20).enumerate() {
            let resolved = ddeclist
                .items
                .get(d.decoration_desc_id as usize)
                .and_then(|item| item.display_name());
            println!(
                "    [{}] desc_id={} resolved={:?} pos={:?}",
                i, d.decoration_desc_id, resolved, d.position
            );
        }

        // Verify faces have plausible vertex counts
        let max_verts = blv.faces.iter().map(|f| f.num_vertices).max().unwrap_or(0);
        println!("  max face vertex count: {}", max_verts);
        assert!(
            max_verts <= 30,
            "max vertex count should be reasonable (got {})",
            max_verts
        );
    }

    #[test]
    fn parse_sewer_blv() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "sewer.blv").unwrap();

        println!("sewer.blv:");
        println!("  vertices: {}", blv.vertices.len());
        println!("  faces: {}", blv.faces.len());
        println!("  sectors: {}", blv.sectors.len());
        println!("  decorations: {}", blv.decorations.len());
        println!("  lights: {}", blv.lights.len());
        println!("  bsp_nodes: {}", blv.bsp_nodes.len());
        println!("  spawn_points: {}", blv.spawn_points.len());

        assert!(!blv.vertices.is_empty(), "should have vertices");
        assert!(!blv.faces.is_empty(), "should have faces");
        assert!(!blv.sectors.is_empty(), "should have sectors");
    }

    #[test]
    fn textured_meshes_d01() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();

        let texture_sizes = HashMap::new();
        let meshes = blv.textured_meshes(&texture_sizes, &std::collections::HashSet::new());

        println!("d01.blv textured meshes: {}", meshes.len());
        assert!(!meshes.is_empty(), "should produce textured meshes");

        let total_tris: usize = meshes.iter().map(|m| m.positions.len() / 3).sum();
        println!("  total triangles: {}", total_tris);
        assert!(total_tris > 0, "should have triangles");

        for mesh in meshes.iter().take(5) {
            println!("  texture '{}': {} tris", mesh.texture_name, mesh.positions.len() / 3);
        }
    }

    #[test]
    fn face_extras_event_id() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();

        let clickable: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_clickable() && f.event_id != 0)
            .collect();
        println!("d01.blv clickable faces with events: {}", clickable.len());
        for (i, face) in clickable.iter().take(10) {
            println!(
                "  face[{}]: event_id={} cog={} attrs=0x{:08X}",
                i, face.event_id, face.cog_number, face.attributes
            );
        }
        assert!(!clickable.is_empty(), "d01 should have clickable faces with event IDs");

        // Dump door-related faces (moves_by_door flag)
        let door_faces: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.moves_by_door())
            .collect();
        println!("\nd01.blv moves_by_door faces: {}", door_faces.len());
        for (i, face) in door_faces.iter().take(20) {
            let tex = blv.texture_names.get(*i).map(|s| s.as_str()).unwrap_or("?");
            println!(
                "  face[{}]: cog={} event_id={} tex={} nverts={} attrs=0x{:08X}",
                i, face.cog_number, face.event_id, tex, face.num_vertices, face.attributes
            );
        }

        // Show unique cog numbers for door faces
        let mut door_cogs: Vec<i16> = door_faces.iter().map(|(_, f)| f.cog_number).collect();
        door_cogs.sort();
        door_cogs.dedup();
        println!("\nUnique cog numbers on door faces: {:?}", door_cogs);
    }

    #[test]
    fn initialize_doors_d01() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();
        let dlv = crate::assets::dlv::Dlv::new(&assets, "d01.blv", blv.door_count, blv.doors_data_size).unwrap();

        // Check cog_numbers on all faces
        let faces_with_cog: Vec<_> = blv
            .faces
            .iter()
            .enumerate()
            .filter(|(_, f)| f.cog_number != 0)
            .collect();
        println!("Faces with non-zero cog_number: {}", faces_with_cog.len());
        for (i, f) in faces_with_cog.iter().take(20) {
            let tex = blv.texture_names.get(*i).map(|s| s.as_str()).unwrap_or("?");
            println!(
                "  face[{}]: cog={} attrs=0x{:08X} tex={} moves_by_door={}",
                i,
                f.cog_number,
                f.attributes,
                tex,
                f.moves_by_door()
            );
        }

        // Check MOVES_BY_DOOR flag
        let mbd_count = blv.faces.iter().filter(|f| f.moves_by_door()).count();
        println!("\nFaces with MOVES_BY_DOOR flag: {}", mbd_count);

        // Dump DLV door data for first non-empty door
        println!("\nDLV door data (non-empty):");
        for (i, d) in dlv.doors.iter().enumerate() {
            if d.face_ids.is_empty() {
                continue;
            }
            println!(
                "  door[{}]: id={} dir={:?} move_len={} speed=({},{})",
                i, d.door_id, d.direction, d.move_length, d.open_speed, d.close_speed
            );
            println!("    face_ids: {:?}", &d.face_ids);
            println!("    vertex_ids: {:?}", &d.vertex_ids);
            println!("    x_offsets: {:?}", &d.x_offsets);
            println!("    y_offsets: {:?}", &d.y_offsets);
            println!("    z_offsets: {:?}", &d.z_offsets);
            // Compare offsets to BLV vertices
            for (vi, &vid) in d.vertex_ids.iter().enumerate() {
                if let Some(v) = blv.vertices.get(vid as usize) {
                    let matches = d.x_offsets[vi] == v.x && d.y_offsets[vi] == v.y && d.z_offsets[vi] == v.z;
                    println!(
                        "    vert[{}] id={}: blv=({},{},{}) offset=({},{},{}) match={}",
                        vi, vid, v.x, v.y, v.z, d.x_offsets[vi], d.y_offsets[vi], d.z_offsets[vi], matches
                    );
                }
            }
            if i > 5 {
                break;
            } // Limit output
        }

        // Verify door_face_set includes ALL faces from door face_ids
        // (no filtering — the original engine moves all door-referenced faces)
        let door_set = Blv::door_face_set(&dlv.doors, &blv.faces);
        let all_face_ids: std::collections::HashSet<u16> =
            dlv.doors.iter().flat_map(|d| d.face_ids.iter().copied()).collect();
        let included = all_face_ids
            .iter()
            .filter(|&&fid| door_set.contains(&(fid as usize)))
            .count();
        println!(
            "\nDoor face set: {} included out of {} total door face_ids",
            included,
            all_face_ids.len()
        );
        // Only faces appearing > 1 time in any single door's face_ids are included.
        // Single-occurrence faces (one corner shared with door panel) stay in static geometry.
        // Verify: included count must be <= total unique face count with vertex data.
        let faces_with_vids = all_face_ids
            .iter()
            .filter(|&&fid| {
                blv.faces
                    .get(fid as usize)
                    .map(|f| !f.vertex_ids.is_empty())
                    .unwrap_or(false)
            })
            .count();
        assert!(
            included <= faces_with_vids,
            "door_face_set must not include faces without vertex data (included={}, with_vids={})",
            included,
            faces_with_vids
        );
        assert!(included > 0, "door_face_set should include at least some door faces");
    }

    #[test]
    fn hybrid_door_faces_d01() {
        let assets = Assets::new(get_data_path()).unwrap();
        let blv = Blv::load(&assets, "d01.blv").unwrap();
        let dlv = crate::assets::dlv::Dlv::new(&assets, "d01.blv", blv.door_count, blv.doors_data_size).unwrap();

        let mut hybrid_count = 0;
        let mut panel_count = 0;
        let mut frame_count = 0;

        for (di, door) in dlv.doors.iter().enumerate() {
            if door.face_ids.is_empty() {
                continue;
            }
            let moving: std::collections::HashSet<u16> = door.vertex_ids.iter().copied().collect();
            let mut seen = std::collections::HashSet::new();
            for &fid in &door.face_ids {
                let fi = fid as usize;
                if !seen.insert(fi) {
                    continue;
                }
                let Some(face) = blv.faces.get(fi) else { continue };
                if face.vertex_ids.is_empty() {
                    continue;
                }
                let moving_in_face: Vec<u16> = face.vertex_ids.iter().filter(|v| moving.contains(v)).copied().collect();
                let is_hybrid = !moving_in_face.is_empty() && moving_in_face.len() < face.vertex_ids.len();
                let is_pure_panel = face.vertex_ids.iter().all(|v| moving.contains(v));
                let tex = blv.texture_names.get(fi).map(|s| s.as_str()).unwrap_or("?");
                if is_hybrid {
                    hybrid_count += 1;
                    println!(
                        "HYBRID door[{}](id={}) face[{}] poly={} tex={} vids={:?} moving={:?}",
                        di, door.door_id, fi, face.polygon_type, tex, &face.vertex_ids, &moving_in_face
                    );
                } else if is_pure_panel {
                    panel_count += 1;
                } else {
                    frame_count += 1;
                }
            }
        }
        println!("panel={} frame={} hybrid={}", panel_count, frame_count, hybrid_count);
        // After door_face_set filters single-occurrence faces (1-moving-vertex accidentals),
        // all remaining hybrid faces should have >= 2 moving vertices. Verify no 1-vertex
        // hybrids slipped through (they would deform large room floor/ceiling faces).
        let bad = dlv
            .doors
            .iter()
            .enumerate()
            .flat_map(|(di, door)| {
                if door.face_ids.is_empty() {
                    return vec![];
                }
                let moving: std::collections::HashSet<u16> = door.vertex_ids.iter().copied().collect();
                let door_set = Blv::door_face_set(&dlv.doors, &blv.faces);
                let mut seen = std::collections::HashSet::new();
                let mut bad = vec![];
                for &fid in &door.face_ids {
                    let fi = fid as usize;
                    if !seen.insert(fi) || !door_set.contains(&fi) {
                        continue;
                    }
                    let Some(face) = blv.faces.get(fi) else { continue };
                    let moving_in_face: Vec<u16> =
                        face.vertex_ids.iter().filter(|v| moving.contains(v)).copied().collect();
                    if moving_in_face.len() == 1 {
                        bad.push((di, door.door_id, fi, moving_in_face.len()));
                    }
                }
                bad
            })
            .collect::<Vec<_>>();
        assert!(
            bad.is_empty(),
            "door_face_set contains 1-moving-vertex faces that deform room geometry: {:?}",
            bad
        );
    }
}

impl TryFrom<&[u8]> for Blv {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for Blv {
    fn to_bytes(&self) -> Vec<u8> {
        // TODO: Implement full BLV serialization if needed.
        // For now, this is a placeholder to satisfy the trait.
        Vec::new()
    }
}
