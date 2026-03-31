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
pub mod dsounds;
pub mod font;
pub mod snd;
pub mod terrain;
pub mod image;

mod lod;
pub mod lod_data;
pub mod palette;
mod utils;
pub(crate) mod zlib;

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

    /// Load raw bytes from an archive path, decompressing if needed.
    pub fn get_decompressed<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, Box<dyn Error>> {
        let raw = self.try_get_bytes(path)?;
        Ok(match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        })
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

    /// Load a .fnt bitmap font from the icons archive.
    pub fn font(&self, name: &str) -> Option<font::Font> {
        let data = self.get_decompressed(format!("icons/{}", name.to_lowercase())).ok()?;
        font::Font::parse(&data).ok()
    }

    /// List all .fnt font names available in the icons archive.
    pub fn font_names(&self) -> Vec<String> {
        self.files_in("icons")
            .map(|files| {
                files
                    .into_iter()
                    .filter(|f| f.ends_with(".fnt"))
                    .map(|f| f.strip_suffix(".fnt").unwrap_or(f).to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn bitmap(&self, name: &str) -> Option<DynamicImage> {
        let bitmap = self.try_get_bytes(format!("bitmaps/{}", name.to_lowercase())).ok()?;
        let bitmap = crate::image::Image::try_from(bitmap).ok()?;
        bitmap.to_image_buffer().ok()
    }

    /// Load a UI image from the icons archive.
    /// Handles PCX format (title screens, loading) and custom bitmap format (buttons).
    pub fn icon(&self, name: &str) -> Option<DynamicImage> {
        let path = format!("icons/{}", name.to_lowercase());
        let raw = self.try_get_bytes(&path).ok()?;
        let data = self.get_decompressed(&path).ok()?;

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

/// Returns the base MM6 game data directory (e.g. for Sounds/).
/// Uses OPENMM_6_PATH env var if set, otherwise falls back to workspace target dir.
pub fn get_data_path() -> String {
    env::var(ENV_OPENMM_6_PATH)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(default_data_path)
}

/// Returns the LOD archive directory (where .lod files live).
/// Uses OPENMM_6_PATH env var if set, otherwise falls back to workspace target dir.
pub fn get_lod_path() -> String {
    env::var(ENV_OPENMM_6_PATH)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(default_lod_path)
}

fn default_data_path() -> String {
    // Try workspace root (two levels up from lod crate manifest)
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = Path::new(manifest).parent().unwrap_or(Path::new("."));
    let candidate = workspace.join("target/mm6");
    if candidate.exists() {
        return candidate.to_string_lossy().into_owned();
    }
    "./target/mm6".into()
}

fn default_lod_path() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = Path::new(manifest).parent().unwrap_or(Path::new("."));
    let candidate = workspace.join("target/mm6/data");
    if candidate.exists() {
        return candidate.to_string_lossy().into_owned();
    }
    "./target/mm6/data".into()
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
    fn font_loading_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();

        let names = lod_manager.font_names();
        assert!(!names.is_empty(), "should find .fnt files");

        let font = lod_manager.font("arrus.fnt").expect("arrus.fnt should load");
        assert_eq!(font.height, 19);
        assert!(font.has_glyph(b'A'));
        assert!(font.glyph_pixels(b'A').is_some());

        // Measure and render
        let width = font.measure("Hello");
        assert!(width > 0);
        let (w, h, buf) = font.render_text("Hi", [255, 255, 255, 255]);
        assert_eq!(h, 19);
        assert!(w > 0);
        assert_eq!(buf.len(), (w * h * 4) as usize);
    }

    #[test]
    fn sprite_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let rock = lod_manager.sprite("rok1");
        assert!(rock.is_some());
    }

    /// Verify that sprite_with_palette produces different pixel data than the default palette.
    /// This is the mechanism used for monster variant B/C coloring (ghosts, skeletons, etc.).
    #[test]
    fn sprite_with_palette_produces_different_pixels() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let dsft = crate::dsft::DSFT::new(&lod_manager).unwrap();
        let monlist = crate::monlist::MonsterList::new(&lod_manager).unwrap();

        // Ghost B has a different DSFT palette than the sprite file header palette.
        // Loading with the DSFT palette should produce visually different pixels.
        let ghost_b = monlist.find_by_name("Ghost", 2).expect("Ghost B should exist");
        let st_group = &ghost_b.sprite_names[0];

        // Find DSFT palette_id for this group
        let frame = dsft.frames.iter().find(|f| {
            f.group_name().map(|g| g.eq_ignore_ascii_case(st_group)).unwrap_or(false)
        }).expect("DSFT frame for ghost B");
        assert!(frame.palette_id > 0, "ghost B should have non-zero DSFT palette");

        // Derive the sprite root from DSFT sprite name
        let sprite_name = frame.sprite_name().unwrap();
        let root = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
        let root = if root.len() > 1 && root.as_bytes()[root.len() - 1] >= b'a' && root.as_bytes()[root.len() - 1] <= b'f' {
            &root[..root.len() - 1]
        } else {
            root
        };
        let test_sprite = format!("{}a0", root);

        // Load with default palette and DSFT palette
        let default_img = lod_manager.sprite(&test_sprite).expect("ghost sprite with default palette");
        let dsft_img = lod_manager.sprite_with_palette(&test_sprite, frame.palette_id as u16)
            .expect("ghost sprite with DSFT palette");

        // Both should be the same dimensions
        assert_eq!(default_img.width(), dsft_img.width());
        assert_eq!(default_img.height(), dsft_img.height());

        // But the pixel data should differ (different palette = different colors)
        let default_bytes = default_img.to_rgba8().into_raw();
        let dsft_bytes = dsft_img.to_rgba8().into_raw();
        assert_ne!(
            default_bytes, dsft_bytes,
            "sprite with DSFT palette {} should produce different pixels than default",
            frame.palette_id
        );
    }
}
