use std::error::Error;

use bevy::prelude::*;
use openmm_data::Assets;

/// Wraps Assets Hub with global game data loaded once at startup.
#[derive(Resource)]
pub struct GameAssets {
    assets: Assets,
}

impl GameAssets {
    pub fn new(path: std::path::PathBuf) -> Result<Self, Box<dyn Error>> {
        let assets = Assets::new(&*path.to_string_lossy())?;
        Ok(Self { assets })
    }

    pub fn assets(&self) -> &Assets {
        &self.assets
    }

    pub fn data(&self) -> &openmm_data::GameData {
        self.assets.data()
    }

    pub fn quests(&self) -> &openmm_data::quests::QuestNames {
        &self.assets.data().quests
    }

    pub fn autonotes(&self) -> Option<&openmm_data::autonotes::AutonotesTable> {
        self.assets.data().autonotes_table.as_ref()
    }

    pub fn npcbtb(&self) -> Option<&openmm_data::npcbtb::NpcBtbTable> {
        self.assets.data().npcbtb_table.as_ref()
    }

    pub fn npctext(&self) -> Option<&openmm_data::npctext::NpcTextTable> {
        self.assets.data().npctext_table.as_ref()
    }

    pub fn npctopic(&self) -> Option<&openmm_data::npctopic::NpcTopicTable> {
        self.assets.data().npctopic_table.as_ref()
    }

    pub fn proftext(&self) -> Option<&openmm_data::proftext::ProfTextTable> {
        self.assets.data().proftext_table.as_ref()
    }

    pub fn scroll(&self) -> Option<&openmm_data::scroll::ScrollTable> {
        self.assets.data().scroll_table.as_ref()
    }

    pub fn trans(&self) -> Option<&openmm_data::trans::TransTable> {
        self.assets.data().trans_table.as_ref()
    }

    pub fn passwords(&self) -> Option<&openmm_data::passwords::PasswordsTable> {
        self.assets.data().passwords_table.as_ref()
    }

    pub fn merchant(&self) -> Option<&openmm_data::merchant::MerchantTable> {
        self.assets.data().merchant_table.as_ref()
    }

    /// Game-engine API: decoded, game-ready assets (sprites, bitmaps, icons, fonts, NPC tables).
    pub fn lod(&self) -> openmm_data::assets::LodDecoder<'_> {
        self.assets.lod()
    }

    /// Load a LOD icon by name as a nearest-neighbor Bevy Image handle.
    /// Returns `None` if the icon is not found.
    pub fn load_icon(&self, name: &str, images: &mut bevy::asset::Assets<Image>) -> Option<bevy::asset::Handle<Image>> {
        let img = self.lod().icon(name)?;
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

/// Convert a raw RGBA8 buffer into a Bevy `Image`. Shortcut for the
/// `dynamic_to_bevy_image(DynamicImage::ImageRgba8(rgba))` pattern used at
/// every billboard / sprite spawn site.
pub fn rgba8_to_bevy_image(rgba: image::RgbaImage) -> Image {
    dynamic_to_bevy_image(image::DynamicImage::ImageRgba8(rgba))
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
