use std::error::Error;

use bevy::prelude::*;
use lod::LodManager;

/// Wraps LodManager with caching for expensive decoded assets.
#[derive(Resource)]
pub struct GameAssets {
    lod_manager: LodManager,
}

impl GameAssets {
    pub fn new(path: std::path::PathBuf) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            lod_manager: LodManager::new(path)?,
        })
    }

    pub fn lod_manager(&self) -> &LodManager {
        &self.lod_manager
    }
}

// ── Shared image/sampler helpers ────────────────────────────

/// Convert an `image::DynamicImage` into a Bevy `Image` suitable for rendering.
pub fn dynamic_to_bevy_image(img: image::DynamicImage) -> Image {
    Image::from_dynamic(img, true, bevy::asset::RenderAssetUsages::RENDER_WORLD)
}

/// Image sampler that repeats in both axes with linear filtering.
pub fn repeat_linear_sampler() -> bevy::image::ImageSampler {
    bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        min_filter: bevy::image::ImageFilterMode::Linear,
        mag_filter: bevy::image::ImageFilterMode::Linear,
        ..default()
    })
}

/// Image sampler that repeats in both axes with default (linear) filtering.
pub fn repeat_sampler() -> bevy::image::ImageSampler {
    bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        ..default()
    })
}

/// Select sampler based on filtering mode string ("nearest" or "linear").
pub fn sampler_for_filtering(mode: &str) -> bevy::image::ImageSampler {
    if mode == "nearest" { nearest_sampler() } else { repeat_linear_sampler() }
}

/// Image sampler using nearest-neighbor filtering (no interpolation).
pub fn nearest_sampler() -> bevy::image::ImageSampler {
    bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        min_filter: bevy::image::ImageFilterMode::Nearest,
        mag_filter: bevy::image::ImageFilterMode::Nearest,
        mipmap_filter: bevy::image::ImageFilterMode::Nearest,
        ..default()
    })
}
