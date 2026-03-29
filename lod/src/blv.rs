use std::{
    collections::HashMap,
    error::Error,
    io::{Cursor, Seek},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{
    lod_data::LodData,
    odm::mm6_to_bevy,
    LodManager,
};

/// Read a fixed-size string block, using lossy UTF-8 conversion for non-ASCII bytes.
fn read_string_lossy(cursor: &mut Cursor<&[u8]>, size: usize) -> Result<String, Box<dyn Error>> {
    let mut buf = vec![0u8; size];
    std::io::Read::read_exact(cursor, &mut buf)?;
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Ok(String::from_utf8_lossy(&buf[..end]).to_string())
}

/// Face attribute flags.
const FACE_ATTR_PORTAL: u32 = 0x00000001;

const FACE_ATTR_INVISIBLE: u32 = 0x00002000;

/// MM6 decoration name size (28 bytes, vs 32 in MM7).
const DECORATION_NAME_SIZE: usize = 32;

/// BLV header (136 bytes): 104 bytes padding, then size fields, then 16 bytes padding.
#[derive(Debug)]
struct BlvHeader {
    face_data_size: i32,
    sector_data_size: i32,
    sector_light_data_size: i32,
    _doors_data_size: i32,
}

/// A vertex in MM6 coordinates.
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug)]
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
}

impl BlvFace {
    pub fn is_portal(&self) -> bool {
        (self.attributes & FACE_ATTR_PORTAL) != 0
    }

    pub fn is_invisible(&self) -> bool {
        (self.attributes & FACE_ATTR_INVISIBLE) != 0
    }

