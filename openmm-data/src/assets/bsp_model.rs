use std::{
    error::Error,
    io::{Cursor, Read, Seek, Write},
    ops::{Add, Div, Mul, Sub},
};

use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};

use crate::{
    assets::enums::{ModelFaceAttributes, PolygonType},
    utils::try_read_string_block,
};

/// Convert an MM6 16.16 fixed-point face normal `[nx, ny, nz]` into a
/// right-handed Y-up float vector for the renderer (axis swap + /65536 unscale).
///
/// `openmm-data` deliberately stays free of engine types, but the BSP mesh
/// builders below pre-bake render-ready face normals, so this purely
/// numerical helper lives here as a private utility.
fn fixed_normal_to_yup(normal: [i32; 3]) -> [f32; 3] {
    const FIXED_ONE: f32 = 65536.0;
    [
        normal[0] as f32 / FIXED_ONE,
        normal[2] as f32 / FIXED_ONE,
        -(normal[1] as f32) / FIXED_ONE,
    ]
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BSPModel {
    pub header: BSPModelHeader,
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<BSPModelFace>,
    /// Painter's-algorithm draw-order index table: faces_count i16 entries.
    /// Each value is a face index; entries are sorted back-to-front for software rendering.
    /// Not needed for BSP tree traversal — stored for completeness and round-tripping.
    pub face_order_indices: Vec<i16>,
    pub texture_names: Vec<String>,
    pub bsp_nodes: Vec<BSPNode>,
    pub indices: Vec<u32>,
}

impl BSPModel {
    pub fn to_bytes(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let header = self.header.to_bytes();
        let mut vertices = Vec::with_capacity(self.vertices.len() * 12);
        use byteorder::{LittleEndian, WriteBytesExt};
        for v in &self.vertices {
            // Bevy (x, y, z) -> MM6 (x, -z, y)
            vertices.write_i32::<LittleEndian>(v[0] as i32).unwrap();
            vertices.write_i32::<LittleEndian>(-v[2] as i32).unwrap();
            vertices.write_i32::<LittleEndian>(v[1] as i32).unwrap();
        }

        let mut faces = Vec::with_capacity(self.faces.len() * 300);
        for f in &self.faces {
            faces.extend_from_slice(&f.to_bytes());
        }

        let mut order = Vec::with_capacity(self.face_order_indices.len() * 2);
        for &idx in &self.face_order_indices {
            order.write_i16::<LittleEndian>(idx).unwrap();
        }

        let mut textures = Vec::with_capacity(self.texture_names.len() * 10);
        for name in &self.texture_names {
            let mut name_bytes = [0u8; 10];
            let len = name.len().min(9);
            name_bytes[..len].copy_from_slice(&name.as_bytes()[..len]);
            textures.extend_from_slice(&name_bytes);
        }

        (header.to_vec(), vertices, faces, textures)
    }
}

#[allow(dead_code)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BSPModelHeader {
    pub name: String,
    pub name2: String,
    pub attributes: i32,
    pub vertex_count: i32,
    // [+4 bytes skipped: p_vertexes runtime pointer]
    pub faces_count: i32,
    // [+12 bytes skipped: p_faces(4), p_face_order(4), p_extra(4) runtime pointers]
    pub bsp_nodes_count: i32,
    // [+8 bytes skipped: p_bsp_nodes(4), p_bsp_nodes2(4) runtime pointers]
    /// Grid cell counts [x, y] — number of collision grid cells spanning this model.
    pub grid: [i32; 2],
    pub position: [i32; 3],
    pub bounding_box: BoundingBox<i32>,
    pub bounding_box_bf: BoundingBox<i32>,
    pub position_box: [i32; 3],
    pub bounding_radius: i32,
}

impl BSPModelHeader {
    pub fn shown_on_map(&self) -> bool {
        (self.attributes & 0x0001) != 0
    }

