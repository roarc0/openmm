//! LodDecoder — high-level asset decoder built on top of raw LOD archive access.
//!
//! `Assets` provides raw archive access (bytes, decompression, palette loading).
//! `LodDecoder` wraps an `Assets` reference and returns decoded, game-ready data:
//! sprites, bitmaps, icons, fonts, and NPC tables.

use crate::assets::ddeclist::DDecListItem;
use crate::assets::dsft::DSFTFrame;
use crate::assets::provider::Assets;
use crate::assets::{font, npc};
use image::{DynamicImage, GenericImageView};

/// High-level LOD decoder: returns game-ready decoded assets from LOD archives.
/// Constructed via `Assets::game()`.
pub struct LodDecoder<'a> {
    assets: &'a Assets,
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

impl<'a> LodDecoder<'a> {
    pub fn new(assets: &'a Assets) -> Self {
        Self { assets }
    }

    /// Load a sprite image from the sprites archive.
    pub fn sprite(&self, name: &str) -> Option<DynamicImage> {
        let sprite = self.assets.get_bytes(format!("sprites/{}", name.to_lowercase())).ok()?;
        let palettes = self.assets.palettes().ok()?;
        let sprite = crate::assets::image::Image::try_from((sprite.as_slice(), palettes)).ok()?;
        sprite.to_image_buffer().ok()
    }

    /// Load a sprite using a specific palette ID (for monster variant palette swaps).
    pub fn sprite_with_palette(&self, name: &str, palette_id: u16) -> Option<DynamicImage> {
        let sprite_data = self.assets.get_bytes(format!("sprites/{}", name.to_lowercase())).ok()?;
        let palettes = self.assets.palettes().ok()?;
        let sprite =
            crate::assets::image::Image::try_from_with_palette(sprite_data.as_slice(), palettes, palette_id).ok()?;
        sprite.to_image_buffer().ok()
    }

    /// Load a bitmap image from the bitmaps archive.
    pub fn bitmap(&self, name: &str) -> Option<DynamicImage> {
        let bitmap = self.assets.get_bytes(format!("bitmaps/{}", name.to_lowercase())).ok()?;
        let bitmap = crate::assets::image::Image::try_from(bitmap.as_slice()).ok()?;
        bitmap.to_image_buffer().ok()
    }

    /// Load a UI image from the icons archive.
    /// Handles PCX format (title screens, loading) and MM6 custom bitmap format (buttons).
    pub fn icon(&self, name: &str) -> Option<DynamicImage> {
        let path = format!("icons/{}", name.to_lowercase());
        let raw = self.assets.get_bytes(&path).ok()?;
        // Try LodData decompression once; fall back to raw bytes if not compressed.
        let data = match crate::assets::lod_data::LodData::try_from(raw.as_slice()) {
            Ok(d) => d.data,
            Err(_) => raw, // move: no copy needed when data is already uncompressed
        };
        if data.len() > 4 && data[0] == 0x0A {
            crate::assets::pcx::decode(&data)
        } else {
            let img = crate::assets::image::Image::try_from(data.as_slice()).ok()?;
            img.to_image_buffer().ok()
        }
    }

    /// Load a bitmap font from the icons archive.
    pub fn font(&self, name: &str) -> Option<font::Font> {
        let data = self
            .assets
            .get_decompressed(format!("icons/{}", name.to_lowercase()))
            .ok()?;
        font::Font::parse(&data).ok()
    }

