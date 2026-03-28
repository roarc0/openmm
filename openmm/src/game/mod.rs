use bevy::prelude::*;

use crate::{despawn_all, GameState};

pub(crate) mod collision;
pub(crate) mod debug;
pub(crate) mod entities;
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
            entities::EntitiesPlugin,
            debug::DebugPlugin,
            MaterialPlugin::<terrain_material::TerrainMaterial>::default(),
        ))
        .add_systems(
            Update,
            terrain_material::update_terrain_time.run_if(in_state(GameState::Game)),
        )
        .add_systems(OnExit(GameState::Game), despawn_all::<InGame>);
    }
}
