use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::{
        default, in_state, info, App, Color, Commands, Component, Input, IntoSystemConfigs,
        KeyCode, OnEnter, OnExit, Plugin, Query, Res, ResMut, Resource, TextBundle, Transform,
        Update, Vec3, With,
    },
    text::{Text, TextSection, TextStyle},
};
use bevy_prototype_debug_lines::{DebugLines, DebugLinesPlugin};

use crate::{despawn_screen, player::FlyCam, GameState};

use super::InWorld;

/// Keeps track of mouse motion events, pitch, and yaw
#[derive(Resource)]
struct DevState {
    debug_area: bool,
}

impl Default for DevState {
    fn default() -> Self {
        Self { debug_area: true }
    }
}

/// Key configuration
#[derive(Resource)]
pub struct KeyBindings {
    pub toggle: KeyCode,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            toggle: KeyCode::BracketRight,
        }
    }
}

fn dev_setup(
    mut commands: Commands,
    state: Res<DevState>,
    mut lines: ResMut<DebugLines>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    wireframe_config.global = false;

    if state.debug_area {
        let val = 88.0 * 512.0 / 2.;
        lines.line_colored(
            Vec3::new(val, 0., val),
            Vec3::new(val, 0., -val),
            0.0,
            Color::RED,
        );
        lines.line_colored(
            Vec3::new(val, 0., val),
            Vec3::new(-val, 0., val),
            0.0,
            Color::LIME_GREEN,
        );
        lines.line_colored(
            Vec3::new(-val, 0., val),
            Vec3::new(-val, 0., -val),
            0.0,
            Color::BLUE,
        );
        lines.line_colored(
            Vec3::new(val, 0., -val),
            Vec3::new(-val, 0., -val),
            0.0,
            Color::ORANGE,
        );
    }

    commands.spawn((
        TextBundle::from_sections([
            TextSection::new(
                "FPS: ",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::GOLD,
                ..default()
            }),
        ]),
        FpsText,
        InWorld,
    ));

    commands.spawn((
        TextBundle::from_sections([
            TextSection::new(
                " POS: ",
                TextStyle {
                    font_size: 15.0,
                    color: Color::WHITE,
                    ..default()
                },
            ),
            TextSection::from_style(TextStyle {
                font_size: 15.0,
                color: Color::GOLD,
                ..default()
            }),
        ]),
        PositionText,
        InWorld,
    ));
}

/// Handles keyboard input for enabling/disabling debug area
fn dev_input(
    mut state: ResMut<DevState>,
    key_bindings: Res<KeyBindings>,
    keys: Res<Input<KeyCode>>,
) {
    if keys.just_pressed(key_bindings.toggle) {
        state.debug_area = !state.debug_area;
    }
}

fn update_wireframe_input(
    keys: Res<Input<KeyCode>>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keys.just_pressed(KeyCode::BracketLeft) {
        info!("Changed wireframe");
        wireframe_config.global = !wireframe_config.global;
    }
}

#[derive(Component)]
pub struct FpsText;

fn update_fps_text(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text, With<FpsText>>) {
    for mut text in &mut query {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                text.sections[1].value = format!("{value:.2}");
            }
        }
    }
}

#[derive(Component)]
pub struct PositionText;

fn update_position_text(
    mut query: Query<&mut Text, With<PositionText>>,
    query2: Query<&Transform, With<FlyCam>>,
) {
    let transform = query2.get_single().unwrap();
    for mut text in &mut query {
        text.sections[1].value = format!("{:?}", transform.translation);
    }
}

pub struct DevPlugin;
impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DevState>()
            .init_resource::<KeyBindings>()
            .add_plugins((
                WireframePlugin,
                LogDiagnosticsPlugin::default(),
                DebugLinesPlugin::default(),
            ))
            .add_systems(
                Update,
                (
                    update_wireframe_input,
                    update_fps_text,
                    update_position_text,
                    dev_input,
                )
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), dev_setup)
            .add_systems(OnExit(GameState::Game), despawn_screen::<InWorld>);
    }
}
