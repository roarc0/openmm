use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use lod::odm::{ODM_HEIGHT_SCALE, ODM_PLAY_SIZE, ODM_SIZE, ODM_TILE_SCALE};

use crate::GameState;
use crate::game::InGame;
use crate::states::loading::PreparedWorld;

/// Marker for the player entity.
#[derive(Component)]
pub struct Player;

/// Marker for the player's camera (child of Player).
#[derive(Component)]
pub struct PlayerCamera;

/// Tracks vertical velocity and whether the player is on the ground.
#[derive(Component)]
pub struct PlayerPhysics {
    pub vertical_velocity: f32,
    pub on_ground: bool,
}

impl Default for PlayerPhysics {
    fn default() -> Self {
        Self {
            vertical_velocity: 0.0,
            on_ground: true,
        }
    }
}

/// Whether the player is currently flying.
#[derive(Resource)]
pub struct FlyMode(pub bool);

impl Default for FlyMode {
    fn default() -> Self {
        Self(false)
    }
}

/// Player movement and look settings.
#[derive(Resource)]
pub struct PlayerSettings {
    pub sensitivity: f32,
    pub speed: f32,
    pub fly_speed: f32,
    pub rotation_speed: f32,
    pub eye_height: f32,
    pub gravity: f32,
    pub max_slope_height: f32,
    pub max_xz: f32,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            sensitivity: 0.00006,
            speed: 2048.,
            fly_speed: 4096.,
            rotation_speed: 1.8,
            eye_height: 300.0,
            gravity: 9800.0,
            // Max terrain height difference the player can step up per move
            max_slope_height: 160.0,
            max_xz: ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.0,
        }
    }
}

/// Key bindings for player controls.
#[derive(Resource)]
pub struct PlayerKeyBindings {
    pub move_forward: KeyCode,
    pub move_backward: KeyCode,
    pub strafe_left: KeyCode,
    pub strafe_right: KeyCode,
    pub rotate_left: KeyCode,
    pub rotate_right: KeyCode,
    pub fly_up: KeyCode,
    pub fly_down: KeyCode,
    pub toggle_fly: KeyCode,
    pub toggle_grab_cursor: KeyCode,
}

impl Default for PlayerKeyBindings {
    fn default() -> Self {
        Self {
            move_forward: KeyCode::ArrowUp,
            move_backward: KeyCode::ArrowDown,
            strafe_left: KeyCode::KeyA,
            strafe_right: KeyCode::KeyD,
            rotate_left: KeyCode::ArrowLeft,
            rotate_right: KeyCode::ArrowRight,
            fly_up: KeyCode::PageUp,
            fly_down: KeyCode::PageDown,
            toggle_fly: KeyCode::F2,
            toggle_grab_cursor: KeyCode::Escape,
        }
    }
}

/// Cached terrain height data for sampling.
#[derive(Resource)]
pub struct TerrainHeightMap {
    pub heights: Vec<u8>,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSettings>()
            .init_resource::<PlayerKeyBindings>()
            .init_resource::<FlyMode>()
            .add_systems(OnEnter(GameState::Game), (setup_player, grab_cursor_on_enter))
            .add_systems(
                Update,
                (
                    toggle_fly_mode,
                    player_movement,
                    player_look,
                    cursor_grab,
                    gravity_system,
                )
                    .chain()
                    .run_if(in_state(GameState::Game)),
            );
    }
}

fn grab_cursor_on_enter(mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor_options) = cursor_query.single_mut() {
        cursor_options.grab_mode = CursorGrabMode::Confined;
        cursor_options.visible = false;
    }
}

fn setup_player(
    mut commands: Commands,
    prepared: Option<Res<PreparedWorld>>,
    settings: Res<PlayerSettings>,
) {
    if let Some(prepared) = &prepared {
        commands.insert_resource(TerrainHeightMap {
            heights: prepared.map.height_map.to_vec(),
        });
    }

    let start_x = 0.0_f32;
    let start_z = 0.0_f32;
    let start_y = if let Some(prepared) = &prepared {
        sample_terrain_height(&prepared.map.height_map, start_x, start_z) + settings.eye_height
    } else {
        1400.0
    };

    commands
        .spawn((
            Name::new("player"),
            Player,
            PlayerPhysics::default(),
            Transform::from_xyz(start_x, start_y, start_z),
            Visibility::default(),
            InGame,
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("player_camera"),
                PlayerCamera,
                Camera3d::default(),
                // Pitch the camera slightly down so the horizon sits at ~25% from top
                Transform::from_rotation(Quat::from_rotation_x(-8.0_f32.to_radians())),
                Projection::Perspective(PerspectiveProjection {
                    fov: 65.0_f32.to_radians(),
                    near: 10.0,
                    ..Default::default()
                }),
                DistanceFog {
                    color: Color::srgba(0.02, 0.02, 0.02, 0.70),
                    falloff: FogFalloff::Linear {
                        start: 20000.0,
                        end: 64000.0,
                    },
                    ..default()
                },
            ));
        });
}

