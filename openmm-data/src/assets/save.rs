/// MM6 save file (.mm6) reader.
///
/// Save files are LOD archives containing: header.bin (metadata), image.pcx (screenshot),
/// party.bin (party state), clock.bin (game time), per-map *.dlv/*.ddm state, and more.
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::assets::provider::archive::{Archive, lod::LodArchive};
use image::{DynamicImage, RgbImage};

/// Metadata from `header.bin` (100 bytes) inside a save LOD.
#[derive(Debug, Clone, Default)]
pub struct SaveHeader {
    /// Player-visible save name (bytes 0–19, null-terminated). Empty in dev saves.
    pub save_name: String,
    /// Current map filename, e.g. `"oute3.odm"` (bytes 20–31, null-terminated).
    pub map_name: String,
}

impl SaveHeader {
    pub fn parse(data: &[u8]) -> Self {
        fn read_str(buf: &[u8]) -> String {
            let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            String::from_utf8_lossy(&buf[..end]).into_owned()
        }
        Self {
            save_name: if data.len() >= 20 {
                read_str(&data[..20])
            } else {
                String::new()
            },
            map_name: if data.len() >= 32 {
                read_str(&data[20..32])
            } else {
                String::new()
            },
        }
    }
}

/// An opened MM6 save file (.mm6 LOD archive).
pub struct SaveFile {
    archive: LodArchive,
    /// Filename stem (slot name), e.g. `"save000"`, `"autosave"`, `"quiksave"`.
    pub slot: String,
    pub path: PathBuf,
}

impl SaveFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let path = path.as_ref().to_path_buf();
        let slot = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let archive = LodArchive::open(&path)?;
        Ok(Self { archive, slot, path })
    }

    pub fn header(&self) -> SaveHeader {
        self.archive
            .get_file("header.bin")
            .as_deref()
            .map(SaveHeader::parse)
            .unwrap_or_default()
    }

    /// Decode the save screenshot (`image.pcx`) to a `DynamicImage`, or `None` if absent/corrupt.
    pub fn screenshot(&self) -> Option<DynamicImage> {
        let data = self.archive.get_file("image.pcx")?;
        decode_pcx(&data)
            .map_err(|e| log::warn!("Failed to decode screenshot in {}: {}", self.slot, e))
            .ok()
    }

    pub fn get_file(&self, name: &str) -> Option<Vec<u8>> {
        self.archive.get_file(name)
    }

    pub fn list_files(&self) -> Vec<String> {
        self.archive.list_files().iter().map(|e| e.name.clone()).collect()
    }
}

/// Open all `.mm6` save files from `dir`, sorted by slot name (autosave/quiksave first, then save000...).
pub fn list_saves<P: AsRef<Path>>(dir: P) -> Vec<SaveFile> {
    let dir = dir.as_ref();
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut saves: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("mm6"))
                .unwrap_or(false)
        })
        .map(|e| {
            let stem = e
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            (stem, e.path())
        })
        .collect();

    saves.sort_by(|a, b| save_slot_order(&a.0).cmp(&save_slot_order(&b.0)));

    saves.into_iter().filter_map(|(_, p)| SaveFile::open(&p).ok()).collect()
}

/// Ordering key so autosave/quiksave sort before numbered saves.
fn save_slot_order(slot: &str) -> (u8, String) {
    match slot {
        "autosave" => (0, String::new()),
        s if s.starts_with("quiksave") => (1, s.to_string()),
        s => (2, s.to_string()),
    }
}

/// Decode a 24-bit three-plane PCX image (as used by MM6 save screenshots).
///
/// Format: 128-byte header, RLE-compressed scanlines with 3 planes (R, G, B) each
/// `bytes_per_line` wide.
fn decode_pcx(data: &[u8]) -> Result<DynamicImage, Box<dyn Error>> {
    if data.len() < 128 {
        return Err("PCX data too short".into());
    }
    if data[0] != 0x0A {
        return Err("Not a PCX file (missing 0x0A magic)".into());
    }

    let xmin = u16::from_le_bytes([data[4], data[5]]) as u32;
    let ymin = u16::from_le_bytes([data[6], data[7]]) as u32;
    let xmax = u16::from_le_bytes([data[8], data[9]]) as u32;
    let ymax = u16::from_le_bytes([data[10], data[11]]) as u32;
    let bpp = data[3];
    let planes = data[65] as u32;
    let bytes_per_line = u16::from_le_bytes([data[66], data[67]]) as u32;

    let width = xmax - xmin + 1;
    let height = ymax - ymin + 1;

    if bpp != 8 || planes != 3 {
        return Err(format!("Unsupported PCX format: {}bpp {} planes", bpp, planes).into());
    }

    let stride = (planes * bytes_per_line) as usize;
    let mut pixels: Vec<u8> = Vec::with_capacity((width * height * 3) as usize);
    let mut pos = 128usize;

    for _row in 0..height {
        // Decode one full row (all planes) via RLE
        let mut row = vec![0u8; stride];
        let mut out = 0usize;

        while out < stride {
            if pos >= data.len() {
                break;
            }
            let byte = data[pos];
            pos += 1;
            if byte >= 0xC0 {
                let count = (byte & 0x3F) as usize;
                if pos >= data.len() {
                    break;
                }
                let val = data[pos];
                pos += 1;
                let end = (out + count).min(stride);
                row[out..end].fill(val);
                out += count;
            } else {
                row[out] = byte;
                out += 1;
            }
        }

        // Interleave R/G/B planes into contiguous RGB pixels
        let bpl = bytes_per_line as usize;
        for x in 0..width as usize {
            pixels.push(row[x]); // R
            pixels.push(row[bpl + x]); // G
            pixels.push(row[2 * bpl + x]); // B
        }
    }

    let img = RgbImage::from_raw(width, height, pixels).ok_or("PCX pixel buffer size mismatch")?;
    Ok(DynamicImage::ImageRgb8(img))
}
