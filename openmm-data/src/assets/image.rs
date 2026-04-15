use byteorder::{LittleEndian, ReadBytesExt};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, imageops};
use std::{
    error::Error,
    io::{Cursor, Seek},
    path::Path,
};

use super::palette::Palettes;
use crate::Assets;

#[derive(Debug)]
pub struct Image {
    pub height: usize,
    pub width: usize,
    pub data: Vec<u8>,
    pub palette: [u8; PALETTE_SIZE],
    pub transparency: bool,
}

const PALETTE_SIZE: usize = 256 * 3;
const BITMAP_HEADER_SIZE: usize = 48;
const SPRITE_HEADER_SIZE: usize = 32;

/// This is for bitmap images
impl TryFrom<&[u8]> for Image {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(16))?;
        let pixel_size = cursor.read_u32::<LittleEndian>()? as usize;
        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let width = cursor.read_u16::<LittleEndian>()? as usize;
        let height = cursor.read_u16::<LittleEndian>()? as usize;
        cursor.seek(std::io::SeekFrom::Current(12))?;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        if pixel_size == 0 {
            return Err("Pixel size is zero, this is not a valid image".into());
        }
        if data.len() <= BITMAP_HEADER_SIZE + PALETTE_SIZE {
            return Err("Not enough data".into());
        }

        let compressed_data = &data[BITMAP_HEADER_SIZE..data.len() - PALETTE_SIZE];
        let uncompressed_data = crate::assets::zlib::decompress(compressed_data, compressed_size, uncompressed_size)?;

        let palette_slice = &data[data.len() - PALETTE_SIZE..];
        let palette: [u8; PALETTE_SIZE] = palette_slice.try_into()?;

        Ok(Self {
            height,
            width,
            data: uncompressed_data,
            palette,
            transparency: false,
        })
    }
}

/// This is for sprite images
impl TryFrom<(&[u8], &Palettes)> for Image {
    type Error = Box<dyn Error>;

    fn try_from(data: (&[u8], &Palettes)) -> Result<Self, Self::Error> {
        let palettes = data.1;
        let data = data.0;

        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(12))?;

        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let width = cursor.read_u16::<LittleEndian>()? as usize;
        let height = cursor.read_u16::<LittleEndian>()? as usize;

        let palette_id = cursor.read_u16::<LittleEndian>()?;
        let palette = palettes
            .get(palette_id)
            .ok_or_else(|| "Palette not found!".to_string())?;

        cursor.seek(std::io::SeekFrom::Current(6))?;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        let table_size: usize = height * 8;

        if data.len() <= SPRITE_HEADER_SIZE + table_size {
            return Err("Not enough data".into());
        }

        let table = &data[SPRITE_HEADER_SIZE..(SPRITE_HEADER_SIZE + table_size)];

        let compressed_data = &data[SPRITE_HEADER_SIZE + table_size..];
        let uncompressed_data = crate::assets::zlib::decompress(compressed_data, compressed_size, uncompressed_size)?;

        let processed_data = process_sprite_data(uncompressed_data.as_slice(), table, width, height)?;

        Ok(Self {
            height,
            width,
            data: processed_data,
            palette: palette.data,
            transparency: true,
        })
    }
}

