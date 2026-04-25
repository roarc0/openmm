use bevy::ecs::system::SystemParam;
use bevy::input::gamepad::{GamepadAxis, GamepadButton, GamepadInput};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::game::map::collision::{
    BuildingColliders, MAX_STEP_UP, TerrainHeightMap, WaterMap, WaterWalking, sample_terrain_height,
};
use crate::game::spawn::WorldObstacle;
use crate::prepare::loading::PreparedIndoorWorld;
use crate::system::config::GameConfig;

use crate::game::actors::Actor;
use super::{
    MouseLookEnabled, MouseSensitivity, Player, PlayerCamera, PlayerKeyBindings, PlayerPhysics, PlayerSettings,
    SpeedMultiplier,
};

// --- Map collision bundle ---

/// All optional per-map collision resources bundled into a single SystemParam
/// to stay within Bevy's 16-parameter limit on system functions.
#[derive(SystemParam)]
pub(super) struct MapColliders<'w> {
    indoor_world: Option<Res<'w, PreparedIndoorWorld>>,
    height_map: Option<Res<'w, TerrainHeightMap>>,
    colliders: Option<Res<'w, BuildingColliders>>,
    door_colliders: Option<Res<'w, crate::game::map::indoor::DoorColliders>>,
    water_map: Option<Res<'w, WaterMap>>,
    water_walking: Option<Res<'w, WaterWalking>>,
}

// --- Constants ---

/// Indoor maps are roughly half the scale of outdoor — reduce walk speed accordingly
/// so the player feels similarly paced relative to the room sizes.
const INDOOR_SPEED_SCALE: f32 = 0.6;

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

pub(super) fn toggle_run_mode(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<PlayerKeyBindings>,
    mut world_state: ResMut<crate::game::state::WorldState>,
) {
    if keys.just_pressed(key_bindings.toggle_run) {
        world_state.player.is_running = !world_state.player.is_running;
        info!("Run mode: {}", if world_state.player.is_running { "ON" } else { "OFF" });
    }
}

pub(super) fn toggle_fly_mode(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<PlayerKeyBindings>,
    gamepads: Query<&Gamepad>,
    physics_query: Query<&PlayerPhysics, With<Player>>,
    mut world_state: ResMut<crate::game::state::WorldState>,
) {
    let gamepad_toggle = gamepads.iter().any(|gp| gp.just_pressed(GamepadButton::Select));
    if keys.just_pressed(key_bindings.toggle_fly) || gamepad_toggle {
        world_state.player.fly_mode = false;
        info!("Fly mode: OFF");
    }
    // Disengage fly when touching ground or a BSP floor surface
    if world_state.player.fly_mode
        && let Ok(physics) = physics_query.single()
        && physics.on_ground
    {
        world_state.player.fly_mode = false;
        info!("Fly mode: OFF (landed)");
    }
}

pub(super) fn toggle_mouse_look(
    keys: Res<ButtonInput<KeyCode>>,
    cfg: Res<GameConfig>,
    mut mouse_look: ResMut<MouseLookEnabled>,
) {
    if cfg.capslock_toggle_mouse_look && keys.just_pressed(KeyCode::F3) {
        mouse_look.0 = !mouse_look.0;
        info!("Mouse look: {}", if mouse_look.0 { "ON" } else { "OFF" });
    }
}

/// Moves from `from` towards `dest` applying wall collision in substeps.
/// Each substep is at most `radius` units long, preventing tunnelling at high speeds.
fn move_with_substeps(
    from: Vec3,
    dest: Vec3,
    radius: f32,
    eye_height: f32,
    colliders: Option<&BuildingColliders>,
    door_colliders: Option<&crate::game::map::indoor::DoorColliders>,
) -> Vec3 {
    let movement = dest - from;
    let dist = movement.length();
    if dist < 0.001 {
        return from;
    }
    let steps = ((dist / radius).ceil() as u32).max(1);
    let step = movement / steps as f32;
    let mut pos = from;
    for _ in 0..steps {
        let step_start = pos; // saved for rollback if a door panel blocks this substep
        let step_dest = pos + step;
        if let Some(c) = colliders {
            pos = c.resolve_movement(pos, step_dest, radius, eye_height);
        } else {
            pos = step_dest;
        }
        // Pass `pos` (already resolved by static collider) as `to` — not `step_dest`.
        // Using step_dest here would override the static collider's push, letting the
        // player walk through static walls whenever the door collider has no active walls.
        if let Some(dc) = door_colliders {
            pos = dc.resolve_movement(step_start, pos, radius, eye_height);
            // Block movement into areas closed off by horizontal door panels (trapdoors, slabs).
            // If the new position intersects a closed panel, roll back this substep.
            let feet_y = pos.y - eye_height;
            if dc.blocks_entry(pos.x, pos.z, feet_y, pos.y, radius) {
                pos = step_start;
            }
        }
    }
    pos
}

