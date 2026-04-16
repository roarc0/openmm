use serde::{Deserialize, Serialize};
use std::io::Cursor;

use crate::assets::enums::{DoorAttributes, FaceAttributes, PolygonType};

pub(crate) fn default_blv_unknown_mid() -> [u8; 40] {
    [0; 40]
}

/// Read a fixed-size string block, using lossy UTF-8 conversion for non-ASCII bytes.
pub(crate) fn read_string_lossy(cursor: &mut Cursor<&[u8]>, size: usize) -> Result<String, Box<dyn std::error::Error>> {
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
pub(crate) struct BlvHeader {
    /// Map display name from the BLV header. Offset 0x04, 60 bytes.
    pub name: String,
    /// Unknown 4 bytes at offset 0x00. Preserved for analysis/round-tripping.
    pub unknown_head: [u8; 4],
    /// Unknown 40 bytes at offset 0x40. Preserved for analysis/round-tripping.
    #[serde(skip, default = "default_blv_unknown_mid")]
    pub unknown_mid: [u8; 40],
    pub face_data_size: i32,
    pub sector_data_size: i32,
    pub sector_light_data_size: i32,
    pub doors_data_size: i32,
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

/// A sector (room) in a BLV indoor map.
///
/// Contains face lists, lighting parameters, and spatial bounds.
#[derive(Debug, Serialize, Deserialize)]
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
