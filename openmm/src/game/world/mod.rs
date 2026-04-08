use bevy::prelude::*;

pub mod events;
pub mod scripting;
pub mod state;
pub mod time;

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