/// Push `pos` out of all `WorldObstacle` cylinders that overlap it in XZ.
///
/// Each obstacle is a vertical cylinder centred at `obs_tf.translation` with
/// the given radius. A Y-height guard skips obstacles whose centre is more
/// than `2 * eye_height` away vertically (different floors / aerial actors).
fn push_out_of_obstacles(
    mut pos: Vec3,
    player_radius: f32,
    eye_height: f32,
    obstacles: &Query<(&Transform, &WorldObstacle, Option<&Actor>), Without<Player>>,
) -> Vec3 {
    for (obs_tf, obs, actor) in obstacles.iter() {
        if let Some(actor) = actor {
            if actor.hp <= 0 {
                continue;
            }
        }
        let obs_pos = obs_tf.translation;
        if (obs_pos.y - pos.y).abs() > eye_height * 2.0 {
            continue;
        }
        let dx = pos.x - obs_pos.x;
        let dz = pos.z - obs_pos.z;
        let dist_sq = dx * dx + dz * dz;
        let min_dist = player_radius + obs.radius;
        if dist_sq < min_dist * min_dist && dist_sq > 0.001 {
            let dist = dist_sq.sqrt();
            let push = min_dist - dist;
            pos.x += dx / dist * push;
            pos.z += dz / dist * push;
        }
    }
    pos
}

