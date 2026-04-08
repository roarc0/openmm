use std::collections::HashSet;

use bevy::asset::AssetId;
use bevy::prelude::*;

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::sprites::{Billboard, SelfLit};
use crate::game::world::GameTime;
use crate::game::outdoor::TerrainMaterial;
use crate::game::player::Player;
use crate::game::sprites::material::SpriteMaterial;

// ── Decoration point lights ─────────────────────────────────────────────────

/// Pre-scale applied to DSFT light_radius before [`decoration_point_light`] for animated
/// decorations (campfires, braziers).
/// campfireon: lr=256 × 8 = 2048 → range=4096, intensity=838M.
/// Keep range below indoor fog end (~2000) so the light cluster doesn't cover the whole dungeon.
pub const DSFT_ANIMATED_LR_SCALE: u16 = 8;

/// Pre-scale for static DSFT decorations (crystals, chandeliers, sconces).
pub const DSFT_STATIC_LR_SCALE: u16 = 6;

/// Build a `PointLight` for a decoration with the given MM6 light radius.
///
/// MM6 `light_radius` values (256–512) were calibrated for the original software renderer
/// and map to small Bevy world-unit spheres without scaling. We decouple range from intensity:
/// - `range  = light_radius * RANGE_SCALE` — controls how far the light reaches.
/// - `intensity = light_radius² * 200`    — controls brightness; tied to the original radius,
///   NOT the scaled range, so doubling the range doesn't quadruple brightness.
///
/// RANGE_SCALE=2: torch (lr=512) → range=1024, campfire (DSFT lr=256×8=2048) → range=4096.
/// Keep RANGE_SCALE small — Bevy clusters every light by its range sphere; a light with
/// range=40960 in a 22000-unit dungeon touches every cluster and tanks frame time.
pub fn decoration_point_light(light_radius: u16) -> impl Bundle {
    const RANGE_SCALE: f32 = 2.0;
    let lr = light_radius as f32;
    PointLight {
        color: Color::srgb(1.0, 0.78, 0.40),
        intensity: lr * lr * 200.0,
        range: lr * RANGE_SCALE,
        shadows_enabled: false,
        ..default()
    }
}

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightingState>()
            .init_resource::<CurrentSpriteTint>()
            .add_systems(OnEnter(GameState::Game), sun_setup)
            .add_systems(Update, animate_day_cycle.run_if(in_state(GameState::Game)));
    }
}

/// The tint currently applied to all billboard/actor materials (linear RGBA).
/// Exposed so other systems (e.g. event dispatch) can immediately apply the correct
/// tint to any newly created or swapped material without waiting for the next lighting tick.
#[derive(Resource)]
pub struct CurrentSpriteTint {
    pub tint: Vec4,
    pub selflit_tint: Vec4,
}

