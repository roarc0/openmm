use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    input::{common_conditions::input_toggle_active, ButtonInput},
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use crate::GameState;
use crate::game::InGame;
use crate::game::odm::OdmName;
use crate::game::player::Player;
use crate::save::{GameSave, PlayerState, MapState};
use crate::states::loading::LoadRequest;

#[derive(Resource)]
struct DevConfig {
    show_play_area: bool,
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            show_play_area: true,
        }
    }
}

#[derive(Resource)]
pub struct DevKeyBindings {
    pub toggle_wireframe: KeyCode,
    pub toggle_play_area: KeyCode,
}

impl Default for DevKeyBindings {
    fn default() -> Self {
        Self {
            toggle_wireframe: KeyCode::BracketRight,
            toggle_play_area: KeyCode::BracketLeft,
        }
    }
}

/// Tracks the current map for dev map switching.
#[derive(Resource)]
pub struct CurrentMapName(pub OdmName);

impl Default for CurrentMapName {
    fn default() -> Self {
        Self(OdmName::default())
    }
}

fn dev_setup(mut commands: Commands, mut wireframe_config: ResMut<WireframeConfig>) {
    wireframe_config.global = false;

    commands.spawn((
        Text::new("FPS: "),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::WHITE),
        FpsText,
        InGame,
    ));

    commands.spawn((
        Text::new(" POS: "),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::WHITE),
        PositionText,
        InGame,
    ));
}

fn dev_input(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<DevKeyBindings>,
    mut dev_config: ResMut<DevConfig>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keys.just_pressed(key_bindings.toggle_wireframe) {
        wireframe_config.global = !wireframe_config.global;
    } else if keys.just_pressed(key_bindings.toggle_play_area) {
        dev_config.show_play_area = !dev_config.show_play_area;
    }
}

/// Dev-only map switching with H/J/K/L keys.
fn dev_change_map(
    keys: Res<ButtonInput<KeyCode>>,
    mut current_map: ResMut<CurrentMapName>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let new_map = if keys.just_pressed(KeyCode::KeyJ) {
        current_map.0.go_north()
    } else if keys.just_pressed(KeyCode::KeyH) {
        current_map.0.go_west()
    } else if keys.just_pressed(KeyCode::KeyK) {
        current_map.0.go_south()
    } else if keys.just_pressed(KeyCode::KeyL) {
        current_map.0.go_east()
    } else {
        None
    };

    if let Some(new_map) = new_map {
        info!("Dev: changing map to {}", &new_map);
        commands.insert_resource(LoadRequest {
            map_name: new_map.clone(),
        });
        current_map.0 = new_map;
        game_state.set(GameState::Loading);
    }
}

#[derive(Component)]
pub struct FpsText;

fn update_fps_text(
    diagnostics: Res<DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsText>>,
) {
    for mut text in &mut query {
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                **text = format!("FPS: {value:.2}");
            }
        }
    }
}

#[derive(Component)]
pub struct PositionText;

fn update_position_text(
    mut query: Query<&mut Text, With<PositionText>>,
    player_query: Query<&Transform, With<Player>>,
) {
    if let Ok(transform) = player_query.single() {
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        for mut text in &mut query {
            **text = format!(
                " POS: ({:.0}, {:.0}, {:.0}) YAW: {:.0}°",
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                yaw.to_degrees(),
            );
        }
    }
}

fn debug_log(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    player_query: Query<&Transform, With<Player>>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(3.0, TimerMode::Repeating));
    timer.tick(time.delta());
    if timer.just_finished() {
        if let Ok(transform) = player_query.single() {
            let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
            info!(
                "pos=({:.0}, {:.0}, {:.0}) yaw={:.1}° pitch={:.1}°",
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                yaw.to_degrees(),
                pitch.to_degrees(),
            );
        }
    }
}

fn quicksave(
    keys: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, With<Player>>,
    current_map: Res<CurrentMapName>,
    mut save_data: ResMut<GameSave>,
) {
    if keys.just_pressed(KeyCode::F3) {
        if let Ok(transform) = player_query.single() {
            let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            save_data.player = PlayerState {
                position: [
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ],
                yaw,
            };
            save_data.map = MapState {
                map_x: current_map.0.x,
                map_y: current_map.0.y,
            };
            match save_data.autosave() {
                Ok(()) => info!("Saved to target/saves/autosave.json"),
                Err(e) => error!("Failed to quicksave: {}", e),
            }
        }
    }
}

pub struct DevPlugin;
impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DevKeyBindings>()
            .init_resource::<DevConfig>()
            .init_resource::<CurrentMapName>()
            .add_plugins((
                WireframePlugin::default(),
                LogDiagnosticsPlugin::default(),
                EguiPlugin::default(),
                WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            ))
            .add_systems(
                Update,
                (dev_input, update_fps_text, update_position_text, dev_change_map, debug_log, quicksave)
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), dev_setup);
    }
}
