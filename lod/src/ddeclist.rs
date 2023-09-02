use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{lod_data::LodData, utils::try_read_name, LodManager};

pub struct DDecList {
    pub items: Vec<DDecListItem>,
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Clone)]
pub struct DDecListItem {
    name: [u8; 32],
    game_name: [u8; 32],
    pub dec_type: u16,
    pub height: u16,
    pub radius: u16,
    pub light_radius: u16,
    pub sft: SFTType,
    pub bits: u16,
    pub sound_id: u16,
    skip: u16,
}

impl Default for DDecListItem {
    fn default() -> Self {
        DDecListItem {
            name: [0; 32],
            game_name: [0; 32],
            dec_type: 0,
            height: 0,
            radius: 0,
            light_radius: 0,
            sft: SFTType { index: 0 },
            bits: 0,
            sound_id: 0,
            skip: 0,
        }
    }
}

impl DDecListItem {
    pub fn is_no_block_movement(&self) -> bool {
        (self.bits & 0x0001) != 0
    }

    pub fn is_no_draw(&self) -> bool {
        (self.bits & 0x0002) != 0
    }

    pub fn is_flicker_slow(&self) -> bool {
        (self.bits & 0x0004) != 0
    }

    pub fn is_flicker_medium(&self) -> bool {
        (self.bits & 0x0008) != 0
    }

    pub fn is_flicker_fast(&self) -> bool {
        (self.bits & 0x0010) != 0
    }

    pub fn is_marker(&self) -> bool {
        (self.bits & 0x0020) != 0
    }

    pub fn is_slow_loop(&self) -> bool {
        (self.bits & 0x0040) != 0
    }

    pub fn is_emit_fire(&self) -> bool {
        (self.bits & 0x0080) != 0
    }

    pub fn is_sound_on_dawn(&self) -> bool {
        (self.bits & 0x0100) != 0
    }

    pub fn is_sound_on_dusk(&self) -> bool {
        (self.bits & 0x0200) != 0
    }

    pub fn is_emit_smoke(&self) -> bool {
        (self.bits & 0x0400) != 0
    }
}

#[repr(C)]
pub union SFTType {
    pub group: [u8; 2],
    pub index: i16,
}

impl Clone for SFTType {
    fn clone(&self) -> Self {
        Self {
            index: unsafe { self.index },
        }
    }
}

impl DDecListItem {
    pub fn name(&self) -> Option<String> {
        try_read_name(&self.game_name)
    }

    pub fn game_name(&self) -> Option<String> {
        try_read_name(&self.game_name)
    }

    pub fn sft_index(&self) -> i16 {
        unsafe { self.sft.index }
    }
}

impl DDecList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/ddeclist.bin")?)?;
        let data = data.data.as_slice();

        let mut cursor = Cursor::new(data);
        let mut items: Vec<DDecListItem> = Vec::new();
        let item_count = cursor.read_u32::<LittleEndian>()?;
        let item_size = std::mem::size_of::<DDecListItem>();

        for _i in 0..item_count {
            let mut item = DDecListItem::default();
            cursor.read_exact(unsafe {
                std::slice::from_raw_parts_mut(&mut item as *mut _ as *mut u8, item_size)
            })?;
            items.push(item);
        }

        Ok(Self { items })
    }
}

#[cfg(test)]
mod tests {
    use crate::{get_lod_path, LodManager};

    use super::DDecList;

    #[test]
    fn read_declist_data_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let d_declist = DDecList::new(&lod_manager).unwrap();
        assert_eq!(d_declist.items.len(), 230);
        assert_eq!(d_declist.items[6].name(), Some("fountain".to_string()));
        assert_eq!(d_declist.items[6].game_name(), Some("fountain".to_string()));
    }
}
