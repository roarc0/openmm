use bevy::input::gamepad::{GamepadAxis, GamepadButton, GamepadInput};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use lod::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::GameState;
use crate::game::InGame;
use crate::game::collision::{
    BuildingColliders, TerrainHeightMap, WaterMap, WaterWalking, sample_terrain_height,
};
use crate::save::GameSave;
use crate::states::loading::{PreparedIndoorWorld, PreparedWorld};

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
    pub jump_velocity: f32,
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
            eye_height: 160.0,
            gravity: 9800.0,
            max_slope_height: 200.0,
            jump_velocity: 1300.0,
            collision_radius: 24.0,
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
    pub jump: KeyCode,
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
            jump: KeyCode::Space,
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
                (spawn_player, grab_cursor_on_enter),
            )
            .add_systems(
                Update,
                (toggle_fly_mode, player_movement, player_look, cursor_grab, log_gamepads)
                    .chain()
                    .run_if(in_state(crate::game::interaction::InGameState::Playing)),
            );
    }
}

fn log_gamepads(gamepads: Query<(Entity, &Gamepad), Added<Gamepad>>) {
    for (entity, gp) in gamepads.iter() {
        info!("Gamepad connected: entity={:?} vendor={:?} product={:?}", entity, gp.vendor_id(), gp.product_id());
    }
}

// --- Setup ---

fn grab_cursor_on_enter(mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut cursor_options) = cursor_query.single_mut() {
        cursor_options.grab_mode = CursorGrabMode::Confined;
        cursor_options.visible = false;
    }
}

fn spawn_player(
    mut commands: Commands,
    prepared: Option<Res<PreparedWorld>>,
    indoor: Option<Res<PreparedIndoorWorld>>,
    settings: Res<PlayerSettings>,
    cfg: Res<crate::config::GameConfig>,
    save_data: Res<GameSave>,
) {
    let is_indoor = indoor.is_some();

    // Resolve spawn position
    let (start_x, start_y, start_z, start_yaw) = if let Some(ref indoor) = indoor {
        // Indoor: use start point directly (no terrain height sampling)
        let party_start = indoor.start_points.first();
        if let Some(sp) = party_start {
            (sp.position.x, sp.position.y + settings.eye_height, sp.position.z, sp.yaw)
        } else {
            // Fallback: origin
            (0.0, settings.eye_height, 0.0, 0.0)
        }
    } else if let Some(ref prepared) = prepared {
        let party_start = prepared.start_points.iter()
            .find(|sp| sp.name.to_lowercase().contains("party start")
                     || sp.name.to_lowercase().contains("party_start"));
        if let Some(sp) = party_start {
            let y = sample_terrain_height(&prepared.map.height_map, sp.position.x, sp.position.z)
                + settings.eye_height;
            (sp.position.x, y, sp.position.z, sp.yaw)
        } else {
            let y = sample_terrain_height(&prepared.map.height_map,
                save_data.player.position[0], save_data.player.position[2]) + settings.eye_height;
            (save_data.player.position[0], y, save_data.player.position[2], save_data.player.yaw)
        }
    } else {
        (save_data.player.position[0], save_data.player.position[1],
         save_data.player.position[2], save_data.player.yaw)
    };

    let mut player_entity = commands.spawn((
        Name::new("player"),
        Player,
        PlayerPhysics::default(),
        Transform::from_xyz(start_x, start_y, start_z)
            .with_rotation(Quat::from_rotation_y(start_yaw)),
        Visibility::default(),
        InGame,
    ));

    player_entity.with_children(|parent| {
        let mut cam = parent.spawn((
            Name::new("player_camera"),
            PlayerCamera,
            Camera3d::default(),
            bevy::ui::IsDefaultUiCamera,
            Transform::from_rotation(Quat::from_rotation_x(-8.0_f32.to_radians())),
            Projection::Perspective(PerspectiveProjection {
                fov: 50.0_f32.to_radians(),
                near: 10.0,
                far: 100000.0,
                ..Default::default()
            }),
        ));
        // Outdoor: distance fog for horizon blending. Indoor: no fog.
        if !is_indoor {
            cam.insert(DistanceFog {
                color: Color::srgba(0.45, 0.55, 0.8, 1.0),
                falloff: FogFalloff::Linear {
                    start: cfg.fog_start,
                    end: cfg.fog_end,
                },
                ..default()
            });
        }
    });
}

// --- Gamepad helpers ---

const STICK_DEADZONE: f32 = 0.15;

fn apply_deadzone(value: f32) -> f32 {
    if value.abs() < STICK_DEADZONE {
        0.0
    } else {
        (value - STICK_DEADZONE.copysign(value)) / (1.0 - STICK_DEADZONE)
    }
}