    pub fn to_bytes(&self) -> [u8; 184] {
        let mut out = [0u8; 184];
        let mut cursor = std::io::Cursor::new(&mut out[..]);
        use byteorder::{LittleEndian, WriteBytesExt};
        use std::io::Seek;

        let mut write_padded = |s: &str, len: usize| {
            let bytes = s.as_bytes();
            let n = bytes.len().min(len - 1);
            cursor.write_all(&bytes[..n]).unwrap();
            cursor.seek(std::io::SeekFrom::Current((len - n) as i64)).unwrap();
        };

        write_padded(&self.name, 32);
        write_padded(&self.name2, 32);
        cursor.write_i32::<LittleEndian>(self.attributes).unwrap();
        cursor.write_i32::<LittleEndian>(self.vertex_count).unwrap();
        cursor.write_i32::<LittleEndian>(0).unwrap(); // p_vertexes
        cursor.write_i32::<LittleEndian>(self.faces_count).unwrap();
        cursor.write_i32::<LittleEndian>(0).unwrap(); // p_faces
        cursor.write_i32::<LittleEndian>(0).unwrap(); // p_face_order
        cursor.write_i32::<LittleEndian>(0).unwrap(); // p_extra
        cursor.write_i32::<LittleEndian>(self.bsp_nodes_count).unwrap();
        cursor.write_i32::<LittleEndian>(0).unwrap(); // p_bsp_nodes
        cursor.write_i32::<LittleEndian>(0).unwrap(); // p_bsp_nodes2
        cursor.write_i32::<LittleEndian>(self.grid[0]).unwrap();
        cursor.write_i32::<LittleEndian>(self.grid[1]).unwrap();
        cursor.write_i32::<LittleEndian>(self.position[0]).unwrap();
        cursor.write_i32::<LittleEndian>(self.position[1]).unwrap();
        cursor.write_i32::<LittleEndian>(self.position[2]).unwrap();

        let mut write_bbox = |bbox: &BoundingBox<i32>| {
            cursor.write_i32::<LittleEndian>(bbox.min_x).unwrap();
            cursor.write_i32::<LittleEndian>(bbox.min_y).unwrap();
            cursor.write_i32::<LittleEndian>(bbox.min_z).unwrap();
            cursor.write_i32::<LittleEndian>(bbox.max_x).unwrap();
            cursor.write_i32::<LittleEndian>(bbox.max_y).unwrap();
            cursor.write_i32::<LittleEndian>(bbox.max_z).unwrap();
        };

        write_bbox(&self.bounding_box);
        write_bbox(&self.bounding_box_bf);

        cursor.write_i32::<LittleEndian>(self.position_box[0]).unwrap();
        cursor.write_i32::<LittleEndian>(self.position_box[1]).unwrap();
        cursor.write_i32::<LittleEndian>(self.position_box[2]).unwrap();
        cursor.write_i32::<LittleEndian>(self.bounding_radius).unwrap();

        out
    }
}

#[repr(C)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Plane {
    pub normal: [i32; 3],
    pub distance: i32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct BoundingBox<T>
where
    T: Add + Sub + Mul + Div + Copy,
{
    pub min_x: T,
    pub max_x: T,
    pub min_y: T,
    pub max_y: T,
    pub min_z: T,
    pub max_z: T,
}

impl<T> BoundingBox<T>
where
    T: Add + Sub + Mul + Div + Copy + Default,
{
    pub fn to_bytes(&self) -> Vec<u8>
    where
        Vec<u8>: FromIterator<u8>,
    {
        // This is a bit generic, we'll just implement it manually in the parent structs
        Vec::new()
    }
}

const MAX_FACE_VERTICES_COUNT: usize = 20;

#[repr(C)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BSPModelFace {
    pub plane: Plane,
    pub z_calc: [i16; 6],
    pub attributes: u32,
    pub vertices_ids: [u16; MAX_FACE_VERTICES_COUNT],
    pub texture_u_ids: [i16; MAX_FACE_VERTICES_COUNT],
    pub texture_v_ids: [i16; MAX_FACE_VERTICES_COUNT],
    pub normal_x: [i16; MAX_FACE_VERTICES_COUNT],
    pub normal_y: [i16; MAX_FACE_VERTICES_COUNT],
    pub normal_z: [i16; MAX_FACE_VERTICES_COUNT],
    pub texture_id: i16,
    pub texture_u: i16,
    pub texture_v: i16,
    pub bounding_box: BoundingBox<i16>,
    pub cog_number: u16,
    pub cog_trigger_id: u16,
    pub cog_trigger_type: u16,
    pub reserved: u16,
    pub gradient_vertices: [u8; 4],
    pub vertices_count: u8,
    pub polygon_type: u8,
    pub shade_type: u8,
    pub visible: u8,
    pub padding: [u8; 2],
}

