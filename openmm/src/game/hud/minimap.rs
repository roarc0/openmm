use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::assets::{self, GameAssets};
use crate::config::GameConfig;
use crate::game::player::Player;
use crate::ui_assets::UiAssets;

use super::borders::{hud_dimensions, logical_size};

#[derive(Component)]
pub(super) struct HudMinimapClip;
#[derive(Component)]
pub(super) struct HudMinimapImage;
#[derive(Component)]
pub(super) struct HudMinimapArrow;
#[derive(Component)]
pub(super) struct HudCompassClip;
#[derive(Component)]
pub(super) struct HudCompassStrip;

/// Cached minimap direction arrow handles (N, NE, E, SE, S, SW, W, NW).
#[derive(Resource)]
pub(super) struct MinimapArrows(pub Vec<Handle<Image>>);

/// Cached tap frame handles (tap1=morning, tap2=day, tap3=evening, tap4=night).
#[derive(Resource)]
pub(super) struct TapFrames(pub Vec<Handle<Image>>);

/// Load the map overview image for the minimap.
pub(super) fn load_map_overview(
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
) -> Option<Handle<Image>> {
    // Try loading the current map's overview (e.g., "oute3")
    // For now, try common map names
    let map_names = ["oute3", "oute2", "oute1", "outa1"];
    for name in map_names {
        if let Some(img) = game_assets.game_lod().icon(name) {
            let mut bevy_img = assets::dynamic_to_bevy_image(img);
            bevy_img.sampler = crate::ui_assets::hud_sampler(cfg);
            return Some(images.add(bevy_img));
        }
    }
    None
}

/// Make green or red color-keyed pixels transparent for tap frame overlays.
pub(super) fn make_tap_key_transparent(img: &mut image::DynamicImage) {
    crate::ui_assets::make_transparent_where(img, |r, g, b| {
        let is_green = g > r && g > b && g >= 128 && r < 100 && b < 100;
        let is_red = r > g && r > b && r >= 128 && g < 100 && b < 100;
        is_green || is_red
    });
}

/// Update minimap image position, arrow direction, and compass strip based on player transform.
pub(super) fn update_minimap(
    ui_assets: Res<UiAssets>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cfg: Res<GameConfig>,
    player_q: Query<&Transform, With<Player>>,
    mut minimap_q: Query<&mut Node, (With<HudMinimapImage>, Without<HudCompassStrip>)>,
    arrows_res: Option<Res<MinimapArrows>>,
    mut arrow_q: Query<&mut ImageNode, With<HudMinimapArrow>>,
    mut compass_q: Query<&mut Node, (With<HudCompassStrip>, Without<HudMinimapImage>)>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let (lw, lh) = logical_size(&window, &cfg);
    let d = hud_dimensions(lw, lh, &ui_assets);

    // Map overview is 512x512 pixels covering the 128x128 tile terrain.
    // Terrain world coords are centered: -half..+half on both axes.
    // Bevy X = right (east), Bevy Z = -MM6_Y (south).
    // Map image: left=west, right=east, top=north, bottom=south.
    use lod::odm::ODM_TILE_SCALE;
    let terrain_size = 128.0 * ODM_TILE_SCALE; // 65536 world units
    let half = terrain_size / 2.0;

    // Player normalized 0..1 on the map (X -> horizontal, Z -> vertical)
    // Bevy Z maps directly to image Y: -half (north) -> 0 (top), +half (south) -> 1 (bottom)
    let nx = (player_tf.translation.x + half) / terrain_size;
    let nz = (player_tf.translation.z + half) / terrain_size;

    // Scale the map image to show reasonable zoom (2x = each pixel covers ~2 tiles)
    let zoom = 3.0;
    let map_img_size = d.tap_w * zoom;

    // Offset so player is at the center of the clip container
    let img_left = d.tap_w / 2.0 - nx * map_img_size;
    let img_top = d.tap_h / 2.0 - nz * map_img_size;

    for mut node in minimap_q.iter_mut() {
        node.left = Val::Px(img_left);
        node.top = Val::Px(img_top);
        node.width = Val::Px(map_img_size);
        node.height = Val::Px(map_img_size);
    }

    let (yaw, _, _) = player_tf.rotation.to_euler(EulerRot::YXZ);
    // Convert yaw to clockwise angle from north (0..TAU)
    let cw_angle = (-yaw).rem_euclid(std::f32::consts::TAU);

    // Update arrow direction based on player yaw
    // mapdir assets are counterclockwise: 1=NE, 2=N, 3=NW, 4=W, 5=SW, 6=S, 7=SE, 8=E
    // sector is clockwise: 0=N, 1=NE, 2=E, 3=SE, 4=S, 5=SW, 6=W, 7=NW
    if let Some(ref arrows) = arrows_res {
        if arrows.0.len() == 8 {
            let sector = ((cw_angle / (std::f32::consts::TAU / 8.0) + 0.5) as usize) % 8;
            // Map clockwise sector to counterclockwise index
            let idx = (9 - sector) % 8;
            for mut arrow_img in arrow_q.iter_mut() {
                arrow_img.image = arrows.0[idx].clone();
            }
        }
    }

    // Update compass strip scroll
    // Strip layout: E(9) . SE(36) . S(68) . SW(98) . W(130) . NW(157) . N(190) . NE(216) . E(249) . SE(276)
    // First E at pixel ~14 (tweaked), second E at ~254 -> cycle = 240px = 360 degrees
    // angle_from_east wraps via rem_euclid so pixel_pos stays in [14, 254] -- within the 325px strip
    let compass_full_w = d.compass_w;
    let compass_strip_h = d.compass_h;
    let compass_clip_w = d.scale_w(38.0);
    let e_start = d.scale_w(28.0);
    let cycle_w = d.scale_w(240.0);
    let angle_from_east = (cw_angle - std::f32::consts::FRAC_PI_2)
        .rem_euclid(std::f32::consts::TAU);
    let pixel_pos = e_start + (angle_from_east / std::f32::consts::TAU) * cycle_w;
    for mut node in compass_q.iter_mut() {
        node.left = Val::Px(compass_clip_w / 2.0 - pixel_pos);
        node.width = Val::Px(compass_full_w);
        node.height = Val::Px(compass_strip_h);
    }
}
