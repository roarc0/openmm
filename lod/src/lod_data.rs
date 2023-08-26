use std::{
    error::Error,
    io::{Cursor, Seek},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt};

#[allow(dead_code)]
#[derive(Debug)]
pub struct LodData<'a> {
    pub header: Option<&'a [u8]>,
    pub data: Vec<u8>,
}

impl<'a> TryFrom<&'a [u8]> for LodData<'a> {
    type Error = Box<dyn Error>;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if let Ok(lod_data) = decompress_with_8_bytes_header(data) {
            Ok(lod_data)
        } else if let Ok(lod_data) = decompress_with_48_bytes_header(data) {
            Ok(lod_data)
        } else {
            Ok(Self {
                header: None,
                data: data.to_vec(),
            })
        }
    }
}

fn decompress_with_48_bytes_header(data: &[u8]) -> Result<LodData, Box<dyn Error>> {
    let mut cursor = Cursor::new(data);
    cursor.seek(std::io::SeekFrom::Start(20))?;
    let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    cursor.seek(std::io::SeekFrom::Current(16))?;
    let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    Ok(LodData {
        header: Some(&data[..48]),
        data: super::zlib::decompress(&data[48..], compressed_size, uncompressed_size)?.to_vec(),
    })
}

fn decompress_with_8_bytes_header(data: &[u8]) -> Result<LodData, Box<dyn Error>> {
    let compressed_size = u32::from_le_bytes(data[0..=3].try_into()?) as usize;
    let decompressed_size = u32::from_le_bytes(data[4..=7].try_into()?) as usize;
    Ok(LodData {
        header: Some(&data[..8]),
        data: super::zlib::decompress(&data[8..], compressed_size, decompressed_size)?.to_vec(),
    })
}

impl LodData<'_> {
    pub fn dump<Q>(&self, path: Q) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        use std::fs::write;
        write(path, self.data.as_slice())?;
        Ok(())
    }
}