/// Read the right stick, falling back to LeftZ/RightZ for unmapped controllers.
fn right_stick_with_fallback(gp: &Gamepad) -> Vec2 {
    let standard = gp.right_stick();
    if standard != Vec2::ZERO {
        return standard;
    }
    // Unmapped controllers (e.g. GameSir) report right stick as LeftZ/RightZ
    Vec2::new(
        gp.get(GamepadInput::Axis(GamepadAxis::LeftZ)).unwrap_or(0.0),
        gp.get(GamepadInput::Axis(GamepadAxis::RightZ)).unwrap_or(0.0),
    )
}

// --- Movement ---

fn toggle_fly_mode(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<PlayerKeyBindings>,
    gamepads: Query<&Gamepad>,
    mut fly_mode: ResMut<FlyMode>,
) {
    let gamepad_toggle = gamepads.iter().any(|gp| gp.just_pressed(GamepadButton::Select));
    if keys.just_pressed(key_bindings.toggle_fly) || gamepad_toggle {
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
    cfg: Res<crate::config::GameConfig>,
    height_map: Option<Res<TerrainHeightMap>>,
    colliders: Option<Res<BuildingColliders>>,
    water_map: Option<Res<WaterMap>>,
    water_walking: Option<Res<WaterWalking>>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
    gamepads: Query<&Gamepad>,
    mut query: Query<(&mut Transform, &mut PlayerPhysics), With<Player>>,
) {
    // Read gamepad left stick (movement)
    let mut gp_left = Vec2::ZERO;
    let mut gp_jump = false;
    let mut gp_fly_up = false;
    let mut gp_fly_down = false;
    for gp in gamepads.iter() {
        let stick = gp.left_stick();
        gp_left.x += apply_deadzone(stick.x);
        gp_left.y += apply_deadzone(stick.y);
        gp_jump = gp_jump || gp.just_pressed(GamepadButton::South);
        gp_fly_up = gp_fly_up || gp.pressed(GamepadButton::RightTrigger2);
        gp_fly_down = gp_fly_down || gp.pressed(GamepadButton::LeftTrigger2);
    }
    let has_gamepad_input = gp_left != Vec2::ZERO || gp_jump || gp_fly_up || gp_fly_down;

    let Ok(cursor_options) = cursor_query.single() else {
        return;
    };
    if !has_gamepad_input && !cfg.auto_move && matches!(cursor_options.grab_mode, CursorGrabMode::None) {
        return;
    }

    let speed = if fly_mode.0 {
        settings.fly_speed
    } else {
        settings.speed
    };

    for (mut transform, mut physics) in query.iter_mut() {
        // Rotation (keyboard only — right stick handles look separately)
        for key in keys.get_pressed() {
            let key = *key;
            if key == key_bindings.rotate_left {
                transform.rotate_y(settings.rotation_speed * time.delta_secs());
            } else if key == key_bindings.rotate_right {
                transform.rotate_y(-settings.rotation_speed * time.delta_secs());
            }
        }

        // Horizontal movement
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

        // Gamepad left stick: Y forward/back, X strafe
        if gp_left != Vec2::ZERO {
            movement += forward_flat * gp_left.y;
            movement += right_flat * gp_left.x;
        }

        // Auto-move: walk forward and slowly rotate for a patrol pattern
        if cfg.auto_move && movement == Vec3::ZERO {
            movement += forward_flat;
            transform.rotate_y(0.15 * time.delta_secs());
        }

        if movement != Vec3::ZERO {
            movement = movement.normalize() * speed * time.delta_secs();

            if fly_mode.0 {
                let from = transform.translation;
                let mut dest = from + movement;
                // BSP wall collision still applies when flying
                if let Some(ref c) = colliders {
                    dest = c.resolve_movement(from, dest, settings.collision_radius, settings.eye_height);
                }
                transform.translation = dest;
            } else {
                let from = transform.translation;
                let mut dest = from + movement;

                // Terrain slope check — block movement if slope angle > ~35 degrees.
                // Uses the actual angle (atan of height/distance) not just height diff.
                if let Some(ref hm) = height_map {
                    let current_ground = sample_terrain_height(&hm.heights, from.x, from.z);
                    let dest_ground = sample_terrain_height(&hm.heights, dest.x, dest.z);
                    let height_diff = dest_ground - current_ground;

                    if height_diff > 0.0 {
                        let horiz_dist = ((dest.x - from.x).powi(2) + (dest.z - from.z).powi(2)).sqrt().max(0.1);
                        let slope_angle = (height_diff / horiz_dist).atan();

                        if slope_angle > 0.6 { // ~35 degrees, matches physics slide threshold
                            // Try sliding along each axis independently
                            let mut slid = from;
                            let gx = sample_terrain_height(&hm.heights, dest.x, from.z);
                            let dx = (dest.x - from.x).abs().max(0.1);
                            if ((gx - current_ground) / dx).atan() <= 0.6 {
                                slid.x = dest.x;
                            }
                            let gz = sample_terrain_height(&hm.heights, slid.x, dest.z);
                            let dz = (dest.z - from.z).abs().max(0.1);
                            if ((gz - current_ground) / dz).atan() <= 0.6 {
                                slid.z = dest.z;
                            }
                            dest = slid;
                        }
                    }
                }

                // Water check
                let can_enter_water = water_walking.as_ref().map_or(false, |w| w.0)
                    || fly_mode.0
                    || !physics.on_ground;
                if !can_enter_water {
                    if let Some(ref wm) = water_map {
                        if wm.is_water_at(dest.x, dest.z) && !wm.is_water_at(from.x, from.z) {
                            let feet_y = from.y - settings.eye_height;
                            let on_bridge = colliders
                                .as_ref()
                                .and_then(|c| c.floor_height_at(dest.x, dest.z, feet_y))
                                .is_some();
                            if !on_bridge {
                                dest = from;
                            }
                        }
                    }
                }

                // BSP wall collision
                if let Some(ref c) = colliders {
                    dest = c.resolve_movement(
                        from,
                        dest,
                        settings.collision_radius,
                        settings.eye_height,
                    );
                }

                transform.translation.x = dest.x;
                transform.translation.z = dest.z;
            }
        }

        // Jump (keyboard or gamepad South/A button)
        if !fly_mode.0 && physics.on_ground && (keys.just_pressed(key_bindings.jump) || gp_jump) {
            physics.vertical_velocity = settings.jump_velocity;
            physics.on_ground = false;
        }

        // Fly vertical with BSP collision
        if fly_mode.0 {
            let mut dy = 0.0;
            if keys.pressed(key_bindings.fly_up) || gp_fly_up {
                dy += speed * time.delta_secs();
            }
            if keys.pressed(key_bindings.fly_down) || gp_fly_down {
                dy -= speed * time.delta_secs();
            }
            if dy.abs() > 0.0 {
                let from = transform.translation;
                let mut dest = from + Vec3::new(0.0, dy, 0.0);
                if let Some(ref c) = colliders {
                    dest = c.resolve_movement(from, dest, settings.collision_radius, settings.eye_height);
                }
                transform.translation = dest;
            }
        }

        // No position clamp — the map transition system in odm.rs
        // detects when the player crosses the play area boundary and
        // loads the adjacent map automatically.
    }
}

// --- Camera look ---

fn player_look(
    settings: Res<PlayerSettings>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
    accumulated: Res<AccumulatedMouseMotion>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    gamepads: Query<&Gamepad>,
    time: Res<Time>,
    mut player_query: Query<&mut Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    // Gamepad right stick look (with fallback for unmapped controllers)
    let mut gp_look = Vec2::ZERO;
    for gp in gamepads.iter() {
        let stick = right_stick_with_fallback(gp);
        gp_look.x += apply_deadzone(stick.x);
        gp_look.y += apply_deadzone(stick.y);
    }

    let Ok(cursor_options) = cursor_query.single() else {
        return;
    };

    // Mouse look (only when cursor is grabbed)
    let mouse_active = !matches!(cursor_options.grab_mode, CursorGrabMode::None);

    let Ok(window) = primary_window.single() else {
        return;
    };

    let window_scale = window.height().min(window.width());
    let delta = accumulated.delta;

    for mut transform in player_query.iter_mut() {
        // Mouse yaw
        if mouse_active {
            let yaw = -(settings.sensitivity * delta.x * window_scale).to_radians();
            transform.rotate_y(yaw);
        }
        // Gamepad yaw (right stick X)
        if gp_look.x.abs() > 0.0 {
            transform.rotate_y(-gp_look.x * settings.rotation_speed * time.delta_secs());
        }
    }

    for mut transform in camera_query.iter_mut() {
        let (_, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        // Mouse pitch
        if mouse_active {
            pitch -= (settings.sensitivity * delta.y * window_scale).to_radians();
        }
        // Gamepad pitch (right stick Y) — reduced sensitivity, pitch isn't critical
        if gp_look.y.abs() > 0.0 {
            pitch += gp_look.y * settings.rotation_speed * 0.3 * time.delta_secs();
        }
        pitch = pitch.clamp(-1.54, 1.54);
        transform.rotation = Quat::from_axis_angle(Vec3::X, pitch);
    }
}

// --- Cursor ---

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
