use bevy::prelude::*;

pub(crate) mod actors;
pub mod coords;
pub(crate) mod indoor;
pub(crate) mod collision;
pub(crate) mod debug;
pub(crate) mod scripting;
pub(crate) mod events;
pub(crate) mod game_time;
pub(crate) mod hud;
pub(crate) mod interaction;
pub(crate) mod lighting;
pub(crate) mod map_name;
pub(crate) mod outdoor;
pub(crate) mod optional;
pub(crate) mod party;
pub(crate) mod physics;
pub(crate) mod player;
pub(crate) mod sky;
pub(crate) mod sound;
pub(crate) mod sprites;
pub(crate) mod world_state;

/// Marker component for all entities spawned during the Game state.
/// Despawned automatically on OnExit(Game).
#[derive(Component)]
pub struct InGame;

/// Top-level plugin for everything that runs once the player is in the game.
///
/// Plugins are grouped into bundles by responsibility (core, rendering, world,
/// gameplay, ui, audio). Each bundle is its own `Plugin` impl below, which
/// makes "what can be disabled" obvious — drop a bundle from the tuple and the
/// optional-plugin guards in `optional.rs` keep the rest of the game running.
pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        // HudView / FooterText are initialised at the top level so optional
        // systems that read them via `Res<HudView>` (e.g. ODM run conditions)
        // don't fail if HudPlugin is disabled.
        app.init_resource::<hud::HudView>()
            .init_resource::<hud::FooterText>()
            .add_plugins((
                CorePlugin,
                RenderingPlugin,
                WorldPlugin,
                GameplayPlugin,
                UiPlugin,
                AudioPlugin,
            ));
    }
}

/// Core simulation: time and persistent world state.
struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((game_time::GameTimePlugin, world_state::WorldStatePlugin));
    }
}

/// Rendering: lighting, sky, custom material pipelines.
///
/// `MaterialPlugin<TerrainMaterial>` and `MaterialPlugin<SpriteMaterial>` can
/// be removed individually for headless / minimal builds — the rest of the
/// game falls back to plain `StandardMaterial` via the optional-plugin guards.
struct RenderingPlugin;
impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            lighting::LightingPlugin,
            sky::SkyPlugin,
            MaterialPlugin::<outdoor::TerrainMaterial>::default(),
            MaterialPlugin::<sprites::material::SpriteMaterial>::default(),
        ));
    }
}

/// World: outdoor (ODM) maps, indoor (BLV) maps, physics, and entity scaffolding.
struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            outdoor::OdmPlugin,
            indoor::BlvPlugin,
            physics::PhysicsPlugin,
            sprites::SpritesPlugin,
        ));
    }
}

/// Gameplay: player, AI, combat, party.
struct GameplayPlugin;
impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((player::PlayerPlugin, actors::ActorsPlugin, party::PartyPlugin));
    }
}

/// UI: HUD, debug overlays, dev console, click interaction, EVT event dispatch.
struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            hud::HudPlugin,
            debug::DebugPlugin,
            debug::console::ConsolePlugin,
            interaction::InteractionPlugin,
            scripting::EventDispatchPlugin,
        ));
    }
}

/// Audio: sound effects, music, spatial audio.
struct AudioPlugin;
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(sound::SoundPlugin);
    }
}
