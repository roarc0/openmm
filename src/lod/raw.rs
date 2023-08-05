use std::{
    error::Error,
    io::{Cursor, Seek},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt};

#[allow(dead_code)]
#[derive(Debug)]
pub struct Raw<'a> {
    pub header: Option<&'a [u8]>,
    pub data: Vec<u8>,
}

impl<'a> TryFrom<&'a [u8]> for Raw<'a> {
    type Error = Box<dyn Error>;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        if let Ok(raw) = decompress_8_bytes_header(data) {
            Ok(raw)
        } else if let Ok(raw) = decompress_48_bytes_header(data) {
            Ok(raw)
        } else {
            Ok(Self {
                header: None,
                data: data.to_vec(),
            })
        }
    }
}

fn decompress_48_bytes_header(data: &[u8]) -> Result<Raw, Box<dyn Error>> {
    let mut cursor = Cursor::new(data);
    cursor.seek(std::io::SeekFrom::Start(20))?;
    let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    cursor.seek(std::io::SeekFrom::Current(16))?;
    let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    Ok(Raw {
        header: Some(&data[..48]),
        data: super::zlib::decompress(&data[48..], compressed_size, uncompressed_size)?.to_vec(),
    })
}

fn decompress_8_bytes_header(data: &[u8]) -> Result<Raw, Box<dyn Error>> {
    let mut cursor = Cursor::new(data);
    let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    let decompressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    Ok(Raw {
        header: Some(&data[..8]),
        data: super::zlib::decompress(&data[8..], compressed_size, decompressed_size)?.to_vec(),
    })
}

impl Raw<'_> {
    pub fn dump<Q>(&self, path: Q) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        use std::fs::write;
        write(path, self.data.as_slice())?;
        Ok(())
    }
}
