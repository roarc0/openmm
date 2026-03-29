//! MM6/MM7 bitmap font parser (.fnt files from LOD archives).
//!
//! Font files are stored in the icons LOD archive as zlib-compressed binary data.
//! After decompression the format is:
//!
//! - **Header** (32 bytes): first_char, last_char, flags, height
//! - **Metrics** (256 × 12 bytes): per-character left_spacing (i32), width (i32), right_spacing (i32)
//! - **Offsets** (256 × 4 bytes): byte offset into pixel data for each character
//! - **Pixels** (variable): grayscale glyph bitmaps (0 = transparent, 1 = shadow, 255 = text)

use std::error::Error;

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

const HEADER_SIZE: usize = 32;
const NUM_CHARS: usize = 256;
const METRICS_SIZE: usize = NUM_CHARS * 12; // 3 × i32 per character
const OFFSETS_SIZE: usize = NUM_CHARS * 4; // 1 × u32 per character
const TABLE_SIZE: usize = METRICS_SIZE + OFFSETS_SIZE; // 4096

/// Per-character spacing and width.
#[derive(Debug, Clone, Copy, Default)]
pub struct GlyphMetrics {
    /// Pixels of empty space before the glyph.
    pub left_spacing: i32,
    /// Width of the glyph bitmap in pixels.
    pub width: i32,
    /// Pixels of empty space after the glyph.
    pub right_spacing: i32,
}

impl GlyphMetrics {
    /// Total advance width (spacing + glyph + spacing).
    pub fn advance(&self) -> i32 {
        self.left_spacing + self.width + self.right_spacing
    }
}

/// A decoded MM6/MM7 bitmap font.
#[derive(Debug, Clone)]
pub struct Font {
    /// First valid character code.
    pub first_char: u8,
    /// Last valid character code.
    pub last_char: u8,
    /// Line height in pixels (all glyphs share this height).
    pub height: u8,
    /// Per-character metrics (spacing and width).
    pub metrics: [GlyphMetrics; NUM_CHARS],
    /// Byte offsets into `pixels` for each character.
    offsets: [u32; NUM_CHARS],
    /// Raw glyph pixel data (0 = transparent, 1 = shadow, 255 = text body).
    pixels: Vec<u8>,
}

impl Font {
    /// Parse a font from decompressed .fnt data.
    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        if data.len() < HEADER_SIZE + TABLE_SIZE {
            return Err(format!(
                "font data too short: {} bytes (need at least {})",
                data.len(),
                HEADER_SIZE + TABLE_SIZE
            )
            .into());
        }

        let first_char = data[0];
        let last_char = data[1];
        let height = data[5];

        if first_char > last_char {
            return Err(format!("invalid char range: {}..{}", first_char, last_char).into());
        }
        if height == 0 {
            return Err("font height is zero".into());
        }

        // Parse metrics (256 entries × 12 bytes each)
        let mut metrics = [GlyphMetrics::default(); NUM_CHARS];
        let mut cursor = Cursor::new(&data[HEADER_SIZE..]);
        for m in &mut metrics {
            m.left_spacing = cursor.read_i32::<LittleEndian>()?;
            m.width = cursor.read_i32::<LittleEndian>()?;
            m.right_spacing = cursor.read_i32::<LittleEndian>()?;
        }

        // Parse offsets (256 entries × 4 bytes each)
        let mut offsets = [0u32; NUM_CHARS];
        for o in &mut offsets {
            *o = cursor.read_u32::<LittleEndian>()?;
        }

        let pixels = data[HEADER_SIZE + TABLE_SIZE..].to_vec();

