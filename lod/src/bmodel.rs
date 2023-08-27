use std::{
    error::Error,
    io::{Cursor, Read, Seek},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::read_string;

#[derive(Debug)]
pub struct BModel {
    pub header: BModelHeader,
    pub vertexes: Vec<[f32; 3]>,
    pub faces: Vec<BModelFace>,
    pub unk: Vec<u8>,
    pub texture_names: Vec<String>,
    pub bsp_nodes: Vec<i32>,

    pub indices: Vec<u32>,
}

#[derive(Debug, Default)]
pub struct BModelHeader {
    pub name1: String,
    pub name2: String,
    pub attrib: i32,
    pub num_vertex: i32,
    // p_vertexes: *mut i32,
    pub num_faces: i32,
    unk02: i32,
    // p_faces: *mut i32,
    // p_unk_array: *mut i32,
    num3: i32,
    unk03a: i32,
    unk03b: i32,
    unk03: [i32; 2],
    pub origin1: [i32; 3],
    pub bbox: [[i32; 3]; 2],
    unk04: [i32; 6],
    pub origin2: [i32; 3],
    unk05: i32,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct plane {
    normal: [i32; 3],
    dist: i32,
}

#[repr(C)]
#[derive(Debug, Default)]
pub struct bbox {
    min_x: i16,
    max_x: i16,
    min_y: i16,
    max_y: i16,
    min_z: i16,
    max_z: i16,
}

const MAXNUMV_BMFACE: usize = 0x14;

#[repr(C)]
#[derive(Debug, Default)]
pub struct BModelFace {
    plane: plane,
    z_calc: [i16; 6],
    attr: u32,
    v_idx: [u16; MAXNUMV_BMFACE],
    tex_x: [i16; MAXNUMV_BMFACE],
    tex_y: [i16; MAXNUMV_BMFACE],
    normal_x: [i16; MAXNUMV_BMFACE],
    normal_y: [i16; MAXNUMV_BMFACE],
    normal_z: [i16; MAXNUMV_BMFACE],
    unk02: i16,
    tex_dx: i16,
    tex_dy: i16,
    bbox: bbox,
    COG_NUMBER: u16,
    COG_TRIGGERED_NUMBER: u16,
    COG_TRIGGER: u16,
    RESERVED: u16,
    GRADIENT_VERTEXES: [u8; 4],
    numv: u8,
    POLYGON_TYPE: u8,
    SHADE: u8,
    VISIBILITY: u8,
    PADDING: [u8; 2],
}

pub(super) fn read_bmodels(
    mut cursor: Cursor<&[u8]>,
    bmodel_count: usize,
) -> Result<Vec<BModel>, Box<dyn Error>> {
    let mut bmodels: Vec<BModel> = Vec::with_capacity(bmodel_count);

    for _i in 0..bmodel_count {
        let mut header = BModelHeader {
            name1: read_string_block(&mut cursor, 32)?,
            name2: read_string_block(&mut cursor, 32)?,
            attrib: cursor.read_i32::<LittleEndian>()?,
            num_vertex: cursor.read_i32::<LittleEndian>()?,
            ..Default::default()
        };
        let _p_vertex: i32 = cursor.read_i32::<LittleEndian>()?;
        header.num_faces = cursor.read_i32::<LittleEndian>()?;
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
        header.bbox = [
            [
                cursor.read_i32::<LittleEndian>()?,
                cursor.read_i32::<LittleEndian>()?,
                cursor.read_i32::<LittleEndian>()?,
            ],
            [
                cursor.read_i32::<LittleEndian>()?,
                cursor.read_i32::<LittleEndian>()?,
                cursor.read_i32::<LittleEndian>()?,
            ],
        ];
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

        bmodels.push(BModel {
            header,
            vertexes: Vec::new(),
            faces: Vec::new(),
            unk: Vec::new(),
            texture_names: Vec::new(),
            bsp_nodes: Vec::new(),
            indices: Vec::new(),
        });
    }

    for i in 0..bmodel_count {
        let bmodel = bmodels.get_mut(i).ok_or("expected bmodel")?;

        let mut vertices: Vec<f32> = Vec::new();
        for _i in 0..bmodel.header.num_vertex * 3 {
            vertices.push(cursor.read_i32::<LittleEndian>()? as f32);
        }

        for _i in 0..bmodel.header.num_faces {
            let mut bmodel_face = BModelFace::default();
            cursor.read_exact(unsafe {
                std::slice::from_raw_parts_mut(
                    &mut bmodel_face as *mut _ as *mut u8,
                    std::mem::size_of::<BModelFace>(),
                )
            })?;
            bmodel.faces.push(bmodel_face);
        }

        let mut unk: Vec<u8> = Vec::new();
        for _i in 0..bmodel.header.num_faces * 2 {
            unk.push(cursor.read_u8()?)
        }
        bmodel.unk = unk;

        for _i in 0..bmodel.header.num_faces {
            let texture_name: String = read_string_block(&mut cursor, 10)?;
            bmodel.texture_names.push(texture_name);
        }

        bmodel.vertexes = decode_vertices(vertices);
        bmodel.indices = generate_indices(bmodel.header.num_vertex as usize);

        let bsp_nodes_count = bmodel.header.num3;
        if bsp_nodes_count > 0 {
            println!("{i} bsp nodes! {bsp_nodes_count}");
            let mut bsp_nodes: Vec<i32> = Vec::with_capacity((bsp_nodes_count * 2) as usize);
            for _i in 0..bsp_nodes_count * 2 {
                bsp_nodes.push(cursor.read_i32::<LittleEndian>()?)
            }
            bmodel.bsp_nodes = bsp_nodes;
        }
    }

    dbg!("here");

    Ok(bmodels)
}

fn read_string_block(cursor: &mut Cursor<&[u8]>, size: u64) -> Result<String, Box<dyn Error>> {
    let pos = cursor.position();
    let s = read_string(cursor)?;
    cursor.seek(std::io::SeekFrom::Start(pos + size))?;
    Ok(s)
}

fn peek_next_bytes(cursor: &mut Cursor<&[u8]>, n: usize) -> Result<String, Box<dyn Error>> {
    let mut t: Vec<u8> = Vec::new();
    for _i in 0..n {
        t.push(cursor.read_u8()?)
    }
    Ok(t.iter()
        .map(|&x| if x != 0 { x as char } else { '.' })
        .collect())
}

fn decode_vertices(input: Vec<f32>) -> Vec<[f32; 3]> {
    input
        .chunks_exact(3)
        .map(|chunk| [chunk[0], chunk[2], -chunk[1]])
        .collect()
}

fn generate_indices(vertices_count: usize) -> Vec<u32> {
    let mut indices = Vec::new();
    for i in (0..vertices_count).step_by(3) {
        indices.push(i as u32);
        indices.push((i + 1) as u32);
        indices.push((i + 2) as u32);

        indices.push((i + 2) as u32);
        indices.push(i as u32);
        indices.push((i + 1) as u32);
    }
    indices
}