impl BSPModelFace {
    /// Get typed face attribute flags.
    pub fn face_attributes(&self) -> ModelFaceAttributes {
        ModelFaceAttributes::from_bits_truncate(self.attributes)
    }

    /// Get typed polygon type.
    pub fn polygon_type_enum(&self) -> Option<PolygonType> {
        PolygonType::from_u8(self.polygon_type)
    }
}

/// A per-texture mesh extracted from a BSP model, ready for rendering.
pub struct BSPTexturedMesh {
    /// Texture name from the LOD archive (bitmap name).
    pub texture_name: String,
    /// Original face indices (into BSPModel::faces) that contributed to this mesh.
    pub face_indices: Vec<u32>,
    /// Triangle vertex positions (3 per triangle), already in Bevy coordinates.
    pub positions: Vec<[f32; 3]>,
    /// Normalized UV coordinates (0-1 range), need texture dimensions for computation.
    pub uvs: Vec<[f32; 2]>,
    /// Per-vertex normals.
    pub normals: Vec<[f32; 3]>,
}

impl BSPModel {
    /// Extract per-texture meshes with proper UVs.
    /// `texture_sizes` maps texture name → (width, height) in pixels.
    pub fn textured_meshes(
        &self,
        texture_sizes: &std::collections::HashMap<String, (u32, u32)>,
    ) -> Vec<BSPTexturedMesh> {
        let mut meshes_by_texture: std::collections::HashMap<String, BSPTexturedMesh> =
            std::collections::HashMap::new();

        for (face_idx, face) in self.faces.iter().enumerate() {
            if face.vertices_count < 3 {
                continue;
            }
            if face.is_invisible() {
                continue;
            }

            // texture_names is 1:1 with faces — texture_names[i] is the texture for faces[i]
            let tex_name = if face_idx < self.texture_names.len() {
                &self.texture_names[face_idx]
            } else {
                continue;
            };

            let (tex_w, tex_h) = texture_sizes.get(tex_name).copied().unwrap_or((128, 128));

            let tex_w_f = tex_w as f32;
            let tex_h_f = tex_h as f32;

            let normal = fixed_normal_to_yup(face.plane.normal);

            let mesh = meshes_by_texture
                .entry(tex_name.clone())
                .or_insert_with(|| BSPTexturedMesh {
                    texture_name: tex_name.clone(),
                    face_indices: Vec::new(),
                    positions: Vec::new(),
                    uvs: Vec::new(),
                    normals: Vec::new(),
                });
            mesh.face_indices.push(face_idx as u32);

            // Fan triangulation: (v0, v1, v2), (v0, v2, v3), ...
            for i in 0..(face.vertices_count as usize - 2) {
                let tri_verts = [0, i + 1, i + 2];
                for &vi in &tri_verts {
                    let vert_idx = face.vertices_ids[vi] as usize;
                    if vert_idx < self.vertices.len() {
                        mesh.positions.push(self.vertices[vert_idx]);
                    } else {
                        mesh.positions.push([0.0, 0.0, 0.0]);
                    }

                    let u = (face.texture_u_ids[vi] as f32 + face.texture_u as f32) / tex_w_f;
                    let v = (face.texture_v_ids[vi] as f32 + face.texture_v as f32) / tex_h_f;
                    mesh.uvs.push([u, v]);
                    mesh.normals.push(normal);
                }
            }
        }

        meshes_by_texture.into_values().collect()
    }
}

impl BSPModelFace {
    pub fn is_portal(&self) -> bool {
        (self.attributes & 0x00000001) != 0
    }

    pub fn is_water(&self) -> bool {
        (self.attributes & 0x00000010) != 0
    }

    pub fn projecting_to_xy(&self) -> bool {
        (self.attributes & 0x000000100) != 0
    }

    pub fn projecting_to_xz(&self) -> bool {
        (self.attributes & 0x000000200) != 0
    }

    pub fn projecting_to_yz(&self) -> bool {
        (self.attributes & 0x000000400) != 0
    }

    pub fn is_invisible(&self) -> bool {
        (self.attributes & 0x000002000) != 0
    }

