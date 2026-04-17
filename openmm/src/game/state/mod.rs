//! World simulation state: persistent variables, player/map runtime, and game time.

pub mod state;
pub mod time;
pub(in crate::game) mod variables;

pub use state::WorldState;
pub use time::GameTime;

use bevy::prelude::*;

/// Core world state plugin: time and persistent state.
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((time::GameTimePlugin, state::WorldStatePlugin));
    }
}
