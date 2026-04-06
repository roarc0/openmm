//! Parser for dchest.bin — chest visual descriptors.
//! 36 bytes per record.

use std::error::Error;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};

use serde::{Serialize, Deserialize};

use crate::LodSerialise;
use crate::Assets;
use crate::assets::lod_data::LodData;

/// A chest visual descriptor from dchest.bin. 36 bytes per record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChestDesc {
    /// Internal name (e.g. "chest1"). Null-terminated, 32 bytes. Offset 0x00.
    pub name: String,
    /// Grid width in inventory slots (columns). Offset 0x20.
    pub width: u8,
    /// Grid height in inventory slots (rows). Offset 0x21.
    pub height: u8,
    /// Index into the chest graphic/bitmap table. Offset 0x22.
    pub image_index: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChestList {
    pub chests: Vec<ChestDesc>,
}

impl ChestList {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/dchest.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut chests = Vec::with_capacity(count);

        for _ in 0..count {
            let mut name_buf = [0u8; 32];
            cursor.read_exact(&mut name_buf)?;
            let name_end = name_buf.iter().position(|&b| b == 0).unwrap_or(32);
            let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();
            let width = cursor.read_u8()?;
            let height = cursor.read_u8()?;
            let image_index = cursor.read_i16::<LittleEndian>()?;

            chests.push(ChestDesc {
                name,
                width,
                height,
                image_index,
            });
        }

        Ok(ChestList { chests })
    }
}

impl TryFrom<&[u8]> for ChestList {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for ChestList {
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        use std::io::Write;
        let mut buf: Vec<u8> = Vec::with_capacity(4 + self.chests.len() * 36);
        buf.write_u32::<LittleEndian>(self.chests.len() as u32).unwrap();
        for c in &self.chests {
            let mut name_buf = [0u8; 32];
            let src = c.name.as_bytes();
            let n = src.len().min(31);
            name_buf[..n].copy_from_slice(&src[..n]);
            buf.write_all(&name_buf).unwrap();
            buf.write_u8(c.width).unwrap();
            buf.write_u8(c.height).unwrap();
            buf.write_i16::<LittleEndian>(c.image_index).unwrap();
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::ChestList;
    use crate::assets::test_lod;

    #[test]
    fn parse_dchest() {
        let Some(assets) = test_lod() else {
            return;
        };
        let chestlist = ChestList::load(&assets).unwrap();
        assert!(!chestlist.chests.is_empty(), "should have chests");
        println!("dchest: {} entries", chestlist.chests.len());
        for chest in &chestlist.chests {
            println!(
                "  {} {}x{} img={}",
                chest.name, chest.width, chest.height, chest.image_index
            );
        }
    }
}
