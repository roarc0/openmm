use bevy::prelude::*;

use crate::{despawn_all, GameState};

pub(crate) mod blv;
pub(crate) mod collision;
pub(crate) mod console;
pub(crate) mod debug;
pub(crate) mod entities;
pub(crate) mod event_dispatch;
pub(crate) mod events;
pub(crate) mod hud;
pub(crate) mod interaction;
pub(crate) mod map_name;
pub(crate) mod odm;
pub(crate) mod physics;
pub(crate) mod player;
pub(crate) mod terrain_material;
pub(crate) mod utils;
pub(crate) mod world;

/// Marker component for all entities spawned during the Game state.
/// Despawned automatically on OnExit(Game).
#[derive(Component)]
pub struct InGame;

pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            world::WorldPlugin,
            player::PlayerPlugin,
            physics::PhysicsPlugin,
            odm::OdmPlugin,
            blv::BlvPlugin,
            entities::EntitiesPlugin,
            debug::DebugPlugin,
            hud::HudPlugin,
            interaction::InteractionPlugin,
            event_dispatch::EventDispatchPlugin,
            console::ConsolePlugin,
            MaterialPlugin::<terrain_material::TerrainMaterial>::default(),
        ))
        .add_systems(
            Update,
            terrain_material::update_terrain_time
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(hud::HudView::World)),
        )
        .add_systems(OnExit(GameState::Game), despawn_all::<InGame>);
    }
}
