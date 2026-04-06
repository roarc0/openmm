//! MM6 bitmap font loading and Bevy text rendering.
//!
//! Loads all .fnt files from the LOD icons archive at startup and provides
//! helper functions to render text as Bevy `Image` / `ImageNode` UI elements.
//!
//! # Usage
//!
//! ```ignore
//! // Render a string to a Bevy Image handle:
//! let handle = game_fonts.render("Hello", "arrus", WHITE, &mut images);
//!
//! // Spawn as a UI image node:
//! commands.spawn(ImageNode::new(handle));
//!
//! // Measure text width without rendering:
//! let px = game_fonts.measure("Gold: 500", "smallnum");
//! ```

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::assets::GameAssets;

/// White text color (opaque).
pub const WHITE: [u8; 4] = [255, 255, 255, 255];
/// Yellow/gold text color.
pub const YELLOW: [u8; 4] = [255, 225, 80, 255];
/// Red text color (damage, warnings).
pub const RED: [u8; 4] = [255, 60, 60, 255];
/// Green text color (positive effects).
pub const GREEN: [u8; 4] = [80, 255, 80, 255];

/// All MM6 bitmap fonts loaded from the LOD archive.
#[derive(Resource)]
pub struct GameFonts {
    fonts: HashMap<String, openmm_data::assets::Font>,
}

impl GameFonts {
    /// Load all .fnt files from the LOD icons archive.
    pub fn load(game_assets: &GameAssets) -> Self {
        let lod = game_assets.lod();
        let mut fonts = HashMap::new();
        for name in lod.font_names() {
            let fnt_file = format!("{}.fnt", name);
            if let Some(font) = lod.font(&fnt_file) {
                info!(
                    "Loaded font '{}' (height={}, chars={}..{})",
                    name, font.height, font.first_char, font.last_char
                );
                fonts.insert(name, font);
            }
        }
        Self { fonts }
    }

    /// Get a font by name (e.g. "arrus", "smallnum", "book").
    pub fn get(&self, name: &str) -> Option<&openmm_data::assets::Font> {
        self.fonts.get(name)
    }

    /// Measure the pixel width of a text string using the named font.
    /// Returns 0 if the font is not found.
    pub fn measure(&self, text: &str, font_name: &str) -> i32 {
        self.fonts.get(font_name).map(|f| f.measure(text)).unwrap_or(0)
    }

    /// Render text to a Bevy `Image` handle using the named font.
    ///
    /// `color` is an RGBA array (e.g. `fonts::WHITE`).
    /// Returns `None` if the font is not found or the text is empty.
    pub fn render(
        &self,
        text: &str,
        font_name: &str,
        color: [u8; 4],
        images: &mut Assets<Image>,
    ) -> Option<Handle<Image>> {
        let font = self.fonts.get(font_name)?;
        if text.is_empty() {
            return None;
        }
        let (w, h, rgba) = font.render_text(text, color);
        if w == 0 || h == 0 {
            return None;
        }
        let mut image = Image::new(
            Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        );
        // Font text uses nearest filtering (always crisp, not affected by hud_filtering)
        image.sampler = bevy::image::ImageSampler::nearest();
        Some(images.add(image))
    }

    /// List all loaded font names.
    pub fn names(&self) -> Vec<&str> {
        self.fonts.keys().map(|s| s.as_str()).collect()
    }
}
