use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};

use serde::{Deserialize, Serialize};

use crate::{
    Assets, LodSerialise,
    assets::enums::{SoundAttributes, SoundType},
    assets::lod_data::LodData,
    utils::try_read_name,
};

fn default_dsounds_runtime() -> [u8; 68] {
    [0; 68]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DSounds {
    pub items: Vec<DSoundInfo>,
}

/// A sound descriptor from dsounds.bin. 112 bytes per record.
///
/// Layout:
///   0x00: name[32], 0x20: sound_id(u32), 0x24: sound_type(u32),
///   0x28: attributes(u32), 0x2C: _runtime[68]
#[allow(dead_code)]
#[repr(C)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DSoundInfo {
    /// WAV filename (without extension), null-terminated, 32 bytes. Offset 0x00.
    name_bytes: [u8; 32],
    /// Unique sound ID referenced by dmonlist.bin, EVT scripts, and sound events. Offset 0x20.
    pub sound_id: u32,
    /// Sound category: determines playback behaviour (SoundType enum). Offset 0x24.
    pub sound_type: u32,
    /// Sound attribute flags (SoundAttributes): bit 1 = 3D spatial audio. Offset 0x28.
    pub attributes: u32,
    /// Runtime-only fields (handles, buffers). Always zero in the file. Offset 0x2C.
    #[serde(skip, default = "default_dsounds_runtime")]
    _runtime: [u8; 68],
}

impl Default for DSoundInfo {
    fn default() -> Self {
        Self {
            name_bytes: [0u8; 32],
            sound_id: 0,
            sound_type: 0,
            attributes: 0,
            _runtime: [0u8; 68],
        }
    }
}

impl DSoundInfo {
    pub fn name(&self) -> Option<String> {
        try_read_name(self.name_bytes.as_slice())
    }

    pub fn is_3d(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }

    /// Get typed sound type enum.
    pub fn sound_type_enum(&self) -> Option<SoundType> {
        SoundType::from_u32(self.sound_type)
    }

    /// Get typed sound attribute flags.
    pub fn sound_attributes(&self) -> SoundAttributes {
        SoundAttributes::from_bits_truncate(self.attributes)
    }
}

impl DSounds {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/dsounds.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let item_count = cursor.read_u32::<LittleEndian>()?;
        let item_size = std::mem::size_of::<DSoundInfo>();
        let mut items = Vec::with_capacity(item_count as usize);

        for _ in 0..item_count {
            let mut item = DSoundInfo::default();
            cursor.read_exact(unsafe { std::slice::from_raw_parts_mut(&mut item as *mut _ as *mut u8, item_size) })?;
            items.push(item);
        }

        Ok(Self { items })
    }

    pub fn get_by_id(&self, id: u32) -> Option<&DSoundInfo> {
        self.items.iter().find(|s| s.sound_id == id)
    }

    /// Look up a sound by name (case-insensitive).
    pub fn get_by_name(&self, name: &str) -> Option<&DSoundInfo> {
        let lower = name.to_lowercase();
        self.items
            .iter()
            .find(|s| s.name().map(|n| n.to_lowercase() == lower).unwrap_or(false))
    }
}

impl TryFrom<&[u8]> for DSounds {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for DSounds {
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(self.items.len() as u32).unwrap();
        for item in &self.items {
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    item as *const DSoundInfo as *const u8,
                    std::mem::size_of::<DSoundInfo>(),
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

    use super::DSounds;

    #[test]
    fn read_dsounds_data_works() {
        let Some(assets) = test_lod() else {
            return;
        };
        let dsounds = DSounds::load(&assets).unwrap();
        assert_eq!(dsounds.items.len(), 1355);
        // Record [1] should be "campfire" with id=4
        let campfire = &dsounds.items[1];
        assert_eq!(campfire.name(), Some("campfire".to_string()));
        assert_eq!(campfire.sound_id, 4);
    }

    #[test]
    fn get_by_id_works() {
        let Some(assets) = test_lod() else {
            return;
        };
        let dsounds = DSounds::load(&assets).unwrap();
        let campfire = dsounds.get_by_id(4).expect("sound id 4 should exist");
        assert_eq!(campfire.name(), Some("campfire".to_string()));
        assert!(dsounds.get_by_id(9999).is_none());
    }
}
