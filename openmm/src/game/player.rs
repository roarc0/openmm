use bevy::input::gamepad::{GamepadAxis, GamepadButton, GamepadInput};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use lod::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::GameState;

/// System set label for player input systems (movement, look, cursor).
/// Used by other systems to order themselves after player input is processed.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct PlayerInputSet;
use crate::config::GameConfig;
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

/// Runtime toggle for mouse look — initialized from config, toggled by CapsLock.
#[derive(Resource)]
pub struct MouseLookEnabled(pub bool);

/// Runtime mouse sensitivity multiplier — adjusted with Home/End keys.
#[derive(Resource)]
pub struct MouseSensitivity {
    pub x: f32,
    pub y: f32,
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
        let cfg = app.world().resource::<GameConfig>().clone();
        app.init_resource::<PlayerSettings>()
            .init_resource::<PlayerKeyBindings>()
            .insert_resource(MouseLookEnabled(cfg.mouse_look))
            .insert_resource(MouseSensitivity {
                x: cfg.mouse_sensitivity_x,
                y: cfg.mouse_sensitivity_y,
            })
            .add_systems(
                OnEnter(GameState::Game),
                (spawn_player, grab_cursor_on_enter),
            )
            .add_systems(
                Update,
                (toggle_fly_mode, toggle_mouse_look, adjust_sensitivity, player_movement, player_look, cursor_grab, log_gamepads)
                    .chain()
                    .in_set(PlayerInputSet)
                    .run_if(in_state(GameState::Game))
                    .run_if(resource_equals(crate::game::hud::HudView::World))
                    .run_if(|console: Res<crate::game::console::ConsoleState>| !console.open),
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
        // Indoor: always use start_points (set from LoadRequest.spawn_position
        // for MoveToMap, or sector center fallback for console/direct load).
        if let Some(sp) = indoor.start_points.first() {
            info!("Indoor spawn: pos={:?} yaw={:.1}", sp.position, sp.yaw.to_degrees());
            (sp.position.x, sp.position.y + settings.eye_height, sp.position.z, sp.yaw)
        } else {
            (0.0, settings.eye_height, 0.0, 0.0)
        }
    } else if let Some(ref prepared) = prepared {
        // Outdoor: prefer save_data position (set by MoveToMap / dungeon exit) if non-zero,
        // otherwise use the map's default "party start" spawn point
        let pos = save_data.player.position;
        let has_save_pos = pos[0] != 0.0 || pos[1] != 0.0 || pos[2] != 0.0;
        if has_save_pos {
            let y = sample_terrain_height(&prepared.map.height_map, pos[0], pos[2])
                + settings.eye_height;
            (pos[0], y, pos[2], save_data.player.yaw)
        } else {
            let party_start = prepared.start_points.iter()
                .find(|sp| sp.name.to_lowercase().contains("party start")
                         || sp.name.to_lowercase().contains("party_start"));
            if let Some(sp) = party_start {
                let y = sample_terrain_height(&prepared.map.height_map, sp.position.x, sp.position.z)
                    + settings.eye_height;
                (sp.position.x, y, sp.position.z, sp.yaw)
            } else {
                (0.0, settings.eye_height, 0.0, 0.0)
            }
        }
    } else {
        (save_data.player.position[0], save_data.player.position[1],
         save_data.player.position[2], save_data.player.yaw)
    };

