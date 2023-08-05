use byteorder::{LittleEndian, ReadBytesExt};
use std::{
    error::Error,
    io::{Cursor, Seek},
    path::Path,
};

const COMPRESSED_HEADER_SIZE: usize = 48;

#[derive(Debug)]
pub struct RawUnpacked {
    data: Vec<u8>,
}

impl TryFrom<&[u8]> for RawUnpacked {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(20))?;
        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        cursor.seek(std::io::SeekFrom::Current(16))?;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        let uncompressed_data = super::zlib::decompress(
            &data[COMPRESSED_HEADER_SIZE..],
            compressed_size,
            uncompressed_size,
        )?;

        Ok(Self {
            data: uncompressed_data,
        })
    }
}

impl RawUnpacked {
    pub fn dump<Q>(&self, path: Q) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        use std::fs::write;
        write(path, &self.data)?;
        Ok(())
    }
}
