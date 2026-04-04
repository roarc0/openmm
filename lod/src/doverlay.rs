//! Parser for doverlay.bin — spell/buff overlay descriptors.
//! 8 bytes per record.

use std::error::Error;
use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{LodManager, lod_data::LodData};

/// A spell/buff overlay visual descriptor from doverlay.bin. 8 bytes per record.
///
/// Layout: 0x00: id(i16), 0x02: overlay_type(i16), 0x04: sft_index(i16), 0x06: pad(i16)
#[derive(Debug)]
pub struct OverlayDesc {
    /// Overlay ID used in EVT scripts and spell effects. Offset 0x00.
    pub id: i16,
    /// Overlay category: 0=on character, 1=on map object. Offset 0x02.
    pub overlay_type: i16,
    /// DSFT sprite frame table index for the overlay animation. Offset 0x04.
    pub sft_index: i16,
    /// Padding byte. Offset 0x06.
    pub _pad: i16,
}

pub struct OverlayList {
    pub overlays: Vec<OverlayDesc>,
}

impl OverlayList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/doverlay.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut overlays = Vec::with_capacity(count);

        for _ in 0..count {
            let id = cursor.read_i16::<LittleEndian>()?;
            let overlay_type = cursor.read_i16::<LittleEndian>()?;
            let sft_index = cursor.read_i16::<LittleEndian>()?;
            let _pad = cursor.read_i16::<LittleEndian>()?;
            overlays.push(OverlayDesc {
                id,
                overlay_type,
                sft_index,
                _pad,
            });
        }

        Ok(OverlayList { overlays })
    }
}

#[cfg(test)]
mod tests {
    use super::OverlayList;
    use crate::test_lod;

    #[test]
    fn parse_doverlay() {
        let Some(lod_manager) = test_lod() else {
            return;
        };
        let overlaylist = OverlayList::new(&lod_manager).unwrap();
        assert!(!overlaylist.overlays.is_empty(), "should have overlays");
        println!("doverlay: {} entries", overlaylist.overlays.len());
        for o in overlaylist.overlays.iter().take(5) {
            println!("  id={} type={} sft_index={}", o.id, o.overlay_type, o.sft_index);
        }
    }
}
