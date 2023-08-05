use byteorder::{LittleEndian, ReadBytesExt};
use image::{ImageBuffer, Rgb};
use std::{
    error::Error,
    io::{Cursor, Seek},
    path::Path,
};

use super::{palette::Palettes, zlib};

#[derive(Debug)]
pub struct Image {
    height: usize,
    width: usize,
    data: Vec<u8>,
    palette: [u8; PALETTE_SIZE],
}

const PALETTE_SIZE: usize = 256 * 3;
const BITMAP_HEADER_SIZE: usize = 48;
const SPRITE_HEADER_SIZE: usize = 32;

/// This is for bitmap images
impl TryFrom<&[u8]> for Image {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(16))?;
        let pixel_size = cursor.read_u32::<LittleEndian>()? as usize;
        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let width = cursor.read_u16::<LittleEndian>()? as usize;
        let height = cursor.read_u16::<LittleEndian>()? as usize;
        cursor.seek(std::io::SeekFrom::Current(12))?;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        if pixel_size == 0 {
            return Err("Pixel size is zero, this is not a valid image".into());
        }
        if data.len() <= BITMAP_HEADER_SIZE + PALETTE_SIZE {
            return Err("Not enough data".into());
        }

        let compressed_data = &data[BITMAP_HEADER_SIZE..data.len() - PALETTE_SIZE];
        let uncompressed_data =
            zlib::decompress(compressed_data, compressed_size, uncompressed_size)?;

        let palette_slice = &data[data.len() - PALETTE_SIZE..];
        let palette: [u8; PALETTE_SIZE] = palette_slice.try_into()?;

        Ok(Self {
            height,
            width,
            data: uncompressed_data,
            palette,
        })
    }
}

/// This is for sprite images
impl TryFrom<(&[u8], &Palettes)> for Image {
    type Error = Box<dyn Error>;

    fn try_from(data: (&[u8], &Palettes)) -> Result<Self, Self::Error> {
        let palettes = data.1;
        let data = data.0;

        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(12))?;

        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let width = cursor.read_u16::<LittleEndian>()? as usize;
        let height = cursor.read_u16::<LittleEndian>()? as usize;

        let palette_id = cursor.read_u16::<LittleEndian>()?;
        let palette = palettes
            .get(palette_id)
            .ok_or_else(|| "Palette not found!".to_string())?;

        cursor.seek(std::io::SeekFrom::Current(6))?;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        let table_size: usize = height * 8;

        if data.len() <= SPRITE_HEADER_SIZE + table_size {
            return Err("Not enough data".into());
        }

        let table = &data[SPRITE_HEADER_SIZE..(SPRITE_HEADER_SIZE + table_size)];

        let compressed_data = &data[SPRITE_HEADER_SIZE + table_size..];
        let uncompressed_data =
            super::zlib::decompress(compressed_data, compressed_size, uncompressed_size)?;

        let processed_data =
            process_sprite_data(uncompressed_data.as_slice(), table, width, height)?;

        Ok(Self {
            height,
            width,
            data: processed_data,
            palette: palette.data,
        })
    }
}

fn process_sprite_data(
    data: &[u8],
    table: &[u8],
    width: usize,
    height: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let img_size = width * height;
    let mut img: Vec<u8> = vec![0; img_size];
    let mut current: usize = 0;
    let mut cursor = Cursor::new(table);

    for _ in 0..height {
        let start = cursor.read_i16::<LittleEndian>()?;
        let end = cursor.read_i16::<LittleEndian>()?;
        let offset = cursor.read_u32::<LittleEndian>()? as usize;

        if start < 0 || end < 0 {
            current += width - 1;
            continue;
        }

        current += start as usize;
        let chunk_size = (end - start + 1) as usize;
        img[current..current + chunk_size].copy_from_slice(&data[offset..offset + chunk_size]);
        current += width - start as usize;
    }
    Ok(img)
}

impl Image {
    pub fn dump<Q>(&self, path: Q) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        raw_to_image_buffer(
            &self.data,
            &self.palette,
            self.width as u32,
            self.height as u32,
        )?
        .save_with_format(path, image::ImageFormat::Png)?;
        Ok(())
    }
}

/// Converts the image into a versatile generic image buffer.
/// The image contains more pixels than needed with dimensions (h*w) to account for mipmaps,
/// but we are currently not utilizing those extra pixels.
/// It PANICS if the input is not appropriate.
pub fn raw_to_image_buffer(
    data: &[u8],
    palette: &[u8; 768],
    width: u32,
    height: u32,
) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
    let mut image_buffer = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(width, height);

    for (i, pixel_index) in data[..(width * height) as usize].iter().enumerate() {
        let x = (i as u32).rem_euclid(width);
        let y = (i as u32).div_euclid(width);
        let idx = 3 * (*pixel_index as usize);
        image_buffer.put_pixel(x, y, Rgb(palette[idx..idx + 3].try_into()?));
    }
    Ok(image_buffer)
}
