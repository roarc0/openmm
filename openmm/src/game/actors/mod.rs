//! Actor systems: AI, combat, and per-actor physics.
//!
//! These run on every entity carrying an [`crate::game::actors::Actor`]
//! component, regardless of whether it spawned from an outdoor (ODM) or
//! indoor (BLV) map. Submodules are kept thin so each one owns one concern:
//!
//! - [`combat`]: melee attack timing, kill events, dying-to-dead transitions.
//! - [`physics`]: per-actor ground snapping and passability checks.
//! - [`ai`]: wander/aggro state machine + obstacle steering.

use bevy::prelude::*;

pub mod actor;
pub mod ai;
pub mod combat;
pub mod npc_dialogue;
pub mod physics;

pub use actor::{Actor, ActorParams, MonsterAiType};
pub use ai::MonsterAiMode;
pub use combat::KillActorEvent;

pub struct ActorsPlugin;

impl Plugin for ActorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            combat::ActorCombatPlugin,
            physics::ActorPhysicsPlugin,
            ai::MonsterAiPlugin,
        ));
    }
}
