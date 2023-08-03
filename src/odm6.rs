use std::{
    error::Error,
    io::{BufRead, Cursor, Read, Seek},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils;

const COMPRESSED_HEADER_SIZE: usize = 8;

const MAP_SIZE: usize = 128;
const MAP_PLAY_SIZE: usize = 88;
//const MAP_TILE_SIZE: usize = 512;
//const MAP_HEIGHT_SIZE: usize = 32;
//const _MAP_OFFSET: u64 = 64;

//const TILE_IDX_SIZE: usize = 16;
//const TILE_HDR_SIZE: usize = 4;

const HEIGHT_MAP_OFFSET: u64 = 176;
const HEIGHT_MAP_SIZE: usize = MAP_SIZE * MAP_SIZE;

const TILE_MAP_OFFSET: u64 = HEIGHT_MAP_OFFSET + HEIGHT_MAP_SIZE as u64;
const TILEMAP_SIZE: usize = MAP_SIZE * MAP_SIZE;

// const ATTRIBUTE_MAP_OFFSET: u64 = HEIGHT_MAP_OFFSET + HEIGHT_MAP_SIZE as u64;
// const ATTRIBUTE_MAP_SIZE: usize = MAP_SIZE * MAP_SIZE;

const BMODELS_OFFSET: u64 = TILE_MAP_OFFSET + TILEMAP_SIZE as u64;
const BMODELS_HDR_SIZE: usize = 0xbc;

const SPRITES_OFFSET: u64 = 0;
const SPRITES_HDR_SIZE: usize = 0x20;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Odm6 {
    odm_version: String,
    sky_texture: String,
    ground_texture: String,
    height_map: Vec<u8>,
    tile_map: Vec<u8>,
}

impl TryFrom<Vec<u8>> for Odm6 {
    type Error = Box<dyn Error>;

    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(data.as_slice())
    }
}

impl TryFrom<&[u8]> for Odm6 {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let mut cursor = Cursor::new(data);
        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        let compressed_data = &data[COMPRESSED_HEADER_SIZE..];
        utils::check_size(compressed_data.len(), compressed_size)?;

        let uncompressed_data = utils::decompress(compressed_data, uncompressed_size)?;
        utils::check_size(uncompressed_data.len(), uncompressed_size)?;

        let mut cursor = Cursor::new(uncompressed_data.as_slice());
        cursor.seek(std::io::SeekFrom::Start(2 * 32))?;
        let odm_version = read_string(&mut cursor)?;

        cursor.seek(std::io::SeekFrom::Start(3 * 32))?;
        let sky_texture = read_string(&mut cursor)?;

        cursor.seek(std::io::SeekFrom::Start(4 * 32))?;
        let ground_texture = read_string(&mut cursor)?;

        cursor.seek(std::io::SeekFrom::Start(HEIGHT_MAP_OFFSET))?;
        let mut height_map: [u8; HEIGHT_MAP_SIZE] = [0; HEIGHT_MAP_SIZE];
        cursor.read_exact(&mut height_map)?;

        cursor.seek(std::io::SeekFrom::Start(TILE_MAP_OFFSET))?;
        let mut tile_map: [u8; HEIGHT_MAP_SIZE] = [0; HEIGHT_MAP_SIZE];
        cursor.read_exact(&mut tile_map)?;

        Ok(Self {
            odm_version,
            sky_texture,
            ground_texture,
            height_map: height_map.to_vec(),
            tile_map: tile_map.to_vec(),
            //attribute_map: attribute_map.to_vec(),
        })
    }
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String, Box<dyn Error>> {
    let mut buf = Vec::new();
    cursor.read_until(0, &mut buf)?;
    Ok(String::from_utf8(buf)?)
}

impl Odm6 {}
