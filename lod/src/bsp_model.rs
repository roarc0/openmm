use std::{
    error::Error,
    io::{Cursor, Read, Seek},
    ops::{Add, Div, Mul, Sub},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils::try_read_string_block;

#[derive(Debug)]
pub struct BSPModel {
    pub header: BSPModelHeader,
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<BSPModelFace>,
    unk: Vec<u8>,
    pub texture_names: Vec<String>,
    pub bsp_nodes: Vec<BSPNode>,
    pub indices: Vec<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct BSPModelHeader {
    pub name: String,
    pub name2: String,
    pub attributes: i32,
    pub vertex_count: i32,
    // p_vertexes: *mut i32,
    pub faces_count: i32, // faces_order_count ?
    // convexFacetsCount: i32, ?? u16
    // p_faces: *mut i32,
    // p_unk_array: *mut i32,
    pub bsp_nodes_count: i32,
    //unk03a: i32,
    //unk03b: i32,
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
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Plane {
    pub normal: [i32; 3],
    pub distance: i32,
}

#[repr(C)]
#[derive(Debug, Default)]
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

const MAX_FACE_VERTICES_COUNT: usize = 20;

#[repr(C)]
#[derive(Debug, Default)]
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

/// A per-texture mesh extracted from a BSP model, ready for rendering.
pub struct BSPTexturedMesh {
    /// Texture name from the LOD archive (bitmap name).
    pub texture_name: String,
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

        for face in &self.faces {
            if face.vertices_count < 3 {
                continue;
            }
            if face.is_invisible() {
                continue;
            }

            let tex_idx = face.texture_id as usize;
            let tex_name = if tex_idx < self.texture_names.len() {
                &self.texture_names[tex_idx]
            } else {
                continue;
            };

            let (tex_w, tex_h) = texture_sizes
                .get(tex_name)
                .copied()
                .unwrap_or((128, 128));

            let tex_w_f = tex_w as f32;
            let tex_h_f = tex_h as f32;

            // Face normal from the plane (MM6: x,y,z → Bevy: x,z,-y)
            let nx = face.plane.normal[0] as f32 / 65536.0;
            let ny = face.plane.normal[2] as f32 / 65536.0;
            let nz = -face.plane.normal[1] as f32 / 65536.0;
            let normal = [nx, ny, nz];

            let mesh = meshes_by_texture
                .entry(tex_name.clone())
                .or_insert_with(|| BSPTexturedMesh {
                    texture_name: tex_name.clone(),
                    positions: Vec::new(),
                    uvs: Vec::new(),
                    normals: Vec::new(),
                });

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
}

#[repr(C)]
#[derive(Debug, Default)]
pub enum PolygonType {
    #[default]
    Invalid = 0,
    VerticalWall = 1,
    Unknown = 2,
    Floor = 3,
    InBetweenFloorAndWall = 4,
    Ceiling = 5,
    InBetweenCeilingAndWall = 6,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct BSPNode {
    pub front: i32,
    pub back: i32,
    pub face_id_offset: i16,
    pub faces_count: i16,
}

const MODEL_NAME_MAX_SIZE: usize = 32;
const TEXTURE_NAME_MAX_SIZE: usize = 10;

pub(super) fn read_bsp_models(
    cursor: &mut Cursor<&[u8]>,
    count: usize,
) -> Result<Vec<BSPModel>, Box<dyn Error>> {
    let mut models: Vec<BSPModel> = Vec::with_capacity(count);
    for header in read_bsp_model_headers(cursor, count)? {
        models.push(read_bsp_model(cursor, header)?);
    }
    Ok(models)
}

fn read_bsp_model(
    cursor: &mut Cursor<&[u8]>,
    header: BSPModelHeader,
) -> Result<BSPModel, Box<dyn Error>> {
    let mut model = BSPModel {
        vertices: Vec::with_capacity(header.vertex_count as usize),
        faces: Vec::with_capacity(header.faces_count as usize),
        unk: Vec::with_capacity((header.faces_count * 2) as usize),
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
            std::slice::from_raw_parts_mut(
                &mut face as *mut _ as *mut u8,
                std::mem::size_of::<BSPModelFace>(),
            )
        })?;
        model.faces.push(face);
    }
    for _i in 0..model.header.faces_count * 2 {
        model.unk.push(cursor.read_u8()?);
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

fn read_bsp_model_headers(
    cursor: &mut Cursor<&[u8]>,
    count: usize,
) -> Result<Vec<BSPModelHeader>, Box<dyn Error>> {
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
    cursor.seek(std::io::SeekFrom::Current(16))?;
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
    let indices = model
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
        .collect();
    indices
}

#[cfg(test)]
mod tests {
    use crate::{get_lod_path, odm::Odm, LodManager};

    #[test]
    fn get_map_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let map = Odm::new(&lod_manager, "oute3.odm").unwrap();

        let model_crate = &map.bsp_models[17];
        println!("{:?}", model_crate.texture_names);

        let model_bridge = &map.bsp_models[19];
        println!("{:?}", model_bridge.texture_names);
    }
}
