//! EVT scripting engine: event queue, control flow, and event dispatch.

mod control_flow;
pub(crate) mod dispatch;
mod queue;

pub use queue::EventQueue;

use crate::GameState;
use bevy::prelude::*;

pub struct EventDispatchPlugin;

impl Plugin for EventDispatchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EventQueue>()
            .add_systems(OnEnter(GameState::Game), dispatch::dispatch_on_map_reload)
            .add_systems(Update, dispatch::process_events.run_if(in_state(GameState::Game)));
    }
}
