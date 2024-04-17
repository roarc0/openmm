use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::ButtonInput,
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::{
        default, in_state, App, Color, Commands, Component, IntoSystemConfigs, KeyCode, OnEnter,
        Plugin, Query, Res, ResMut, Resource, TextBundle, Transform, Update, Vec3, With,
    },
    text::{Text, TextSection, TextStyle},
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
//use bevy_prototype_debug_lines::{DebugLines, DebugLinesPlugin};
use lod::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::{player::FlyCam, GameState};

/// Keeps track of mouse motion events, pitch, and yaw
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

/// Key configuration
#[derive(Resource)]
pub struct KeyBindings {
    pub toggle_wireframe: KeyCode,
    pub toggle_play_area: KeyCode,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            toggle_wireframe: KeyCode::BracketRight,
            toggle_play_area: KeyCode::BracketLeft,
        }
    }
}

fn dev_setup(
    mut commands: Commands,
    dev_config: Res<DevConfig>,
    //mut lines: ResMut<DebugLines>,
    //mut wireframe_config: ResMut<WireframeConfig>,
) {
    //wireframe_config.global = false;

    //if cfg.show_play_area {
    let val = ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.;
    let points = [
        (
            Color::RED,
            Vec3::new(val, 0., val),
            Vec3::new(val, 0., -val),
        ),
        (
            Color::LIME_GREEN,
            Vec3::new(val, 0., val),
            Vec3::new(-val, 0., val),
        ),
        (
            Color::BLUE,
            Vec3::new(-val, 0., val),
            Vec3::new(-val, 0., -val),
        ),
        (
            Color::ORANGE,
            Vec3::new(val, 0., -val),
            Vec3::new(-val, 0., -val),
        ),
    ];
    // for (color, start, end) in &points {
    //     lines.line_colored(*start, *end, 0.0, *color);
    // }
    //}

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
        //InWorld,
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
        //InWorld,
    ));
}

/// Handles keyboard input for enabling/disabling dev options
fn dev_input(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<KeyBindings>,
    mut dev_config: ResMut<DevConfig>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keys.just_pressed(key_bindings.toggle_wireframe) {
        dev_config.show_play_area = !dev_config.show_play_area;
    } else if keys.just_pressed(key_bindings.toggle_play_area) {
        wireframe_config.global = !wireframe_config.global;
    }
}

#[derive(Component)]
pub struct FpsText;

fn update_fps_text(diagnostics: Res<DiagnosticsStore>, mut query: Query<&mut Text, With<FpsText>>) {
    for mut text in &mut query {
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
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
        app.init_resource::<KeyBindings>()
            .insert_resource(DevConfig::default())
            .add_plugins((
                WireframePlugin,
                //LogDiagnosticsPlugin::default(),
                //DebugLinesPlugin::default(),
                //WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            ))
            .add_systems(
                Update,
                (dev_input, update_fps_text, update_position_text)
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), dev_setup);
        //.add_systems(OnExit(GameState::Game), despawn_all::<DevStuff>);
    }
}
