//! Parser for dobjlist.bin — object/projectile visual descriptors.
//! MM6 record size: 52 bytes per entry.

use std::error::Error;
use std::io::{Cursor, Read};

use bitflags::bitflags;
use byteorder::{LittleEndian, ReadBytesExt};

use crate::{LodManager, lod_data::LodData};

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

/// A projectile/object visual descriptor from dobjlist.bin. 52 bytes per record.
///
/// Layout:
///   0x00: name[32], 0x20: id(i16), 0x22: radius(i16), 0x24: height(i16),
///   0x26: flags(u16), 0x28: sft_index(i16), 0x2A: lifetime(i16),
///   0x2C: particles_color(u16), 0x2E: speed(u16),
///   0x30: particle_r(u8), 0x31: particle_g(u8), 0x32: particle_b(u8), 0x33: pad(u8)
#[derive(Debug)]
pub struct ObjectDesc {
    /// Internal name (e.g. "arrow01"). Null-terminated, 32 bytes. Offset 0x00.
    pub name: String,
    /// Object type ID used in scripting. Offset 0x20.
    pub id: i16,
    /// Collision/hit radius in MM6 units. Offset 0x22.
    pub radius: i16,
    /// Collision height in MM6 units. Offset 0x24.
    pub height: i16,
    /// Behavior flags (ObjectDescFlags). Offset 0x26.
    pub flags: ObjectDescFlags,
    /// DSFT sprite frame table index. Offset 0x28.
    pub sft_index: i16,
    /// Lifetime in game ticks before auto-removal. Offset 0x2A.
    pub lifetime: i16,
    /// Particle trail color packed as RGB. Offset 0x2C.
    pub particles_color: u16,
    /// Initial projectile speed in MM6 units/tick. Offset 0x2E.
    pub speed: u16,
    /// Particle trail red component. Offset 0x30.
    pub particle_r: u8,
    /// Particle trail green component. Offset 0x31.
    pub particle_g: u8,
    /// Particle trail blue component. Offset 0x32.
    pub particle_b: u8,
    /// Padding byte. Offset 0x33.
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
    use crate::test_lod;

    #[test]
    fn parse_dobjlist() {
        let Some(lod_manager) = test_lod() else {
            return;
        };
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