impl Image {
    /// Decode a sprite using a specific palette ID instead of the one in the sprite header.
    /// Used for monster variant palette swaps (e.g., GoblinB uses pal226 instead of pal225).
    pub fn try_from_with_palette(
        data: &[u8],
        palettes: &Palettes,
        override_palette_id: u16,
    ) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(12))?;

        let compressed_size = cursor.read_u32::<LittleEndian>()? as usize;
        let width = cursor.read_u16::<LittleEndian>()? as usize;
        let height = cursor.read_u16::<LittleEndian>()? as usize;

        let _original_palette_id = cursor.read_u16::<LittleEndian>()?;
        // Use the override palette instead of the sprite's embedded one
        let palette = palettes
            .get(override_palette_id)
            .ok_or_else(|| format!("Palette {} not found", override_palette_id))?;

        cursor.seek(std::io::SeekFrom::Current(6))?;
        let uncompressed_size = cursor.read_u32::<LittleEndian>()? as usize;

        let table_size: usize = height * 8;
        if data.len() <= SPRITE_HEADER_SIZE + table_size {
            return Err("Not enough data".into());
        }

        let table = &data[SPRITE_HEADER_SIZE..(SPRITE_HEADER_SIZE + table_size)];
        let compressed_data = &data[SPRITE_HEADER_SIZE + table_size..];
        let uncompressed_data = crate::assets::zlib::decompress(compressed_data, compressed_size, uncompressed_size)?;
        let processed_data = process_sprite_data(uncompressed_data.as_slice(), table, width, height)?;

        Ok(Self {
            height,
            width,
            data: processed_data,
            palette: palette.data,
            transparency: true,
        })
    }
}

/// Extract the palette_id from raw sprite header bytes (offset 20, u16 LE).
/// Returns None if data is too short to contain the field.
pub fn sprite_palette_id(data: &[u8]) -> Option<u16> {
    if data.len() < 22 {
        return None;
    }
    Some(u16::from_le_bytes([data[20], data[21]]))
}

fn process_sprite_data(data: &[u8], table: &[u8], width: usize, height: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let img_size = width * height;
    let mut img: Vec<u8> = vec![0; img_size];
    let mut current: usize = 0;
    let mut cursor = Cursor::new(table);

    for _ in 0..height {
        let start = cursor.read_i16::<LittleEndian>()?;
        let end = cursor.read_i16::<LittleEndian>()?;
        let offset = cursor.read_u32::<LittleEndian>()? as usize;

        if start < 0 || end < 0 {
            current += width;
            continue;
        }

        if end < start {
            current += width;
            continue;
        }
        let start = start as usize;
        let chunk_size = (end - start as i16 + 1) as usize;
        if current + start + chunk_size > img.len() || offset + chunk_size > data.len() {
            current += width;
            continue;
        }
        current += start;
        img[current..current + chunk_size].copy_from_slice(&data[offset..offset + chunk_size]);
        current += width - start;
    }
    Ok(img)
}

impl Image {
    pub fn to_image_buffer(&self) -> Result<DynamicImage, Box<dyn Error>> {
        let image = raw_to_image_buffer(
            &self.data,
            &self.palette,
            |index, pixel: &[u8; 3]| {
                if self.transparency && index == self.data[0] {
                    Rgba([0, 0, 0, 0])
                } else {
                    Rgba([pixel[0], pixel[1], pixel[2], 255])
                }
            },
            self.width as u32,
            self.height as u32,
        )?;
        Ok(DynamicImage::ImageRgba8(image))
    }

    #[allow(dead_code)]
    pub fn save<Q>(&self, path: Q) -> Result<(), Box<dyn Error>>
    where
        Q: AsRef<Path>,
    {
        self.to_image_buffer()?
            .save_with_format(path, image::ImageFormat::Png)?;
        Ok(())
    }
}

/// Converts the image into a versatile generic image buffer.
/// The image contains more pixels than needed with dimensions (h*w) to account for mipmaps,
/// but we are currently not utilizing those extra pixels.
/// # Panics
/// if the input accesses outside the bounds of the palette.
fn raw_to_image_buffer<P>(
    data: &[u8],
    palette: &[u8; 768],
    pixel_converter: impl Fn(u8, &[u8; 3]) -> P,
    width: u32,
    height: u32,
) -> Result<ImageBuffer<P, Vec<P::Subpixel>>, Box<dyn Error>>
where
    P: image::Pixel<Subpixel = u8> + 'static,
{
    let mut image_buffer = ImageBuffer::<P, Vec<P::Subpixel>>::new(width, height);
    let pixel_count = (width * height) as usize;
    if data.len() < pixel_count {
        return Err(format!("data too short: {} < {}", data.len(), pixel_count).into());
    }

    for (i, pi) in data[..pixel_count].iter().enumerate() {
        let x = (i as u32).rem_euclid(width);
        let y = (i as u32).div_euclid(width);
        let index = 3 * (*pi as usize);
        let pixel = pixel_converter(*pi, &palette[index..index + 3].try_into()?);
        image_buffer.put_pixel(x, y, pixel);
    }
    Ok(image_buffer)
}