/// Sample terrain height at a world position using bilinear interpolation.
/// Terrain mesh: x = (w - 64) * 512, z = (d - 64) * 512, y = height_map[d * 128 + w] * 32
pub fn sample_terrain_height(height_map: &[u8], world_x: f32, world_z: f32) -> f32 {
    let col_f = (world_x / ODM_TILE_SCALE) + 64.0;
    let row_f = (world_z / ODM_TILE_SCALE) + 64.0;

    let col0 = (col_f.floor() as usize).clamp(0, ODM_SIZE - 2);
    let row0 = (row_f.floor() as usize).clamp(0, ODM_SIZE - 2);
    let col1 = col0 + 1;
    let row1 = row0 + 1;

    let frac_col = (col_f - col0 as f32).clamp(0.0, 1.0);
    let frac_row = (row_f - row0 as f32).clamp(0.0, 1.0);

    let h00 = height_map[row0 * ODM_SIZE + col0] as f32 * ODM_HEIGHT_SCALE;
    let h10 = height_map[row0 * ODM_SIZE + col1] as f32 * ODM_HEIGHT_SCALE;
    let h01 = height_map[row1 * ODM_SIZE + col0] as f32 * ODM_HEIGHT_SCALE;
    let h11 = height_map[row1 * ODM_SIZE + col1] as f32 * ODM_HEIGHT_SCALE;

    let h_top = h00 + (h10 - h00) * frac_col;
    let h_bot = h01 + (h11 - h01) * frac_col;
    h_top + (h_bot - h_top) * frac_row
}

fn toggle_fly_mode(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<PlayerKeyBindings>,
    mut fly_mode: ResMut<FlyMode>,
) {
    if keys.just_pressed(key_bindings.toggle_fly) {
        fly_mode.0 = !fly_mode.0;
        info!("Fly mode: {}", if fly_mode.0 { "ON" } else { "OFF" });
    }
}

fn player_movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    settings: Res<PlayerSettings>,
    key_bindings: Res<PlayerKeyBindings>,
    fly_mode: Res<FlyMode>,
    height_map: Option<Res<TerrainHeightMap>>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(cursor_options) = cursor_query.single() else {
        return;
    };
    if matches!(cursor_options.grab_mode, CursorGrabMode::None) {
        return;
    }

    let speed = if fly_mode.0 {
        settings.fly_speed
    } else {
        settings.speed
    };

    for mut transform in query.iter_mut() {
        // Rotation via arrow keys
        for key in keys.get_pressed() {
            let key = *key;
            if key == key_bindings.rotate_left {
                let rotation = Quat::from_rotation_y(
                    settings.rotation_speed * time.delta_secs(),
                );
                transform.rotate(rotation);
            } else if key == key_bindings.rotate_right {
                let rotation = Quat::from_rotation_y(
                    -settings.rotation_speed * time.delta_secs(),
                );
                transform.rotate(rotation);
            }
        }

        // Horizontal movement — flatten to XZ plane
        let forward = transform.forward().as_vec3();
        let right = transform.right().as_vec3();
        let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
        let right_flat = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();

        let mut movement = Vec3::ZERO;
        if keys.pressed(key_bindings.move_forward) {
            movement += forward_flat;
        }
        if keys.pressed(key_bindings.move_backward) {
            movement -= forward_flat;
        }
        if keys.pressed(key_bindings.strafe_left) {
            movement -= right_flat;
        }
        if keys.pressed(key_bindings.strafe_right) {
            movement += right_flat;
        }

        if movement != Vec3::ZERO {
            movement = movement.normalize() * speed * time.delta_secs();

            if !fly_mode.0 {
                if let Some(ref hm) = height_map {
                    // Terrain-aware movement: check if destination is walkable
                    let dest = transform.translation + movement;
                    let current_ground = sample_terrain_height(
                        &hm.heights,
                        transform.translation.x,
                        transform.translation.z,
                    );
                    let dest_ground = sample_terrain_height(
                        &hm.heights,
                        dest.x,
                        dest.z,
                    );
                    let height_diff = dest_ground - current_ground;

                    if height_diff > settings.max_slope_height {
                        // Too steep uphill — slide along the slope
                        // Try each axis independently
                        let dest_x = Vec3::new(
                            transform.translation.x + movement.x,
                            transform.translation.y,
                            transform.translation.z,
                        );
                        let ground_x = sample_terrain_height(
                            &hm.heights, dest_x.x, dest_x.z,
                        );
                        if ground_x - current_ground <= settings.max_slope_height {
                            transform.translation.x = dest_x.x;
                        }

                        let dest_z = Vec3::new(
                            transform.translation.x,
                            transform.translation.y,
                            transform.translation.z + movement.z,
                        );
                        let ground_z = sample_terrain_height(
                            &hm.heights, dest_z.x, dest_z.z,
                        );
                        if ground_z - current_ground <= settings.max_slope_height {
                            transform.translation.z = dest_z.z;
                        }
                    } else {
                        // Walkable slope — move normally
                        transform.translation += movement;
                    }
                } else {
                    transform.translation += movement;
                }
            } else {
                transform.translation += movement;
            }
        }

        // Vertical movement in fly mode
        if fly_mode.0 {
            if keys.pressed(key_bindings.fly_up) {
                transform.translation.y += speed * time.delta_secs();
            }
            if keys.pressed(key_bindings.fly_down) {
                transform.translation.y -= speed * time.delta_secs();
            }
        }

        // Clamp to play area
        transform.translation.x = transform
            .translation
            .x
            .clamp(-settings.max_xz, settings.max_xz);
        transform.translation.z = transform
            .translation
            .z
            .clamp(-settings.max_xz, settings.max_xz);
    }
}

