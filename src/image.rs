use byteorder::{LittleEndian, ReadBytesExt};
use image::{ImageBuffer, Rgb};
use std::{
    error::Error,
    io::{Cursor, Seek},
};

use crate::utils;

#[derive(Debug)]
pub struct Image {
    height: usize,
    width: usize,
    data: Vec<u8>,
    palette: [u8; PALETTE_SIZE],
}

const PALETTE_SIZE: usize = 256 * 3;
const IMAGE_HEADER_SIZE: usize = 48;

impl TryFrom<Vec<u8>> for Image {
    type Error = Box<dyn Error>;

    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(data.as_slice())
    }
}

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

        let compressed_data = &data[IMAGE_HEADER_SIZE..data.len() - PALETTE_SIZE];
        utils::check_size(compressed_data.len(), compressed_size)?;

        let uncompressed_data = utils::decompress(compressed_data, uncompressed_size)?;
        utils::check_size(uncompressed_data.len(), uncompressed_size)?;

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

impl Image {
    pub fn to_png_file(&self, path: &str) -> Result<(), Box<dyn Error>> {
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
