use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};

use ::image::DynamicImage;
use lod::Lod;
use palette::Palettes;

pub mod blv;
pub mod bsp_model;
pub mod ddm;
pub mod dlv;
pub mod dtile;
pub mod evt;
pub mod mapstats;
pub mod monlist;
pub mod odm;
pub mod twodevents;

pub mod billboard;
pub mod ddeclist;
pub mod dsft;
pub mod image;

mod lod;
pub mod lod_data;
pub mod palette;
mod utils;
mod zlib;

pub const ENV_OPENMM_6_PATH: &str = "OPENMM_6_PATH";

pub struct LodManager {
    lods: HashMap<String, Lod>,
}

impl LodManager {
    pub fn new<P>(path: P) -> Result<Self, Box<dyn Error>>
    where
        P: AsRef<Path>,
    {
        let lod_files = Self::list_lod_files(path)?;
        let lod_map = Self::create_lod_file_map(lod_files)?;
        Ok(Self { lods: lod_map })
    }

    fn list_lod_files<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
    where
        P: AsRef<Path>,
    {
        let mut lod_files = Vec::new();
        let entries = fs::read_dir(&path)?;

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            if let Some(name) = file_name.to_str() {
                if name.to_lowercase().ends_with(".lod") {
                    lod_files.push(Path::join(path.as_ref(), name));
                }
            }
        }

        Ok(lod_files)
    }

    fn create_lod_file_map(
        lod_files: Vec<PathBuf>,
    ) -> Result<HashMap<String, Lod>, Box<dyn Error>> {
        let mut lod_file_map: HashMap<String, Lod> = HashMap::new();

        for path in lod_files.iter() {
            let lod = Lod::open(path)?;
            let key = path
                .file_stem()
                .ok_or("file should have a .lod extension")?
                .to_string_lossy()
                .to_lowercase();
            lod_file_map.insert(key, lod);
        }

        Ok(lod_file_map)
    }

    pub fn try_get_bytes<P: AsRef<Path>>(&self, path: P) -> Result<&[u8], Box<dyn Error>> {
        let lod_archive: String = path
            .as_ref()
            .parent()
            .ok_or("invalid path")?
            .to_string_lossy()
            .to_string();
        let lod = self
            .lods
            .get(&lod_archive)
            .ok_or(format!("lod file not found in {lod_archive} "))?;
        let lod_entry: String = path
            .as_ref()
            .file_name()
            .ok_or("invalid lod entry")?
            .to_string_lossy()
            .to_string();
        let lod_data = lod.try_get_bytes(&lod_entry).ok_or(format!(
            "unable to open lod entry {:?}",
            path.as_ref().to_str()
        ))?;
        Ok(lod_data)
    }

    pub fn palettes(&self) -> Result<Palettes, Box<dyn Error>> {
        let bitmaps_lod = self
            .lods
            .get("bitmaps")
            .ok_or("expected to have bitmaps.lod")?;
        let palettes = palette::Palettes::try_from(bitmaps_lod)?;
        Ok(palettes)
    }

    /// Returns a list of archive names (e.g. "bitmaps", "sprites", "icons").
    pub fn archives(&self) -> Vec<&str> {
        self.lods.keys().map(|s| s.as_str()).collect()
    }

    /// Returns a list of file names within a specific archive.
    pub fn files_in(&self, archive: &str) -> Option<Vec<&str>> {
        self.lods.get(archive).map(|lod| lod.files())
    }

    /// Dumps all files from all archives to the given directory.
    /// Images/sprites are saved as PNG, other data as raw files.
    pub fn dump_all(&self, output_dir: &Path) -> Result<(), Box<dyn Error>> {
        let palettes = self.palettes()?;
        for (name, lod) in &self.lods {
            let archive_dir = output_dir.join(name);
            lod.save_all(&archive_dir, &palettes)?;
        }
        Ok(())
    }

    pub fn sprite(&self, name: &str) -> Option<DynamicImage> {
        let sprite = self.try_get_bytes(format!("sprites/{}", name.to_lowercase())).ok()?;
        let palettes = self.palettes().ok()?;
        let sprite = crate::image::Image::try_from((sprite, &palettes)).ok()?;
        sprite.to_image_buffer().ok()
    }

    /// Load a sprite using a specific palette ID (for monster variant palette swaps).
    /// The sprite data is the same but decoded with a different color palette.
    pub fn sprite_with_palette(&self, name: &str, palette_id: u16) -> Option<DynamicImage> {
        let sprite_data = self.try_get_bytes(format!("sprites/{}", name.to_lowercase())).ok()?;
        let palettes = self.palettes().ok()?;
        let sprite = crate::image::Image::try_from_with_palette(sprite_data, &palettes, palette_id).ok()?;
        sprite.to_image_buffer().ok()
    }

    pub fn bitmap(&self, name: &str) -> Option<DynamicImage> {
        let bitmap = self.try_get_bytes(format!("bitmaps/{}", name.to_lowercase())).ok()?;
        let bitmap = crate::image::Image::try_from(bitmap).ok()?;
        bitmap.to_image_buffer().ok()
    }

    /// Load a UI image from the icons archive.
    /// Handles PCX format (title screens, loading) and custom bitmap format (buttons).
    pub fn icon(&self, name: &str) -> Option<DynamicImage> {
        let raw = self.try_get_bytes(format!("icons/{}", name.to_lowercase())).ok()?;

        // Try decompressing LOD entry first (may have LOD header)
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(decompressed) => decompressed.data,
            Err(_) => raw.to_vec(),
        };

        if data.len() > 4 && data[0] == 0x0A {
            decode_pcx(&data)
        } else {
            // Try as custom bitmap format
            let img = crate::image::Image::try_from(raw).ok()?;
            img.to_image_buffer().ok()
        }
    }
}

/// Decode a PCX image. Handles both 8-bit paletted (1 plane) and
/// 24-bit RGB (3 planes) formats used in MM6.
fn decode_pcx(data: &[u8]) -> Option<DynamicImage> {
    if data.len() < 128 { return None; }
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

    if bpp != 8 || encoding != 1 || width == 0 || height == 0 { return None; }

    // Decode RLE scanlines
    let scanline_len = bytes_per_line * n_planes;
    let total = scanline_len * height as usize;
    let mut pixels = Vec::with_capacity(total);
    let mut i = 128;
    while pixels.len() < total && i < data.len() {
        let byte = data[i]; i += 1;
        if byte >= 0xC0 {
            let count = (byte & 0x3F) as usize;
            let value = if i < data.len() { let v = data[i]; i += 1; v } else { 0 };
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
            &data[16..16 + 48] // fallback to header palette (16 colors)
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

pub fn get_data_path() -> String {
    env::var(ENV_OPENMM_6_PATH).unwrap_or("./target/mm6".into())
}

pub fn get_lod_path() -> String {
    env::var(ENV_OPENMM_6_PATH).unwrap_or("./target/mm6/data".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_manager_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let grastyl = lod_manager.try_get_bytes("bitmaps/grastyl");
        assert_eq!(17676, grastyl.unwrap().len());
    }

    #[test]
    fn sprite_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let rock = lod_manager.sprite("rok1");
        assert!(rock.is_some());
    }
}
