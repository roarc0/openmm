//! Outdoor map (ODM) plugin: terrain, BSP buildings, decorations, NPCs and monsters.
//!
//! Submodules carve the work into single-responsibility files:
//! - [`boundary`]: detect player crossing the play-area edge and queue the next map.
//! - [`spawn`]: one-shot world spawning at `OnEnter(Game)` (terrain + BSP + spawn queues).
//! - [`lazy_spawn`]: per-frame, time-budgeted spawning of decorations / NPCs / monsters.
//! - [`spawn_decorations`]: billboard / animated / directional decoration spawning.
//! - [`spawn_actors`]: NPC and monster entity spawning.
//! - [`texture_swap`]: runtime texture replacement for outdoor BSP faces.

use bevy::prelude::*;

use crate::GameState;

mod boundary;
mod lazy_spawn;
mod spawn;
mod spawn_actors;
mod spawn_decorations;
pub mod terrain;
mod texture_swap;

pub use boundary::PLAY_WIDTH;
pub use lazy_spawn::SpawnProgress;
pub use openmm_data::OdmName;
pub use terrain::TerrainMaterial;
pub use texture_swap::ApplyTextureOutdoors;

pub struct OdmPlugin;

impl Plugin for OdmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnProgress>()
            .add_message::<ApplyTextureOutdoors>()
            .add_systems(OnEnter(GameState::Game), spawn::spawn_world)
            .add_systems(
                Update,
                (
                    lazy_spawn::lazy_spawn,
                    boundary::check_map_boundary,
                    texture_swap::apply_texture_outdoors,
                )
                    .run_if(in_state(GameState::Game))
                    .run_if(resource_equals(crate::game::hud::HudView::World)),
            );
    }
}

// ── Decoration point lights ─────────────────────────────────────────────────

/// Pre-scale applied to DSFT light_radius before `decoration_point_light` for animated
/// decorations (campfires, braziers).
/// campfireon: lr=256 × 8 = 2048 → range=4096, intensity=838M.
/// Keep range below indoor fog end (~2000) so the light cluster doesn't cover the whole dungeon.
pub(crate) const DSFT_ANIMATED_LR_SCALE: u16 = 8;

/// Pre-scale for static DSFT decorations (crystals, chandeliers, sconces).
pub(crate) const DSFT_STATIC_LR_SCALE: u16 = 6;

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
pub(crate) fn decoration_point_light(light_radius: u16) -> impl Bundle {
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
