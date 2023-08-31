use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{
    lod_data::LodData,
    utils::{read_string, read_string_block},
    LodManager,
};

#[derive(Debug)]
pub struct DDecList {
    pub entries: Vec<DDecListEntry>,
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct DDecListEntry {
    name: [u8; 32],
    game_name: [u8; 32],
    dec_type: u16,
    height: u16,
    radius: u16,
    light_radius: u16,
    sft: u16,  // union [u8; 2] | i16
    bits: u16, //  bool noBlockMovement,noDraw,flickerSlow,flickerMedium,flickerFast,marker,slowLoop,emitFire,soundOnDawn,soundOnDusk,emitSmoke
    sound_id: u16,
    skip: u16,
}

// #[repr(C)]
// union SFTData {
//     sft_group: [u8; 2],
//     sft_index: i16,
// }

impl DDecListEntry {
    pub fn name(&self) -> Option<String> {
        let mut cursor = Cursor::new(self.name.as_slice());
        read_string(&mut cursor).ok()
    }
}

impl DDecList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/ddeclist.bin")?)?;
        let data = data.data.as_slice();

        let mut entries = Vec::new();
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()?;
        for _i in 0..count {
            let mut entry = DDecListEntry::default();
            cursor.read_exact(unsafe {
                std::slice::from_raw_parts_mut(
                    &mut entry as *mut _ as *mut u8,
                    std::mem::size_of::<DDecListEntry>(),
                )
            })?;
            entries.push(entry);
        }

        Ok(Self { entries })
    }
}

#[cfg(test)]
mod tests {
    use crate::{get_lod_path, lod_data::LodData, LodManager};

    use super::DDecList;

    #[test]
    fn read_declist_data_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let ddeclist = DDecList::new(&lod_manager).unwrap();
        assert_eq!(ddeclist.entries.len(), 230);
    }
}
