use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;

/// Mouse sensitivity and movement speed
#[derive(Resource)]
pub struct MovementSettings {
    pub sensitivity: f32,
    pub speed: f32,
    pub rotation_speed: f32,
    pub max_xz: f32,
    pub max_y: f32,
}

impl Default for MovementSettings {
    fn default() -> Self {
        Self {
            sensitivity: 0.00012,
            speed: 4096.,
            rotation_speed: 3.5,
            max_xz: 512.0 * 64.0,
            max_y: 512.0 * 64.0,
        }
    }
}

/// Key configuration
#[derive(Resource)]
pub struct KeyBindings {
    pub move_forward: KeyCode,
    pub move_backward: KeyCode,
    pub rotate_left: KeyCode,
    pub rotate_right: KeyCode,
    pub move_ascend: KeyCode,
    pub move_descend: KeyCode,
    pub toggle_grab_cursor: KeyCode,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            move_forward: KeyCode::ArrowUp,
            move_backward: KeyCode::ArrowDown,
            rotate_left: KeyCode::ArrowLeft,
            rotate_right: KeyCode::ArrowRight,
            move_ascend: KeyCode::PageUp,
            move_descend: KeyCode::Insert,
            toggle_grab_cursor: KeyCode::Escape,
        }
    }
}

/// A marker component used in queries when you want flycams and not other cameras
#[derive(Component)]
pub struct FlyCam;

/// Grabs/ungrabs mouse cursor
fn toggle_grab_cursor(cursor_options: &mut CursorOptions) {
    match cursor_options.grab_mode {
        CursorGrabMode::None => {
            cursor_options.grab_mode = CursorGrabMode::Confined;
            cursor_options.visible = false;
        }
        _ => {
            cursor_options.grab_mode = CursorGrabMode::None;
            cursor_options.visible = true;
        }
    }
}

/// Spawns the `Camera3d` to be controlled
fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-11700.0, 1400.0, 11300.0).looking_at(Vec3::ZERO, Vec3::Y),
        Projection::Perspective(PerspectiveProjection {
            fov: 65.0_f32.to_radians(),
            ..Default::default()
        }),
        FlyCam,
        DistanceFog {
            color: Color::srgba(0.02, 0.02, 0.02, 0.70),
            falloff: FogFalloff::Linear {
                start: 20000.0,
                end: 64000.0,
            },
            ..default()
        },
    ));
}

/// Handles keyboard input and movement
fn player_controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
    settings: Res<MovementSettings>,
    key_bindings: Res<KeyBindings>,
    mut query: Query<(&FlyCam, &mut Transform)>,
) {
    if let Ok(cursor_options) = cursor_query.single() {
        for (_camera, mut transform) in query.iter_mut() {
            for key in keys.get_pressed() {
                match cursor_options.grab_mode {
                    CursorGrabMode::None => (),
                    _ => {
                        let key = *key;

                        let rotation = if key == key_bindings.rotate_left {
                            1
                        } else if key == key_bindings.rotate_right {
                            -1
                        } else {
                            0
                        };
                        if rotation != 0 {
                            let rotation = Quat::from_rotation_y(
                                rotation as f32 * settings.rotation_speed.to_radians(),
                            );
                            transform.rotate(rotation);
                        } else {
                            handle_movement(&settings, &key_bindings, key, &mut transform, &time);
                        }
                    }
                }
            }
        }
    } else {
        warn!("Primary window not found for `player_move`!");
    }
}

fn handle_movement(
    settings: &Res<MovementSettings>,
    key_bindings: &KeyBindings,
    key: KeyCode,
    transform: &mut Transform,
    time: &Time,
) {
    let local_z = transform.local_z();
    let movement = match key {
        k if k == key_bindings.move_forward => -local_z,
        k if k == key_bindings.move_backward => local_z,
        k if k == key_bindings.move_ascend => Dir3::from_xyz(0.0, 1.0, 0.0).unwrap(),
        k if k == key_bindings.move_descend => Dir3::from_xyz(0.0, -1.0, 0.0).unwrap(),
        _ => return,
    };

    transform.translation += movement * time.delta_secs() * settings.speed;

    limit_movement_to_game_area(settings, transform);
}

fn limit_movement_to_game_area(settings: &Res<'_, MovementSettings>, transform: &mut Transform) {
    if transform.translation.x.abs() > settings.max_xz {
        transform.translation.x = settings.max_xz * transform.translation.x.signum();
    }
    if transform.translation.z.abs() > settings.max_xz {
        transform.translation.z = settings.max_xz * transform.translation.z.signum();
    }
    if transform.translation.y > settings.max_y {
        transform.translation.y = settings.max_y * transform.translation.y.signum();
    }
    if transform.translation.y < 0. {
        transform.translation.y = 0. * transform.translation.y.signum();
    }
}

/// Handles looking around if cursor is locked
fn player_look(
    settings: Res<MovementSettings>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
    accumulated: Res<AccumulatedMouseMotion>,
    mut query: Query<&mut Transform, With<FlyCam>>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
) {
    if let (Ok(cursor_options), Ok(window)) = (cursor_query.single(), primary_window.single()) {
        for mut transform in query.iter_mut() {
            let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
            match cursor_options.grab_mode {
                CursorGrabMode::None => (),
                _ => {
                    let window_scale = window.height().min(window.width());
                    let delta = accumulated.delta;
                    pitch -= (settings.sensitivity * delta.y * window_scale).to_radians();
                    yaw -= (settings.sensitivity * delta.x * window_scale).to_radians();
                }
            }

            pitch = pitch.clamp(-1.54, 1.54);
            transform.rotation =
                Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch);
        }
    } else {
        warn!("Primary window not found for `player_look`!");
    }
}

fn cursor_grab(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<KeyBindings>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if let Ok(mut cursor_options) = cursor_query.single_mut() {
        if keys.just_pressed(key_bindings.toggle_grab_cursor) {
            toggle_grab_cursor(&mut cursor_options);
        }
    } else {
        warn!("Primary window not found for `cursor_grab`!");
    }
}

/// Contains everything needed to add first-person fly camera behaviour to your game
pub struct PlayerPlugin;
impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MovementSettings>()
            .init_resource::<KeyBindings>()
            .add_systems(OnEnter(GameState::Game), setup_camera)
            .add_systems(
                Update,
                (player_controls, player_look, cursor_grab).run_if(in_state(GameState::Game)),
            );
    }
}
