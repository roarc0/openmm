//! Parser for dift.bin — Interface Frame Table (IFT) from the icons LOD.
//!
//! Controls animations for UI icons (spells, effects shown in the interface).
//! 4-byte count header, then 32 bytes per entry:
//!   group_name[12], icon_name[12], icon_index i16, time i16, total_time i16, bits u16
//!
//! A group starts when group_name is non-empty.
//! A group ends on the last entry where total_time becomes zero or the next
//! group starts.

use std::error::Error;
use std::io::{Cursor, Read};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{LodManager, lod_data::LodData, utils::try_read_name};

/// One frame in an IFT animation group.
#[derive(Debug, Clone)]
pub struct IftFrame {
    /// Group name — non-empty only on the first frame of each group.
    pub group_name: String,
    /// Icon name (e.g. "glow01a").
    pub icon_name: String,
    /// Index into the icon sprite sheet / icon LOD entry.
    pub icon_index: i16,
    /// Frame duration in 1/32 s increments.
    pub time: i16,
    /// Total cycle time of the owning group.
    pub total_time: i16,
    /// Raw attribute bits (reserved / unused in MM6).
    pub bits: u16,
}

impl IftFrame {
    /// Returns `true` if this frame begins a new animation group.
    pub fn is_group_start(&self) -> bool {
        !self.group_name.is_empty()
    }
}

/// Interface Frame Table loaded from `dift.bin`.
pub struct IFT {
    pub frames: Vec<IftFrame>,
}

impl IFT {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/dift.bin")?)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut frames = Vec::with_capacity(count);

        for _ in 0..count {
            let mut group_buf = [0u8; 12];
            cursor.read_exact(&mut group_buf)?;
            let mut icon_buf = [0u8; 12];
            cursor.read_exact(&mut icon_buf)?;
            let icon_index = cursor.read_i16::<LittleEndian>()?;
            let time = cursor.read_i16::<LittleEndian>()?;
            let total_time = cursor.read_i16::<LittleEndian>()?;
            let bits = cursor.read_u16::<LittleEndian>()?;

            frames.push(IftFrame {
                group_name: try_read_name(&group_buf).unwrap_or_default(),
                icon_name: try_read_name(&icon_buf).unwrap_or_default(),
                icon_index,
                time,
                total_time,
                bits,
            });
        }

        Ok(IFT { frames })
    }

    /// Return the slice of frames belonging to the named animation group
    /// (case-insensitive match on `group_name`).
    pub fn find_group(&self, name: &str) -> Option<&[IftFrame]> {
        let start = self
            .frames
            .iter()
            .position(|f| f.is_group_start() && f.group_name.eq_ignore_ascii_case(name))?;
        // Group ends when a new group starts or we reach end of table
        let len = self.frames[start + 1..]
            .iter()
            .position(|f| f.is_group_start())
            .map(|p| p + 1)
            .unwrap_or(self.frames.len() - start);
        Some(&self.frames[start..start + len])
    }
}
