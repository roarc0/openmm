use std::{
    error::Error,
    io::{Cursor, Read, Seek},
};

use byteorder::{LittleEndian, ReadBytesExt};
use image::{Rgb, RgbImage};

use crate::{
    bmodel::{read_bmodels, BModel},
    read_string,
};

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
        let bmodels = read_bmodels(cursor, bmodel_count)?;

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