/// Padding (in pixels) added around each tile in the atlas.
///
/// Each tile gets a 1-pixel border of replicated edge pixels on all four sides.
/// This prevents linear filtering from bleeding into adjacent tiles at tile UV
/// boundaries, without requiring a UV inset that cuts into tile content.
/// `tile_uvs` in `odm.rs` must use the same constant to compute correct UVs.
pub const ATLAS_TILE_PAD: u32 = 1;

fn join_images_in_grid(
    images: &[DynamicImage],
    grid_width: usize,
    image_width: u32,
    image_height: u32,
) -> DynamicImage {
    let num_images = images.len();
    if num_images == 0 {
        panic!("No images provided.");
    }

    let pad = ATLAS_TILE_PAD;
    let slot_w = image_width + 2 * pad;
    let slot_h = image_height + 2 * pad;

    let combined_width = slot_w * grid_width as u32;
    let combined_height = slot_h * ((num_images as f32 / grid_width as f32).ceil() as u32);

    let mut combined_image = ImageBuffer::new(combined_width, combined_height);

    for (i, image) in images.iter().enumerate() {
        let col = (i % grid_width) as u32;
        let row = (i / grid_width) as u32;
        let x_offset = col * slot_w;
        let y_offset = row * slot_h;

        // Fill the full slot (content + padding) by clamping source coords.
        // Pixels outside [0, image_width) / [0, image_height) clamp to the
        // nearest edge pixel, replicating the tile edge into the border.
        for sy in 0..slot_h {
            for sx in 0..slot_w {
                let src_x = (sx as i32 - pad as i32).clamp(0, image_width as i32 - 1) as u32;
                let src_y = (sy as i32 - pad as i32).clamp(0, image_height as i32 - 1) as u32;
                let pixel = image.get_pixel(src_x, src_y);
                combined_image.put_pixel(x_offset + sx, y_offset + sy, pixel);
            }
        }
    }
    DynamicImage::ImageRgba8(combined_image)
}

pub fn get_atlas(assets: &Assets, names: &[&str], row_size: usize) -> Result<DynamicImage, Box<dyn Error>> {
    let mut images: Vec<DynamicImage> = Vec::with_capacity(names.len());

    for name in names {
        // Full water tiles ("wtrtyl"): replace with solid cyan so the
        // shader can detect them and render animated water at runtime.
        // Transition tiles ("wtrdr*") already contain cyan marker pixels.
        if *name == "wtrtyl" {
            let mut cyan = ImageBuffer::new(128, 128);
            for pixel in cyan.pixels_mut() {
                *pixel = Rgba([0, 255, 255, 255]);
            }
            images.push(DynamicImage::ImageRgba8(cyan));
            continue;
        }

        let mut image = assets.lod().bitmap(name).ok_or("image not found")?;
        if image.dimensions() != (128, 128) {
            image = DynamicImage::ImageRgba8(imageops::resize(&image, 128, 128, imageops::FilterType::Triangle));
        }
        images.push(image);
    }
    Ok(join_images_in_grid(&images, row_size, 128, 128))
}