fn player_look(
    settings: Res<PlayerSettings>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
    accumulated: Res<AccumulatedMouseMotion>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut player_query: Query<&mut Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    let (Ok(cursor_options), Ok(window)) = (cursor_query.single(), primary_window.single()) else {
        return;
    };
    if matches!(cursor_options.grab_mode, CursorGrabMode::None) {
        return;
    }

    let window_scale = window.height().min(window.width());
    let delta = accumulated.delta;

    // Yaw rotates the player entity
    for mut transform in player_query.iter_mut() {
        let yaw = -(settings.sensitivity * delta.x * window_scale).to_radians();
        transform.rotate_y(yaw);
    }

    // Pitch rotates the camera child
    for mut transform in camera_query.iter_mut() {
        let (_, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        pitch -= (settings.sensitivity * delta.y * window_scale).to_radians();
        pitch = pitch.clamp(-1.54, 1.54);
        transform.rotation = Quat::from_axis_angle(Vec3::X, pitch);
    }
}

/// Applies gravity when walking, or terrain clamping when flying below ground.
fn gravity_system(
    time: Res<Time>,
    height_map: Option<Res<TerrainHeightMap>>,
    settings: Res<PlayerSettings>,
    fly_mode: Res<FlyMode>,
    mut query: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
) {
    let Some(height_map) = height_map else {
        return;
    };

    let dt = time.delta_secs();

    for (mut transform, mut physics) in query.iter_mut() {
        let ground_y = sample_terrain_height(
            &height_map.heights,
            transform.translation.x,
            transform.translation.z,
        ) + settings.eye_height;

        if fly_mode.0 {
            physics.vertical_velocity = 0.0;
            physics.on_ground = false;
            if transform.translation.y < ground_y {
                transform.translation.y = ground_y;
            }
        } else {
            // Apply gravity
            physics.vertical_velocity -= settings.gravity * dt;
            transform.translation.y += physics.vertical_velocity * dt;

            // Always enforce terrain floor
            if transform.translation.y < ground_y {
                transform.translation.y = ground_y;
                physics.vertical_velocity = 0.0;
                physics.on_ground = true;
            } else if transform.translation.y - ground_y < 2.0 {
                // Snap when very close
                transform.translation.y = ground_y;
                physics.vertical_velocity = 0.0;
                physics.on_ground = true;
            } else {
                physics.on_ground = false;
            }
        }
    }
}

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

fn cursor_grab(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<PlayerKeyBindings>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if let Ok(mut cursor_options) = cursor_query.single_mut() {
        if keys.just_pressed(key_bindings.toggle_grab_cursor) {
            toggle_grab_cursor(&mut cursor_options);
        }
    }
}
