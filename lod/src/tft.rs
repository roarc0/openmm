//! Parser for dtft.bin — texture frame table (animated textures).
//! 20 bytes per entry.

use std::error::Error;
use std::io::{Cursor, Read};

use bitflags::bitflags;
use byteorder::{LittleEndian, ReadBytesExt};

use crate::{LodManager, lod_data::LodData};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TFTFlags: u16 {
        const NOT_GROUP_END = 0x0001;
        const GROUP_START   = 0x0002;
    }
}

#[derive(Debug)]
pub struct TFTEntry {
    pub name: String,
    pub index: i16,
    pub time: i16,
    pub total_time: i16,
    pub flags: TFTFlags,
}

pub struct TextureFrameTable {
    pub entries: Vec<TFTEntry>,
}

impl TextureFrameTable {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dtft.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut entries = Vec::with_capacity(count);

        for _ in 0..count {
            let mut name_buf = [0u8; 12];
            cursor.read_exact(&mut name_buf)?;
            let name_end = name_buf.iter().position(|&b| b == 0).unwrap_or(12);
            let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();
            let index = cursor.read_i16::<LittleEndian>()?;
            let time = cursor.read_i16::<LittleEndian>()?;
            let total_time = cursor.read_i16::<LittleEndian>()?;
            let flags = TFTFlags::from_bits_truncate(cursor.read_u16::<LittleEndian>()?);

            entries.push(TFTEntry {
                name,
                index,
                time,
                total_time,
                flags,
            });
        }

        Ok(TextureFrameTable { entries })
    }

    /// Find the animation group for a texture name.
    pub fn find_group(&self, texture_name: &str) -> Option<&[TFTEntry]> {
        let start = self
            .entries
            .iter()
            .position(|e| e.name.eq_ignore_ascii_case(texture_name) && e.flags.contains(TFTFlags::GROUP_START))?;
        let end = self.entries[start..]
            .iter()
            .position(|e| !e.flags.contains(TFTFlags::NOT_GROUP_END))?
            + start
            + 1;
        Some(&self.entries[start..end])
    }
}

#[cfg(test)]
mod tests {
    use super::TextureFrameTable;
    use crate::{LodManager, get_lod_path};

    #[test]
    fn parse_dtft() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let tft = TextureFrameTable::new(&lod_manager).unwrap();
        assert!(!tft.entries.is_empty(), "should have TFT entries");
        println!("dtft: {} entries", tft.entries.len());
        for e in tft.entries.iter().take(10) {
            println!("  {} idx={} time={} flags={:?}", e.name, e.index, e.time, e.flags);
        }
    }
}