    pub fn is_animated_tft(&self) -> bool {
        (self.attributes & 0x000004000) != 0
    }

    pub fn moves_by_door(&self) -> bool {
        (self.attributes & 0x000010000) != 0
    }

    pub fn is_event_just_hint(&self) -> bool {
        (self.attributes & 0x000040000) != 0
    }

    pub fn is_alternative_sound(&self) -> bool {
        (self.attributes & 0x000080000) != 0
    }

    pub fn is_sky(&self) -> bool {
        (self.attributes & 0x000100000) != 0
    }

    pub fn flip_u(&self) -> bool {
        (self.attributes & 0x000200000) != 0
    }

    pub fn flip_v(&self) -> bool {
        (self.attributes & 0x000400000) != 0
    }

    pub fn trigger_by_click(&self) -> bool {
        (self.attributes & 0x000800000) != 0
    }

    pub fn trigger_by_step(&self) -> bool {
        (self.attributes & 0x001000000) != 0
    }

    pub fn trigger_by_monster(&self) -> bool {
        (self.attributes & 0x002000000) != 0
    }

    pub fn trigger_by_object(&self) -> bool {
        (self.attributes & 0x004000000) != 0
    }

    pub fn is_untouchable(&self) -> bool {
        (self.attributes & 0x008000000) != 0
    }

    pub fn is_lava(&self) -> bool {
        (self.attributes & 0x010000000) != 0
    }

    pub fn has_data(&self) -> bool {
        (self.attributes & 0x020000000) != 0
    }

    pub fn to_bytes(&self) -> [u8; 300] {
        let mut out = [0u8; 300];
        unsafe {
            let src = self as *const _ as *const u8;
            std::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), 300);
        }
        out
    }
}

#[repr(C)]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct BSPNode {
    pub front: i32,
    pub back: i32,
    pub face_id_offset: i16,
    pub faces_count: i16,
}

impl BSPNode {
    pub fn to_bytes(&self) -> [u8; 12] {
        let mut out = [0u8; 12];
        let mut cursor = std::io::Cursor::new(&mut out[..]);
        use byteorder::{LittleEndian, WriteBytesExt};
        cursor.write_i32::<LittleEndian>(self.front).unwrap();
        cursor.write_i32::<LittleEndian>(self.back).unwrap();
        cursor.write_i16::<LittleEndian>(self.face_id_offset).unwrap();
        cursor.write_i16::<LittleEndian>(self.faces_count).unwrap();
        out
    }
}

const MODEL_NAME_MAX_SIZE: usize = 32;
const TEXTURE_NAME_MAX_SIZE: usize = 10;

pub(super) fn read_bsp_models(cursor: &mut Cursor<&[u8]>, count: usize) -> Result<Vec<BSPModel>, Box<dyn Error>> {
    let mut models: Vec<BSPModel> = Vec::with_capacity(count);
    for header in read_bsp_model_headers(cursor, count)? {
        models.push(read_bsp_model(cursor, header)?);
    }
    Ok(models)
}

fn read_bsp_model(cursor: &mut Cursor<&[u8]>, header: BSPModelHeader) -> Result<BSPModel, Box<dyn Error>> {
    let mut model = BSPModel {
        vertices: Vec::with_capacity(header.vertex_count as usize),
        faces: Vec::with_capacity(header.faces_count as usize),
        face_order_indices: Vec::with_capacity(header.faces_count as usize),
        texture_names: Vec::with_capacity(header.faces_count as usize),
        bsp_nodes: Vec::new(),
        indices: Vec::new(),
        header,
    };
    let mut vertices: Vec<f32> = Vec::new();
    for _i in 0..model.header.vertex_count * 3 {
        vertices.push(cursor.read_i32::<LittleEndian>()? as f32);
    }
    model.vertices = decode_vertices(vertices);
    for _i in 0..model.header.faces_count {
        let mut face = BSPModelFace::default();
        cursor.read_exact(unsafe {
            std::slice::from_raw_parts_mut(&mut face as *mut _ as *mut u8, std::mem::size_of::<BSPModelFace>())
        })?;
        model.faces.push(face);
    }
    for _i in 0..model.header.faces_count {
        model.face_order_indices.push(cursor.read_i16::<LittleEndian>()?);
    }
    for _i in 0..model.header.faces_count {
        let texture_name: String = try_read_string_block(cursor, TEXTURE_NAME_MAX_SIZE)?;
        model.texture_names.push(texture_name);
    }
    model.indices = decode_indices(&model);
    let bsp_nodes_count = model.header.bsp_nodes_count;
    if bsp_nodes_count > 0 {
        for _i in 0..bsp_nodes_count * 2 {
            model.bsp_nodes.push(BSPNode {
                front: cursor.read_i32::<LittleEndian>()?,
                back: cursor.read_i32::<LittleEndian>()?,
                face_id_offset: cursor.read_i16::<LittleEndian>()?,
                faces_count: cursor.read_i16::<LittleEndian>()?,
            });
        }
    }
    Ok(model)
}

