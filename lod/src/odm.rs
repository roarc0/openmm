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

const TILE_HDR_SIZE: usize = 4;
const TILE_IDX_SIZE: usize = 16;

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

        Ok(Self {
            name: "test".into(),
            odm_version,
            sky_texture,
            ground_texture,
            tile_data,
            height_map,
            tile_map,
            attribute_map,
        })
    }
}

impl Odm {
    fn save_heightmap(&self) {
        raw_to_image_buffer(
            &self.height_map,
            &self.tile_map,
            ODM_MAP_SIZE as u32,
            ODM_MAP_SIZE as u32,
        )
        .unwrap()
        .save_with_format(
            format!("{}_height_map.bmp", self.name),
            image::ImageFormat::Bmp,
        )
        .unwrap();
    }
}

fn raw_to_image_buffer(
    data: &[u8],
    data2: &[u8],
    width: u32,
    height: u32,
) -> Result<RgbImage, Box<dyn std::error::Error>> {
    let mut image_buffer = RgbImage::new(width, height);

    for (i, pi) in data[..(width * height) as usize].iter().enumerate() {
        let x = (i as u32).rem_euclid(width);
        let y = (i as u32).div_euclid(width);
        let pixel = Rgb([*pi, *pi, data2[i]]);
        image_buffer.put_pixel(x, y, pixel);
    }
    Ok(image_buffer)
}

#[cfg(test)]
mod tests {
    use std::fs::write;
    use std::path::Path;

    use crate::{get_lod_path, raw, Lod};

    use super::*;

    #[test]
    fn get_image_works() {
        let lod_path = get_lod_path();
        let lod_path = Path::new(&lod_path);

        let games_lod = Lod::open(lod_path.join("games.lod")).unwrap();

        let map_name = "oute3";
        let map = raw::Raw::try_from(
            games_lod
                .try_get_bytes(&format!("{}.odm", map_name))
                .unwrap(),
        )
        .unwrap();
        let _ = write(format!("{}.odm", map_name), &map.data);
        let map = Odm::try_from(map.data.as_slice()).unwrap();

        let _ = write(
            format!("{}.rs", map_name),
            format!(
                "pub const HEIGHT_MAP: [u8; 128 * 128] = {:?};\npub const TILE_MAP: [u8; 128 * 128] = {:?};",
                map.height_map, map.tile_map
            ),
        );

        map.save_heightmap();
    }
}