    /// List all .fnt font names available in the icons archive.
    pub fn font_names(&self) -> Vec<String> {
        self.assets
            .files_in("icons")
            .map(|files| {
                files
                    .into_iter()
                    .filter_map(|f: String| {
                        let lower = f.to_lowercase();
                        lower.strip_suffix(".fnt").map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Load and parse the global NPC metadata table from `npcdata.txt`.
    pub fn npc_table(&self) -> Option<npc::StreetNpcs> {
        let data = self.assets.get_decompressed("icons/npcdata.txt").ok()?;
        let name_pool = self.npc_name_pool();
        npc::StreetNpcs::parse(&data, name_pool.as_ref()).ok()
    }

    /// Load the NPC name pool from `npcnames.txt` for generating street NPC names.
    pub fn npc_name_pool(&self) -> Option<npc::NpcNamePool> {
        let data = self.assets.get_decompressed("icons/npcnames.txt").ok()?;
        npc::NpcNamePool::parse(&data).ok()
    }

    /// Get the decoration list item for a given declist_id.
    pub fn billboard_item(&self, id: u16) -> Option<&DDecListItem> {
        self.assets.data().ddeclist.items.get(id as usize)
    }

    /// Find a declist item by case-insensitive name match.
    pub fn billboard_item_by_name(&self, name: &str) -> Option<(u16, &DDecListItem)> {
        let lower = name.to_lowercase();
        self.assets
            .data()
            .ddeclist
            .items
            .iter()
            .enumerate()
            .find_map(|(i, item)| {
                item.name()
                    .filter(|n| n.to_lowercase() == lower)
                    .map(|_| (i as u16, item))
            })
    }

    /// Get the DSFT scale factor for a decoration item.
    pub fn billboard_scale(&self, item: &DDecListItem) -> Option<f32> {
        let frame = self.assets.data().dsft.frames.get(item.sft_index() as usize)?;
        if frame.scale > 0 {
            Some(frame.scale as f32 / 65536.0)
        } else {
            None
        }
    }

    pub fn dsft_scale_for_group(&self, group: &str) -> f32 {
        self.assets.data().dsft.scale_for_group(group)
    }

    pub fn billboard_luminous_light_radius(&self, id: u16) -> u16 {
        let data = self.assets.data();
        let Some(item) = data.ddeclist.items.get(id as usize) else {
            return 0;
        };
        if item.light_radius > 0 {
            return 0;
        }
        let sft_idx = item.sft_index();
        if sft_idx < 0 {
            return 0;
        }
        let Some(frame) = data.dsft.frames.get(sft_idx as usize) else {
            return 0;
        };
        if frame.is_luminous() && frame.light_radius > 0 {
            frame.light_radius as u16
        } else {
            0
        }
    }

    pub fn billboard_animation_frame_count(&self, id: u16) -> usize {
        let data = self.assets.data();
        let Some(item) = data.ddeclist.items.get(id as usize) else {
            return 1;
        };
        let sft_idx = item.sft_index();
        if sft_idx < 0 {
            return 1;
        }
        let mut count = 0;
        let mut idx = sft_idx as usize;
        loop {
            let Some(frame) = data.dsft.frames.get(idx) else {
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

    pub fn billboard_animation_frames(&self, name: &str, id: u16) -> Vec<BillboardSprite> {
        let data = self.assets.data();
        let Some(item) = data.ddeclist.items.get(id as usize) else {
            return vec![];
        };
        let sft_idx = item.sft_index();
        if sft_idx < 0 {
            return vec![];
        }

        let dec_name = item.name().unwrap_or_default();
        let mut results = vec![];
        let mut idx = sft_idx as usize;
        loop {
            let Some(frame) = data.dsft.frames.get(idx) else {
                break;
            };
            let sft_name = frame.sprite_name().unwrap_or_default();
            let is_first = idx == sft_idx as usize;

            let image = if is_first {
                self.sprite(&dec_name)
                    .or_else(|| self.sprite(&sft_name))
                    .or_else(|| self.sprite(name))
                    .or_else(|| self.sprite(&format!("{}0", dec_name)))
                    .or_else(|| self.sprite(&format!("{}0", sft_name)))
                    .or_else(|| self.sprite(&format!("{}0", name)))
            } else {
                self.sprite(&sft_name).or_else(|| self.sprite(name))
            };

            match image {
                Some(img) => results.push(BillboardSprite {
                    image: img,
                    d_declist_item: item.clone(),
                    d_sft_frame: frame.clone(),
                }),
                Option::None => {
                    log::warn!(
                        "billboard: animation frame {} for '{}' not found",
                        idx - sft_idx as usize,
                        name
                    );
                }
            }
            if !frame.is_not_group_end() {
                break;
            }
            idx += 1;
        }
        results
    }

    pub fn billboard(&self, name: &str, id: u16) -> Option<BillboardSprite> {
        let data = self.assets.data();
        let item = data.ddeclist.items.get(id as usize)?;
        let frame = data.dsft.frames.get(item.sft_index() as usize)?;

        let dec_name = item.name().unwrap_or_default();
        let sft_name = frame.sprite_name().unwrap_or_default();
        let image = self
            .sprite(&dec_name)
            .or_else(|| self.sprite(&sft_name))
            .or_else(|| self.sprite(name))
            .or_else(|| self.sprite(&format!("{}0", dec_name)))
            .or_else(|| self.sprite(&format!("{}0", sft_name)))
            .or_else(|| self.sprite(&format!("{}0", name)));

        let image = match image {
            Some(img) => img,
            Option::None => {
                log::warn!(
                    "billboard sprite not found: declist[{}] name='{}' sft='{}'",
                    id,
                    dec_name,
                    sft_name
                );
                self.sprite("pending")?
            }
        };

        Some(BillboardSprite {
            image,
            d_declist_item: item.clone(),
            d_sft_frame: frame.clone(),
        })
    }
}
