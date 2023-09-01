use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{
    lod_data::LodData,
    utils::{try_read_name, try_read_string},
    LodManager,
};

pub struct DSFT {
    pub frames: Vec<DSFTFrame>,
    pub groups: Vec<u16>,
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Default)]
pub struct DSFTFrame {
    group_name: [u8; 12],
    sprite_name: [u8; 12],
    pub sprite_index: [i16; 8],
    pub scale: i32,
    pub bits: u16,
    pub light_radius: i16,
    pub palette_id: i16,
    pub palette_index: i16,
    pub time: i16,
    pub time_total: i16,
}

impl DSFTFrame {
    pub fn is_not_group_end(&self) -> bool {
        (self.bits & 0x0001) != 0
    }

    pub fn is_luminous(&self) -> bool {
        (self.bits & 0x0002) != 0
    }

    pub fn is_group_start(&self) -> bool {
        (self.bits & 0x0004) != 0
    }

    pub fn is_image1(&self) -> bool {
        (self.bits & 0x0010) != 0
    }

    pub fn is_center(&self) -> bool {
        (self.bits & 0x0020) != 0
    }

    pub fn is_fidget(&self) -> bool {
        (self.bits & 0x0040) != 0
    }

    pub fn is_loaded(&self) -> bool {
        (self.bits & 0x0080) != 0
    }

    pub fn is_mirror0(&self) -> bool {
        (self.bits & 0x0100) != 0
    }

    pub fn is_mirror1(&self) -> bool {
        (self.bits & 0x0200) != 0
    }

    pub fn is_mirror2(&self) -> bool {
        (self.bits & 0x0400) != 0
    }

    pub fn is_mirror3(&self) -> bool {
        (self.bits & 0x0800) != 0
    }

    pub fn is_mirror4(&self) -> bool {
        (self.bits & 0x1000) != 0
    }

    pub fn is_mirror5(&self) -> bool {
        (self.bits & 0x2000) != 0
    }

    pub fn is_mirror7(&self) -> bool {
        (self.bits & 0x4000) != 0
    }

    pub fn is_mirror8(&self) -> bool {
        (self.bits & 0x8000) != 0
    }
}

impl DSFTFrame {
    pub fn group_name(&self) -> Option<String> {
        try_read_name(&self.group_name)
    }

    pub fn sprite_name(&self) -> Option<String> {
        try_read_name(&self.sprite_name)
    }
}

impl DSFT {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/dsft.bin")?)?;
        let data = data.data.as_slice();

        let mut cursor = Cursor::new(data);
        let mut frames = Vec::new();
        let frame_count = cursor.read_u32::<LittleEndian>()?;
        let group_count = cursor.read_u32::<LittleEndian>()?;
        let size = std::mem::size_of::<DSFTFrame>();
        for _ in 0..frame_count {
            let mut frame = DSFTFrame::default();
            cursor.read_exact(unsafe {
                std::slice::from_raw_parts_mut(&mut frame as *mut _ as *mut u8, size)
            })?;
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
mod tests {
    use crate::{get_lod_path, LodManager};

    use super::DSFT;

    #[test]
    fn read_declist_data_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsft = DSFT::new(&lod_manager).unwrap();
        assert_eq!(dsft.frames.len(), 6455);
        assert_eq!(dsft.frames[0].group_name(), Some("null".to_string()));
        assert_eq!(dsft.frames[1].group_name(), Some("key".to_string()));
        assert_eq!(dsft.frames[1].sprite_name(), Some("3gem7".to_string()));
        assert_eq!(dsft.frames[1017].group_name(), Some("rok1".to_string()));
        assert_eq!(dsft.frames[1017].sprite_name(), Some("rok1".to_string()));
        assert_eq!(dsft.groups.len(), 1656);
    }
}
