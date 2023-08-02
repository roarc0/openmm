use byteorder::{LittleEndian, ReadBytesExt};
use std::{error::Error, io::Cursor};

use crate::utils;

const RAW_HEADER_6_SIZE: usize = 8;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Raw {
    pub data: Vec<u8>,
}

impl TryFrom<Vec<u8>> for Raw {
    type Error = Box<dyn Error>;

    fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(data.as_slice())
    }
}

impl TryFrom<&[u8]> for Raw {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let mut cursor = Cursor::new(data);
        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        let compressed_data = &data[RAW_HEADER_6_SIZE..];
        utils::check_size(compressed_data.len(), compressed_size)?;

        let uncompressed_data = utils::decompress(compressed_data, uncompressed_size)?;
        utils::check_size(uncompressed_data.len(), uncompressed_size)?;

        Ok(Self {
            data: uncompressed_data,
        })
    }
}

impl Raw {
    pub fn to_file(&self, path: &str) -> Result<(), Box<dyn Error>> {
        use std::fs::write;
        write(path, &self.data)?;
        Ok(())
    }
}
