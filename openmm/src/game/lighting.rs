use bevy::prelude::*;

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::entities::Billboard;
use crate::game::game_time::GameTime;
use crate::game::terrain_material::TerrainMaterial;

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightingState>()
            .add_systems(OnEnter(GameState::Game), sun_setup)
            .add_systems(Update, animate_day_cycle.run_if(in_state(GameState::Game)));
    }
}

#[derive(Component)]
struct AmbientMarker;

fn sun_setup(mut commands: Commands, cfg: Res<GameConfig>, game_time: Res<GameTime>) {
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.85, 0.85, 0.95),
            brightness: 2500.0,
            ..default()
        },
        AmbientMarker,
        InGame,
    ));

    let tod = game_time.time_of_day();
    let (dir_transform, color, illuminance) = sun_from_time(tod);

    commands.spawn((
        Name::new("sun"),
        DirectionalLight {
            shadows_enabled: cfg.shadows,
            illuminance,
            color,
            ..default()
        },
        dir_transform,
        InGame,
    ));
}

/// Compute sun transform, color, and illuminance from time of day [0, 1].
fn sun_from_time(tod: f32) -> (Transform, Color, f32) {
    // Sun arc: rises at tod=0.25 (6am), sets at tod=0.75 (6pm).
    let sun_progress = ((tod - 0.25) / 0.5).clamp(0.0, 1.0);
    let angle = sun_progress * std::f32::consts::PI;

    let radius = 50000.0;
    let x = angle.cos() * radius;
    let y = angle.sin() * radius;
    let transform = Transform::from_xyz(x, y, 0.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Elevation: 0 at horizon, 1 at zenith
    let elevation = angle.sin().max(0.0);

    // Warm orange at horizon → white at noon
    let r = 1.0_f32;
    let g = 0.75 + 0.25 * elevation;
    let b = 0.55 + 0.45 * elevation;
    let color = Color::srgb(r, g, b);

    // Illuminance in lux. 0 at night; peaks at noon.
    let is_day = tod > 0.22 && tod < 0.78;
    let illuminance = if is_day { 300.0 + 900.0 * elevation } else { 0.0 };

    (transform, color, illuminance)
}

/// Compute ambient light color and brightness from time of day [0, 1].
fn ambient_from_time(tod: f32) -> (Color, f32) {
    let day_amount = 1.0_f32 - (tod * 2.0 - 1.0).abs();
    let dawn_dusk: f32 = {
        let d1 = (tod - 0.25).abs();
        let d2 = (tod - 0.75).abs();
        (1.0 - (d1.min(d2) * 10.0).min(1.0)).max(0.0)
    };

    let r = 0.15 + 0.65 * day_amount + 0.20 * dawn_dusk;
    let g = 0.15 + 0.60 * day_amount + 0.10 * dawn_dusk;
    let b = 0.25 + 0.55 * day_amount - 0.10 * dawn_dusk;
    let brightness = 1500.0 + 2500.0 * day_amount;

    (
        Color::srgb(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)),
        brightness,
    )
}

/// Tint applied to unlit billboard materials to simulate ambient light variation.
///
/// Sprites stay `unlit: true` to avoid directional-light flicker (billboard normals
/// always face the camera, so dot(normal, sun) varies wildly with camera yaw).
/// Instead we multiply `base_color` by this tint each frame in enhanced mode.
/// Night floor: dark blue moonlight. Noon: white (no change to texture color).
fn sprite_tint_from_time(tod: f32) -> Color {
    let day_amount = (1.0_f32 - (tod * 2.0 - 1.0).abs()).max(0.0);
    let dawn_dusk: f32 = {
        let d1 = (tod - 0.25).abs();
        let d2 = (tod - 0.75).abs();
        (1.0 - (d1.min(d2) * 10.0).min(1.0)).max(0.0)
    };

    // Night floor (0.05, 0.05, 0.10) → noon white (0.95, 0.95, 0.95)
    let r = (0.05 + 0.90 * day_amount + 0.05 * dawn_dusk).clamp(0.0, 1.0);
    let g = (0.05 + 0.90 * day_amount).clamp(0.0, 1.0);
    let b = (0.10 + 0.85 * day_amount - 0.05 * dawn_dusk).clamp(0.0, 1.0);

    Color::srgb(r, g, b)
}

