use std::{
    error::Error,
    io::{Cursor, Seek},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

#[allow(dead_code)]
#[derive(Debug)]
pub enum CompressionKind {
    None,
    Zlib8,  // 8-byte header (compressed_size, uncompressed_size)
    Zlib48, // 48-byte header (complex MM6 variant)
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct LodData {
    pub kind: CompressionKind,
    pub data: Vec<u8>,
}

impl TryFrom<&[u8]> for LodData {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if let Ok(decompressed) = decompress_with_8_bytes_header(data) {
            Ok(Self {
                kind: CompressionKind::Zlib8,
                data: decompressed,
            })
        } else if let Ok(decompressed) = decompress_with_48_bytes_header(data) {
            Ok(Self {
                kind: CompressionKind::Zlib48,
                data: decompressed,
            })
        } else {
            Ok(Self {
                kind: CompressionKind::None,
                data: data.to_vec(),
            })
        }
    }
}

fn decompress_with_48_bytes_header(data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    if data.len() < 48 {
        return Err("data too short for 48-byte header".into());
    }
    let mut cursor = Cursor::new(data);
    cursor.seek(std::io::SeekFrom::Start(20))?;
    let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    cursor.seek(std::io::SeekFrom::Current(16))?;
    let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;
    crate::assets::zlib::decompress(&data[48..], compressed_size, uncompressed_size)
}

fn decompress_with_8_bytes_header(data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    if data.len() < 8 {
        return Err("data too short for 8-byte header".into());
    }
    let header_compressed_size = u32::from_le_bytes(data[0..=3].try_into()?) as usize;
    let decompressed_size = u32::from_le_bytes(data[4..=7].try_into()?) as usize;
    let payload = &data[8..];
    // MM6 save LOD entries store total size (including header) in compressed_size.
    // Normal LOD entries store just the payload size. Handle both.
    let compressed_size = if header_compressed_size == data.len() {
        payload.len()
    } else {
        header_compressed_size
    };
    crate::assets::zlib::decompress(payload, compressed_size, decompressed_size)
}

impl LodData {
    pub fn pack(&self) -> Vec<u8> {
        match self.kind {
            CompressionKind::None => self.data.clone(),
            CompressionKind::Zlib8 => {
                let compressed = crate::assets::zlib::compress(&self.data);
                if compressed.len() >= self.data.len() {
                    // Optimization: if compressed is larger, MM6 usually stores raw.
                    // But for round-trip verification, we might want to follow original exactly.
                }
                let mut out = Vec::with_capacity(8 + compressed.len());
                out.write_u32::<LittleEndian>(compressed.len() as u32).unwrap();
                out.write_u32::<LittleEndian>(self.data.len() as u32).unwrap();
                out.extend_from_slice(&compressed);
                out
            }
            CompressionKind::Zlib48 => {
                // MM6 48-byte header is complex and usually only for ODM or specific files.
                // For now, if we can't perfectly replicate the 48-byte header bits, we fall back to raw
                // or just re-compress with the 48-byte structure.
                let compressed = crate::assets::zlib::compress(&self.data);
                let mut out = vec![0u8; 48];
                let mut cursor = Cursor::new(&mut out);
                cursor.seek(std::io::SeekFrom::Start(20)).unwrap();
                cursor.write_u32::<LittleEndian>(compressed.len() as u32).unwrap();
                cursor.seek(std::io::SeekFrom::Current(16)).unwrap();
                cursor.write_u32::<LittleEndian>(self.data.len() as u32).unwrap();
                out.extend_from_slice(&compressed);
                out
            }
        }
    }

    pub fn dump<Q>(&self, path: Q) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        use std::fs::write;
        write(path, self.data.as_slice())?;
        Ok(())
    }
}