/// Extract a water mask from a terrain atlas and clean the cyan markers.
///
/// Scans the atlas for cyan marker pixels (R<26, G>230, B>230) and produces:
/// - A grayscale mask image (same dimensions) where white=water, black=terrain
/// - The atlas is modified in-place: cyan pixels are replaced with a dark
///   water-like color so they don't bleed when the atlas uses linear filtering.
pub fn extract_water_mask(atlas: &mut DynamicImage) -> DynamicImage {
    let (w, h) = atlas.dimensions();
    let rgba = atlas.as_mut_rgba8().expect("atlas must be RGBA8");
    let mut mask = ImageBuffer::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let p = rgba.get_pixel(x, y);
            let is_cyan = p[0] < 26 && p[1] > 230 && p[2] > 230;
            if is_cyan {
                // Mark as water in the mask
                mask.put_pixel(x, y, image::Luma([255u8]));
                // Replace cyan with a dark blue-green so linear filtering
                // doesn't bleed bright cyan into neighboring terrain.
                rgba.put_pixel(x, y, Rgba([15, 40, 50, 255]));
            } else {
                mask.put_pixel(x, y, image::Luma([0u8]));
            }
        }
    }

    DynamicImage::ImageLuma8(mask)
}

/// Tint an RGBA image for monster difficulty variants.
///
/// MM6 monsters come in A/B/C variants with different color tints.
/// `variant` selects the tint: 2 = blue (B variant), 3 = red (C variant).
/// Variant 1 (A) or 0 leaves the image unchanged.
///
/// Uses RGB channel mixing to produce visible color shifts while preserving
/// skin tones on humanoid sprites.
pub fn tint_variant(image: &mut DynamicImage, variant: u8) {
    if variant <= 1 {
        return;
    }
    let Some(rgba) = image.as_mut_rgba8() else {
        return;
    };

    // Channel mixing matrix — visible but not garish.
    let (rr, rg, rb, gr, gg, gb, br, bg, bb) = if variant == 2 {
        // Blue tint: strong blue, muted red/green
        (0.35f32, 0.1, 0.15, 0.1, 0.45, 0.25, 0.2, 0.25, 0.9)
    } else {
        // Red tint: strong red, muted blue/green
        (0.9f32, 0.25, 0.15, 0.2, 0.45, 0.1, 0.15, 0.1, 0.35)
    };

    for pixel in rgba.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        if a == 0 {
            continue;
        }

        let rf = r as f32;
        let gf = g as f32;
        let bf = b as f32;

        let nr = (rf * rr + gf * rg + bf * rb).min(255.0) as u8;
        let ng = (rf * gr + gf * gg + bf * gb).min(255.0) as u8;
        let nb = (rf * br + gf * bg + bf * bb).min(255.0) as u8;
        *pixel = Rgba([nr, ng, nb, a]);
    }
}

#[cfg(test)]
mod test {
    use super::{ATLAS_TILE_PAD, get_atlas, sprite_palette_id};
    use crate::assets::test_lod;
    use image::GenericImageView;

    #[test]
    fn sprite_palette_id_short_data_returns_none() {
        assert!(sprite_palette_id(&[]).is_none());
        assert!(sprite_palette_id(&[0u8; 21]).is_none());
    }

    #[test]
    fn sprite_palette_id_reads_offset_20_le() {
        let mut data = [0u8; 22];
        data[20] = 0x2A; // 42 low byte
        data[21] = 0x00; // high byte
        assert_eq!(sprite_palette_id(&data), Some(42));
    }

    #[test]
    fn sprite_palette_id_exact_boundary() {
        // exactly 22 bytes: should succeed
        let data = [0u8; 22];
        assert_eq!(sprite_palette_id(&data), Some(0));
    }

    #[test]
    fn join_images() {
        let Some(assets) = test_lod() else {
            return;
        };

        let atlas_image = get_atlas(&assets, &["grastyl", "dirttyl", "voltyl", "wtrtyl", "pending"][..], 2).unwrap();
        let slot = 128 + 2 * ATLAS_TILE_PAD;
        assert_eq!(atlas_image.dimensions(), (slot * 2, slot * 3));
    }
}
