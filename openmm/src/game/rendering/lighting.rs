use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::world::ui_state::{UiMode, UiState};
use crate::game::outdoor::TerrainMaterial;
use crate::game::player::Player;
use crate::game::sprites::Billboard;
use crate::game::sprites::tint_buffer::SpriteTintBuffers;
use crate::game::world::{CurrentMap, GameTime, is_outdoor};

// ── Decoration point lights ─────────────────────────────────────────────────

/// Pre-scales applied to DSFT `light_radius` before building a `PointLight`.
/// campfireon: lr=256 × 8 = 2048 → range=4096, intensity=838M.
/// Keep range below indoor fog end (~2000) so the light cluster doesn't cover the whole dungeon.
const DSFT_ANIMATED_LR_SCALE: u16 = 8;
/// Static DSFT decorations (crystals, chandeliers, sconces) — slightly smaller scale.
const DSFT_STATIC_LR_SCALE: u16 = 6;

/// Source of the `light_radius` value used to build a decoration `PointLight`.
/// Baked into the helper so callers don't need to know about DSFT scale factors.
pub enum DecorationLight {
    /// `light_radius` comes directly from `DDeclist` (torches, signs) — no pre-scale.
    Ddeclist(u16),
    /// `light_radius` comes from an animated DSFT frame (campfireon, brazier).
    AnimatedDsft(u16),
    /// `light_radius` comes from a luminous static DSFT frame.
    StaticDsft(u16),
}

/// Build a `PointLight` bundle for a decoration.
///
/// MM6 `light_radius` values (256–512) were calibrated for the original software renderer
/// and map to small Bevy world-unit spheres without scaling. We decouple range from intensity:
/// - `range  = light_radius * RANGE_SCALE` — controls how far the light reaches.
/// - `intensity = light_radius² * 200`    — brightness, tied to the original radius so
///   doubling the range doesn't quadruple brightness.
///
/// RANGE_SCALE=2: torch (lr=512) → range=1024, campfire (DSFT lr=256×8=2048) → range=4096.
/// Keep RANGE_SCALE small — Bevy clusters every light by its range sphere; a light with
/// range=40960 in a 22000-unit dungeon touches every cluster and tanks frame time.
///
/// Decoration point lights never cast shadows. Each shadow-casting point light
/// cube-maps the scene 6× per frame — with dozens of torches, braziers, and
/// campfires on a typical map, enabling them tanks frame rate from 120 to 40 fps.
/// The directional sun handles outdoor shadows; decorations just need local glow.
pub fn decoration_point_light(source: DecorationLight, _shadows: bool) -> impl Bundle {
    const RANGE_SCALE: f32 = 2.0;
    let radius = match source {
        DecorationLight::Ddeclist(lr) => lr,
        DecorationLight::AnimatedDsft(lr) => lr.saturating_mul(DSFT_ANIMATED_LR_SCALE),
        DecorationLight::StaticDsft(lr) => lr.saturating_mul(DSFT_STATIC_LR_SCALE),
    };
    let lr = radius as f32;
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
            .add_systems(OnEnter(GameState::Game), (ambient_setup, sun_setup.run_if(is_outdoor)))
            // Gate on UiMode::World so day/night/ambient advance freezes in
            // dialogues, inventory, and overlays — otherwise GameTime pauses
            // but this system would still tick sun/ambient from stale values.
            .add_systems(
                Update,
                animate_day_cycle
                    .run_if(in_state(GameState::Game))
                    .run_if(|ui: Res<UiState>| ui.mode == UiMode::World),
            );
    }
}

#[derive(Component)]
struct AmbientMarker;

/// Spawn the AmbientLight entity. Reused by both indoor and outdoor maps —
/// `animate_day_cycle` sets its colour and brightness from sector data (indoor)
/// or time of day (outdoor) each frame.
fn ambient_setup(mut commands: Commands) {
    commands.spawn((
        AmbientLight {
            color: Color::srgb(0.85, 0.85, 0.95),
            brightness: 2500.0,
            ..default()
        },
        AmbientMarker,
        InGame,
    ));
}

