use byteorder::{LittleEndian, ReadBytesExt};
use image::{ImageBuffer, Rgb};
use std::{
    error::Error,
    io::{Cursor, Seek},
};

use crate::{palette, utils};

const SPRITE_HEADER_SIZE: usize = 32;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Sprite {
    height: usize,
    width: usize,
    pal_index: u16,
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
        let pal_index = cursor.read_u16::<LittleEndian>()?;
        cursor.seek(std::io::SeekFrom::Current(6))?;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        let table_size: usize = (height as usize) * 8;
        let table = &data[SPRITE_HEADER_SIZE..(SPRITE_HEADER_SIZE + table_size)];

        let compressed_data = &data[SPRITE_HEADER_SIZE + table_size..];
        utils::check_size(compressed_data.len(), compressed_size)?;

        let uncompressed_data = utils::decompress(compressed_data, uncompressed_size)?;
        utils::check_size(uncompressed_data.len(), uncompressed_size)?;

        let data = process_image_data(height, width, table, uncompressed_data.as_slice())?;

        Ok(Self {
            height: height as usize,
            width: width as usize,
            pal_index,
            data,
        })
    }
}

fn process_image_data(
    height: u16,
    width: u16,
    table: &[u8],
    data: &[u8],
) -> Result<Vec<u8>, Box<dyn Error>> {
    let img_size = (width * height) as usize;
    let mut img: Vec<u8> = vec![0; img_size];
    let mut img_index = 0;

    for i in 0..height {
        let mut cursor = Cursor::new(table);
        cursor.seek(std::io::SeekFrom::Start((i as u64) * 8))?;
        let start = cursor.read_i16::<LittleEndian>()?;
        let end = cursor.read_i16::<LittleEndian>()?;
        let offset = cursor.read_u32::<LittleEndian>()?;

        if img_index >= img_size {
            continue;
        }

        if start != -1 && end != -1 {
            for _ in 0..start {
                img[img_index] = 0;
                img_index += 1;
            }
            let mut off_index = offset as usize;
            for _ in start..=end {
                img[img_index] += data[off_index];
                img_index += 1;
                off_index += 1;
            }
        }
        for _ in (end as u16)..(width - 1) {
            img[img_index] = 0;
            img_index += 1;
        }
    }

    Ok(img)
}

impl Sprite {
    pub fn to_image_buffer(
        &self,
        palettes: &palette::Palettes,
    ) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, Box<dyn Error>> {
        let palette_name = format!("pal{:03}", self.pal_index);
        let palette = palettes
            .map
            .get(&palette_name)
            .ok_or_else(|| "palette not found!".to_string())?;
        let palette = palette.data;

        let mut image_buffer =
            ImageBuffer::<Rgb<u8>, Vec<u8>>::new(self.width as u32, self.height as u32);

        for (i, pixel_index) in self.data[..(self.width * self.height)].iter().enumerate() {
            let x = (i).rem_euclid(self.width) as u32;
            let y = (i).div_euclid(self.width) as u32;
            let idx = 3 * (*pixel_index as usize);
            let pixel = Rgb([palette[idx], palette[idx + 1], palette[idx + 2]]);
            image_buffer.put_pixel(x, y, pixel);
        }
        Ok(image_buffer)
    }

    pub fn to_png_file(
        &self,
        path: &str,
        palettes: &palette::Palettes,
    ) -> Result<(), Box<dyn Error>> {
        self.to_image_buffer(palettes)?
            .save_with_format(path, image::ImageFormat::Png)?;
        Ok(())
    }
}
