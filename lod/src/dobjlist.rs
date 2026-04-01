//! Parser for dobjlist.bin — object/projectile visual descriptors.
//! MM6 record size: 52 bytes per entry.

use std::error::Error;
use std::io::{Cursor, Read};

use bitflags::bitflags;
use byteorder::{LittleEndian, ReadBytesExt};

use crate::{lod_data::LodData, LodManager};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObjectDescFlags: u16 {
        const INVISIBLE        = 0x0001;
        const UNTOUCHABLE      = 0x0002;
        const TEMPORARY        = 0x0004;
        const LIFETIME_IN_SFT  = 0x0008;
        const NO_PICKUP        = 0x0010;
        const NO_GRAVITY       = 0x0020;
        const INTERCEPT_ACTION = 0x0040;
        const BOUNCE           = 0x0080;
        const TRAIL_PARTICLES  = 0x0100;
        const TRAIL_FIRE       = 0x0200;
        const TRAIL_LINE       = 0x0400;
    }
}

#[derive(Debug)]
pub struct ObjectDesc {
    pub name: String,
    pub id: i16,
    pub radius: i16,
    pub height: i16,
    pub flags: ObjectDescFlags,
    pub sft_index: i16,
    pub lifetime: i16,
    pub particles_color: u16,
    pub speed: u16,
    pub particle_r: u8,
    pub particle_g: u8,
    pub particle_b: u8,
    pub _pad: u8,
}

pub struct ObjectList {
    pub objects: Vec<ObjectDesc>,
}

impl ObjectList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dobjlist.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut objects = Vec::with_capacity(count);

        for _ in 0..count {
            let mut name_buf = [0u8; 32];
            cursor.read_exact(&mut name_buf)?;
            let name_end = name_buf.iter().position(|&b| b == 0).unwrap_or(32);
            let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();

            let id = cursor.read_i16::<LittleEndian>()?;
            let radius = cursor.read_i16::<LittleEndian>()?;
            let height = cursor.read_i16::<LittleEndian>()?;
            let flags = ObjectDescFlags::from_bits_truncate(cursor.read_u16::<LittleEndian>()?);
            let sft_index = cursor.read_i16::<LittleEndian>()?;
            let lifetime = cursor.read_i16::<LittleEndian>()?;
            let particles_color = cursor.read_u16::<LittleEndian>()?;
            let speed = cursor.read_u16::<LittleEndian>()?;
            let particle_r = cursor.read_u8()?;
            let particle_g = cursor.read_u8()?;
            let particle_b = cursor.read_u8()?;
            let _pad = cursor.read_u8()?;

            objects.push(ObjectDesc {
                name,
                id,
                radius,
                height,
                flags,
                sft_index,
                lifetime,
                particles_color,
                speed,
                particle_r,
                particle_g,
                particle_b,
                _pad,
            });
        }

        Ok(ObjectList { objects })
    }

    pub fn get_by_id(&self, id: i16) -> Option<&ObjectDesc> {
        self.objects.iter().find(|o| o.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectList;
    use crate::{get_lod_path, LodManager};

    #[test]
    fn parse_dobjlist() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let objlist = ObjectList::new(&lod_manager).unwrap();
        assert!(!objlist.objects.is_empty(), "should have objects");
        println!("dobjlist: {} entries", objlist.objects.len());
        for obj in objlist.objects.iter().take(5) {
            println!(
                "  {} id={} radius={} height={}",
                obj.name, obj.id, obj.radius, obj.height
            );
        }
    }
}
