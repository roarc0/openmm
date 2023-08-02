use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    error::Error,
    io::{Cursor, Seek},
};

use crate::utils;

const SPRITE_HEADER_SIZE: usize = 32;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Sprite {
    height: u16,
    width: u16,
    palette_count: u16,
    palette: Vec<u8>,
    data: Vec<u8>,
}

impl TryFrom<Vec<u8>> for Sprite {
    type Error = Box<dyn Error>;

    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(data.as_slice())
    }
}

impl TryFrom<&[u8]> for Sprite {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        // let first_zero_idx = data.iter().position(|&x| x == 0).unwrap_or(data.len());
        // let name: &str = std::str::from_utf8(&data[0..first_zero_idx])?;

        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(12))?;

        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let width = cursor.read_u16::<LittleEndian>()?;
        let height = cursor.read_u16::<LittleEndian>()?;
        let palette_count = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(std::io::SeekFrom::Current(6))?;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        let palette_size: usize = (height * 8) as usize;
        let palette = &data[SPRITE_HEADER_SIZE..palette_size];

        let compressed_data = &data[SPRITE_HEADER_SIZE + palette_size..];
        utils::check_size(compressed_data.len(), compressed_size)?;

        let uncompressed_data = utils::decompress(compressed_data, uncompressed_size)?;
        utils::check_size(uncompressed_data.len(), uncompressed_size)?;

        Ok(Self {
            height,
            width,
            palette_count,
            palette: palette.to_vec(),
            data: uncompressed_data,
        })
    }
}

fn process_image_data(sprite: &Sprite, table_data: &[u8], dec_data: &[u32]) -> Vec<u8> {
    let img_data_size = (sprite.width * sprite.height) as usize;
    let mut img_data: Vec<u8> = vec![0; img_data_size];
    let mut img_index = 0;

    // for i in 0..sprite.height {
    //     for _ in s.1..sprite.width - 1 {
    //         img_data[img_index] = 0;
    //         img_index += 1;
    //     }
    // }

    img_data
}
