use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{LodManager, enums::SpriteFrameFlags, lod_data::LodData, utils::try_read_name};

pub struct DSFT {
    pub frames: Vec<DSFTFrame>,
    pub groups: Vec<u16>,
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Default, Clone)]
pub struct DSFTFrame {
    group_name: [u8; 12],
    sprite_name: [u8; 12],
    pub sprite_index: [i16; 8],
    pub scale: i32,
    pub attributes: u16,
    pub light_radius: i16,
    pub palette_id: i16,
    pub palette_index: i16,
    pub time: i16,
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

    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/dsft.bin")?)?;
        let data = data.data.as_slice();

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

#[cfg(test)]
#[path = "dsft_tests.rs"]
mod tests;