/// Spawn the directional sun light. Outdoor only — gated by a `run_if` on
/// `PreparedIndoorWorld` absence. Indoor dungeons are lit entirely by the
/// party torch and decoration point lights, so skipping the sun entity avoids
/// running the shadow cascade pass on a light with 0 illuminance.
fn sun_setup(mut commands: Commands, cfg: Res<GameConfig>, game_time: Res<GameTime>) {
    let tod = game_time.time_of_day();
    let (dir_transform, color, illuminance) = sun_from_time(tod);

    let mut sun = commands.spawn((
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
    if cfg.shadows {
        sun.insert(
            CascadeShadowConfigBuilder {
                maximum_distance: 10000.0,
                first_cascade_far_bound: 50.0,
                overlap_proportion: 0.2,
                ..default()
            }
            .build(),
        );
    }
}

/// World-space direction from origin toward the sun for a given time of day.
/// Sun rises at tod=0.25 (6am, +X horizon), sets at tod=0.75 (6pm, -X horizon).
/// Shared between the directional light setup and the sky shader so the
/// visible disc stays locked to the shadow direction.
pub fn sun_direction_from_time(tod: f32) -> Vec3 {
    let sun_progress = ((tod - 0.25) / 0.5).clamp(0.0, 1.0);
    let angle = sun_progress * std::f32::consts::PI;
    Vec3::new(angle.cos(), angle.sin(), 0.0)
}

/// Compute sun transform, color, and illuminance from time of day [0, 1].
fn sun_from_time(tod: f32) -> (Transform, Color, f32) {
    let dir = sun_direction_from_time(tod);
    let radius = 50000.0;
    let transform = Transform::from_translation(dir * radius).looking_at(Vec3::ZERO, Vec3::Y);
    // Elevation echoes the unit-vector y component for the colour/illuminance ramp below.
    let angle = dir.y.asin();

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
    let brightness = 2000.0 + 3000.0 * day_amount;

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
/// Compute the day/night sprite tints for the current time and environment.
///
/// Returns `(regular_linear, selflit_linear, base_srgb_rgb)` — the first two
/// are ready to drop into `SpriteExtension::tint`, the last is the sRGB
/// triple used by `animate_day_cycle`'s threshold-crossing detector.
///
/// Shared between `animate_day_cycle` (on threshold crossings) and
/// `init_sprite_tints_for_loading` (on entering `GameState::Loading`, so that
/// `SpriteTintBuffers` already has the correct values before any sprite spawn
/// reads them — otherwise sprites spawn with `Vec4::ONE` and visibly pop to
/// the real tint on the first render of `GameState::Game`).
pub fn compute_sprite_tints(tod: f32, is_indoor: bool) -> (Vec4, Vec4, [f32; 3]) {
    let tint = if is_indoor {
        Color::srgb(0.35, 0.30, 0.22)
    } else {
        sprite_tint_from_time(tod)
    };
    let t = tint.to_srgba();
    let tint_rgb = [t.red, t.green, t.blue];

    let tl = tint.to_linear();
    let regular = Vec4::new(tl.red, tl.green, tl.blue, 1.0);

    // SelfLit sprites (campfires, torches) blend only a small fraction of
    // the ambient tint so they feel grounded in the scene rather than
    // floating at pure full-bright.
    const SELFLIT_TINT_BLEND: f32 = 0.12;
    let selflit_srgb = Color::srgb(
        1.0 - (1.0 - t.red) * SELFLIT_TINT_BLEND,
        1.0 - (1.0 - t.green) * SELFLIT_TINT_BLEND,
        1.0 - (1.0 - t.blue) * SELFLIT_TINT_BLEND,
    );
    let stl = selflit_srgb.to_linear();
    let selflit = Vec4::new(stl.red, stl.green, stl.blue, 1.0);

    (regular, selflit, tint_rgb)
}

pub fn sprite_tint_from_time(tod: f32) -> Color {
    let day_amount = (1.0_f32 - (tod * 2.0 - 1.0).abs()).max(0.0);
    let dawn_dusk: f32 = {
        let d1 = (tod - 0.25).abs();
        let d2 = (tod - 0.75).abs();
        (1.0 - (d1.min(d2) * 10.0).min(1.0)).max(0.0)
    };

    // Night floor raised to roughly match the ambient light level terrain and
    // buildings receive at night — sprites are unlit so they only see this
    // tint, and a too-low floor leaves them visibly darker than the world
    // around them. (0.18, 0.18, 0.24) → noon white (~0.95).
    let r = (0.18 + 0.77 * day_amount + 0.05 * dawn_dusk).clamp(0.0, 1.0);
    let g = (0.18 + 0.77 * day_amount).clamp(0.0, 1.0);
    let b = (0.24 + 0.71 * day_amount - 0.05 * dawn_dusk).clamp(0.0, 1.0);

    Color::srgb(r, g, b)
}

/// Drive all frame-to-frame lighting state: sun direction/ambient, lit/unlit
/// mode toggle for terrain and models, and the two global sprite tint vec4s.
///
/// Sprite tints are pushed into [`SpriteTintBuffers`] every frame; the
/// render-world plugin in `sprites::tint_buffer` extracts that resource and
/// uploads it into a single shared uniform buffer via `queue.write_buffer`,
/// which every sprite material already binds. No per-material iteration
/// happens here — day/night transitions are continuous and free.
#[allow(clippy::too_many_arguments)]
fn animate_day_cycle(
    game_time: Res<GameTime>,
    cfg: Res<GameConfig>,
    mut lighting_state: ResMut<LightingState>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    mut terrain_materials: Option<ResMut<Assets<TerrainMaterial>>>,
    mut tint_buffers: ResMut<SpriteTintBuffers>,
    map: Res<CurrentMap>,
    indoor: Option<Res<crate::states::loading::PreparedIndoorWorld>>,
    // Non-billboard (terrain, BSP models) — toggled between lit/unlit on mode change.
    model_query: Query<&MeshMaterial3d<StandardMaterial>, Without<Billboard>>,
    mut sun_query: Query<(&mut Transform, &mut DirectionalLight), Without<Player>>,
    mut ambient_query: Query<&mut AmbientLight, With<AmbientMarker>>,
    player_query: Query<&Transform, With<Player>>,
) {
    let is_indoor = map.is_indoor();
    let tod = game_time.time_of_day();

    apply_lighting_mode_switch(
        &cfg,
        &mut lighting_state,
        &mut std_materials,
        terrain_materials.as_deref_mut(),
        &model_query,
        is_indoor,
    );

    update_sun_and_ambient(
        tod,
        &cfg,
        is_indoor,
        indoor.as_deref(),
        &mut lighting_state,
        &mut sun_query,
        &mut ambient_query,
        &player_query,
    );

    // One CPU write per frame, propagated to every sprite via the shared
    // uniform buffer. `compute_sprite_tints` is cheap — a handful of float
    // ops — so running it unconditionally is fine and removes the threshold
    // gate that used to fragment day/night into discrete steps.
    let (regular, selflit, _) = compute_sprite_tints(tod, is_indoor);
    tint_buffers.regular = regular;
    tint_buffers.selflit = selflit;
}

/// Toggle model/terrain materials between lit (enhanced) and unlit (flat) on mode change.
/// Runs only on the frame where the lighting mode actually flips. Indoor walls stay
/// PBR in every mode because they rely on the party torch + decoration point lights.
fn apply_lighting_mode_switch(
    cfg: &GameConfig,
    lighting_state: &mut LightingState,
    std_materials: &mut Assets<StandardMaterial>,
    terrain_materials: Option<&mut Assets<TerrainMaterial>>,
    model_query: &Query<&MeshMaterial3d<StandardMaterial>, Without<Billboard>>,
    is_indoor: bool,
) {
    if cfg.lighting == lighting_state.last_mode {
        return;
    }
    lighting_state.last_mode = cfg.lighting.clone();
    let unlit = cfg.lighting != "enhanced";

    if !is_indoor {
        let mut toggled = std::collections::HashSet::new();
        for mat_handle in model_query.iter() {
            if toggled.insert(mat_handle.id())
                && let Some(mat) = std_materials.get_mut(mat_handle.id())
            {
                mat.unlit = unlit;
                mat.base_color = if unlit {
                    Color::srgb(0.69, 0.69, 0.69)
                } else {
                    Color::srgb(1.4, 1.4, 1.4)
                };
            }
        }
        if let Some(tm) = terrain_materials {
            for (_, mat) in tm.iter_mut() {
                mat.base.unlit = unlit;
                mat.base.base_color = if unlit {
                    Color::srgb(0.69, 0.69, 0.69)
                } else {
                    Color::srgb(1.2, 1.2, 1.2)
                };
            }
        }
    }
    info!("Lighting mode: {}", cfg.lighting);
}

/// Update the sun direction/colour and ambient light based on time of day and
/// (indoor) current sector. Throttles sun updates — per-frame writes only when
/// shadows are on so cascades sweep smoothly.
fn update_sun_and_ambient(
    tod: f32,
    cfg: &GameConfig,
    is_indoor: bool,
    indoor: Option<&crate::states::loading::PreparedIndoorWorld>,
    lighting_state: &mut LightingState,
    sun_query: &mut Query<(&mut Transform, &mut DirectionalLight), Without<Player>>,
    ambient_query: &mut Query<&mut AmbientLight, With<AmbientMarker>>,
    player_query: &Query<&Transform, With<Player>>,
) {
    let sun_tod_threshold: f32 = if cfg.shadows { 0.00007 } else { 0.0014 };
    let sun_needs_update =
        (tod - lighting_state.last_sun_tod).abs() > sun_tod_threshold || lighting_state.last_sun_tod == 0.0;

    if sun_needs_update {
        let (new_transform, sun_color, sun_illuminance) = sun_from_time(tod);
        for (mut transform, mut light) in sun_query.iter_mut() {
            *transform = new_transform;
            light.color = sun_color;
            // Indoors: the directional sun is fully off — dungeon lighting comes
            // from the party torch and decoration point lights.
            light.illuminance = if is_indoor {
                0.0
            } else if cfg.lighting == "enhanced" {
                sun_illuminance * 1.06
            } else {
                0.0
            };
        }
        lighting_state.last_sun_tod = tod;
    }

    if is_indoor {
        let ambient_brightness = indoor_sector_ambient_brightness(indoor, player_query, lighting_state);
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
}

/// Look up the sector containing the player and return an ambient brightness based
/// on its `min_ambient_light` value. Indoor-only; falls back to the first sector
/// or a small default if the player is outside every bbox.
///
/// Uses `LightingState::last_sector_index` to short-circuit the common case
/// where the player is still inside the previously-found sector.
fn indoor_sector_ambient_brightness(
    indoor: Option<&crate::states::loading::PreparedIndoorWorld>,
    player_query: &Query<&Transform, With<Player>>,
    lighting_state: &mut LightingState,
) -> f32 {
    let Some(indoor_data) = indoor else {
        lighting_state.last_sector_index = None;
        return 0.0;
    };
    let Ok(player_tf) = player_query.single() else {
        return 0.0;
    };
    let pos = player_tf.translation;

    let contains = |s: &crate::states::loading::SectorAmbient| {
        pos.x >= s.bbox_min.x
            && pos.x <= s.bbox_max.x
            && pos.y >= s.bbox_min.y
            && pos.y <= s.bbox_max.y
            && pos.z >= s.bbox_min.z
            && pos.z <= s.bbox_max.z
    };

    // Fast path: player is still inside the cached sector's bbox.
    if let Some(idx) = lighting_state.last_sector_index
        && let Some(sector) = indoor_data.sector_ambients.get(idx)
        && contains(sector)
    {
        return sector.min_ambient as f32 * 0.8 + 25.0;
    }

    // Slow path: linear scan. Fall back to the first sector if the player is
    // outside every bbox (can happen near geometry seams).
    let found = indoor_data
        .sector_ambients
        .iter()
        .enumerate()
        .find(|(_, s)| contains(s));

    match found {
        Some((idx, s)) => {
            lighting_state.last_sector_index = Some(idx);
            s.min_ambient as f32 * 0.8 + 25.0
        }
        None => {
            // Don't cache fallbacks — keep re-scanning until the player is
            // inside a real sector again.
            lighting_state.last_sector_index = None;
            indoor_data
                .sector_ambients
                .first()
                .map_or(25.0, |s| s.min_ambient as f32 * 0.8 + 25.0)
        }
    }
}

/// Tracks applied lighting state to detect changes.
#[derive(Resource, Default)]
struct LightingState {
    last_mode: String,
    /// Last tod at which sun/ambient were updated. Skip update below this threshold.
    last_sun_tod: f32,
    /// Index into `PreparedIndoorWorld::sector_ambients` of the sector the
    /// player was last known to be inside. Used to short-circuit the linear
    /// scan in `indoor_sector_ambient_brightness` — the cached sector is
    /// rechecked first and the scan only runs when the player leaves its bbox.
    last_sector_index: Option<usize>,
}
