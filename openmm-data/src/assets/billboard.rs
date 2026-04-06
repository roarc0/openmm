use std::{
    error::Error,
    io::{Cursor, Read},
};

use image::{DynamicImage, GenericImageView};
use serde::{Deserialize, Serialize};

use crate::assets::{
    ddeclist::{DDecList, DDecListItem},
    dsft::{DSFT, DSFTFrame},
};
use crate::{Assets, utils::try_read_string_block};

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
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let d_declist = DDecList::load(assets)?;
        let d_sft = DSFT::load(assets)?;
        Ok(Self { d_declist, d_sft })
    }

    /// Get the decoration list item for a given declist_id.
    pub fn get_declist_item(&self, id: u16) -> Option<&DDecListItem> {
        self.d_declist.items.get(id as usize)
    }

    /// Find a declist item by case-insensitive name match.
    /// Returns (index, item) so callers can use the index as declist_id.
    /// Used for BLV decorations whose `decoration_desc_id` is always 0 in pristine files;
    /// the name field (e.g. "Torch01") is the only reliable key.
    pub fn get_declist_item_by_name(&self, name: &str) -> Option<(u16, &DDecListItem)> {
        let lower = name.to_lowercase();
        self.d_declist.items.iter().enumerate().find_map(|(i, item)| {
            item.name()
                .filter(|n| n.to_lowercase() == lower)
                .map(|_| (i as u16, item))
        })
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

    /// Return the DSFT frame light_radius for a luminous decoration whose ddeclist.light_radius is 0.
    /// Used for animated fire sources (campfireon, flamb00, etc.) and static luminous decs
    /// (crystals, chandeliers) where illumination is encoded in the DSFT frame, not the ddeclist.
    /// Returns 0 if not DSFT-luminous, or if ddeclist already has a non-zero light_radius.
    pub fn dsft_luminous_light_radius(&self, declist_id: u16) -> u16 {
        let Some(item) = self.d_declist.items.get(declist_id as usize) else {
            return 0;
        };
        if item.light_radius > 0 {
            return 0; // ddeclist handles it
        }
        let sft_idx = item.sft_index();
        if sft_idx < 0 {
            return 0;
        }
        let Some(frame) = self.d_sft.frames.get(sft_idx as usize) else {
            return 0;
        };
        if frame.is_luminous() && frame.light_radius > 0 {
            frame.light_radius as u16
        } else {
            0
        }
    }

    /// Count the animation frames in a decoration's DSFT group.
    /// Returns 1 for single-frame (static) decorations.
    pub fn animation_frame_count(&self, declist_id: u16) -> usize {
        let Some(declist_item) = self.d_declist.items.get(declist_id as usize) else {
            return 1;
        };
        let sft_index = declist_item.sft_index();
        if sft_index < 0 {
            return 1;
        }
        let mut count = 0;
        let mut idx = sft_index as usize;
        loop {
            let Some(frame) = self.d_sft.frames.get(idx) else {
                break;
            };
            count += 1;
            if !frame.is_not_group_end() {
                break;
            }
            idx += 1;
        }
        count.max(1)
    }

    /// Load all animation frames for a decoration by walking the DSFT group chain.
    /// Returns a `Vec` with one entry per frame; single-frame decorations return `vec![...]` of length 1.
    pub fn get_animation_frames(&self, assets: &Assets, name: &str, declist_id: u16) -> Vec<BillboardSprite> {
        let Some(declist_item) = self.d_declist.items.get(declist_id as usize) else {
            return vec![];
        };
        let sft_index = declist_item.sft_index();
        if sft_index < 0 {
            return vec![];
        }
        let dec_name = declist_item.name().unwrap_or_default();
        let mut results = vec![];
        let mut idx = sft_index as usize;
        loop {
            let Some(sft_frame) = self.d_sft.frames.get(idx) else {
                break;
            };
            let sft_name = sft_frame.sprite_name().unwrap_or_default();
            let is_first = idx == sft_index as usize;
            let image = if is_first {
                assets
                    .game()
                    .sprite(&dec_name)
                    .or_else(|| assets.game().sprite(&sft_name))
                    .or_else(|| assets.game().sprite(name))
                    .or_else(|| assets.game().sprite(&format!("{}0", dec_name)))
                    .or_else(|| assets.game().sprite(&format!("{}0", sft_name)))
                    .or_else(|| assets.game().sprite(&format!("{}0", name)))
            } else {
                assets.game().sprite(&sft_name).or_else(|| assets.game().sprite(name))
            };
            match image {
                Some(img) => results.push(BillboardSprite {
                    image: img,
                    d_declist_item: declist_item.clone(),
                    d_sft_frame: sft_frame.clone(),
                }),
                None => {
                    log::warn!(
                        "billboard: animation frame {} for '{}' not found in LOD",
                        idx - sft_index as usize,
                        name
                    );
                }
            }
            if !sft_frame.is_not_group_end() {
                break;
            }
            idx += 1;
        }
        results
    }

    pub fn get(&self, assets: &Assets, name: &str, declist_id: u16) -> Option<BillboardSprite> {
        let declist_item = self.d_declist.items.get(declist_id as usize)?;
        let sft_frame = self.d_sft.frames.get(declist_item.sft_index() as usize)?;

        let dec_name = declist_item.name().unwrap_or_default();
        let sft_name = sft_frame.sprite_name().unwrap_or_default();
        let image = assets
            .game()
            .sprite(&dec_name)
            .or_else(|| assets.game().sprite(&sft_name))
            .or_else(|| assets.game().sprite(name))
            // Directional sprites use frame suffixes (e.g. shp0, shp1). Try frame 0.
            .or_else(|| assets.game().sprite(&format!("{}0", dec_name)))
            .or_else(|| assets.game().sprite(&format!("{}0", sft_name)))
            .or_else(|| assets.game().sprite(&format!("{}0", name)));

        let image = match image {
            Some(img) => img,
            None => {
                eprintln!(
                    "WARN: billboard sprite not found: declist[{}] name='{}' sft='{}'",
                    declist_id, dec_name, sft_name
                );
                assets.game().sprite("pending").unwrap()
            }
        };

        Some(BillboardSprite {
            image,
            d_declist_item: declist_item.clone(),
            d_sft_frame: sft_frame.clone(),
        })
    }
}
