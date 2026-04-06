use std::{
    error::Error,
    io::{Cursor, Read},
};

use serde::{Deserialize, Serialize};

use crate::utils::try_read_string_block;

// BillboardSprite is a compositor — it combines ddeclist + dsft + GameLod.
pub use crate::assets::provider::lod_decoder::BillboardSprite;

#[repr(C)]
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct BillboardData {
    pub declist_id: u16,
    pub attributes: u16,
    pub position: [i32; 3],
    pub direction: i32,
    pub event_variable: i16,
    pub event: i16,
    pub trigger_radius: i16,
    pub direction_degrees: i16,
}

impl BillboardData {
    pub fn is_triggered_by_touch(&self) -> bool {
        (self.attributes & 0x0001) != 0
    }

    pub fn is_triggered_by_monster(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }

    pub fn is_triggered_by_object(&self) -> bool {
        (self.attributes & 0x0004) != 0
    }

    pub fn is_visible_on_map(&self) -> bool {
        (self.attributes & 0x0008) != 0
    }

    pub fn is_chest(&self) -> bool {
        (self.attributes & 0x0010) != 0
    }

    pub fn is_original_invisible(&self) -> bool {
        (self.attributes & 0x0020) != 0
    }

    pub fn is_obelisk_chest(&self) -> bool {
        (self.attributes & 0x0040) != 0
    }

    pub fn to_bytes(&self) -> [u8; 24] {
        let mut out = [0u8; 24];
        let mut cursor = std::io::Cursor::new(&mut out[..]);
        use byteorder::{LittleEndian, WriteBytesExt};
        cursor.write_u16::<LittleEndian>(self.declist_id).unwrap();
        cursor.write_u16::<LittleEndian>(self.attributes).unwrap();
        cursor.write_i32::<LittleEndian>(self.position[0]).unwrap();
        cursor.write_i32::<LittleEndian>(self.position[1]).unwrap();
        cursor.write_i32::<LittleEndian>(self.position[2]).unwrap();
        cursor.write_i32::<LittleEndian>(self.direction).unwrap();
        cursor.write_i16::<LittleEndian>(self.event_variable).unwrap();
        cursor.write_i16::<LittleEndian>(self.event).unwrap();
        cursor.write_i16::<LittleEndian>(self.trigger_radius).unwrap();
        cursor.write_i16::<LittleEndian>(self.direction_degrees).unwrap();
        out
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Billboard {
    pub declist_name: String,
    pub data: BillboardData,
}

impl Billboard {
    pub fn to_bytes_header(&self) -> [u8; 24] {
        self.data.to_bytes()
    }

    pub fn to_bytes_name(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        let bytes = self.declist_name.as_bytes();
        let len = bytes.len().min(31);
        out[..len].copy_from_slice(&bytes[..len]);
        out
    }
}

pub(super) fn read_billboards(cursor: &mut Cursor<&[u8]>, count: usize) -> Result<Vec<Billboard>, Box<dyn Error>> {
    let mut billboards_data = Vec::new();

    for _i in 0..count {
        let size = std::mem::size_of::<BillboardData>();
        let mut entity_data = BillboardData::default();
        cursor.read_exact(unsafe { std::slice::from_raw_parts_mut(&mut entity_data as *mut _ as *mut u8, size) })?;
        billboards_data.push(entity_data);
    }

    let mut billboard_names = Vec::new();
    for _i in 0..count {
        let name = try_read_string_block(cursor, 32);
        billboard_names.push(name?.to_lowercase());
    }

    let billboards = billboards_data
        .into_iter()
        .zip(billboard_names)
        .map(|(data, name)| Billboard {
            declist_name: name,
            data,
        })
        .collect();

    Ok(billboards)
}
