use bevy::prelude::{App, AppExtStates, ClearColor, Color, Commands, Component, Entity, Plugin, Query, States, With};
use game::rendering::engine::EngineConfigPlugin;

use assets::GameAssets;
use game::InGamePlugin;
use prepare::loading::LoadingPlugin;
use system::config::GameConfig;
use system::save::GameSave;

pub(crate) mod assets;
#[cfg(feature = "editor")]
pub(crate) mod editor;
pub(crate) mod game;
pub(crate) mod prepare;
pub(crate) mod screens;
pub(crate) mod system;

const APP_NAME: &str = "openmm";

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub(crate) enum GameState {
    #[default]
    Menu,
    Loading,
    Game,
    #[cfg(feature = "editor")]
    Editor,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        let cfg = GameConfig::load();
        let game_assets = GameAssets::new(openmm_data::get_data_path().into()).expect("unable to load game data files");
        let game_fonts = screens::fonts::GameFonts::load(&game_assets);
        let save_data = GameSave::load_or_default();

        let editor_mode = cfg.editor;
        let initial_state = if editor_mode {
            #[cfg(feature = "editor")]
            {
                bevy::log::info!("starting dedicated editor mode (--editor)");
                GameState::Editor
            }
            #[cfg(not(feature = "editor"))]
            {
                bevy::log::warn!("--editor requires the 'editor' feature; starting normally");
                GameState::Menu
            }
        } else if cfg.map.is_some() {
            #[cfg(feature = "editor")]
            bevy::log::info!("starting game mode (editor is isolated; use --editor for screen editor)");
            GameState::Loading
        } else {
            #[cfg(feature = "editor")]
            bevy::log::info!("starting game mode (editor is isolated; use --editor for screen editor)");
            GameState::Menu
        };

        app.insert_resource(ClearColor(Color::BLACK))
            .insert_resource(cfg)
            .insert_resource(game_assets)
            .insert_resource(game_fonts)
            .insert_resource(save_data)
            .init_resource::<screens::ui_assets::UiAssets>()
            .add_plugins(EngineConfigPlugin);

        // Insert state only after EngineConfigPlugin, which installs
        // DefaultPlugins (including the state transition schedule).
        // This must still happen before EditorPlugin is added so egui's first
        // pass sees a valid GameState when starting directly in editor mode.
        app.insert_state(initial_state);

        // Editor mode is fully isolated from gameplay runtime: only editor
        // systems/plugins are loaded when explicitly started with --editor.
        #[cfg(feature = "editor")]
        if editor_mode {
            app.add_plugins(editor::EditorPlugin);
            return;
        }

        // Game-only plugins — never loaded in dedicated editor mode.
        app.add_plugins((LoadingPlugin, InGamePlugin, screens::runtime::ScreenRuntimePlugin));
    }
}

pub fn despawn_all<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in &to_despawn {
        commands.entity(entity).despawn();
    }
}
