//! Parser for dpft.bin — Particle Frame Table (PFT) from the icons LOD.
//!
//! Controls animations for spell effects / particles.
//! 4-byte count header, then 10 bytes per entry:
//!   group_id u16, frame_index u16, time i16, total_time i16, bits u16
//!
//! Unlike IFT/DSFT the PFT does not use string names — it indexes into
//! other tables by group_id and frame_index numbers.

use std::error::Error;
use std::io::Cursor;
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Serialize, Deserialize};

use crate::LodSerialise;
use crate::{Assets, assets::lod_data::LodData};

/// One frame in a PFT particle animation group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PftFrame {
    /// Animation group identifier (shared across frames in the same group).
    pub group_id: u16,
    /// Index into the particle / sprite sheet for this frame.
    pub frame_index: u16,
    /// Frame duration in 1/32 s increments.
    pub time: i16,
    /// Total cycle time for the animation group.
    pub total_time: i16,
    /// Attribute bits — bit 0 = NotGroupEnd, bit 1 = GroupStart.
    pub bits: u16,
}

impl PftFrame {
    /// Returns `true` if this frame is the last one in its group.
    pub fn is_group_end(&self) -> bool {
        (self.bits & 0x0001) == 0
    }

    /// Returns `true` if this frame begins a new animation group.
    pub fn is_group_start(&self) -> bool {
        (self.bits & 0x0002) != 0
    }
}

/// Particle Frame Table loaded from `dpft.bin`.
#[derive(Debug, Serialize, Deserialize)]
pub struct PFT {
    pub frames: Vec<PftFrame>,
}

impl PFT {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/dpft.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut frames = Vec::with_capacity(count);

        for _ in 0..count {
            let group_id = cursor.read_u16::<LittleEndian>()?;
            let frame_index = cursor.read_u16::<LittleEndian>()?;
            let time = cursor.read_i16::<LittleEndian>()?;
            let total_time = cursor.read_i16::<LittleEndian>()?;
            let bits = cursor.read_u16::<LittleEndian>()?;
            frames.push(PftFrame {
                group_id,
                frame_index,
                time,
                total_time,
                bits,
            });
        }

        Ok(PFT { frames })
    }

    /// Return all frames belonging to the given group_id.
    pub fn group(&self, group_id: u16) -> impl Iterator<Item = &PftFrame> {
        self.frames.iter().filter(move |f| f.group_id == group_id)
    }
}

impl TryFrom<&[u8]> for PFT {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for PFT {
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut buf: Vec<u8> = Vec::with_capacity(4 + self.frames.len() * 10);
        buf.write_u32::<LittleEndian>(self.frames.len() as u32).unwrap();
        for f in &self.frames {
            buf.write_u16::<LittleEndian>(f.group_id).unwrap();
            buf.write_u16::<LittleEndian>(f.frame_index).unwrap();
            buf.write_i16::<LittleEndian>(f.time).unwrap();
            buf.write_i16::<LittleEndian>(f.total_time).unwrap();
            buf.write_u16::<LittleEndian>(f.bits).unwrap();
        }
        buf
    }
}
