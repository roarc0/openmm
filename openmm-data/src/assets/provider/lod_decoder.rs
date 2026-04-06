//! LodDecoder — high-level asset decoder built on top of raw LOD archive access.
//!
//! `Assets` provides raw archive access (bytes, decompression, palette loading).
//! `LodDecoder` wraps an `Assets` reference and returns decoded, game-ready data:
//! sprites, bitmaps, icons, fonts, and NPC tables.

use crate::assets::provider::Assets;
use crate::assets::{font, npc};
use ::image::DynamicImage;

/// High-level LOD decoder: returns game-ready decoded assets from LOD archives.
/// Constructed via `Assets::game()`.
pub struct LodDecoder<'a> {
    assets: &'a Assets,
}

impl<'a> LodDecoder<'a> {
    pub fn new(assets: &'a Assets) -> Self {
        Self { assets }
    }

    /// Load a sprite image from the sprites archive.
    pub fn sprite(&self, name: &str) -> Option<DynamicImage> {
        let sprite = self.assets.get_bytes(format!("sprites/{}", name.to_lowercase())).ok()?;
        let palettes = self.assets.palettes().ok()?;
        let sprite = crate::assets::image::Image::try_from((sprite.as_slice(), &palettes)).ok()?;
        sprite.to_image_buffer().ok()
    }

    /// Load a sprite using a specific palette ID (for monster variant palette swaps).
    pub fn sprite_with_palette(&self, name: &str, palette_id: u16) -> Option<DynamicImage> {
        let sprite_data = self.assets.get_bytes(format!("sprites/{}", name.to_lowercase())).ok()?;
        let palettes = self.assets.palettes().ok()?;
        let sprite =
            crate::assets::image::Image::try_from_with_palette(sprite_data.as_slice(), &palettes, palette_id).ok()?;
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
        let data = self.assets.get_decompressed(&path).ok()?;

        if data.len() > 4 && data[0] == 0x0A {
            decode_pcx(&data)
        } else {
            let img = crate::assets::image::Image::try_from(raw.as_slice()).ok()?;
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
}

/// Decode a PCX image. Handles both 8-bit paletted (1 plane) and
/// 24-bit RGB (3 planes) formats used in MM6.
fn decode_pcx(data: &[u8]) -> Option<DynamicImage> {
    if data.len() < 128 {
        return None;
    }
    let encoding = data[2];
    let bpp = data[3];
    let x_min = u16::from_le_bytes([data[4], data[5]]) as u32;
    let y_min = u16::from_le_bytes([data[6], data[7]]) as u32;
    let x_max = u16::from_le_bytes([data[8], data[9]]) as u32;
    let y_max = u16::from_le_bytes([data[10], data[11]]) as u32;
    let width = x_max - x_min + 1;
    let height = y_max - y_min + 1;
    let n_planes = data[65] as usize;
    let bytes_per_line = u16::from_le_bytes([data[66], data[67]]) as usize;

    if bpp != 8 || encoding != 1 || width == 0 || height == 0 {
        return None;
    }

    // Decode RLE scanlines
    let scanline_len = bytes_per_line * n_planes;
    let total = scanline_len * height as usize;
    let mut pixels = Vec::with_capacity(total);
    let mut i = 128;
    while pixels.len() < total && i < data.len() {
        let byte = data[i];
        i += 1;
        if byte >= 0xC0 {
            let count = (byte & 0x3F) as usize;
            let value = if i < data.len() {
                let v = data[i];
                i += 1;
                v
            } else {
                0
            };
            pixels.extend(std::iter::repeat_n(value, count));
        } else {
            pixels.push(byte);
        }
    }

    let mut img = ::image::RgbaImage::new(width, height);

    if n_planes == 3 {
        // 24-bit RGB: each scanline has R plane, then G plane, then B plane
        for y in 0..height as usize {
            let line = &pixels[y * scanline_len..];
            for x in 0..width as usize {
                let r = line.get(x).copied().unwrap_or(0);
                let g = line.get(bytes_per_line + x).copied().unwrap_or(0);
                let b = line.get(bytes_per_line * 2 + x).copied().unwrap_or(0);
                img.put_pixel(x as u32, y as u32, ::image::Rgba([r, g, b, 255]));
            }
        }
    } else {
        // 8-bit paletted: palette at end of file (0x0C marker + 768 bytes)
        let pal_off = data.len().saturating_sub(769);
        let palette = if data.len() >= 769 && data[pal_off] == 0x0C {
            &data[pal_off + 1..]
        } else {
            &data[16..16 + 48]
        };
        for y in 0..height as usize {
            for x in 0..width as usize {
                let idx = pixels.get(y * bytes_per_line + x).copied().unwrap_or(0) as usize;
                let r = palette.get(idx * 3).copied().unwrap_or(0);
                let g = palette.get(idx * 3 + 1).copied().unwrap_or(0);
                let b = palette.get(idx * 3 + 2).copied().unwrap_or(0);
                img.put_pixel(x as u32, y as u32, ::image::Rgba([r, g, b, 255]));
            }
        }
    }
    Some(DynamicImage::ImageRgba8(img))
}
