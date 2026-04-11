use bevy::prelude::{App, AppExtStates, ClearColor, Color, Commands, Component, Entity, Plugin, Query, States, With};
use engine::EngineConfigPlugin;

use assets::GameAssets;
use config::GameConfig;
use game::InGamePlugin;
use save::GameSave;
use states::loading::LoadingPlugin;

pub(crate) mod assets;
pub mod config;
#[cfg(feature = "editor")]
pub(crate) mod editor;
pub(crate) mod engine;
pub(crate) mod fonts;
pub(crate) mod game;
pub(crate) mod save;
pub(crate) mod screens;
pub(crate) mod states;

const APP_NAME: &str = "openmm";

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub(crate) enum GameState {
    #[default]
    Video,
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
        let game_fonts = fonts::GameFonts::load(&game_assets);
        let save_data = GameSave::load_or_default();

        let editor_mode = cfg.editor;
        let initial_state = if editor_mode {
            #[cfg(feature = "editor")]
            {
                GameState::Editor
            }
            #[cfg(not(feature = "editor"))]
            {
                bevy::log::warn!("--editor requires the 'editor' feature; starting normally");
                GameState::Menu
            }
        } else if cfg.map.is_some() {
            GameState::Loading
        } else {
            GameState::Menu
        };

        app.insert_resource(ClearColor(Color::BLACK))
            .insert_resource(cfg)
            .insert_resource(game_assets)
            .insert_resource(game_fonts)
            .insert_resource(save_data)
            .init_resource::<game::hud::UiAssets>()
            .add_plugins(EngineConfigPlugin);

        #[cfg(feature = "editor")]
        if editor_mode {
            // State must be inserted before EditorPlugin so EguiPrimaryContextPass
            // systems see GameState::Editor from the very first frame. If the state
            // isn't ready when egui runs its first pass, the context may never
            // initialize — causing a permanently frozen editor.
            app.insert_state(initial_state);
            app.add_plugins(editor::EditorPlugin);
            return;
        }

        // Game-only plugins — not loaded in editor mode.
        app.add_plugins((LoadingPlugin, InGamePlugin, screens::runtime::ScreenRuntimePlugin));

        app.insert_state(initial_state);
    }
}

pub fn despawn_all<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in &to_despawn {
        commands.entity(entity).despawn();
    }
}
