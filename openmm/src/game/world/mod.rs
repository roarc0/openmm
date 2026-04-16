use bevy::prelude::*;
use openmm_data::utils::MapName;

/// Resource indicating the currently active and fully loaded map type.
/// Replaces scattered `resource_exists::<Prepared(Indoor)World>` checks.
#[derive(Resource, Deref, DerefMut, Clone, Debug)]
pub struct CurrentMap(pub MapName);

/// Run condition: the current map is outdoor.
pub fn is_outdoor(current: Option<Res<CurrentMap>>) -> bool {
    current.as_ref().is_some_and(|c| c.0.is_outdoor())
}

/// Run condition: the current map is indoor.
pub fn is_indoor(current: Option<Res<CurrentMap>>) -> bool {
    current.as_ref().is_some_and(|c| c.0.is_indoor())
}

mod event_handlers;
pub mod events;
pub mod npc_dialogue;
pub mod scripting;
pub mod state;
pub mod time;
mod variables;

pub use events::{GENERATED_NPC_ID_BASE, MapEvents, load_map_events};
pub use scripting::EventQueue;
pub use state::WorldState;
pub use time::GameTime;

/// Core world simulation: time, persistent state, and event scripting.
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            time::GameTimePlugin,
            state::WorldStatePlugin,
            scripting::EventDispatchPlugin,
        ));
    }
}
