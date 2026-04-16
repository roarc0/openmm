//! Data-driven screen bindings: components spawned from RON `bindings` field,
//! systems that update them each frame based on game state (player position,
//! yaw, current map, etc.).
//!
//! The runtime spawns the components; this module owns the update logic.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use super::REF_H;
use super::REF_W;
use super::ui_assets::UiAssets;
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::player::Player;

// ── Components ─────────────────────────────────────────────────────────────

/// Inner image inside a crop viewport — scrollable within its clip container.
#[derive(Component)]
pub struct CroppedImage {
    pub crop_w: f32,
    pub crop_h: f32,
}

/// Compass strip — scrolls horizontally by player yaw.
#[derive(Component)]
pub struct CompassBinding;

/// Minimap — scrolls by player world position, texture auto-loaded from map.
#[derive(Component)]
pub struct MinimapBinding {
    pub zoom: f32,
}

/// Direction arrow — swaps mapdir1-8 texture by player yaw.
#[derive(Component)]
pub struct ArrowBinding;

/// Tap frame — swaps tap1-4 texture by time of day.
#[derive(Component)]
pub struct TapBinding;

/// Cached arrow texture handles (mapdir1-8 with black transparency).
#[derive(Resource)]
struct ArrowHandles(Vec<Handle<Image>>);

/// Cached tap frame handles (tap1-4 with green/red transparency).
#[derive(Resource)]
struct TapHandles(Vec<Handle<Image>>);

/// Loading animation frame — attached to overlay sprites in loading.ron.
/// The `frame` number (1-based) determines when this sprite becomes visible
/// during the loading sequence.
#[derive(Component)]
pub struct LoadingFrameBinding {
    pub frame: u32,
}

/// Tracks the loading animation timeline independently from the actual loading
/// pipeline. Each frame stays visible for at least `MIN_FRAME_SECS`.
#[derive(Resource)]
struct LoadingAnimState {
    /// Current animation frame (0 = no overlays, 1..=5 = progressive reveals).
    current_frame: u32,
    /// Elapsed time in the current frame.
    elapsed: f32,
}

const MIN_FRAME_SECS: f32 = 0.3;
const TOTAL_FRAMES: u32 = 5;

// ── Plugin ─────────────────────────────────────────────────────────────────

pub struct BindingsPlugin;

impl Plugin for BindingsPlugin {
    fn build(&self, app: &mut App) {
        use crate::GameState;
        app.add_systems(
            Update,
            (compass_scroll, minimap_scroll, arrow_update, tap_update)
                .run_if(in_state(GameState::Menu).or(in_state(GameState::Game))),
        )
        .add_systems(Update, loading_anim_update.run_if(in_state(GameState::Loading)));
    }
}

// ── Compass ────────────────────────────────────────────────────────────────

/// Scroll the compass strip based on player yaw.
/// Strip layout: E(9)..SE(36)..S(68)..SW(98)..W(130)..NW(157)..N(190)..NE(216)..E(249)..SE(276)
/// First E at pixel ~28, cycle = 240px = 360 degrees.
fn compass_scroll(
    player_q: Query<&Transform, With<Player>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut strip_q: Query<(&CroppedImage, &mut Node), With<CompassBinding>>,
) {
    let Ok(player_tf) = player_q.single() else { return };
    let Ok(window) = windows.single() else { return };

    let (yaw, _, _) = player_tf.rotation.to_euler(EulerRot::YXZ);
    let cw_angle = (-yaw).rem_euclid(std::f32::consts::TAU);

    let sx = window.width() / REF_W;

    let e_start = 28.0 * sx;
    let cycle_w = 240.0 * sx;
    let angle_from_east = (cw_angle - std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::TAU);
    let pixel_pos = e_start + (angle_from_east / std::f32::consts::TAU) * cycle_w;

    for (crop, mut node) in &mut strip_q {
        let clip_w = crop.crop_w * sx;
        // Round to nearest pixel to avoid floating-point jitter triggering UI layout every frame.
        let target = Val::Px((clip_w / 2.0 - pixel_pos).round());
        if node.left != target {
            node.left = target;
        }
    }
}

