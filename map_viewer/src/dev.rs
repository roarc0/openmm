use bevy::{
    color::palettes::css,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    input::{common_conditions::input_toggle_active, ButtonInput},
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
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
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    wireframe_config.global = false;

    let val = ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.;
    let _points = [
        (
            Color::from(css::RED),
            Vec3::new(val, 0., val),
            Vec3::new(val, 0., -val),
        ),
        (
            Color::from(css::LIMEGREEN),
            Vec3::new(val, 0., val),
            Vec3::new(-val, 0., val),
        ),
        (
            Color::from(css::BLUE),
            Vec3::new(-val, 0., val),
            Vec3::new(-val, 0., -val),
        ),
        (
            Color::from(css::ORANGE),
            Vec3::new(val, 0., -val),
            Vec3::new(-val, 0., -val),
        ),
    ];

    commands.spawn((
        Text::new("FPS: "),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::WHITE),
        FpsText,
    ));

    commands.spawn((
        Text::new(" POS: "),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::WHITE),
        PositionText,
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
        wireframe_config.global = !wireframe_config.global;
    } else if keys.just_pressed(key_bindings.toggle_play_area) {
        dev_config.show_play_area = !dev_config.show_play_area;
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
    query2: Query<&Transform, With<FlyCam>>,
) {
    if let Ok(transform) = query2.single() {
        for mut text in &mut query {
            **text = format!(" POS: {:?}", transform.translation);
        }
    }
}

pub struct DevPlugin;
impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KeyBindings>()
            .insert_resource(DevConfig::default())
            .add_plugins((
                WireframePlugin::default(),
                LogDiagnosticsPlugin::default(),
                EguiPlugin::default(),
                WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            ))
            .add_systems(
                Update,
                (dev_input, update_fps_text, update_position_text)
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), dev_setup);
    }
}
