use bevy::prelude::*;
use image::{DynamicImage, GenericImageView};
use std::collections::HashMap;
use std::sync::Arc;

use crate::assets::{self, GameAssets};
use crate::game::sprites::loading::AlphaMask;
use crate::system::config::GameConfig;

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
    /// CPU-side alpha masks for pixel-perfect hit testing.
    masks: HashMap<String, Arc<AlphaMask>>,
}

impl UiAssets {
    /// Drop all cached UI handles and dimensions.
    ///
    /// Used by the optional screen editor when restarting its canvas context.
    #[cfg(feature = "editor")]
    pub fn clear_cache(&mut self) {
        self.textures.clear();
        self.dimensions.clear();
        self.masks.clear();
    }

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
            if images.get(handle).is_some() {
                return Some(handle.clone());
            }
            // Stale cache entry: handle no longer exists in Assets<Image>.
            self.textures.remove(name);
        }
        let img = game_assets.lod().icon(name)?;
        let (w, h) = img.dimensions();
        let mask = Arc::new(AlphaMask::from_image(&img.to_rgba8()));

        let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
        bevy_img.sampler = hud_sampler(cfg);
        let handle = images.add(bevy_img);
        self.textures.insert(name.to_string(), handle.clone());
        self.dimensions.insert(name.to_string(), (w, h));
        self.masks.insert(name.to_string(), mask);
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
            if images.get(handle).is_some() {
                return Some(handle.clone());
            }
            // Stale cache entry: transformed image was dropped from asset storage.
            self.textures.remove(cache_key);
        }
        let mut img = game_assets.lod().icon(name)?;
        let (w, h) = img.dimensions();
        transform(&mut img);

        let mask = Arc::new(AlphaMask::from_image(&img.to_rgba8()));

        let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
        bevy_img.sampler = hud_sampler(cfg);
        let handle = images.add(bevy_img);
        self.textures.insert(cache_key.to_string(), handle.clone());
        self.dimensions.insert(cache_key.to_string(), (w, h));
        self.masks.insert(cache_key.to_string(), mask);
        self.dimensions.entry(name.to_string()).or_insert((w, h));
        Some(handle)
    }

    /// Load a save file screenshot (`image.pcx`) into a cached handle.
    pub fn get_or_load_screenshot(
        &mut self,
        slot_name: &str,
        images: &mut Assets<Image>,
        cfg: &GameConfig,
    ) -> Option<Handle<Image>> {
        let cache_key = format!("saveslot:preview:{}", slot_name);
        if let Some(handle) = self.textures.get(&cache_key) {
            if images.get(handle).is_some() {
                return Some(handle.clone());
            }
            self.textures.remove(&cache_key);
        }

        let path = crate::game::save::slots::slot_path(slot_name);
        let save = openmm_data::save::file::SaveFile::open(path).ok()?;
        let img = save.screenshot()?;
        let (w, h) = img.dimensions();

        let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
        bevy_img.sampler = hud_sampler(cfg);
        let handle = images.add(bevy_img);
        self.textures.insert(cache_key, handle.clone());
        self.dimensions.insert(slot_name.to_string(), (w, h));
        Some(handle)
    }

    /// Get the original pixel dimensions of a loaded asset.
    pub fn dimensions(&self, name: &str) -> Option<(u32, u32)> {
        self.dimensions.get(name).copied()
    }

    /// Get the alpha mask for a loaded asset by its name or cache key.
    pub fn mask(&self, key: &str) -> Option<Arc<AlphaMask>> {
        self.masks.get(key).cloned()
    }
}

/// Make black (or near-black) pixels fully transparent.
/// Useful for UI sprites with solid black backgrounds (e.g. minimap arrows).
pub fn make_black_transparent(img: &mut DynamicImage) {
    make_transparent_where(img, |r, g, b| r < 8 && g < 8 && b < 8);
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

/// Load the overview image for `map_name` (e.g. `"oute3"`).
/// Returns `None` for indoor maps or if no icon is found in the LOD.
pub fn load_map_overview(
    map_name: &str,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
) -> Option<Handle<Image>> {
    let img = game_assets.lod().icon(map_name)?;
    let mut bevy_img = assets::dynamic_to_bevy_image(img);
    bevy_img.sampler = hud_sampler(cfg);
    Some(images.add(bevy_img))
}

/// Make green or red color-keyed pixels transparent for tap frame overlays.
/// MM6 uses exact #00FF00 (green) and #FF0000 (red) as transparency keys.
pub fn make_tap_key_transparent(img: &mut image::DynamicImage) {
    make_transparent_where(img, |r, g, b| {
        (r == 0 && g == 255 && b == 0) || (r == 255 && g == 0 && b == 0)
    });
}
