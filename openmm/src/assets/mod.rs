use std::error::Error;

use bevy::prelude::*;
use openmm_data::Assets;

/// Wraps Assets Hub with global game data loaded once at startup.
#[derive(Resource)]
pub struct GameAssets {
    assets: Assets,
    /// Billboard manager (DDecList + DSFT) for decoration sprite lookups.
    billboard_manager: openmm_data::billboard::BillboardManager,
    /// QBit ID → human-readable label for debug logging.
    quest_bits: openmm_data::quest_bits::QuestBitNames,
}

impl GameAssets {
    pub fn new(path: std::path::PathBuf) -> Result<Self, Box<dyn Error>> {
        let assets = Assets::new(&*path.to_string_lossy())?;
        // GameData is now inside Assets and lazy-loaded via assets.data()
        let billboard_manager = openmm_data::billboard::BillboardManager::load(&assets)?;
        let quest_bits = openmm_data::quest_bits::QuestBitNames::load(&assets)?;
        Ok(Self {
            assets,
            billboard_manager,
            quest_bits,
        })
    }

    pub fn assets(&self) -> &Assets {
        &self.assets
    }

    pub fn game_data(&self) -> &openmm_data::GameData {
        self.assets.data()
    }

    pub fn billboard_manager(&self) -> &openmm_data::billboard::BillboardManager {
        &self.billboard_manager
    }

    pub fn quest_bits(&self) -> &openmm_data::quest_bits::QuestBitNames {
        &self.quest_bits
    }

    /// Game-engine API: decoded, game-ready assets (sprites, bitmaps, icons, fonts, NPC tables).
    pub fn game_lod(&self) -> openmm_data::assets::LodDecoder<'_> {
        self.assets.game()
    }

    /// Compatibility method for old code expecting lod_manager.
    pub fn lod_manager(&self) -> &Assets {
        &self.assets
    }

    /// Load a LOD icon by name as a nearest-neighbor Bevy Image handle.
    /// Returns `None` if the icon is not found.
    pub fn load_icon(&self, name: &str, images: &mut bevy::asset::Assets<Image>) -> Option<bevy::asset::Handle<Image>> {
        let img = self.game_lod().icon(name)?;
        let mut bevy_img = dynamic_to_bevy_image(img);
        bevy_img.sampler = bevy::image::ImageSampler::nearest();
        Some(images.add(bevy_img))
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
/// Always uses Repeat address mode so tiling textures work correctly.
pub fn sampler_for_filtering(mode: &str) -> bevy::image::ImageSampler {
    if mode == "nearest" {
        repeat_nearest_sampler()
    } else {
        repeat_linear_sampler()
    }
}

/// Image sampler using nearest-neighbor filtering with Repeat address mode.
pub fn repeat_nearest_sampler() -> bevy::image::ImageSampler {
    bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        address_mode_u: bevy::image::ImageAddressMode::Repeat,
        address_mode_v: bevy::image::ImageAddressMode::Repeat,
        min_filter: bevy::image::ImageFilterMode::Nearest,
        mag_filter: bevy::image::ImageFilterMode::Nearest,
        mipmap_filter: bevy::image::ImageFilterMode::Nearest,
        ..default()
    })
}

/// Image sampler using nearest-neighbor filtering (no interpolation, no repeat).
/// Use only for non-tiling textures (UI sprites, overlays, etc.).
pub fn nearest_sampler() -> bevy::image::ImageSampler {
    bevy::image::ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
        min_filter: bevy::image::ImageFilterMode::Nearest,
        mag_filter: bevy::image::ImageFilterMode::Nearest,
        mipmap_filter: bevy::image::ImageFilterMode::Nearest,
        ..default()
    })
}