// ── Minimap ────────────────────────────────────────────────────────────────

/// Load minimap texture and scroll based on player position.
fn minimap_scroll(
    player_q: Query<&Transform, With<Player>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    world_state: Option<Res<crate::game::world::WorldState>>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
    mut query: Query<(&CroppedImage, &MinimapBinding, &mut Node, &mut ImageNode)>,
    mut last_map: Local<String>,
    mut map_handle: Local<Option<Handle<Image>>>,
) {
    let Ok(window) = windows.single() else { return };
    let sx = window.width() / REF_W;
    let sy = window.height() / REF_H;

    // Resolve map overview texture (changes when map changes).
    let map_name = world_state
        .as_ref()
        .map(|ws| match &ws.map.name {
            openmm_data::utils::MapName::Outdoor(odm) => odm.base_name(),
            openmm_data::utils::MapName::Indoor(_) => String::new(),
        })
        .unwrap_or_default();

    if map_name != *last_map {
        *last_map = map_name.clone();
        *map_handle = if map_name.is_empty() {
            None
        } else {
            super::ui_assets::load_map_overview(&map_name, &game_assets, &mut images, &cfg)
        };
    }

    let Some(handle) = map_handle.as_ref() else {
        // Indoor or no map — hide the minimap.
        let offscreen = Val::Px(-9999.0);
        for (_, _, mut node, _) in &mut query {
            if node.left != offscreen {
                node.left = offscreen;
            }
        }
        return;
    };

    let Ok(player_tf) = player_q.single() else { return };

    use openmm_data::odm::ODM_TILE_SCALE;
    let terrain_size = 128.0 * ODM_TILE_SCALE;
    let half = terrain_size / 2.0;
    let nx = (player_tf.translation.x + half) / terrain_size;
    let nz = (player_tf.translation.z + half) / terrain_size;

    for (crop, minimap, mut node, mut img_node) in &mut query {
        let crop_w_px = crop.crop_w * sx;
        let crop_h_px = crop.crop_h * sy;
        // Square map image sized to crop width (matches original HUD behavior).
        let map_img_size = crop_w_px * minimap.zoom;

        // Center player in viewport, offset to match TAP transparent window.
        // Round to avoid subpixel jitter triggering UI layout every frame.
        let offset_x = 3.0 * sx;
        let offset_y = 20.0 * sy;
        let target_left = Val::Px((crop_w_px / 2.0 + offset_x - nx * map_img_size).round());
        let target_top = Val::Px((crop_h_px / 2.0 + offset_y - nz * map_img_size).round());
        let target_w = Val::Px(map_img_size);
        let target_h = Val::Px(map_img_size);

        if node.left != target_left {
            node.left = target_left;
        }
        if node.top != target_top {
            node.top = target_top;
        }
        if node.width != target_w {
            node.width = target_w;
        }
        if node.height != target_h {
            node.height = target_h;
        }
        if img_node.image != *handle {
            img_node.image = handle.clone();
        }
    }
}

// ── Arrow ──────────────────────────────────────────────────────────────────

/// Swap arrow texture based on player yaw direction.
/// mapdir assets: 1=NE, 2=N, 3=NW, 4=W, 5=SW, 6=S, 7=SE, 8=E (counterclockwise)
fn arrow_update(
    player_q: Query<&Transform, With<Player>>,
    mut query: Query<&mut ImageNode, With<ArrowBinding>>,
    arrow_handles: Option<Res<ArrowHandles>>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
    mut commands: Commands,
    mut initialized: Local<bool>,
) {
    // Load arrow textures once (mapdir1-8 with black transparency).
    if !*initialized {
        *initialized = true;
        let make_black_transparent = super::ui_assets::make_black_transparent;
        let handles: Vec<Handle<Image>> = (1..=8)
            .filter_map(|i| {
                let name = format!("mapdir{}", i);
                let key = format!("{}_transparent", name);
                ui_assets.get_or_load_transformed(&name, &key, &game_assets, &mut images, &cfg, make_black_transparent)
            })
            .collect();
        if handles.len() == 8 {
            commands.insert_resource(ArrowHandles(handles));
        }
        return;
    }

    let Some(arrows) = arrow_handles else { return };
    if arrows.0.len() != 8 {
        return;
    }
    let Ok(player_tf) = player_q.single() else { return };

    let (yaw, _, _) = player_tf.rotation.to_euler(EulerRot::YXZ);
    let cw_angle = (-yaw).rem_euclid(std::f32::consts::TAU);

    // Map clockwise sector (0=N,1=NE,2=E...) to counterclockwise mapdir index
    let sector = ((cw_angle / (std::f32::consts::TAU / 8.0) + 0.5) as usize) % 8;
    let idx = (9 - sector) % 8;

    let target = &arrows.0[idx];
    for mut img in &mut query {
        if img.image != *target {
            img.image = target.clone();
        }
    }
}

