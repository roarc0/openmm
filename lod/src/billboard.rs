use std::{
    error::Error,
    io::{Cursor, Read},
};

use image::{DynamicImage, GenericImageView};

use crate::{
    LodManager,
    ddeclist::{DDecList, DDecListItem},
    dsft::{DSFT, DSFTFrame},
    utils::try_read_string_block,
};

#[repr(C)]
#[derive(Default, Debug)]
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

    pub fn is_invisible(&self) -> bool {
        (self.attributes & 0x0020) != 0
    }

    pub fn is_obelisk_chest(&self) -> bool {
        (self.attributes & 0x0040) != 0
    }
}

#[derive(Default, Debug)]
pub struct Billboard {
    pub declist_name: String,
    pub data: BillboardData,
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

pub struct BillboardManager {
    d_declist: DDecList,
    d_sft: DSFT,
}

pub struct BillboardSprite {
    pub image: DynamicImage,
    pub d_declist_item: DDecListItem,
    pub d_sft_frame: DSFTFrame,
}

impl BillboardSprite {
    pub fn dimensions(&self) -> (f32, f32) {
        let (px_w, px_h) = self.image.dimensions();
        let mut height = px_h as f32;
        let mut width = height * (px_w as f32 / px_h as f32);

        // Apply dsft scale (fixed-point 16.16: divide by 65536)
        if self.d_sft_frame.scale > 0 {
            let scale = self.d_sft_frame.scale as f32 / 65536.0;
            width *= scale;
            height *= scale;
        }

        (width, height)
    }
}

impl BillboardManager {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let d_declist = DDecList::new(lod_manager)?;
        let d_sft = DSFT::new(lod_manager)?;
        Ok(Self { d_declist, d_sft })
    }

    /// Get the decoration list item for a given declist_id.
    pub fn get_declist_item(&self, id: u16) -> Option<&DDecListItem> {
        self.d_declist.items.get(id as usize)
    }

    /// Get the DSFT scale factor for a decoration item (fixed-point 16.16 → f32).
    /// Returns None if scale is 0 or the SFT frame is not found.
    pub fn get_dsft_scale(&self, item: &DDecListItem) -> Option<f32> {
        let frame = self.d_sft.frames.get(item.sft_index() as usize)?;
        if frame.scale > 0 {
            Some(frame.scale as f32 / 65536.0)
        } else {
            None
        }
    }

    /// Get the DSFT scale factor for a sprite group name (e.g. monster/NPC sprite root).
    /// Returns the fixed-point 16.16 scale as f32, or 1.0 if not found.
    pub fn dsft_scale_for_group(&self, group: &str) -> f32 {
        self.d_sft.scale_for_group(group)
    }

    pub fn get(&self, lod_manager: &LodManager, name: &str, declist_id: u16) -> Option<BillboardSprite> {
        let declist_item = self.d_declist.items.get(declist_id as usize)?;
        let sft_frame = self.d_sft.frames.get(declist_item.sft_index() as usize)?;

        let dec_name = declist_item.name().unwrap_or_default();
        let sft_name = sft_frame.sprite_name().unwrap_or_default();
        let image = lod_manager
            .game()
            .sprite(&dec_name)
            .or_else(|| lod_manager.game().sprite(&sft_name))
            .or_else(|| lod_manager.game().sprite(name))
            // Directional sprites use frame suffixes (e.g. shp0, shp1). Try frame 0.
            .or_else(|| lod_manager.game().sprite(&format!("{}0", dec_name)))
            .or_else(|| lod_manager.game().sprite(&format!("{}0", sft_name)))
            .or_else(|| lod_manager.game().sprite(&format!("{}0", name)));

        let image = match image {
            Some(img) => img,
            None => {
                eprintln!(
                    "WARN: billboard sprite not found: declist[{}] name='{}' sft='{}'",
                    declist_id, dec_name, sft_name
                );
                lod_manager.game().sprite("pending").unwrap()
            }
        };

        Some(BillboardSprite {
            image,
            d_declist_item: declist_item.clone(),
            d_sft_frame: sft_frame.clone(),
        })
    }
}
