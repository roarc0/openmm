use bevy::prelude::*;

use crate::assets::{self, GameAssets};
use crate::config::GameConfig;

/// Load the overview image for `map_name` (e.g. `"oute3"`).
/// Returns `None` for indoor maps or if no icon is found in the LOD.
pub(crate) fn load_map_overview(
    map_name: &str,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
) -> Option<Handle<Image>> {
    let img = game_assets.lod().icon(map_name)?;
    let mut bevy_img = assets::dynamic_to_bevy_image(img);
    bevy_img.sampler = super::hud_sampler(cfg);
    Some(images.add(bevy_img))
}

/// Make green or red color-keyed pixels transparent for tap frame overlays.
pub(crate) fn make_tap_key_transparent(img: &mut image::DynamicImage) {
    super::make_transparent_where(img, |r, g, b| {
        let is_green = g > r && g > b && g >= 128 && r < 100 && b < 100;
        let is_red = r > g && r > b && r >= 128 && g < 100 && b < 100;
        is_green || is_red
    });
}
