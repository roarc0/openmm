use std::{
    error::Error,
    io::{Cursor, Read, Seek},
};

use byteorder::{LittleEndian, ReadBytesExt};
use image::{Rgb, RgbImage};

use crate::read_string;

pub const ODM_MAP_SIZE: usize = 128;
pub const ODM_MAP_PLAY_SIZE: usize = 88;
pub const ODM_MAP_AREA: usize = ODM_MAP_SIZE * ODM_MAP_SIZE;
pub const ODM_MAP_TILE_SIZE: usize = 512;
pub const ODM_MAP_HEIGHT_SIZE: usize = 32;

const HEIGHT_MAP_OFFSET: u64 = 176;
const HEIGHT_MAP_SIZE: usize = ODM_MAP_AREA;

const TILE_MAP_OFFSET: u64 = HEIGHT_MAP_OFFSET + HEIGHT_MAP_SIZE as u64;
const TILEMAP_SIZE: usize = ODM_MAP_AREA;

const ATTRIBUTE_MAP_OFFSET: u64 = TILE_MAP_OFFSET + ATTRIBUTE_MAP_SIZE as u64;
const ATTRIBUTE_MAP_SIZE: usize = ODM_MAP_AREA;

// const BMODELS_OFFSET: u64 = TILE_MAP_OFFSET + TILEMAP_SIZE as u64;
// const BMODELS_HDR_SIZE: usize = 0xbc;

// const SPRITES_OFFSET: u64 = 0;
// const SPRITES_HDR_SIZE: usize = 0x20;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Odm {
    pub name: String,
    pub odm_version: String,
    pub sky_texture: String,
    pub ground_texture: String,
    pub tile_data: [u16; 8],
    pub height_map: [u8; HEIGHT_MAP_SIZE],
    pub tile_map: [u8; TILEMAP_SIZE],
    pub attribute_map: [u8; ATTRIBUTE_MAP_SIZE],
    pub bmodels: Vec<BModel>,
}

#[derive(Debug)]
pub struct BModel {
    pub header: BModelHeader,
    pub vertexes: Vec<[f32; 3]>,
    pub faces: Vec<BModelFace>,
}

#[derive(Debug)]
pub struct BModelHeader {
    pub name1: String,
    pub name2: String,
    pub attrib: i32,
    pub num_vertex: i32,
    // p_vertexes: *mut i32, // Change to appropriate pointer type
    pub num_faces: i32,
    unk02: i32,
    // p_faces: *mut i32,pub  // Change to appropriate pointer type
    // p_unk_array: *mut i32, // Change to appropriate pointer type
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

#[derive(Debug)]
pub struct BModelFace {}

impl TryFrom<&[u8]> for Odm {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(2 * 32))?;
        let odm_version = read_string(&mut cursor)?;
        cursor.seek(std::io::SeekFrom::Start(3 * 32))?;
        let sky_texture = read_string(&mut cursor)?;
        cursor.seek(std::io::SeekFrom::Start(4 * 32))?;
        let ground_texture = read_string(&mut cursor)?;
        cursor.seek(std::io::SeekFrom::Start(5 * 32))?;
        let tile_data: [u16; 8] = [
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
        ];

        cursor.seek(std::io::SeekFrom::Start(HEIGHT_MAP_OFFSET))?;
        let mut height_map: [u8; HEIGHT_MAP_SIZE] = [0; HEIGHT_MAP_SIZE];
        cursor.read_exact(&mut height_map)?;

        cursor.seek(std::io::SeekFrom::Start(TILE_MAP_OFFSET))?;
        let mut tile_map: [u8; TILEMAP_SIZE] = [0; TILEMAP_SIZE];
        cursor.read_exact(&mut tile_map)?;

        cursor.seek(std::io::SeekFrom::Start(ATTRIBUTE_MAP_OFFSET))?;
        let mut attribute_map: [u8; ATTRIBUTE_MAP_SIZE] = [0; ATTRIBUTE_MAP_SIZE];
        cursor.read_exact(&mut attribute_map)?;

