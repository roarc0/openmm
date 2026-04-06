use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{Cursor, Read};

use crate::LodSerialise;
use crate::{Assets, assets::enums::DecorationDescFlags, assets::lod_data::LodData, utils::try_read_name};

#[derive(Debug, Serialize, Deserialize)]
pub struct DDecList {
    pub items: Vec<DDecListItem>,
}

/// A decoration descriptor from ddeclist.bin. 80 bytes per record.
///
/// Layout:
///   0x00: name[32], 0x20: display_name[32],
///   0x40: dec_type(u16), 0x42: height(u16), 0x44: radius(u16),
///   0x46: light_radius(u16), 0x48: sft(i16), 0x4A: attributes(u16),
///   0x4C: sound_id(u16), 0x4E: skip(u16)
#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DDecListItem {
    /// Internal/file name (e.g. "fount1"). Null-terminated, 32 bytes. Offset 0x00.
    name: [u8; 32],
    /// Display/game name (e.g. "fountain"). Null-terminated, 32 bytes. Offset 0x20.
    display_name: [u8; 32],
    /// Decoration category type. Offset 0x40.
    pub dec_type: u16,
    /// Sprite height in MM6 units. Offset 0x42.
    pub height: u16,
    /// Collision radius in MM6 units. Offset 0x44.
    pub radius: u16,
    /// Emitted point-light radius (0 = no light). Offset 0x46.
    pub light_radius: u16,
    /// DSFT sprite frame table index. Negative = no sprite. Offset 0x48.
    pub sft: SFTType,
    /// Decoration attribute flags (DecorationDescFlags). Offset 0x4A.
    pub attributes: u16,
    /// Ambient sound ID played near this decoration (0 = none). Offset 0x4C.
    pub sound_id: u16,
    /// Padding/unused. Offset 0x4E.
    skip: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct SFTType {
    pub index: i16,
}

// impl Default removed, replaced by #[derive(Default)]

impl DDecListItem {
    pub fn is_no_block_movement(&self) -> bool {
        (self.attributes & 0x0001) != 0
    }

    pub fn is_no_draw(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }

    pub fn is_flicker_slow(&self) -> bool {
        (self.attributes & 0x0004) != 0
    }

    pub fn is_flicker_medium(&self) -> bool {
        (self.attributes & 0x0008) != 0
    }

    pub fn is_flicker_fast(&self) -> bool {
        (self.attributes & 0x0010) != 0
    }

    pub fn is_marker(&self) -> bool {
        (self.attributes & 0x0020) != 0
    }

    pub fn is_slow_loop(&self) -> bool {
        (self.attributes & 0x0040) != 0
    }

    pub fn is_emit_fire(&self) -> bool {
        (self.attributes & 0x0080) != 0
    }

    pub fn is_sound_on_dawn(&self) -> bool {
        (self.attributes & 0x0100) != 0
    }

    pub fn is_sound_on_dusk(&self) -> bool {
        (self.attributes & 0x0200) != 0
    }

    pub fn is_emit_smoke(&self) -> bool {
        (self.attributes & 0x0400) != 0
    }

    /// Get typed decoration description flags.
    pub fn desc_flags(&self) -> DecorationDescFlags {
        DecorationDescFlags::from_bits_truncate(self.attributes)
    }
}

// SFTType union removed, replaced by struct

// SFTType clone removed, replaced by #[derive(Clone)]

impl DDecListItem {
    pub fn name(&self) -> Option<String> {
        try_read_name(&self.name[..])
    }

    pub fn display_name(&self) -> Option<String> {
        try_read_name(&self.display_name)
    }

    pub fn sft_index(&self) -> i16 {
        self.sft.index
    }
}

impl DDecList {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/ddeclist.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let mut items: Vec<DDecListItem> = Vec::new();
        let item_count = cursor.read_u32::<LittleEndian>()?;
        let item_size = std::mem::size_of::<DDecListItem>();

        for _i in 0..item_count {
            let mut item = DDecListItem::default();
            cursor.read_exact(unsafe { std::slice::from_raw_parts_mut(&mut item as *mut _ as *mut u8, item_size) })?;
            items.push(item);
        }

        Ok(Self { items })
    }
}

impl TryFrom<&[u8]> for DDecList {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for DDecList {
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(self.items.len() as u32).unwrap();
        for item in &self.items {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    item as *const DDecListItem as *const u8,
                    std::mem::size_of::<DDecListItem>(),
                )
            };
            buf.extend_from_slice(bytes);
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use crate::assets::test_lod;

    use super::DDecList;

    #[test]
    fn read_declist_data_works() {
        let Some(assets) = test_lod() else {
            return;
        };
        let d_declist = DDecList::load(&assets).unwrap();
        assert_eq!(d_declist.items.len(), 230);
        assert_eq!(d_declist.items[6].name(), Some("fount1".to_string()));
        assert_eq!(d_declist.items[6].display_name(), Some("fountain".to_string()));
    }
}
