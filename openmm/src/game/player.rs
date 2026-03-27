use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use lod::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::GameState;
use crate::game::InGame;
use crate::game::collision::{
    BuildingColliders, CollisionTriangle, TerrainHeightMap,
    ground_height_at, sample_terrain_height,
};
use crate::states::loading::PreparedWorld;

// --- Components ---

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component, Default)]
pub struct PlayerPhysics {
    pub vertical_velocity: f32,
    pub on_ground: bool,
}

// --- Resources ---

#[derive(Resource)]
pub struct FlyMode(pub bool);

impl Default for FlyMode {
    fn default() -> Self {
        Self(false)
    }
}

#[derive(Resource)]
pub struct PlayerSettings {
    pub sensitivity: f32,
    pub speed: f32,
    pub fly_speed: f32,
    pub rotation_speed: f32,
    pub eye_height: f32,
    pub gravity: f32,
    pub max_slope_height: f32,
    pub collision_radius: f32,
    pub max_xz: f32,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            sensitivity: 0.00006,
            speed: 2048.,
            fly_speed: 4096.,
            rotation_speed: 1.8,
            eye_height: 180.0,
            gravity: 9800.0,
            max_slope_height: 160.0,
            collision_radius: 64.0,
            max_xz: ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.0,
        }
    }
}

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

// --- Plugin ---

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerSettings>()
            .init_resource::<PlayerKeyBindings>()
            .init_resource::<FlyMode>()
            .add_systems(
                OnEnter(GameState::Game),
                (setup_player, grab_cursor_on_enter),
            )
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

// --- Setup ---

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

        // Build collision triangles from BSP model faces
        let mut walls = Vec::new();
        let mut floors = Vec::new();
        for model in &prepared.map.bsp_models {
            for face in &model.faces {
                if face.vertices_count < 3 || face.is_invisible() {
                    continue;
                }
                // Face normal: MM6 (x,y,z) → Bevy (x,z,-y)
                let nx = face.plane.normal[0] as f32 / 65536.0;
                let ny = face.plane.normal[2] as f32 / 65536.0;
                let nz = -face.plane.normal[1] as f32 / 65536.0;
                let normal = Vec3::new(nx, ny, nz);

                let is_floor = ny > 0.5;
                let is_wall = ny.abs() < 0.7;

                for i in 0..(face.vertices_count as usize).saturating_sub(2) {
                    let i0 = face.vertices_ids[0] as usize;
                    let i1 = face.vertices_ids[i + 1] as usize;
                    let i2 = face.vertices_ids[i + 2] as usize;
                    if i0 >= model.vertices.len()
                        || i1 >= model.vertices.len()
                        || i2 >= model.vertices.len()
                    {
                        continue;
                    }
                    let tri = CollisionTriangle {
                        v0: Vec3::from(model.vertices[i0]),
                        v1: Vec3::from(model.vertices[i1]),
                        v2: Vec3::from(model.vertices[i2]),
                        normal,
                    };
                    if is_wall {
                        walls.push(tri.clone());
                    }
                    if is_floor {
                        floors.push(tri);
                    }
                }
            }
        }
        commands.insert_resource(BuildingColliders { walls, floors });
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

// --- Systems ---

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
    colliders: Option<Res<BuildingColliders>>,
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
        // Rotation
        for key in keys.get_pressed() {
            let key = *key;
            if key == key_bindings.rotate_left {
                transform.rotate_y(settings.rotation_speed * time.delta_secs());
            } else if key == key_bindings.rotate_right {
                transform.rotate_y(-settings.rotation_speed * time.delta_secs());
            }
        }

        // Compute desired movement
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

            if fly_mode.0 {
                transform.translation += movement;
            } else {
                // Ground movement with terrain slope + BSP wall collision
                let from = transform.translation;
                let dest = from + movement;

                // Check BSP wall collision
                let wall_blocked = colliders
                    .as_ref()
                    .map_or(false, |c| c.blocked_by_wall(from, dest, settings.collision_radius));

                if wall_blocked {
                    // Try sliding along each axis independently
                    let dest_x = Vec3::new(from.x + movement.x, from.y, from.z);
                    let x_blocked = colliders
                        .as_ref()
                        .map_or(false, |c| c.blocked_by_wall(from, dest_x, settings.collision_radius));
                    if !x_blocked {
                        transform.translation.x = dest_x.x;
                    }

                    let dest_z = Vec3::new(transform.translation.x, from.y, from.z + movement.z);
                    let z_blocked = colliders
                        .as_ref()
                        .map_or(false, |c| c.blocked_by_wall(from, dest_z, settings.collision_radius));
                    if !z_blocked {
                        transform.translation.z = dest_z.z;
                    }
                } else if let Some(ref hm) = height_map {
                    // Terrain slope check
                    let current_ground = sample_terrain_height(
                        &hm.heights, from.x, from.z,
                    );
                    let dest_ground = sample_terrain_height(
                        &hm.heights, dest.x, dest.z,
                    );

                    if dest_ground - current_ground > settings.max_slope_height {
                        // Slide per-axis on steep terrain
                        let gx = sample_terrain_height(&hm.heights, dest.x, from.z);
                        if gx - current_ground <= settings.max_slope_height {
                            transform.translation.x = dest.x;
                        }
                        let gz = sample_terrain_height(
                            &hm.heights, transform.translation.x, dest.z,
                        );
                        if gz - current_ground <= settings.max_slope_height {
                            transform.translation.z = dest.z;
                        }
                    } else {
                        transform.translation += movement;
                    }
                } else {
                    transform.translation += movement;
                }
            }
        }

        // Fly mode vertical
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
            .translation.x
            .clamp(-settings.max_xz, settings.max_xz);
        transform.translation.z = transform
            .translation.z
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

    for mut transform in player_query.iter_mut() {
        let yaw = -(settings.sensitivity * delta.x * window_scale).to_radians();
        transform.rotate_y(yaw);
    }

    for mut transform in camera_query.iter_mut() {
        let (_, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        pitch -= (settings.sensitivity * delta.y * window_scale).to_radians();
        pitch = pitch.clamp(-1.54, 1.54);
        transform.rotation = Quat::from_axis_angle(Vec3::X, pitch);
    }
}

fn gravity_system(
    time: Res<Time>,
    height_map: Option<Res<TerrainHeightMap>>,
    colliders: Option<Res<BuildingColliders>>,
    settings: Res<PlayerSettings>,
    fly_mode: Res<FlyMode>,
    mut query: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
) {
    let Some(height_map) = height_map else {
        return;
    };

    let dt = time.delta_secs();

    for (mut transform, mut physics) in query.iter_mut() {
        let ground_y = ground_height_at(
            &height_map.heights,
            colliders.as_deref(),
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
            physics.vertical_velocity -= settings.gravity * dt;
            transform.translation.y += physics.vertical_velocity * dt;

            if transform.translation.y < ground_y {
                transform.translation.y = ground_y;
                physics.vertical_velocity = 0.0;
                physics.on_ground = true;
            } else if transform.translation.y - ground_y < 2.0 {
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