        Ok(Self {
            first_char,
            last_char,
            height,
            metrics,
            offsets,
            pixels,
        })
    }

    /// Returns true if the character has glyph data in this font.
    pub fn has_glyph(&self, ch: u8) -> bool {
        ch >= self.first_char && ch <= self.last_char && self.metrics[ch as usize].width > 0
    }

    /// Get the raw pixel slice for a character glyph.
    /// Returns `width × height` bytes (0 = transparent, 1 = shadow, 255 = text).
    /// Returns `None` if the character is out of range or has zero width.
    pub fn glyph_pixels(&self, ch: u8) -> Option<&[u8]> {
        if !self.has_glyph(ch) {
            return None;
        }
        let m = &self.metrics[ch as usize];
        let offset = self.offsets[ch as usize] as usize;
        let size = m.width as usize * self.height as usize;
        if offset + size > self.pixels.len() {
            return None;
        }
        Some(&self.pixels[offset..offset + size])
    }

    /// Measure the width of a text string in pixels.
    pub fn measure(&self, text: &str) -> i32 {
        text.bytes()
            .map(|ch| {
                if self.has_glyph(ch) {
                    self.metrics[ch as usize].advance()
                } else {
                    self.metrics[b' ' as usize].advance()
                }
            })
            .sum()
    }

    /// Render a text string into an RGBA pixel buffer.
    ///
    /// Returns `(width, height, rgba_pixels)`. Text body pixels use `color`,
    /// shadow pixels use semi-transparent black.
    pub fn render_text(&self, text: &str, color: [u8; 4]) -> (u32, u32, Vec<u8>) {
        let total_w = self.measure(text).max(1) as u32;
        let total_h = self.height as u32;
        let mut buf = vec![0u8; (total_w * total_h * 4) as usize];

        let mut cursor_x: i32 = 0;
        for ch in text.bytes() {
            let ch = if self.has_glyph(ch) { ch } else { b' ' };
            let m = &self.metrics[ch as usize];

            cursor_x += m.left_spacing;

            if let Some(pixels) = self.glyph_pixels(ch) {
                let glyph_w = m.width as usize;
                for row in 0..self.height as usize {
                    for col in 0..glyph_w {
                        let px = pixels[row * glyph_w + col];
                        if px == 0 {
                            continue;
                        }
                        let x = cursor_x as usize + col;
                        let y = row;
                        if x >= total_w as usize {
                            continue;
                        }
                        let idx = (y * total_w as usize + x) * 4;
                        if px == 255 {
                            buf[idx] = color[0];
                            buf[idx + 1] = color[1];
                            buf[idx + 2] = color[2];
                            buf[idx + 3] = color[3];
                        } else {
                            // Shadow pixel — semi-transparent black
                            buf[idx] = 0;
                            buf[idx + 1] = 0;
                            buf[idx + 2] = 0;
                            buf[idx + 3] = 128;
                        }
                    }
                }
            }

            cursor_x += m.width + m.right_spacing;
        }

        (total_w, total_h, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;

    /// Build a minimal synthetic font with one glyph ('A' = 65) of width 3, height 2.
    fn make_test_font() -> Vec<u8> {
        let mut data = vec![0u8; HEADER_SIZE + TABLE_SIZE];
        // Header
        data[0] = 32; // first_char
        data[1] = 65; // last_char (just 'A')
        data[2] = 8; // field_3
        data[5] = 2; // height

        // Metrics for 'A' at index 65: left=0, width=3, right=1
        let m_off = HEADER_SIZE + 65 * 12;
        let mut c = Cursor::new(&mut data[m_off..m_off + 12]);
        c.write_i32::<LittleEndian>(0).unwrap(); // left
        c.write_i32::<LittleEndian>(3).unwrap(); // width
        c.write_i32::<LittleEndian>(1).unwrap(); // right

        // Metrics for ' ' at index 32: left=0, width=0, right=2
        let m_off = HEADER_SIZE + 32 * 12;
        let mut c = Cursor::new(&mut data[m_off..m_off + 12]);
        c.write_i32::<LittleEndian>(0).unwrap();
        c.write_i32::<LittleEndian>(2).unwrap(); // space width
        c.write_i32::<LittleEndian>(0).unwrap();

        // Offset for 'A' = 0 (pixel data starts at beginning)
        let o_off = HEADER_SIZE + METRICS_SIZE + 65 * 4;
        let mut c = Cursor::new(&mut data[o_off..o_off + 4]);
        c.write_u32::<LittleEndian>(0).unwrap();

        // Pixel data: 3 * 2 = 6 bytes for 'A'
        data.extend_from_slice(&[255, 0, 255, 1, 255, 1]);

        data
    }

    #[test]
    fn parse_synthetic_font() {
        let data = make_test_font();
        let font = Font::parse(&data).unwrap();
        assert_eq!(font.height, 2);
        assert_eq!(font.first_char, 32);
        assert_eq!(font.last_char, 65);
        assert!(font.has_glyph(b'A'));
        assert!(!font.has_glyph(b'B'));
    }

    #[test]
    fn glyph_pixels() {
        let data = make_test_font();
        let font = Font::parse(&data).unwrap();
        let px = font.glyph_pixels(b'A').unwrap();
        assert_eq!(px, &[255, 0, 255, 1, 255, 1]);
    }

    #[test]
    fn measure_text() {
        let data = make_test_font();
        let font = Font::parse(&data).unwrap();
        // 'A' advance = 0 + 3 + 1 = 4
        assert_eq!(font.measure("A"), 4);
        // "AA" = 4 + 4 = 8
        assert_eq!(font.measure("AA"), 8);
    }

    #[test]
    fn render_text() {
        let data = make_test_font();
        let font = Font::parse(&data).unwrap();
        let (w, h, buf) = font.render_text("A", [255, 255, 255, 255]);
        assert_eq!(w, 4); // advance width
        assert_eq!(h, 2);
        assert_eq!(buf.len(), (4 * 2 * 4) as usize);
        // First pixel should be white (255 in glyph)
        assert_eq!(&buf[0..4], &[255, 255, 255, 255]);
        // Second pixel should be transparent (0 in glyph)
        assert_eq!(&buf[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn reject_invalid_data() {
        assert!(Font::parse(&[0; 10]).is_err()); // too short
        let mut data = vec![0u8; HEADER_SIZE + TABLE_SIZE];
        data[0] = 100; // first_char > last_char
        data[1] = 50;
        assert!(Font::parse(&data).is_err());
    }
}
