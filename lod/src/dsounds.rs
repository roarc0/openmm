use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{lod_data::LodData, utils::try_read_name, LodManager};

pub struct DSounds {
    pub items: Vec<DSoundInfo>,
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Clone)]
pub struct DSoundInfo {
    name_bytes: [u8; 32],
    pub sound_id: u32,
    pub sound_type: u32,
    pub attributes: u32,
    _runtime: [u8; 68],
}

impl Default for DSoundInfo {
    fn default() -> Self {
        Self {
            name_bytes: [0; 32],
            sound_id: 0,
            sound_type: 0,
            attributes: 0,
            _runtime: [0; 68],
        }
    }
}

impl DSoundInfo {
    pub fn name(&self) -> Option<String> {
        try_read_name(&self.name_bytes)
    }

    pub fn is_3d(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }
}

impl DSounds {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/dsounds.bin")?)?;
        let data = data.data.as_slice();

        let mut cursor = Cursor::new(data);
        let item_count = cursor.read_u32::<LittleEndian>()?;
        let item_size = std::mem::size_of::<DSoundInfo>();
        let mut items = Vec::with_capacity(item_count as usize);

        for _ in 0..item_count {
            let mut item = DSoundInfo::default();
            cursor.read_exact(unsafe {
                std::slice::from_raw_parts_mut(&mut item as *mut _ as *mut u8, item_size)
            })?;
            items.push(item);
        }

        Ok(Self { items })
    }

    pub fn get_by_id(&self, id: u32) -> Option<&DSoundInfo> {
        self.items.iter().find(|s| s.sound_id == id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{get_lod_path, LodManager};

    use super::DSounds;

    #[test]
    fn read_dsounds_data_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsounds = DSounds::new(&lod_manager).unwrap();
        assert_eq!(dsounds.items.len(), 1355);
        // Record [1] should be "campfire" with id=4
        let campfire = &dsounds.items[1];
        assert_eq!(campfire.name(), Some("campfire".to_string()));
        assert_eq!(campfire.sound_id, 4);
    }

    #[test]
    fn get_by_id_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsounds = DSounds::new(&lod_manager).unwrap();
        let campfire = dsounds.get_by_id(4).expect("sound id 4 should exist");
        assert_eq!(campfire.name(), Some("campfire".to_string()));
        assert!(dsounds.get_by_id(9999).is_none());
    }
}