    pub fn moves_by_door(&self) -> bool {
        (self.attributes & 0x00010000) != 0
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
#[derive(Debug)]
pub struct BlvSector {
    pub flags: i32,
    pub floor_count: u16,
    pub wall_count: u16,
    pub ceiling_count: u16,
    pub fluid_count: u16,
    pub portal_count: u16,
    pub num_faces: u16,
    pub num_non_bsp_faces: u16,
    pub cylinder_count: u16,
    pub cog_count: u16,
    pub decoration_count: u16,
    pub marker_count: u16,
    pub light_count: u16,
    pub water_level: i16,
    pub mist_level: i16,
    pub light_dist_mul: i16,
    pub min_ambient_light: i16,
    pub first_bsp_node: i16,
    pub exit_tag: i16,
    pub bbox_min: [i16; 3],
    pub bbox_max: [i16; 3],

    // Assigned from sector data blob:
    pub face_ids: Vec<u16>,
}

/// A decoration in a BLV indoor map (32 bytes on disk + 28-byte name in MM6).
#[derive(Debug)]
pub struct BlvDecoration {
    pub decoration_desc_id: u16,
    pub flags: u16,
    pub position: [i32; 3],
    pub yaw: i32,
    pub name: String,
}

/// A light source (12 bytes, MM6 format).
#[derive(Debug)]
pub struct BlvLight {
    pub position: [i16; 3],
    pub radius: i16,
    pub attributes: i16,
    pub brightness: u16,
}

/// A BSP node (8 bytes).
#[derive(Debug)]
pub struct BlvBspNode {
    pub front: i16,
    pub back: i16,
    pub face_id_offset: i16,
    pub num_faces: i16,
}

/// A spawn point (20 bytes, MM6 format).
#[derive(Debug)]
pub struct BlvSpawnPoint {
    pub position: [i32; 3],
    pub radius: u16,
    pub spawn_type: u16,
    pub monster_index: u16,
    pub attributes: u16,
}

/// A map outline entry (12 bytes).
#[derive(Debug)]
pub struct BlvMapOutline {
    pub vertex1_id: u16,
    pub vertex2_id: u16,
    pub face1_id: u16,
    pub face2_id: u16,
    pub z: i16,
    pub flags: u16,
}

/// A per-texture mesh extracted from a BLV map, ready for rendering.
pub struct BlvTexturedMesh {
    pub texture_name: String,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub normals: Vec<[f32; 3]>,
}

/// Parsed BLV indoor map.
#[derive(Debug)]
pub struct Blv {
    pub vertices: Vec<BlvVertex>,
    pub faces: Vec<BlvFace>,
    pub texture_names: Vec<String>,
    pub sectors: Vec<BlvSector>,
    pub decorations: Vec<BlvDecoration>,
    pub lights: Vec<BlvLight>,
    pub bsp_nodes: Vec<BlvBspNode>,
    pub spawn_points: Vec<BlvSpawnPoint>,
    pub map_outlines: Vec<BlvMapOutline>,
    pub door_count: u32,
}

impl Blv {
    /// Parse a BLV file from a LOD archive.
    pub fn new(lod_manager: &LodManager, name: &str) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes(&format!("games/{}", name))?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data.as_ref());

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

        // 6. Face extras: u32 count, then count x 36 bytes (MM7 size)
        //    sTextureDeltaU at offset 0x16, sTextureDeltaV at offset 0x18.
        let face_extras_count = cursor.read_u32::<LittleEndian>()? as usize;
        let face_extra_size = 36;
        let mut face_extra_deltas: Vec<(i16, i16)> = Vec::with_capacity(face_extras_count);
        for _ in 0..face_extras_count {
            let start = cursor.position();
            cursor.seek(std::io::SeekFrom::Current(0x16))?;
            let delta_u = cursor.read_i16::<LittleEndian>()?;
            let delta_v = cursor.read_i16::<LittleEndian>()?;
            face_extra_deltas.push((delta_u, delta_v));
            cursor.seek(std::io::SeekFrom::Start(start + face_extra_size as u64))?;
        }
        // Assign deltas to faces via face_extra_id
        for face in &mut faces {
            if let Some(&(du, dv)) = face_extra_deltas.get(face.face_extra_id as usize) {
                face.texture_delta_u = du;
                face.texture_delta_v = dv;
            }
        }

        // 7. Face extra texture names: face_extras.len x 10 bytes (skip)
        cursor.seek(std::io::SeekFrom::Current(face_extras_count as i64 * 10))?;

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

        // 10. Sector light data blob: (header.sector_light_data_size / 2) x u16
        let sector_light_count = (header.sector_light_data_size / 2) as usize;
        cursor.seek(std::io::SeekFrom::Current(sector_light_count as i64 * 2))?;

        // 11. Door count (actual doors stored in DLV)
        let door_count = cursor.read_u32::<LittleEndian>()?;

        // 12. Decorations: u32 count, then count x 32 bytes
        let decoration_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut decorations = Vec::with_capacity(decoration_count);
        for _ in 0..decoration_count {
            decorations.push(Self::read_decoration(&mut cursor)?);
        }

        // 13. Decoration names
        for dec in &mut decorations {
            dec.name = read_string_lossy(&mut cursor, 28)?;
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
            vertices,
            faces,
            texture_names,
            sectors,
            decorations,
            lights,
            bsp_nodes,
            spawn_points,
            map_outlines,
            door_count,
        })
    }

    fn read_header(cursor: &mut Cursor<&[u8]>) -> Result<BlvHeader, Box<dyn Error>> {
        cursor.seek(std::io::SeekFrom::Current(104))?;
        let face_data_size = cursor.read_i32::<LittleEndian>()?;
        let sector_data_size = cursor.read_i32::<LittleEndian>()?;
        let sector_light_data_size = cursor.read_i32::<LittleEndian>()?;
        let doors_data_size = cursor.read_i32::<LittleEndian>()?;
        cursor.seek(std::io::SeekFrom::Current(16))?;
        Ok(BlvHeader {
            face_data_size,
            sector_data_size,
            sector_light_data_size,
            _doors_data_size: doors_data_size,
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
        })
    }

    /// Unpack the face data blob into per-face vertex IDs and UV coordinates.
    /// For each face, 6 sub-arrays of (num_vertices + 1) i16 values:
    /// [vertexIds, xDisp, yDisp, zDisp, textureUs, textureVs].
    fn unpack_face_data(faces: &mut [BlvFace], blob: &[i16]) {
        let mut offset = 0;
        for face in faces.iter_mut() {
            let n = face.num_vertices as usize;
            let stride = n + 1;
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

    /// Read a decoration (32 bytes).
    fn read_decoration(cursor: &mut Cursor<&[u8]>) -> Result<BlvDecoration, Box<dyn Error>> {
        let decoration_desc_id = cursor.read_u16::<LittleEndian>()?;
        let flags = cursor.read_u16::<LittleEndian>()?;
        let position = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let yaw = cursor.read_i32::<LittleEndian>()?;
        // Skip remaining 12 bytes (cog, eventID, triggerRange, field_1A, eventVarId, field_1E)
        cursor.seek(std::io::SeekFrom::Current(12))?;
        Ok(BlvDecoration {
            decoration_desc_id,
            flags,
            position,
            yaw,
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
    fn triangulate_face(face: &BlvFace, _vertices: &[BlvVertex]) -> Vec<[usize; 3]> {
        let n = face.num_vertices as usize;
        if n < 3 { return vec![]; }
        // Fan triangulation: (0,1,2), (0,2,3), (0,3,4), ...
        (1..n - 1).map(|i| [0, i, i + 1]).collect()
    }

    /// Convert visible, non-portal faces into per-texture mesh data for rendering.
    /// `texture_sizes` maps texture name -> (width, height) in pixels.
    pub fn textured_meshes(
        &self,
        texture_sizes: &HashMap<String, (u32, u32)>,
    ) -> Vec<BlvTexturedMesh> {
        let mut meshes_by_texture: HashMap<String, BlvTexturedMesh> = HashMap::new();

        for (face_idx, face) in self.faces.iter().enumerate() {
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

            let (tex_w, tex_h) = texture_sizes
                .get(tex_name)
                .copied()
                .unwrap_or((128, 128));
            let tex_w_f = tex_w as f32;
            let tex_h_f = tex_h as f32;

            // Convert face normal from MM6 fixed-point (x, y, z) to Bevy float (x, z, -y)
            let mm6_normal = face.normal_f32();
            let normal = [mm6_normal[0], mm6_normal[2], -mm6_normal[1]];

            let mesh = meshes_by_texture
                .entry(tex_name.clone())
                .or_insert_with(|| BlvTexturedMesh {
                    texture_name: tex_name.clone(),
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
                            mesh.positions.push(mm6_to_bevy(
                                v.x as i32, v.y as i32, v.z as i32,
                            ));
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
    use crate::{get_lod_path, LodManager};

    #[test]
    fn parse_d01_blv() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let blv = Blv::new(&lod_manager, "d01.blv").unwrap();

        println!("d01.blv:");
        println!("  vertices: {}", blv.vertices.len());
        println!("  faces: {}", blv.faces.len());
        println!("  sectors: {}", blv.sectors.len());
        println!("  texture_names: {}", blv.texture_names.len());
        println!("  decorations: {}", blv.decorations.len());
        println!("  lights: {}", blv.lights.len());
        println!("  bsp_nodes: {}", blv.bsp_nodes.len());
        println!("  spawn_points: {}", blv.spawn_points.len());
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
        let mut unique: Vec<&str> = blv.texture_names.iter()
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
        let wall_faces: Vec<_> = blv.faces.iter().enumerate()
            .filter(|(_, f)| f.polygon_type == 1 && f.num_vertices == 4) // walls with 4 verts
            .take(3)
            .collect();
        println!("  sample wall faces (4-vert quads):");
        for (i, face) in &wall_faces {
            let verts: Vec<_> = face.vertex_ids.iter()
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
        let door_faces: Vec<_> = blv.faces.iter().enumerate()
            .filter(|(i, _)| {
                blv.texture_names.get(*i).map(|t| {
                    let tl = t.to_lowercase();
                    tl.contains("door") || tl.contains("dr") || tl.contains("gate")
                }).unwrap_or(false)
            })
            .take(5)
            .collect();
        println!("  door-textured faces ({} found, showing 5):", door_faces.len());
        for (i, face) in &door_faces {
            let verts: Vec<_> = face.vertex_ids.iter()
                .map(|&vid| { let v = &blv.vertices[vid as usize]; (v.x, v.y, v.z) })
                .collect();
            let tex = blv.texture_names.get(*i).unwrap();
            println!("    face[{}]: nverts={} tex={} attrs=0x{:08X} extra_id={}",
                i, face.num_vertices, tex, face.attributes, face.face_extra_id);
            println!("      verts: {:?}", verts);
            println!("      Us: {:?}", &face.texture_us);
            println!("      Vs: {:?}", &face.texture_vs);
            println!("      delta: ({}, {})", face.texture_delta_u, face.texture_delta_v);
        }

        // Print sector bounding boxes
        for (i, s) in blv.sectors.iter().take(5).enumerate() {
            println!("  sector[{}]: bbox min={:?} max={:?} floors={} walls={} ceilings={}",
                i, s.bbox_min, s.bbox_max, s.floor_count, s.wall_count, s.ceiling_count);
        }

        // Print decoration info, resolving names via ddeclist
        let ddeclist = crate::ddeclist::DDecList::new(&lod_manager).unwrap();
        println!("  first 20 decorations:");
        for (i, d) in blv.decorations.iter().take(20).enumerate() {
            let resolved = ddeclist.items.get(d.decoration_desc_id as usize)
                .and_then(|item| item.game_name());
            println!("    [{}] desc_id={} resolved={:?} pos={:?}", i, d.decoration_desc_id, resolved, d.position);
        }

        // Verify faces have plausible vertex counts
        let max_verts = blv.faces.iter().map(|f| f.num_vertices).max().unwrap_or(0);
        println!("  max face vertex count: {}", max_verts);
        assert!(max_verts <= 30, "max vertex count should be reasonable (got {})", max_verts);
    }

    #[test]
    fn parse_sewer_blv() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let blv = Blv::new(&lod_manager, "sewer.blv").unwrap();

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
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let blv = Blv::new(&lod_manager, "d01.blv").unwrap();

        let texture_sizes = HashMap::new();
        let meshes = blv.textured_meshes(&texture_sizes);

        println!("d01.blv textured meshes: {}", meshes.len());
        assert!(!meshes.is_empty(), "should produce textured meshes");

        let total_tris: usize = meshes.iter().map(|m| m.positions.len() / 3).sum();
        println!("  total triangles: {}", total_tris);
        assert!(total_tris > 0, "should have triangles");

        for mesh in meshes.iter().take(5) {
            println!(
                "  texture '{}': {} tris",
                mesh.texture_name,
                mesh.positions.len() / 3
            );
        }
    }
}
