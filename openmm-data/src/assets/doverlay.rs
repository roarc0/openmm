use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::Cursor;

use crate::LodSerialise;
use crate::{Assets, assets::lod_data::LodData};

/// A spell/buff overlay visual descriptor from doverlay.bin. 8 bytes per record.
#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct OverlayList {
    pub overlays: Vec<OverlayDesc>,
}

impl OverlayList {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/doverlay.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
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

impl TryFrom<&[u8]> for OverlayList {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for OverlayList {
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(self.overlays.len() as u32).unwrap();
        for o in &self.overlays {
            buf.write_i16::<LittleEndian>(o.id).unwrap();
            buf.write_i16::<LittleEndian>(o.overlay_type).unwrap();
            buf.write_i16::<LittleEndian>(o.sft_index).unwrap();
            buf.write_i16::<LittleEndian>(o._pad).unwrap();
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::OverlayList;
    use crate::assets::test_lod;

    #[test]
    fn parse_doverlay() {
        let Some(assets) = test_lod() else {
            return;
        };
        let overlaylist = OverlayList::load(&assets).unwrap();
        assert!(!overlaylist.overlays.is_empty(), "should have overlays");
        println!("doverlay: {} entries", overlaylist.overlays.len());
        for o in overlaylist.overlays.iter().take(5) {
            println!("  id={} type={} sft_index={}", o.id, o.overlay_type, o.sft_index);
        }
    }
}
