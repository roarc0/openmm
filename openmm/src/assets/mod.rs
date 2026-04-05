use std::error::Error;

use bevy::prelude::*;
use lod::LodManager;

/// Wraps LodManager with global game data loaded once at startup.
#[derive(Resource)]
pub struct GameAssets {
    lod_manager: LodManager,
    /// Global map-independent data: DSFT, MonsterList, MapStats, StreetNpcs.
    /// Passed to Actors::new, Monsters::new, etc. to avoid per-call LOD reads.
    game_data: lod::game::global::GameData,
    /// Billboard manager (DDecList + DSFT) for decoration sprite lookups.
    billboard_manager: lod::billboard::BillboardManager,
    /// QBit ID → human-readable label for debug logging.
    quest_bits: lod::quest_bits::QuestBitNames,
}

impl GameAssets {
    pub fn new(path: std::path::PathBuf) -> Result<Self, Box<dyn Error>> {
        let lod_manager = LodManager::new(path)?;
        let game_data = lod::game::global::GameData::load(&lod_manager)?;
        let billboard_manager = lod::billboard::BillboardManager::load(&lod_manager)?;
        let quest_bits = lod::quest_bits::QuestBitNames::load(&lod_manager)?;
        Ok(Self {
            lod_manager,
            game_data,
            billboard_manager,
            quest_bits,
        })
    }

    pub fn lod_manager(&self) -> &LodManager {
        &self.lod_manager
    }

    pub fn game_data(&self) -> &lod::game::global::GameData {
        &self.game_data
    }

    pub fn billboard_manager(&self) -> &lod::billboard::BillboardManager {
        &self.billboard_manager
    }

    pub fn quest_bits(&self) -> &lod::quest_bits::QuestBitNames {
        &self.quest_bits
    }

    /// Game-engine API: decoded, game-ready assets (sprites, bitmaps, icons, fonts, NPC tables).
    pub fn game_lod(&self) -> lod::game::GameLod<'_> {
        self.lod_manager.game()
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
