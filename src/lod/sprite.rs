use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    error::Error,
    io::{Cursor, Seek},
    path::Path,
};

const SPRITE_HEADER_SIZE: usize = 32;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Sprite {
    height: usize,
    width: usize,
    palette_id: u16,
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
        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(12))?;

        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let width = cursor.read_u16::<LittleEndian>()? as usize;
        let height = cursor.read_u16::<LittleEndian>()? as usize;
        let palette_id = cursor.read_u16::<LittleEndian>()?;
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

        let data = process_sprite_data(uncompressed_data.as_slice(), table, width, height)?;

        Ok(Self {
            height,
            width,
            palette_id,
            data,
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

impl Sprite {
    pub fn dump<Q>(
        &self,
        palettes: &super::palette::Palettes,
        path: Q,
    ) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        super::image::raw_to_image_buffer(
            &self.data,
            &palettes
                .get(self.palette_id)
                .ok_or_else(|| "palette not found!".to_string())?
                .data,
            self.width as u32,
            self.height as u32,
        )?
        .save_with_format(path, ::image::ImageFormat::Png)?;
        Ok(())
    }
}
