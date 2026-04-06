use bitflags::bitflags;
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{Cursor, Read};

use crate::LodSerialise;
use crate::{Assets, assets::lod_data::LodData};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectList {
    pub objects: Vec<ObjectDesc>,
}

impl ObjectList {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/dobjlist.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
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

impl TryFrom<&[u8]> for ObjectList {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for ObjectList {
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        use std::io::Write;
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(self.objects.len() as u32).unwrap();
        for o in &self.objects {
            let mut name_buf = [0u8; 32];
            let src = o.name.as_bytes();
            let n = src.len().min(31);
            name_buf[..n].copy_from_slice(&src[..n]);
            buf.write_all(&name_buf).unwrap();
            buf.write_i16::<LittleEndian>(o.id).unwrap();
            buf.write_i16::<LittleEndian>(o.radius).unwrap();
            buf.write_i16::<LittleEndian>(o.height).unwrap();
            buf.write_u16::<LittleEndian>(o.flags.bits()).unwrap();
            buf.write_i16::<LittleEndian>(o.sft_index).unwrap();
            buf.write_i16::<LittleEndian>(o.lifetime).unwrap();
            buf.write_u16::<LittleEndian>(o.particles_color).unwrap();
            buf.write_u16::<LittleEndian>(o.speed).unwrap();
            buf.write_u8(o.particle_r).unwrap();
            buf.write_u8(o.particle_g).unwrap();
            buf.write_u8(o.particle_b).unwrap();
            buf.write_u8(o._pad).unwrap();
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectList;
    use crate::assets::test_lod;

    #[test]
    fn parse_dobjlist() {
        let Some(assets) = test_lod() else {
            return;
        };
        let objlist = ObjectList::load(&assets).unwrap();
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
