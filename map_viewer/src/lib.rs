use bevy::prelude::{
    App, Commands, Component, DespawnRecursiveExt, Entity, Plugin, Query, States, With,
};
use bevy_config::BevyConfigPlugin;
use menu::MenuPlugin;
use splash::SplashPlugin;
use world::WorldPlugin;

pub(crate) mod bevy_config;
pub(crate) mod menu;
pub(crate) mod odm;
pub(crate) mod player;
pub(crate) mod splash;
pub(crate) mod world;

//mod debug_area;

const APP_NAME: &str = "openmm";

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
enum GameState {
    #[default]
    Splash,
    Menu,
    Game,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<GameState>().add_plugins((
            BevyConfigPlugin,
            MenuPlugin,
            SplashPlugin,
            WorldPlugin,
        ));
    }
}

pub fn despawn_screen<T: Component>(to_despawn: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in &to_despawn {
        commands.entity(entity).despawn_recursive();
    }
}
