use bevy::prelude::*;

pub(crate) mod actors;
pub(crate) mod controls;
pub(crate) mod events;
pub(crate) mod interaction;
pub(crate) mod map;
pub(crate) mod optional;
pub(crate) mod player;
pub(crate) mod rendering;
pub(crate) mod sound;
pub(crate) mod spawn;
pub(crate) mod sprites;
pub(crate) mod state;
pub(crate) mod ui;

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
        // UiMode initialized at top level so run conditions never fail.
        app.init_resource::<ui::UiState>()
            .add_systems(Update, ui::tick_footer_text)
            .add_plugins((
                RenderingPlugin,
                MapPlugin,
                state::WorldPlugin,
                events::EventDispatchPlugin,
                GameplayPlugin,
                UiPlugin,
                AudioPlugin,
            ));
    }
}

/// Rendering: lighting, sky, custom material pipelines.
struct RenderingPlugin;
impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            rendering::lighting::LightingPlugin,
            rendering::sky::SkyPlugin,
            sprites::tint_buffer::SpriteTintBufferPlugin,
            MaterialPlugin::<map::outdoor::TerrainMaterial>::default(),
            MaterialPlugin::<map::outdoor::BspWaterMaterial>::default(),
            MaterialPlugin::<sprites::material::SpriteMaterial>::default(),
        ));
    }
}

/// World: outdoor (ODM) maps, indoor (BLV) maps, physics, and entity scaffolding.
struct MapPlugin;
impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            map::outdoor::OdmPlugin,
            map::indoor::BlvPlugin,
            player::physics::PhysicsPlugin,
            map::spatial_index::SpatialIndexPlugin,
            sprites::SpritesPlugin,
        ));
    }
}

/// Gameplay: player, AI, combat, party.
struct GameplayPlugin;
impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((player::PlayerPlugin, actors::ActorsPlugin, player::party::PartyPlugin));
    }
}

/// UI: viewport, debug overlays, dev console, click interaction, EVT event dispatch.
struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            crate::screens::debug::DebugPlugin,
            crate::screens::debug::console::ConsolePlugin,
            interaction::InteractionPlugin,
        ))
        // Viewport clipping — keeps the 3D camera inside the HUD frame.
        .add_systems(
            Update,
            rendering::viewport::update_viewport.run_if(in_state(crate::GameState::Game)),
        );
    }
}

/// Audio: sound effects, music, spatial audio.
struct AudioPlugin;
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(sound::SoundPlugin);
    }
}
