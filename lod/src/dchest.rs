//! Parser for dchest.bin — chest visual descriptors.
//! 36 bytes per record.

use std::error::Error;
use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{LodManager, lod_data::LodData};

/// A chest visual descriptor from dchest.bin. 36 bytes per record.
///
/// Layout: 0x00: name[32], 0x20: width(u8), 0x21: height(u8), 0x22: image_index(i16)
#[derive(Debug)]
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

pub struct ChestList {
    pub chests: Vec<ChestDesc>,
}

impl ChestList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dchest.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
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

#[cfg(test)]
mod tests {
    use super::ChestList;
    use crate::test_lod;

    #[test]
    fn parse_dchest() {
        let Some(lod_manager) = test_lod() else {
            return;
        };
        let chestlist = ChestList::new(&lod_manager).unwrap();
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
