use std::{
    error::Error,
    io::{Cursor, Read, Seek},
    ops::{Add, Div, Mul, Sub},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils::read_string_block;

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

#[derive(Debug, Default)]
pub struct BSPModelHeader {
    pub name1: String,
    pub name2: String,
    pub attributes: i32,
    pub vertex_count: i32,
    // p_vertexes: *mut i32,
    pub faces_count: i32,
    unk02: i32,
    // p_faces: *mut i32,
    // p_unk_array: *mut i32,
    num3: i32,
    unk03a: i32,
    unk03b: i32,
    unk03: [i32; 2],
    pub origin1: [i32; 3],
    pub bounding_box: BoundingBox<i32>,
    unk04: [i32; 6],
    pub origin2: [i32; 3],
    unk05: i32,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct Plane {
    normal: [i32; 3],
    dist: i32,
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
    plane: Plane,
    z_calc: [i16; 6],
    attributes: u32,
    vertices_ids: [u16; MAX_FACE_VERTICES_COUNT],
    texture_u_ids: [i16; MAX_FACE_VERTICES_COUNT],
    texture_v_ids: [i16; MAX_FACE_VERTICES_COUNT],
    normal_x: [i16; MAX_FACE_VERTICES_COUNT],
    normal_y: [i16; MAX_FACE_VERTICES_COUNT],
    normal_z: [i16; MAX_FACE_VERTICES_COUNT],
    unk02: i16,
    texture_delta_x: i16,
    texture_delta_y: i16,
    bounding_box: BoundingBox<i16>,
    cog_number: u16,
    cog_trigger_id: u16,
    cog_trigger_type: u16,
    reserved: u16,
    gradient_vertices: [u8; 4],
    vertices_count: u8,
    polygon_type: u8,
    shade_type: u8,
    visible: u8,
    padding: [u8; 2],
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
    mut cursor: Cursor<&[u8]>,
    model_count: usize,
) -> Result<Vec<BSPModel>, Box<dyn Error>> {
    let mut models: Vec<BSPModel> = Vec::with_capacity(model_count);
    for header in read_bsp_model_headers(&mut cursor, model_count)? {
        models.push(read_bsp_model(&mut cursor, header)?);
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
        let face_size = std::mem::size_of::<BSPModelFace>();

        //hexdump_next_bytes(&mut cursor.clone(), face_size);

        cursor.read_exact(unsafe {
            std::slice::from_raw_parts_mut(&mut face as *mut _ as *mut u8, face_size)
        })?;
        model.faces.push(face);
    }
    for _i in 0..model.header.faces_count * 2 {
        model.unk.push(cursor.read_u8()?);
    }
    for _i in 0..model.header.faces_count {
        let texture_name: String = read_string_block(cursor, TEXTURE_NAME_MAX_SIZE)?;
        model.texture_names.push(texture_name);
    }
    model.indices = decode_indices(&model);
    let bsp_nodes_count = model.header.num3;
    if bsp_nodes_count > 0 {
        println!("bsp node: {bsp_nodes_count}");
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
        name1: read_string_block(cursor, MODEL_NAME_MAX_SIZE)?,
        name2: read_string_block(cursor, MODEL_NAME_MAX_SIZE)?,
        attributes: cursor.read_i32::<LittleEndian>()?,
        vertex_count: cursor.read_i32::<LittleEndian>()?,
        ..Default::default()
    };
    let _p_vertex: i32 = cursor.read_i32::<LittleEndian>()?;
    header.faces_count = cursor.read_i32::<LittleEndian>()?;
    header.unk02 = cursor.read_i32::<LittleEndian>()?;
    let _p_faces = cursor.read_i32::<LittleEndian>()?;
    let _p_unk_array = cursor.read_i32::<LittleEndian>()?;
    header.num3 = cursor.read_i32::<LittleEndian>()?;
    header.unk03a = cursor.read_i32::<LittleEndian>()?;
    header.unk03b = cursor.read_i32::<LittleEndian>()?;
    header.unk03 = [
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
    ];
    header.origin1 = [
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
    header.unk04 = [
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
    ];
    header.origin2 = [
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
        cursor.read_i32::<LittleEndian>()?,
    ];
    header.unk05 = cursor.read_i32::<LittleEndian>()?;
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
            (0..f.vertices_count - 2)
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
