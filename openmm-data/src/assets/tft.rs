use std::error::Error;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use bitflags::bitflags;
use serde::{Serialize, Deserialize};

use crate::LodSerialise;
use crate::{Assets, assets::lod_data::LodData};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TFTFlags: u16 {
        const NOT_GROUP_END = 0x0001;
        const GROUP_START   = 0x0002;
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TFTEntry {
    pub name: String,
    pub index: i16,
    pub time: i16,
    pub total_time: i16,
    pub flags: TFTFlags,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextureFrameTable {
    pub entries: Vec<TFTEntry>,
}

impl TextureFrameTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/dtft.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
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

impl TryFrom<&[u8]> for TextureFrameTable {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for TextureFrameTable {
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        use std::io::Write;
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(self.entries.len() as u32).unwrap();
        for e in &self.entries {
            let mut name_buf = [0u8; 12];
            let src = e.name.as_bytes();
            let n = src.len().min(11);
            name_buf[..n].copy_from_slice(&src[..n]);
            buf.write_all(&name_buf).unwrap();
            buf.write_i16::<LittleEndian>(e.index).unwrap();
            buf.write_i16::<LittleEndian>(e.time).unwrap();
            buf.write_i16::<LittleEndian>(e.total_time).unwrap();
            buf.write_u16::<LittleEndian>(e.flags.bits()).unwrap();
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::TextureFrameTable;
    use crate::assets::test_lod;

    #[test]
    fn parse_dtft() {
        let Some(assets) = test_lod() else {
            return;
        };
        let tft = TextureFrameTable::load(&assets).unwrap();
        assert!(!tft.entries.is_empty(), "should have TFT entries");
        println!("dtft: {} entries", tft.entries.len());
        for e in tft.entries.iter().take(10) {
            println!("  {} idx={} time={} flags={:?}", e.name, e.index, e.time, e.flags);
        }
    }
}
