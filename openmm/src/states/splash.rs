use bevy::prelude::*;

use crate::{despawn_all, GameState};

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

fn splash_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let splash = asset_server.load("splash.png");

    commands.spawn((Camera2d, OnSplashScreen));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                ..default()
            },
            OnSplashScreen,
        ))
        .with_children(|parent| {
            parent.spawn((
                Node {
                    flex_grow: 1.,
                    ..default()
                },
                ImageNode::new(splash),
            ));
        });

    commands.insert_resource(SplashTimer(Timer::from_seconds(1.0, TimerMode::Once)));
}

fn countdown(
    mut game_state: ResMut<NextState<GameState>>,
    time: Res<Time>,
    mut timer: ResMut<SplashTimer>,
) {
    if timer.tick(time.delta()).just_finished() {
        game_state.set(GameState::Menu);
    }
}
