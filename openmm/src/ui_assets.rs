use bevy::prelude::*;
use image::{DynamicImage, GenericImageView};
use std::collections::HashMap;

use crate::assets::GameAssets;
use crate::config::GameConfig;

/// Resolve the image sampler from the `hud_filtering` config value.
///
/// - `"nearest"` — crisp pixel art, no interpolation (default)
/// - `"linear"` — bilinear filtering, smooth upscaling
pub fn hud_sampler(cfg: &GameConfig) -> bevy::image::ImageSampler {
    match cfg.hud_filtering.as_str() {
        "linear" => bevy::image::ImageSampler::linear(),
        _ => bevy::image::ImageSampler::nearest(),
    }
}

/// Cached UI texture handles and their original pixel dimensions.
#[derive(Resource, Default)]
pub struct UiAssets {
    textures: HashMap<String, Handle<Image>>,
    /// Original pixel dimensions (width, height) of each loaded asset.
    dimensions: HashMap<String, (u32, u32)>,
}

impl UiAssets {
    /// Load a UI texture by name from the LOD icons archive.
    /// Handles both PCX and custom bitmap formats.
    /// Caches the result — subsequent calls return the cached handle.
    pub fn get_or_load(
        &mut self,
        name: &str,
        game_assets: &GameAssets,
        images: &mut Assets<Image>,
        cfg: &GameConfig,
    ) -> Option<Handle<Image>> {
        if let Some(handle) = self.textures.get(name) {
            return Some(handle.clone());
        }
        let img = game_assets.lod_manager().icon(name)?;
        let (w, h) = img.dimensions();
        let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
        bevy_img.sampler = hud_sampler(cfg);
        let handle = images.add(bevy_img);
        self.textures.insert(name.to_string(), handle.clone());
        self.dimensions.insert(name.to_string(), (w, h));
        Some(handle)
    }

    /// Load a UI texture with a transform applied before caching.
    /// `cache_key` is the key used for deduplication (e.g. "mapdir1_transparent").
    /// The transform runs once; subsequent calls with the same key return the cached handle.
    pub fn get_or_load_transformed(
        &mut self,
        name: &str,
        cache_key: &str,
        game_assets: &GameAssets,
        images: &mut Assets<Image>,
        cfg: &GameConfig,
        transform: impl FnOnce(&mut DynamicImage),
    ) -> Option<Handle<Image>> {
        if let Some(handle) = self.textures.get(cache_key) {
            return Some(handle.clone());
        }
        let mut img = game_assets.lod_manager().icon(name)?;
        let (w, h) = img.dimensions();
        transform(&mut img);
        let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
        bevy_img.sampler = hud_sampler(cfg);
        let handle = images.add(bevy_img);
        self.textures.insert(cache_key.to_string(), handle.clone());
        self.dimensions.insert(name.to_string(), (w, h));
        Some(handle)
    }

    /// Get the original pixel dimensions of a loaded asset.
    pub fn dimensions(&self, name: &str) -> Option<(u32, u32)> {
        self.dimensions.get(name).copied()
    }
}

/// Make black (or near-black) pixels fully transparent.
/// Useful for UI sprites with solid black backgrounds (e.g. minimap arrows).
pub fn make_black_transparent(img: &mut DynamicImage) {
    make_transparent_where(img, |r, g, b| r < 30 && g < 30 && b < 30);
}

/// Make pixels matching a color-key predicate fully transparent.
/// The predicate receives (r, g, b) and returns true for pixels that should become transparent.
pub fn make_transparent_where(img: &mut DynamicImage, is_key: impl Fn(u8, u8, u8) -> bool) {
    let rgba = img.to_rgba8();
    let mut buf = rgba.into_raw();
    for chunk in buf.chunks_exact_mut(4) {
        if is_key(chunk[0], chunk[1], chunk[2]) {
            chunk[0] = 0;
            chunk[1] = 0;
            chunk[2] = 0;
            chunk[3] = 0;
        }
    }
    let (w, h) = img.dimensions();
    *img = DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w, h, buf).unwrap());
}
