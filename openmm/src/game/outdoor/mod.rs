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
use crate::game::world::is_outdoor;

mod boundary;
mod bsp;
pub mod bsp_water;
mod lazy_spawn;
mod spawn;
mod spawn_actors;
mod spawn_decorations;
pub mod spawn_terrain;
mod texture_swap;

pub use boundary::PLAY_WIDTH;
pub use bsp_water::BspWaterMaterial;
pub use lazy_spawn::SpawnProgress;
pub use openmm_data::utils::OdmName;
pub use spawn_terrain::TerrainMaterial;
pub use texture_swap::ApplyTextureOutdoors;

pub struct OdmPlugin;

impl Plugin for OdmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnProgress>()
            .init_resource::<texture_swap::SwapMaterialCache>()
            .add_message::<ApplyTextureOutdoors>()
            .add_systems(OnEnter(GameState::Game), spawn::spawn_world.run_if(is_outdoor))
            .add_systems(
                Update,
                (
                    lazy_spawn::lazy_spawn,
                    boundary::check_map_boundary,
                    texture_swap::apply_texture_outdoors,
                )
                    .run_if(in_state(GameState::Game))
                    .run_if(|ui: Res<crate::game::world::ui_state::UiState>| ui.mode == crate::game::world::ui_state::UiMode::World)
                    .run_if(is_outdoor),
            );
    }
}