pub(super) fn player_movement(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    settings: Res<PlayerSettings>,
    cfg: Res<GameConfig>,
    key_bindings: Res<PlayerKeyBindings>,
    world_state: Res<crate::game::state::WorldState>,
    speed_mul: Res<SpeedMultiplier>,
    map: MapColliders<'_>,
    obstacles: Query<(&Transform, &WorldObstacle, Option<&Actor>), Without<Player>>,
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

    let mut base_speed = if world_state.player.fly_mode {
        settings.fly_speed
    } else if world_state.player.is_running {
        settings.run_speed
    } else {
        settings.speed
    };
    if map.indoor_world.is_some() {
        base_speed *= INDOOR_SPEED_SCALE;
    }
    base_speed *= speed_mul.0;

    // turn_speed from config (degrees/sec → radians/sec)
    let turn_speed = cfg.turn_speed.to_radians();

    for (mut transform, mut physics) in query.iter_mut() {
        // Keyboard locomotion is arrow-only:
        // - Up/Down: move forward/back
        // - Left/Right: rotate (or strafe when always_strafe is enabled)
        // Other keyboard movement keys are intentionally ignored.
        // Rotation / strafe based on always_strafe config
        if cfg.always_strafe {
            // Arrow left/right strafe instead of rotate
            // (rotation only via mouse look)
        } else {
            for key in keys.get_pressed() {
                let key = *key;
                if key == KeyCode::ArrowLeft {
                    transform.rotate_y(turn_speed * time.delta_secs());
                } else if key == KeyCode::ArrowRight {
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
        if keys.pressed(KeyCode::ArrowUp) {
            movement += forward_flat;
        }
        if keys.pressed(KeyCode::ArrowDown) {
            movement -= forward_flat;
        }
        // always_strafe: arrow keys also strafe
        if cfg.always_strafe {
            if keys.pressed(KeyCode::ArrowLeft) {
                movement -= right_flat;
            }
            if keys.pressed(KeyCode::ArrowRight) {
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
                let dest = from + movement;
                let resolved = move_with_substeps(
                    from,
                    dest,
                    settings.collision_radius,
                    settings.eye_height,
                    map.colliders.as_deref(),
                    map.door_colliders.as_deref(),
                );
                transform.translation =
                    push_out_of_obstacles(resolved, settings.collision_radius, settings.eye_height, &obstacles);
            } else {
                let from = transform.translation;
                let mut dest = from + movement;

                // Terrain slope check — block movement if slope angle > ~35 degrees.
                // Skip entirely when the player is on BSP floor geometry (e.g. inside a castle):
                // the terrain heightmap doesn't match the building floor and would wrongly block stairs.
                let on_bsp_floor = map
                    .colliders
                    .as_ref()
                    .and_then(|c| {
                        let feet_y = from.y - settings.eye_height;
                        c.floor_height_at(from.x, from.z, feet_y, MAX_STEP_UP)
                    })
                    .is_some();
                if !on_bsp_floor && let Some(ref hm) = map.height_map {
                    let current_ground = sample_terrain_height(&hm.heights, from.x, from.z);
                    let dest_ground = sample_terrain_height(&hm.heights, dest.x, dest.z);
                    let height_diff = dest_ground - current_ground;

                    if height_diff > 0.0 {
                        let horiz_dist = ((dest.x - from.x).powi(2) + (dest.z - from.z).powi(2)).sqrt().max(0.1);
                        let slope_angle = (height_diff / horiz_dist).atan();

                        if slope_angle > 0.6 {
                            // ~35 degrees, matches physics slide threshold
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
                } // !on_bsp_floor

                // Water check
                let can_enter_water = map.water_walking.as_ref().is_some_and(|w| w.0)
                    || world_state.player.fly_mode
                    || !physics.on_ground;
                if !can_enter_water
                    && let Some(ref wm) = map.water_map
                    && wm.is_water_at(dest.x, dest.z)
                    && !wm.is_water_at(from.x, from.z)
                {
                    let feet_y = from.y - settings.eye_height;
                    let on_bridge = map
                        .colliders
                        .as_ref()
                        .and_then(|c| c.floor_height_at(dest.x, dest.z, feet_y, MAX_STEP_UP))
                        .is_some();
                    if !on_bridge {
                        dest = from;
                    }
                }

                let resolved = move_with_substeps(
                    from,
                    dest,
                    settings.collision_radius,
                    settings.eye_height,
                    map.colliders.as_deref(),
                    map.door_colliders.as_deref(),
                );
                let resolved =
                    push_out_of_obstacles(resolved, settings.collision_radius, settings.eye_height, &obstacles);
                transform.translation.x = resolved.x;
                transform.translation.z = resolved.z;
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
                if let Some(ref c) = map.colliders {
                    dest = c.resolve_movement(from, dest, settings.collision_radius, settings.eye_height);
                }
                if let Some(ref dc) = map.door_colliders {
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

pub(super) fn player_look(
    keys: Res<ButtonInput<KeyCode>>,
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

    const PITCH_STEP: f32 = 10.0 * std::f32::consts::PI / 180.0;

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
        // Keyboard pitch: PageDown = look up, Delete = look down, End = reset to level
        if keys.just_pressed(KeyCode::PageDown) {
            pitch += PITCH_STEP;
        }
        if keys.just_pressed(KeyCode::Delete) {
            pitch -= PITCH_STEP;
        }
        if keys.just_pressed(KeyCode::End) {
            pitch = 0.0;
        }
        // Constrain pitch to match MM6's limited vertical view angle (~45 degrees)
        pitch = pitch.clamp(-std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_4);
        transform.rotation = Quat::from_axis_angle(Vec3::X, pitch);
    }
}

// --- Cursor ---

/// ESC two-stage: first press ungrab cursor, second press (cursor already free) opens options.
pub(super) fn cursor_grab(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<PlayerKeyBindings>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut actions: Option<bevy::ecs::message::MessageWriter<crate::screens::runtime::ScreenActions>>,
    screen_layers: Option<Res<crate::screens::runtime::ScreenLayers>>,
) {
    if !keys.just_pressed(key_bindings.toggle_grab_cursor) {
        return;
    }

    // Mutually exclusive: if an interactive Modal screen is open, it handles Escape exclusively.
    if let Some(ref layers) = screen_layers
        && layers.has_modal()
    {
        return;
    }

    let Ok(mut cursor_options) = cursor_query.single_mut() else {
        return;
    };
    if cursor_options.grab_mode != CursorGrabMode::None {
        // First ESC: ungrab cursor.
        cursor_options.grab_mode = CursorGrabMode::None;
        cursor_options.visible = true;
    } else {
        // Second ESC: open options (keep cursor unlocked).
        if let Some(ref mut writer) = actions {
            writer.write(crate::screens::runtime::ScreenActions {
                actions: vec!["ShowScreen(\"options_main\")".to_string()],
            });
        }
    }
}