        let bmodel_count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut bmodels: Vec<BModel> = Vec::with_capacity(bmodel_count);

        read_bmodels(cursor, bmodel_count, &mut bmodels)?;

        Ok(Self {
            name: "test".into(),
            odm_version,
            sky_texture,
            ground_texture,
            tile_data,
            height_map,
            tile_map,
            attribute_map,
            bmodels,
        })
    }
}

fn read_bmodels(
    mut cursor: Cursor<&[u8]>,
    bmodel_count: usize,
    bmodels: &mut Vec<BModel>,
) -> Result<(), Box<dyn Error>> {
    let pos = cursor.position();
    for i in 0..bmodel_count {
        cursor.seek(std::io::SeekFrom::Start(pos + i as u64 * 0xbc_u64))?; // BModelSize==0xbc
        let pos = cursor.position();
        let name1 = read_string(&mut cursor)?.to_owned();
        cursor.seek(std::io::SeekFrom::Start(pos + 0x20))?;
        let pos = cursor.position();
        let name2 = read_string(&mut cursor)?.to_owned();
        cursor.seek(std::io::SeekFrom::Start(pos + 0x20))?;
        let attrib = cursor.read_i32::<LittleEndian>()?;
        let num_vertex = cursor.read_i32::<LittleEndian>()?;
        let _p_vertex = cursor.read_i32::<LittleEndian>()?;
        let num_faces = cursor.read_i32::<LittleEndian>()?;
        let unk02 = cursor.read_i32::<LittleEndian>()?;
        let _p_faces = cursor.read_i32::<LittleEndian>()?;
        let _p_unk_array = cursor.read_i32::<LittleEndian>()?;
        let num3 = cursor.read_i32::<LittleEndian>()?;
        let unk03a = cursor.read_i32::<LittleEndian>()?;
        let unk03b = cursor.read_i32::<LittleEndian>()?;
        let unk03 = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let origin1 = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let bbox = [
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

        let unk04 = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];

        let origin2 = [
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
            cursor.read_i32::<LittleEndian>()?,
        ];
        let unk05 = cursor.read_i32::<LittleEndian>()?;

        bmodels.push(BModel {
            header: BModelHeader {
                name1,
                name2,
                attrib,
                num_vertex,
                // p_vertexes,
                num_faces,
                unk02,
                // p_faces,
                // p_unk_array,
                num3,
                unk03a,
                unk03b,
                unk03,
                origin1,
                bbox,
                unk04,
                origin2,
                unk05,
            },
            vertexes: Vec::new(),
            faces: Vec::new(),
        });
    }

    // read other stuff
    for i in 0..bmodel_count {
        let bmodel = bmodels.get_mut(i).ok_or("expected bmodel")?;

        let mut vs: Vec<f32> = Vec::new();
        for _i in 0..bmodel.header.num_vertex {
            vs.push(cursor.read_i32::<LittleEndian>()? as f32);
        }

        let mut indices: Vec<u32> = Vec::new();
        for i in 0..(vs.len() - 2) {
            indices.push(i as u32);
            indices.push((i + 2) as u32);
            indices.push((i + 1) as u32);
        }

        fn group_into_triplets(input: Vec<f32>) -> Vec<[f32; 3]> {
            input
                .chunks_exact(3)
                .map(|chunk| [chunk[0], chunk[2], -chunk[1]])
                .collect()
        }

        bmodel.vertexes = group_into_triplets(vs)
    }

    dbg!("here");

    Ok(())
}

impl Odm {
    pub fn size(&self) -> (usize, usize) {
        (ODM_MAP_SIZE, ODM_MAP_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, lod_data::LodData, LodManager};

    #[test]
    fn get_map_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let map_name = "oute3";
        let map = LodData::try_from(
            lod_manager
                .try_get_bytes(&format!("games/{}.odm", map_name))
                .unwrap(),
        )
        .unwrap();
        //let _ = write(format!("{}.odm", map_name), &map.data);
        let _map = Odm::try_from(map.data.as_slice()).unwrap();
    }
}
