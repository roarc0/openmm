use bevy::prelude::*;

pub(crate) mod actors;
pub(crate) mod collision;
pub mod coords;
pub(crate) mod debug;
pub(crate) mod hud;
pub(crate) mod indoor;
pub(crate) mod interaction;
pub(crate) mod lighting;
pub(crate) mod optional;
pub(crate) mod outdoor;
pub(crate) mod party;
pub(crate) mod physics;
pub(crate) mod player;
pub(crate) mod sky;
pub(crate) mod sound;
pub(crate) mod spatial_index;
pub(crate) mod sprites;
pub(crate) mod world;

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
                RenderingPlugin,
                MapPlugin,
                world::WorldPlugin,
                GameplayPlugin,
                UiPlugin,
                AudioPlugin,
            ));
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
            sprites::tint_buffer::SpriteTintBufferPlugin,
            MaterialPlugin::<outdoor::TerrainMaterial>::default(),
            MaterialPlugin::<outdoor::BspWaterMaterial>::default(),
            MaterialPlugin::<sprites::material::SpriteMaterial>::default(),
        ));
    }
}

/// World: outdoor (ODM) maps, indoor (BLV) maps, physics, and entity scaffolding.
struct MapPlugin;
impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            outdoor::OdmPlugin,
            indoor::BlvPlugin,
            physics::PhysicsPlugin,
            spatial_index::SpatialIndexPlugin,
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

/// UI: viewport, debug overlays, dev console, click interaction, EVT event dispatch.
struct UiPlugin;
impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            debug::DebugPlugin,
            debug::console::ConsolePlugin,
            interaction::InteractionPlugin,
        ))
        // Viewport clipping and debug coords — the only bits needed from the old HUD.
        .add_plugins(hud::debug_hud::DebugHudCoordsPlugin)
        .add_systems(
            Update,
            hud::borders::update_viewport.run_if(in_state(crate::GameState::Game)),
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
