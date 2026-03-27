use bevy::prelude::*;

use crate::{despawn_all, GameState};

pub(crate) mod dev;
pub(crate) mod odm;
pub(crate) mod player;
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
            odm::OdmPlugin,
            dev::DevPlugin,
        ))
        .add_systems(OnExit(GameState::Game), despawn_all::<InGame>);
    }
}