// ── Tap frame ──────────────────────────────────────────────────────────────

/// Swap tap frame texture based on time of day.
/// tap1=morning (6am-noon), tap2=day (noon-6pm), tap3=evening (6pm-midnight), tap4=night (midnight-6am).
fn tap_update(
    mut query: Query<&mut ImageNode, With<TapBinding>>,
    tap_handles: Option<Res<TapHandles>>,
    game_time: Option<Res<crate::game::world::time::GameTime>>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
    mut commands: Commands,
    mut initialized: Local<bool>,
) {
    if !*initialized {
        *initialized = true;
        let handles: Vec<Handle<Image>> = (1..=4)
            .filter_map(|i| {
                let name = format!("tap{}", i);
                let key = format!("{}_transparent", name);
                ui_assets.get_or_load_transformed(
                    &name,
                    &key,
                    &game_assets,
                    &mut images,
                    &cfg,
                    super::ui_assets::make_tap_key_transparent,
                )
            })
            .collect();
        if handles.len() == 4 {
            commands.insert_resource(TapHandles(handles));
        }
        return;
    }

    let Some(taps) = tap_handles else { return };
    if taps.0.len() != 4 {
        return;
    }

    // time_of_day: 0.0=midnight, 0.25=6am, 0.5=noon, 0.75=6pm
    let tod = game_time.as_ref().map(|gt| gt.time_of_day()).unwrap_or(0.35);
    let idx = if tod < 0.25 {
        3 // night (tap4)
    } else if tod < 0.5 {
        0 // morning (tap1)
    } else if tod < 0.75 {
        1 // day (tap2)
    } else {
        2 // evening (tap3)
    };

    let target = &taps.0[idx];
    for mut img in &mut query {
        if img.image != *target {
            img.image = target.clone();
        }
    }
}

// ── Loading animation ─────────────────────────────────────────────────────

/// Advances the loading animation based on LoadingStep progress and wall-clock
/// time. Each overlay frame stays visible for at least MIN_FRAME_SECS. The
/// animation plays once (no cycling). Frame visibility is cumulative: once a
/// frame is shown, it stays visible.
fn loading_anim_update(
    time: Res<Time>,
    loading_step: Option<Res<crate::states::loading::LoadingStep>>,
    mut anim: Local<Option<LoadingAnimState>>,
    mut query: Query<(&LoadingFrameBinding, &mut Visibility)>,
) {
    let Some(step) = loading_step else { return };

    // Init anim state on first frame.
    let state = anim.get_or_insert_with(|| LoadingAnimState {
        current_frame: 0,
        elapsed: 0.0,
    });

    state.elapsed += time.delta_secs();

    // Target frame: map loading step index to animation frame.
    // Steps 0-5 map to frames 0-5, but we cap at TOTAL_FRAMES.
    // Frame 0 = just the background (no overlays).
    let target_frame = (step.index() as u32).min(TOTAL_FRAMES);

    // Advance one frame at a time, respecting min duration.
    if state.current_frame < target_frame && state.elapsed >= MIN_FRAME_SECS {
        state.current_frame += 1;
        state.elapsed = 0.0;
    }

    // Show all frames up to and including current_frame (cumulative reveal).
    for (binding, mut vis) in &mut query {
        let should_show = binding.frame <= state.current_frame;
        vis.set_if_neq(if should_show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}