impl Default for CurrentSpriteTint {
    fn default() -> Self {
        Self {
            tint: Vec4::ONE,
            selflit_tint: Vec4::ONE,
        }
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
pub fn sprite_tint_from_time(tod: f32) -> Color {
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
    mut sprite_materials: ResMut<Assets<SpriteMaterial>>,
    mut terrain_materials: Option<ResMut<Assets<TerrainMaterial>>>,
    mut current_tint: ResMut<CurrentSpriteTint>,
    indoor: Option<Res<crate::states::loading::PreparedIndoorWorld>>,
    // Non-billboard (terrain, BSP models) — toggled between lit/unlit on mode change.
    model_query: Query<&MeshMaterial3d<StandardMaterial>, Without<Billboard>>,
    // Billboard decorations — tinted per-frame (except SelfLit ones like torches/campfires).
    billboard_query: Query<&MeshMaterial3d<SpriteMaterial>, (With<Billboard>, Without<SelfLit>)>,
    // All SpriteSheet actors (NPCs/monsters): tint ALL frames/directions, not just the active one.
    // Querying only the active MeshMaterial3d misses other frames → they flash when animation advances.
    actor_sheets: Query<&crate::game::sprites::loading::SpriteSheet, Without<SelfLit>>,
    // SelfLit sprites (campfires, torches, braziers): get a very subtle tint so they don't
    // feel disconnected from the scene, but remain mostly full-bright as light sources.
    selflit_sheets: Query<&crate::game::sprites::loading::SpriteSheet, With<SelfLit>>,
    selflit_billboard_query: Query<
        &MeshMaterial3d<SpriteMaterial>,
        (
            With<SelfLit>,
            With<Billboard>,
            Without<crate::game::sprites::loading::SpriteSheet>,
        ),
    >,
    mut sun_query: Query<(&mut Transform, &mut DirectionalLight), Without<Player>>,
    mut ambient_query: Query<&mut AmbientLight, With<AmbientMarker>>,
    player_query: Query<&Transform, With<Player>>,
) {
    let is_indoor = indoor.is_some();
    // ── Lighting mode switch ───────────────────────────────────────────────────
    // Sync lit/unlit toggle for model materials when the mode changes.
    // Billboards are always unlit — their day/night effect comes from base_color tinting.
    if cfg.lighting != lighting_state.last_mode {
        lighting_state.last_mode = cfg.lighting.clone();
        // Force sprite tint re-apply after mode change.
        lighting_state.last_tint = None;
        let unlit = cfg.lighting != "enhanced";

        // Indoor walls must stay PBR regardless of mode — they need to respond
        // to the party torch and decoration point lights.
        if !is_indoor {
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

            if let Some(tm) = terrain_materials.as_mut() {
                for (_, mat) in tm.iter_mut() {
                    mat.base.unlit = unlit;
                    if unlit {
                        mat.base.base_color = Color::srgb(0.69, 0.69, 0.69);
                    } else {
                        mat.base.base_color = Color::srgb(1.2, 1.2, 1.2);
                    }
                }
            }
        }

        info!("Lighting mode: {}", cfg.lighting);
    }

    let tod = game_time.time_of_day();

    // ── Sun and ambient ────────────────────────────────────────────────────────
    // 1 real second = 1 game minute. Update sun/ambient at most once per ~2 game seconds
    // (threshold 0.0014 ≈ 2/1440) to avoid marking DirectionalLight changed every frame,
    // which triggers shadow re-renders.
    const SUN_TOD_THRESHOLD: f32 = 0.0014;
    let sun_needs_update =
        (tod - lighting_state.last_sun_tod).abs() > SUN_TOD_THRESHOLD || lighting_state.last_sun_tod == 0.0;

    if sun_needs_update {
        lighting_state.last_sun_tod = tod;

        let (new_transform, color, illuminance) = sun_from_time(tod);
        for (mut transform, mut light) in sun_query.iter_mut() {
            *transform = new_transform;
            light.color = color;
            // Disable the directional sun indoors — dungeon lighting comes entirely
            // from the party torch (PointLight) and decoration lights.
            light.illuminance = if is_indoor {
                0.0
            } else if cfg.lighting == "enhanced" {
                illuminance * 1.06
            } else {
                0.0
            };
        }
    }

    if is_indoor {
        // Set ambient based on the sector the player is currently in.
        // min_ambient_light (0–255) is the original MM6 floor for that room.
        // Scale: 0 → 0 lux, 255 → ~200 lux. Typical dark dungeon rooms have ~19,
        // giving ~15 lux — just enough to prevent absolute black in far corners.
        let indoor_data = indoor.as_ref().unwrap();
        let ambient_brightness = if let Ok(player_tf) = player_query.single() {
            let pos = player_tf.translation;
            let sector = indoor_data
                .sector_ambients
                .iter()
                .find(|s| {
                    pos.x >= s.bbox_min.x
                        && pos.x <= s.bbox_max.x
                        && pos.y >= s.bbox_min.y
                        && pos.y <= s.bbox_max.y
                        && pos.z >= s.bbox_min.z
                        && pos.z <= s.bbox_max.z
                })
                .or_else(|| indoor_data.sector_ambients.first());
            sector.map_or(25.0, |s| s.min_ambient as f32 * 0.8 + 25.0)
        } else {
            0.0
        };
        for mut ambient in ambient_query.iter_mut() {
            ambient.color = Color::srgb(0.85, 0.80, 0.70); // warm stone
            ambient.brightness = ambient_brightness;
        }
    } else if cfg.lighting == "enhanced" {
        if sun_needs_update {
            let (ambient_color, ambient_brightness) = ambient_from_time(tod);
            for mut ambient in ambient_query.iter_mut() {
                ambient.color = ambient_color;
                ambient.brightness = ambient_brightness * 3.3;
            }
        }
    } else {
        for mut ambient in ambient_query.iter_mut() {
            ambient.color = Color::WHITE;
            ambient.brightness = 3000.0;
        }
    }

    // ── Sprite tint ───────────────────────────────────────────────────────────
    // Sprites are unlit to avoid directional-light flicker.
    // Outdoors: tint follows time of day. Indoors: fixed dim torch-warm tint so
    // sprites look naturally lit by the surrounding dungeon ambiance.
    //
    // Performance: `get_mut` on a material marks it as changed → GPU re-upload.
    // We cache the last applied tint and skip all material writes if the tint
    // hasn't changed by a perceptible amount (threshold ≈ 1/512 ≈ ~3 real seconds).
    let tint = if is_indoor {
        Color::srgb(0.35, 0.30, 0.22)
    } else {
        sprite_tint_from_time(tod)
    };
    let t = tint.to_srgba();
    let tint_rgb = [t.red, t.green, t.blue];

    const TINT_THRESHOLD: f32 = 1.0 / 512.0;
    let tint_changed = lighting_state.last_tint.is_none_or(|last| {
        (last[0] - tint_rgb[0]).abs() > TINT_THRESHOLD
            || (last[1] - tint_rgb[1]).abs() > TINT_THRESHOLD
            || (last[2] - tint_rgb[2]).abs() > TINT_THRESHOLD
    });

    if tint_changed {
        lighting_state.last_tint = Some(tint_rgb);
        lighting_state.tinted.clear();

        // Convert sRGB tint to linear for the shader uniform.
        let tl = tint.to_linear();
        let tint_vec4 = Vec4::new(tl.red, tl.green, tl.blue, 1.0);
        current_tint.tint = tint_vec4;

        // Static billboard decorations (single material per entity).
        for mat_handle in billboard_query.iter() {
            if lighting_state.tinted.insert(mat_handle.id())
                && let Some(mat) = sprite_materials.get_mut(mat_handle.id())
            {
                mat.extension.tint = tint_vec4;
            }
        }

        // Actor/NPC SpriteSheets: tint ALL frames × directions, not just the active handle.
        // Updating only the active MeshMaterial3d causes the other frames to flash full-bright
        // when the animation advances to an un-tinted frame.
        for sheet in actor_sheets.iter() {
            for state in &sheet.states {
                for frame in state {
                    for handle in frame {
                        if lighting_state.tinted.insert(handle.id())
                            && let Some(mat) = sprite_materials.get_mut(handle.id())
                        {
                            mat.extension.tint = tint_vec4;
                        }
                    }
                }
            }
        }

        // SelfLit sprites (campfires, torches) blend a small fraction of the ambient tint
        // so they feel grounded in the scene rather than floating at pure full-bright.
        const SELFLIT_TINT_BLEND: f32 = 0.12;
        let selflit_tint = Color::srgb(
            1.0 - (1.0 - t.red) * SELFLIT_TINT_BLEND,
            1.0 - (1.0 - t.green) * SELFLIT_TINT_BLEND,
            1.0 - (1.0 - t.blue) * SELFLIT_TINT_BLEND,
        );
        let stl = selflit_tint.to_linear();
        let selflit_vec4 = Vec4::new(stl.red, stl.green, stl.blue, 1.0);
        current_tint.selflit_tint = selflit_vec4;
        for mat_handle in selflit_billboard_query.iter() {
            if lighting_state.tinted.insert(mat_handle.id())
                && let Some(mat) = sprite_materials.get_mut(mat_handle.id())
            {
                mat.extension.tint = selflit_vec4;
            }
        }
        for sheet in selflit_sheets.iter() {
            for state in &sheet.states {
                for frame in state {
                    for handle in frame {
                        if lighting_state.tinted.insert(handle.id())
                            && let Some(mat) = sprite_materials.get_mut(handle.id())
                        {
                            mat.extension.tint = selflit_vec4;
                        }
                    }
                }
            }
        }
    }
}

/// Tracks applied lighting state to detect changes.
#[derive(Resource, Default)]
struct LightingState {
    last_mode: String,
    /// Last tod at which sun/ambient were updated. Skip update below this threshold.
    last_sun_tod: f32,
    /// Last tint applied to billboard/actor materials (rgb). `None` = never applied → force update.
    last_tint: Option<[f32; 3]>,
    /// Reused every frame to deduplicate material updates (avoids per-frame allocation).
    tinted: HashSet<AssetId<SpriteMaterial>>,
}
