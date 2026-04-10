use bevy::prelude::{App, AppExtStates, ClearColor, Color, Commands, Component, Entity, Plugin, Query, States, With};
use engine::EngineConfigPlugin;

use assets::GameAssets;
use config::GameConfig;
use game::InGamePlugin;
use save::GameSave;
use states::video::VideoRequest;
use states::{loading::LoadingPlugin, menu::MenuPlugin, video::VideoPlugin};

pub(crate) mod assets;
pub mod config;
#[cfg(feature = "editor")]
pub(crate) mod editor;
pub(crate) mod engine;
pub(crate) mod fonts;
pub(crate) mod game;
pub(crate) mod save;
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
        } else if cfg.skip_intro {
            GameState::Menu
        } else {
            GameState::Video
        };

        app.insert_resource(ClearColor(Color::BLACK))
            .insert_resource(cfg)
            .insert_resource(game_assets)
            .insert_resource(game_fonts)
            .insert_resource(save_data)
            .init_resource::<game::hud::UiAssets>()
            .insert_resource(VideoRequest {
                name: "3dologo".into(),
                skippable: false,
                next: GameState::Menu,
            })
            .add_plugins((EngineConfigPlugin, VideoPlugin, MenuPlugin, LoadingPlugin, InGamePlugin));

        #[cfg(feature = "editor")]
        if editor_mode {
            app.add_plugins(editor::EditorPlugin);
        }

        app.insert_state(initial_state);
    }
}

pub fn despawn_all<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in &to_despawn {
        commands.entity(entity).despawn();
    }
}
