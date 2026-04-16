//! Indoor map (BLV) plugin — door animation, interaction, and world spawning.
use bevy::prelude::*;

use crate::GameState;
use crate::game::world::is_indoor;
use crate::game::world::ui_state::{UiMode, UiState};

mod doors;
mod interact;
mod spawn;
mod types;

pub(crate) use doors::door_animation_system;
pub use doors::trigger_door;
pub(crate) use interact::{indoor_interact_system, indoor_touch_trigger_system};
pub(crate) use spawn::spawn_indoor_world;
pub use types::{BlvDoors, DoorColliders, OccluderFaceInfo, OccluderFaces, TouchTriggerFaces};

pub struct BlvPlugin;

impl Plugin for BlvPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Game), spawn_indoor_world.run_if(is_indoor))
            .add_systems(
                Update,
                (
                    indoor_interact_system,
                    indoor_touch_trigger_system,
                    door_animation_system,
                )
                    .run_if(in_state(GameState::Game))
                    .run_if(|ui: Res<UiState>| ui.mode == UiMode::World),
            );
    }
}
