use std::error::Error;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Serialize, Deserialize};

use crate::LodSerialise;
use crate::{LodManager, raw::enums::SpriteFrameFlags, raw::lod_data::LodData, utils::try_read_name};

/// Sprite Frame Table: maps animation group names to sprites and frame timing.
#[derive(Debug, Serialize, Deserialize)]
pub struct DSFT {
    pub frames: Vec<DSFTFrame>,
    /// Group start indices into `frames`.
    pub groups: Vec<u16>,
}

/// One entry in the Sprite Frame Table (dsft.bin / SFT archive). 56 bytes per record.
///
/// Verified against MMExtension `SFTItem` struct (MM6: Bits=u16, no extra MM7 fields).
/// Layout:
///   0x00: group_name[12], 0x0C: sprite_name[12],
///   0x18: sprite_index[8](i16), 0x28: scale(i32),
///   0x2C: attributes(u16), 0x2E: light_radius(i16),
///   0x30: palette_id(i16), 0x32: palette_index(i16),
///   0x34: time(i16), 0x36: time_total(i16)
#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DSFTFrame {
    /// Animation group name, null-terminated, 12 bytes (e.g. "gob1"). Offset 0x00.
    group_name: [u8; 12],
    /// Sprite file name, null-terminated, 12 bytes (e.g. "gob1a0"). Offset 0x0C.
    sprite_name: [u8; 12],
    /// Sprite indices for each of 8 viewing angles. Populated at runtime by `LoadFrames()`.
    /// Zero in the raw file. Offset 0x18.
    pub sprite_index: [i16; 8],
    /// Sprite scale factor (fixed-point). Controls billboard display size. Offset 0x28.
    pub scale: i32,
    /// Frame attribute flags (SpriteFrameFlags). In MM6 this field is 2 bytes (u16).
    /// Bit meanings: 0=NotGroupEnd, 1=Luminous, 2=GroupStart, 4=Image1(single angle),
    /// 5=Center, 6=Fidget, 7=Loaded, 8-15=Mirror[0-7]. Offset 0x2C.
    pub attributes: u16,
    /// Point-light radius emitted by this frame (0 = no light). Offset 0x2E.
    pub light_radius: i16,
    /// Palette ID for color variant lookup. Offset 0x30.
    pub palette_id: i16,
    /// Palette index (0 if not yet loaded at runtime). Offset 0x32.
    pub palette_index: i16,
    /// Duration of this frame in 1/32 second units. Offset 0x34.
    pub time: i16,
    /// Total duration of the animation group in 1/32 second units. Offset 0x36.
    pub time_total: i16,
}

impl DSFTFrame {
    pub fn is_not_group_end(&self) -> bool {
        (self.attributes & 0x0001) != 0
    }

    pub fn is_luminous(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }

    pub fn is_group_start(&self) -> bool {
        (self.attributes & 0x0004) != 0
    }

    pub fn is_image1(&self) -> bool {
        (self.attributes & 0x0010) != 0
    }

    pub fn is_center(&self) -> bool {
        (self.attributes & 0x0020) != 0
    }

    pub fn is_fidget(&self) -> bool {
        (self.attributes & 0x0040) != 0
    }

    pub fn is_loaded(&self) -> bool {
        (self.attributes & 0x0080) != 0
    }

    pub fn is_mirror0(&self) -> bool {
        (self.attributes & 0x0100) != 0
    }

    pub fn is_mirror1(&self) -> bool {
        (self.attributes & 0x0200) != 0
    }

    pub fn is_mirror2(&self) -> bool {
        (self.attributes & 0x0400) != 0
    }

    pub fn is_mirror3(&self) -> bool {
        (self.attributes & 0x0800) != 0
    }

    pub fn is_mirror4(&self) -> bool {
        (self.attributes & 0x1000) != 0
    }

    pub fn is_mirror5(&self) -> bool {
        (self.attributes & 0x2000) != 0
    }

    pub fn is_mirror7(&self) -> bool {
        (self.attributes & 0x4000) != 0
    }

    pub fn is_mirror8(&self) -> bool {
        (self.attributes & 0x8000) != 0
    }

    /// Get typed sprite frame flags.
    pub fn flags(&self) -> SpriteFrameFlags {
        SpriteFrameFlags::from_bits_truncate(self.attributes)
    }

    pub fn group_name(&self) -> Option<String> {
        try_read_name(&self.group_name)
    }

    pub fn sprite_name(&self) -> Option<String> {
        try_read_name(&self.sprite_name)
    }
}

impl DSFT {
    /// Look up the scale factor for a sprite group name (case-insensitive).
    /// Returns the fixed-point 16.16 scale as f32, or 1.0 if not found or zero.
    pub fn scale_for_group(&self, group: &str) -> f32 {
        for frame in &self.frames {
            if let Some(name) = frame.group_name()
                && name.eq_ignore_ascii_case(group)
            {
                if frame.scale > 0 {
                    return frame.scale as f32 / 65536.0;
                }
                return 1.0;
            }
        }
        1.0
    }

    pub fn load(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dsft.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);

        let mut frames = Vec::new();
        let frame_count = cursor.read_u32::<LittleEndian>()?;
        let group_count = cursor.read_u32::<LittleEndian>()?;
        let frame_size = std::mem::size_of::<DSFTFrame>();

        for _ in 0..frame_count {
            let mut frame = DSFTFrame::default();
            cursor
                .read_exact(unsafe { std::slice::from_raw_parts_mut(&mut frame as *mut _ as *mut u8, frame_size) })?;
            frames.push(frame);
        }

        let mut groups = Vec::new();
        for _ in 0..group_count {
            let g = cursor.read_u16::<LittleEndian>()?;
            groups.push(g)
        }

        Ok(Self { frames, groups })
    }
}

impl TryFrom<&[u8]> for DSFT {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for DSFT {
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(self.frames.len() as u32).unwrap();
        buf.write_u32::<LittleEndian>(self.groups.len() as u32).unwrap();
        for f in &self.frames {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    f as *const DSFTFrame as *const u8,
                    std::mem::size_of::<DSFTFrame>(),
                )
            };
            buf.extend_from_slice(bytes);
        }
        for &g in &self.groups {
            buf.write_u16::<LittleEndian>(g).unwrap();
        }
        buf
    }
}

#[cfg(test)]
#[path = "dsft_tests.rs"]
mod tests;
