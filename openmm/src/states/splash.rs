use bevy::prelude::*;

use crate::config::GameConfig;
use crate::{GameState, despawn_all};

// This plugin will display a splash screen for 1 second before switching to the menu
pub struct SplashPlugin;
impl Plugin for SplashPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Splash), splash_setup)
            .add_systems(Update, countdown.run_if(in_state(GameState::Splash)))
            .add_systems(OnExit(GameState::Splash), despawn_all::<OnSplashScreen>);
    }
}

#[derive(Component)]
struct OnSplashScreen;

#[derive(Resource, Deref, DerefMut)]
struct SplashTimer(Timer);

fn splash_setup(mut commands: Commands) {
    commands.spawn((Camera2d, OnSplashScreen));

    // Black screen — placeholder for future intro video
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        OnSplashScreen,
    ));

    // Skip quickly
    commands.insert_resource(SplashTimer(Timer::from_seconds(0.1, TimerMode::Once)));
}

fn countdown(
    mut game_state: ResMut<NextState<GameState>>,
    time: Res<Time>,
    mut timer: ResMut<SplashTimer>,
    cfg: Res<GameConfig>,
) {
    if timer.tick(time.delta()).just_finished() {
        if cfg.skip_intro {
            game_state.set(GameState::Loading);
        } else {
            game_state.set(GameState::Menu);
        }
    }
}