    info!("Player spawn: pos=({:.1}, {:.1}, {:.1}) yaw={:.1}deg indoor={}",
        start_x, start_y, start_z, start_yaw.to_degrees(), is_indoor);

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
            SpatialListener::new(4.0),
            crate::bevy_config::camera_msaa(&cfg),
            Transform::from_rotation(Quat::from_rotation_x(-8.0_f32.to_radians())),
            Projection::Perspective(PerspectiveProjection {
                // MM6 FOV: 75 degrees outdoor, 60 degrees indoor (from OpenEnroth)
                fov: if is_indoor { 60.0_f32.to_radians() } else { 75.0_f32.to_radians() },
                near: 10.0,
                far: 100000.0,
                ..Default::default()
            }),
        ));
        if let Some(fxaa) = crate::bevy_config::camera_fxaa(&cfg) {
            cam.insert(fxaa);
        }
        if let Some(smaa) = crate::bevy_config::camera_smaa(&cfg) {
            cam.insert(smaa);
        }
        if let Some(taa) = crate::bevy_config::camera_taa(&cfg) {
            cam.insert(taa);
        }
        if let Some(bloom) = crate::bevy_config::camera_bloom(&cfg) {
            cam.insert(bloom);
        }
        // Depth/normal prepasses — only when SSAO is active (it requires them).
        // Note: prepasses break DOF and other effects, so don't enable unconditionally.
        if cfg.ssao {
            cam.insert(bevy::core_pipeline::prepass::DepthPrepass);
            cam.insert(bevy::core_pipeline::prepass::NormalPrepass);
        }
        if let Some(ssao) = crate::bevy_config::camera_ssao(&cfg) {
            cam.insert(ssao);
        }
        if let Some(motion_blur) = crate::bevy_config::camera_motion_blur(&cfg) {
            cam.insert(motion_blur);
        }
        if let Some(dof) = crate::bevy_config::camera_dof(&cfg) {
            cam.insert(dof);
        }
        // Always apply tonemapping + exposure for consistent physically-based rendering.
        // Bloom requires real tonemapping (not "none") — force AgX if needed.
        let tonemapping = if cfg.bloom && cfg.tonemapping == "none" {
            bevy::core_pipeline::tonemapping::Tonemapping::AgX
        } else {
            crate::bevy_config::camera_tonemapping(&cfg)
        };
        cam.insert(tonemapping);
        cam.insert(crate::bevy_config::camera_exposure(&cfg));
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
    mut world_state: ResMut<crate::game::world_state::WorldState>,
) {
    let gamepad_toggle = gamepads.iter().any(|gp| gp.just_pressed(GamepadButton::Select));
    if keys.just_pressed(key_bindings.toggle_fly) || gamepad_toggle {
        world_state.player.fly_mode = !world_state.player.fly_mode;
        info!("Fly mode: {}", if world_state.player.fly_mode { "ON" } else { "OFF" });
    }
}

fn toggle_mouse_look(
    keys: Res<ButtonInput<KeyCode>>,
    cfg: Res<GameConfig>,
    mut mouse_look: ResMut<MouseLookEnabled>,
) {
    if cfg.capslock_toggle_mouse_look && keys.just_pressed(KeyCode::CapsLock) {
        mouse_look.0 = !mouse_look.0;
        info!("Mouse look: {}", if mouse_look.0 { "ON" } else { "OFF" });
    }
}

/// Home/End adjust mouse sensitivity at runtime.
fn adjust_sensitivity(
    keys: Res<ButtonInput<KeyCode>>,
    mut sens: ResMut<MouseSensitivity>,
) {
    let step = 5.0;
    if keys.just_pressed(KeyCode::Home) {
        sens.x = (sens.x + step).min(200.0);
        sens.y = (sens.y + step).min(200.0);
        info!("Mouse sensitivity: {:.0}/{:.0}", sens.x, sens.y);
    }
    if keys.just_pressed(KeyCode::End) {
        sens.x = (sens.x - step).max(1.0);
        sens.y = (sens.y - step).max(1.0);
        info!("Mouse sensitivity: {:.0}/{:.0}", sens.x, sens.y);
    }
}

