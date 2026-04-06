use bevy::prelude::*;

pub(crate) mod actor_combat;
pub(crate) mod actor_physics;
pub(crate) mod blv;
pub(crate) mod collision;
pub(crate) mod debug;
pub(crate) mod entities;
pub(crate) mod event_dispatch;
pub(crate) mod events;
pub(crate) mod game_time;
pub(crate) mod hud;
pub(crate) mod interaction;
pub(crate) mod lighting;
pub(crate) mod map_name;
pub(crate) mod monster_ai;
pub(crate) mod odm;
pub(crate) mod party;
pub(crate) mod physics;
pub(crate) mod player;
pub(crate) mod raycast;
pub(crate) mod sky;
pub(crate) mod sound;
pub(crate) mod sprite_material;
pub(crate) mod terrain_material;
pub(crate) mod world_state;

/// Marker component for all entities spawned during the Game state.
/// Despawned automatically on OnExit(Game).
#[derive(Component)]
pub struct InGame;

pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            game_time::GameTimePlugin,
            lighting::LightingPlugin,
            sky::SkyPlugin,
            player::PlayerPlugin,
            physics::PhysicsPlugin,
            odm::OdmPlugin,
            blv::BlvPlugin,
            entities::EntitiesPlugin,
            debug::DebugPlugin,
            hud::HudPlugin,
            interaction::InteractionPlugin,
            event_dispatch::EventDispatchPlugin,
            debug::console::ConsolePlugin,
            world_state::WorldStatePlugin,
            sound::SoundPlugin,
        ))
        .add_plugins(actor_combat::ActorCombatPlugin)
        .add_plugins(actor_physics::ActorPhysicsPlugin)
        .add_plugins(monster_ai::MonsterAiPlugin)
        .add_plugins(MaterialPlugin::<terrain_material::TerrainMaterial>::default())
        .add_plugins(MaterialPlugin::<sprite_material::SpriteMaterial>::default())
        .add_plugins(party::PartyPlugin);
    }
}