fn read_bsp_model_headers(cursor: &mut Cursor<&[u8]>, count: usize) -> Result<Vec<BSPModelHeader>, Box<dyn Error>> {
    let mut headers = Vec::with_capacity(count);
    for _i in 0..count {
        headers.push(read_bsp_model_header(cursor)?);
    }
    Ok(headers)
}

fn read_bsp_model_header(cursor: &mut Cursor<&[u8]>) -> Result<BSPModelHeader, Box<dyn Error>> {
    let mut header = BSPModelHeader {
        name: try_read_string_block(cursor, MODEL_NAME_MAX_SIZE)?,
        name2: try_read_string_block(cursor, MODEL_NAME_MAX_SIZE)?,
        attributes: cursor.read_i32::<LittleEndian>()?,
        vertex_count: cursor.read_i32::<LittleEndian>()?,
        ..Default::default()
    };
    cursor.seek(std::io::SeekFrom::Current(4))?;
    header.faces_count = cursor.read_i32::<LittleEndian>()?;
    cursor.seek(std::io::SeekFrom::Current(12))?;
    header.bsp_nodes_count = cursor.read_i32::<LittleEndian>()?;
    cursor.seek(std::io::SeekFrom::Current(8))?;
    header.grid = [cursor.read_i32::<LittleEndian>()?, cursor.read_i32::<LittleEndian>()?];
    header.position = [
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
    ];
    header.bounding_box = BoundingBox {
        min_x: cursor.read_i32::<LittleEndian>()?,
        min_y: cursor.read_i32::<LittleEndian>()?,
        min_z: cursor.read_i32::<LittleEndian>()?,
        max_x: cursor.read_i32::<LittleEndian>()?,
        max_y: cursor.read_i32::<LittleEndian>()?,
        max_z: cursor.read_i32::<LittleEndian>()?,
    };
    header.bounding_box_bf = BoundingBox {
        min_x: cursor.read_i32::<LittleEndian>()?,
        min_y: cursor.read_i32::<LittleEndian>()?,
        min_z: cursor.read_i32::<LittleEndian>()?,
        max_x: cursor.read_i32::<LittleEndian>()?,
        max_y: cursor.read_i32::<LittleEndian>()?,
        max_z: cursor.read_i32::<LittleEndian>()?,
    };
    header.position_box = [
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
    ];
    header.bounding_radius = cursor.read_i32::<LittleEndian>()?;
    Ok(header)
}

fn decode_vertices(input: Vec<f32>) -> Vec<[f32; 3]> {
    input
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[2], -chunk[1]])
        .collect()
}

fn decode_indices(model: &BSPModel) -> Vec<u32> {
    model
        .faces
        .iter()
        .flat_map(|f| {
            (0..f.vertices_count.saturating_sub(2))
                .flat_map(|i| {
                    vec![
                        f.vertices_ids[0] as u32,
                        f.vertices_ids[i as usize + 1] as u32,
                        f.vertices_ids[i as usize + 2] as u32,
                    ]
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::assets::odm::Odm;
    use crate::utils::test_lod;

    #[test]
    fn get_map_works() {
        let Some(assets) = test_lod() else {
            return;
        };
        let map = Odm::load(&assets, "oute3.odm").unwrap();

        let model_crate = &map.bsp_models[17];
        println!("{:?}", model_crate.texture_names);

        let model_bridge = &map.bsp_models[19];
        println!("{:?}", model_bridge.texture_names);
    }
}