fn player_movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    settings: Res<PlayerSettings>,
    cfg: Res<GameConfig>,
    key_bindings: Res<PlayerKeyBindings>,
    world_state: Res<crate::game::world_state::WorldState>,
    height_map: Option<Res<TerrainHeightMap>>,
    colliders: Option<Res<BuildingColliders>>,
    door_colliders: Option<Res<crate::game::blv::DoorColliders>>,
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
    if !has_gamepad_input && matches!(cursor_options.grab_mode, CursorGrabMode::None) {
        return;
    }

    // always_run: use full speed; walk would be half speed
    let base_speed = if world_state.player.fly_mode {
        settings.fly_speed
    } else if cfg.always_run {
        settings.speed
    } else {
        settings.speed * 0.5
    };

    // turn_speed from config (degrees/sec → radians/sec)
    let turn_speed = cfg.turn_speed.to_radians();

    for (mut transform, mut physics) in query.iter_mut() {
        // Rotation / strafe based on always_strafe config
        if cfg.always_strafe {
            // Arrow left/right strafe instead of rotate
            // (rotation only via mouse look)
        } else {
            for key in keys.get_pressed() {
                let key = *key;
                if key == key_bindings.rotate_left {
                    transform.rotate_y(turn_speed * time.delta_secs());
                } else if key == key_bindings.rotate_right {
                    transform.rotate_y(-turn_speed * time.delta_secs());
                }
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
        // always_strafe: arrow keys also strafe
        if cfg.always_strafe {
            if keys.pressed(key_bindings.rotate_left) {
                movement -= right_flat;
            }
            if keys.pressed(key_bindings.rotate_right) {
                movement += right_flat;
            }
        }

        // Gamepad left stick: Y forward/back, X strafe
        if gp_left != Vec2::ZERO {
            movement += forward_flat * gp_left.y;
            movement += right_flat * gp_left.x;
        }

        if movement != Vec3::ZERO {
            movement = movement.normalize() * base_speed * time.delta_secs();

            if world_state.player.fly_mode {
                let from = transform.translation;
                let mut dest = from + movement;
                // BSP wall collision still applies when flying
                if let Some(ref c) = colliders {
                    dest = c.resolve_movement(from, dest, settings.collision_radius, settings.eye_height);
                }
                if let Some(ref dc) = door_colliders {
                    dest = dc.resolve_movement(from, dest, settings.collision_radius, settings.eye_height);
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
                    || world_state.player.fly_mode
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
                // Door collision
                if let Some(ref dc) = door_colliders {
                    dest = dc.resolve_movement(from, dest, settings.collision_radius, settings.eye_height);
                }

                transform.translation.x = dest.x;
                transform.translation.z = dest.z;
            }
        }

        // Jump (keyboard or gamepad South/A button)
        if !world_state.player.fly_mode && physics.on_ground && (keys.just_pressed(key_bindings.jump) || gp_jump) {
            physics.vertical_velocity = settings.jump_velocity;
            physics.on_ground = false;
        }

        // Fly vertical with BSP collision
        if world_state.player.fly_mode {
            let mut dy = 0.0_f32;
            if keys.pressed(key_bindings.fly_up) || gp_fly_up {
                dy += base_speed * time.delta_secs();
            }
            if keys.pressed(key_bindings.fly_down) || gp_fly_down {
                dy -= base_speed * time.delta_secs();
            }
            if dy.abs() > 0.0 {
                let from = transform.translation;
                let mut dest = from + Vec3::new(0.0, dy, 0.0);
                if let Some(ref c) = colliders {
                    dest = c.resolve_movement(from, dest, settings.collision_radius, settings.eye_height);
                }
                if let Some(ref dc) = door_colliders {
                    dest = dc.resolve_movement(from, dest, settings.collision_radius, settings.eye_height);
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
    mouse_sens: Res<MouseSensitivity>,
    mouse_look: Res<MouseLookEnabled>,
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

    // Mouse look active when runtime toggle is on AND cursor is grabbed
    let mouse_active = mouse_look.0 && !matches!(cursor_options.grab_mode, CursorGrabMode::None);

    let Ok(window) = primary_window.single() else {
        return;
    };

    let window_scale = window.height().min(window.width());
    let delta = accumulated.delta;

    // Per-axis sensitivity from runtime resource (scaled by base sensitivity factor)
    let sens_x = settings.sensitivity * mouse_sens.x;
    let sens_y = settings.sensitivity * mouse_sens.y;

    for mut transform in player_query.iter_mut() {
        // Mouse yaw
        if mouse_active {
            let yaw = -(sens_x * delta.x * window_scale).to_radians();
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
            pitch -= (sens_y * delta.y * window_scale).to_radians();
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
