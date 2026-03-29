use bevy::prelude::{
    App, AppExtStates, Commands, Component, Entity, Plugin, Query, States, With,
};
use bevy_config::BevyConfigPlugin;

use assets::GameAssets;
use config::GameConfig;
use game::InGamePlugin;
use save::GameSave;
use states::{loading::LoadingPlugin, menu::MenuPlugin, splash::SplashPlugin};

pub(crate) mod assets;
pub(crate) mod bevy_config;
pub mod config;
pub(crate) mod fonts;
pub(crate) mod game;
pub(crate) mod save;
pub(crate) mod states;
pub(crate) mod ui_assets;

const APP_NAME: &str = "openmm";

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub(crate) enum GameState {
    #[default]
    Splash,
    Menu,
    Loading,
    Game,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        let cfg = GameConfig::load();
        let game_assets = GameAssets::new(lod::get_lod_path().into())
            .expect("unable to load game data files");
        let game_fonts = fonts::GameFonts::load(&game_assets);
        let save_data = GameSave::load_or_default();

        app.insert_resource(cfg)
            .insert_resource(game_assets)
            .insert_resource(game_fonts)
            .insert_resource(save_data)
            .init_resource::<ui_assets::UiAssets>()
            .add_plugins((
                BevyConfigPlugin,
                SplashPlugin,
                MenuPlugin,
                LoadingPlugin,
                InGamePlugin,
            ))
            .insert_state(GameState::Splash);
    }
}

pub fn despawn_all<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in &to_despawn {
        commands.entity(entity).despawn();
    }
}