fn animate_day_cycle(
    game_time: Res<GameTime>,
    cfg: Res<GameConfig>,
    mut lighting_state: ResMut<LightingState>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    mut terrain_materials: ResMut<Assets<TerrainMaterial>>,
    // Non-billboard (terrain, BSP models) — toggled between lit/unlit on mode change.
    model_query: Query<&MeshMaterial3d<StandardMaterial>, Without<Billboard>>,
    // Billboard decorations — always unlit; tinted per-frame.
    billboard_query: Query<&MeshMaterial3d<StandardMaterial>, With<Billboard>>,
    // Actor sprites (NPCs/monsters) — SpriteSheet, no Billboard marker.
    actor_query: Query<&MeshMaterial3d<StandardMaterial>, With<crate::game::entities::sprites::SpriteSheet>>,
    mut sun_query: Query<(&mut Transform, &mut DirectionalLight)>,
    mut ambient_query: Query<&mut AmbientLight, With<AmbientMarker>>,
) {
    // ── Lighting mode switch ───────────────────────────────────────────────────
    // Sync lit/unlit toggle for model materials when the mode changes.
    // Billboards are always unlit — their day/night effect comes from base_color tinting.
    if cfg.lighting != lighting_state.last_mode {
        lighting_state.last_mode = cfg.lighting.clone();
        let unlit = cfg.lighting != "enhanced";

        let mut toggled = std::collections::HashSet::new();
        for mat_handle in model_query.iter() {
            if toggled.insert(mat_handle.id())
                && let Some(mat) = std_materials.get_mut(mat_handle.id())
            {
                mat.unlit = unlit;
                if unlit {
                    mat.base_color = Color::srgb(0.69, 0.69, 0.69);
                } else {
                    mat.base_color = Color::srgb(1.4, 1.4, 1.4);
                }
            }
        }

        for (_, mat) in terrain_materials.iter_mut() {
            mat.base.unlit = unlit;
            if unlit {
                mat.base.base_color = Color::srgb(0.69, 0.69, 0.69);
            } else {
                mat.base.base_color = Color::srgb(1.2, 1.2, 1.2);
            }
        }

        info!("Lighting mode: {}", cfg.lighting);
    }

    let tod = game_time.time_of_day();

    // ── Sun and ambient ────────────────────────────────────────────────────────
    let (new_transform, color, illuminance) = sun_from_time(tod);

    for (mut transform, mut light) in sun_query.iter_mut() {
        *transform = new_transform;
        light.color = color;
        light.illuminance = if cfg.lighting == "enhanced" {
            illuminance * 1.06
        } else {
            0.0
        };
    }

    if cfg.lighting == "enhanced" {
        let (ambient_color, ambient_brightness) = ambient_from_time(tod);
        for mut ambient in ambient_query.iter_mut() {
            ambient.color = ambient_color;
            ambient.brightness = ambient_brightness * 3.3;
        }
    } else {
        for mut ambient in ambient_query.iter_mut() {
            ambient.color = Color::WHITE;
            ambient.brightness = 3000.0;
        }
    }

    // ── Sprite tint ───────────────────────────────────────────────────────────
    // Sprites are always unlit to avoid directional-light flicker. Apply a
    // base_color tint derived from time of day so they darken at night.
    let tint = sprite_tint_from_time(tod);
    let mut tinted = std::collections::HashSet::new();
    for mat_handle in billboard_query.iter().chain(actor_query.iter()) {
        if tinted.insert(mat_handle.id())
            && let Some(mat) = std_materials.get_mut(mat_handle.id())
        {
            mat.base_color = tint;
        }
    }
}

/// Tracks applied lighting state to detect changes.
#[derive(Resource, Default)]
struct LightingState {
    last_mode: String,
}
