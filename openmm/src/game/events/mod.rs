//! EVT event dispatch: queue, scripting engine, and side-effect handlers.

pub(crate) mod event_handlers;
pub mod events;
pub mod scripting;

pub use events::{GENERATED_NPC_ID_BASE, MapEvents, load_map_events};
pub use scripting::{EventDispatchPlugin, EventQueue};
