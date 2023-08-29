use bevy::prelude::{App, Color, Input, KeyCode, Plugin, Res, ResMut, Resource, Update, Vec3};
use bevy_prototype_debug_lines::{DebugLines, DebugLinesPlugin};

/// Keeps track of mouse motion events, pitch, and yaw
#[derive(Resource)]
struct DebugAreaState {
    enabled: bool,
}

impl Default for DebugAreaState {
    fn default() -> Self {
        Self { enabled: true }
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

fn debug_area(state: Res<DebugAreaState>, mut lines: ResMut<DebugLines>) {
    if !state.enabled {
        return;
    }
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

/// Handles keyboard input for enabling/disabling debug area
fn debug_area_input(
    mut state: ResMut<DebugAreaState>,
    key_bindings: Res<KeyBindings>,
    keys: Res<Input<KeyCode>>,
) {
    if keys.just_pressed(key_bindings.toggle) {
        state.enabled = !state.enabled;
    }
}

pub struct DebugAreaPlugin;
impl Plugin for DebugAreaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugAreaState>()
            .add_plugins(DebugLinesPlugin::default())
            .init_resource::<KeyBindings>()
            .add_systems(Update, debug_area)
            .add_systems(Update, debug_area_input);
    }
}
