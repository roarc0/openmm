use bevy::input::gamepad::{GamepadAxis, GamepadButton, GamepadInput};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use openmm_data::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::GameState;

/// System set label for player input systems (movement, look, cursor).
/// Used by other systems to order themselves after player input is processed.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct PlayerInputSet;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::collision::{
    BuildingColliders, MAX_STEP_UP, TerrainHeightMap, WaterMap, WaterWalking, sample_terrain_height,
};
use crate::save::GameSave;
use crate::states::loading::{PreparedIndoorWorld, PreparedWorld};

// --- Constants ---

const WALK_SPEED: f32 = 1024.0;
const RUN_SPEED: f32 = WALK_SPEED * 1.25;
const FLY_SPEED: f32 = 2048.0;
const ROTATION_SPEED: f32 = 1.8;
const EYE_HEIGHT: f32 = 140.0;
const GRAVITY: f32 = 9800.0;
const MAX_SLOPE_HEIGHT: f32 = 200.0;
const JUMP_VELOCITY: f32 = 1300.0;
const COLLISION_RADIUS: f32 = 24.0;

// --- Components ---

#[derive(Component)]
pub struct Player;

/// Marker for the bright inner torch light (close-range hot core).
#[derive(Component)]
pub struct PartyTorch;

/// Marker for the dim outer torch fill (wide ambient bleed).
#[derive(Component)]
pub struct PartyTorchFill;

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
    pub run_speed: f32,
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
            speed: WALK_SPEED,
            run_speed: RUN_SPEED,
            fly_speed: FLY_SPEED,
            rotation_speed: ROTATION_SPEED,
            eye_height: EYE_HEIGHT,
            gravity: GRAVITY,
            max_slope_height: MAX_SLOPE_HEIGHT,
            jump_velocity: JUMP_VELOCITY,
            collision_radius: COLLISION_RADIUS,
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
    pub toggle_run: KeyCode,
    pub toggle_grab_cursor: KeyCode,
}

impl Default for PlayerKeyBindings {
    fn default() -> Self {
        Self {
            move_forward: KeyCode::KeyW,
            move_backward: KeyCode::KeyS,
            strafe_left: KeyCode::KeyQ,
            strafe_right: KeyCode::KeyE,
            rotate_left: KeyCode::ArrowLeft,
            rotate_right: KeyCode::ArrowRight,
            jump: KeyCode::Space,
            fly_up: KeyCode::Insert,
            fly_down: KeyCode::PageUp,
            toggle_fly: KeyCode::Home,
            toggle_run: KeyCode::CapsLock,
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
            .insert_resource(MouseLookEnabled(true))
            .insert_resource(MouseSensitivity {
                x: cfg.mouse_sensitivity_x,
                y: cfg.mouse_sensitivity_y,
            })
            .add_systems(OnEnter(GameState::Game), (spawn_player, grab_cursor_on_enter))
            .add_systems(
                Update,
                (
                    toggle_run_mode,
                    toggle_fly_mode,
                    toggle_mouse_look,
                    player_movement,
                    player_look,
                    cursor_grab,
                    log_gamepads,
                )
                    .chain()
                    .in_set(PlayerInputSet)
                    .run_if(in_state(GameState::Game))
                    .run_if(crate::game::hud::game_input_active),
            )
            .add_systems(Update, party_torch_system.run_if(in_state(GameState::Game)));
    }
}

fn log_gamepads(gamepads: Query<(Entity, &Gamepad), Added<Gamepad>>) {
    for (entity, gp) in gamepads.iter() {
        info!(
            "Gamepad connected: entity={:?} vendor={:?} product={:?}",
            entity,
            gp.vendor_id(),
            gp.product_id()
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
            (
                sp.position.x,
                sp.position.y + settings.eye_height,
                sp.position.z,
                sp.yaw,
            )
        } else {
            (0.0, settings.eye_height, 0.0, 0.0)
        }
    } else if let Some(ref prepared) = prepared {
        // Outdoor: prefer save_data position (set by MoveToMap / dungeon exit) if non-zero,
        // otherwise use the map's default "party start" spawn point
        let pos = save_data.player.position;
        let has_save_pos = pos[0] != 0.0 || pos[1] != 0.0 || pos[2] != 0.0;
        if has_save_pos {
            let y = sample_terrain_height(&prepared.map.height_map, pos[0], pos[2]) + settings.eye_height;
            (pos[0], y, pos[2], save_data.player.yaw)
        } else {
            let party_start = prepared.start_points.iter().find(|sp| {
                sp.name.to_lowercase().contains("party start") || sp.name.to_lowercase().contains("party_start")
            });
            if let Some(sp) = party_start {
                let y =
                    sample_terrain_height(&prepared.map.height_map, sp.position.x, sp.position.z) + settings.eye_height;
                (sp.position.x, y, sp.position.z, sp.yaw)
            } else {
                (0.0, settings.eye_height, 0.0, 0.0)
            }
        }
    } else {
        (
            save_data.player.position[0],
            save_data.player.position[1],
            save_data.player.position[2],
            save_data.player.yaw,
        )
    };

    info!(
        "Player spawn: pos=({:.1}, {:.1}, {:.1}) yaw={:.1}deg indoor={}",
        start_x,
        start_y,
        start_z,
        start_yaw.to_degrees(),
        is_indoor
    );

    let mut player_entity = commands.spawn((
        Name::new("player"),
        Player,
        PlayerPhysics::default(),
        Transform::from_xyz(start_x, start_y, start_z).with_rotation(Quat::from_rotation_y(start_yaw)),
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
            Transform::from_rotation(Quat::from_rotation_x(10.0_f32.to_radians())),
            Projection::Perspective(PerspectiveProjection {
                // MM6 FOV: 75 degrees outdoor, 60 degrees indoor (from OpenEnroth)
                fov: if is_indoor {
                    60.0_f32.to_radians()
                } else {
                    75.0_f32.to_radians()
                },
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
        if !is_indoor {
            // Outdoor: horizon haze blending into sky colour.
            cam.insert(DistanceFog {
                color: Color::srgba(0.45, 0.55, 0.8, 1.0),
                falloff: FogFalloff::Linear {
                    start: cfg.fog_start,
                    end: cfg.fog_end,
                },
                ..default()
            });
        } else {
            // Indoor: black void beyond the torchlight.
            // Rooms are ~500–3500 units diagonal; fog starts at 1 room length and
            // reaches full black at ~3 room lengths so corridors vanish into darkness.
            cam.insert(DistanceFog {
                color: Color::srgba(0.0, 0.0, 0.0, 1.0),
                falloff: FogFalloff::Linear {
                    start: 500.0,
                    end: 2000.0,
                },
                ..default()
            });
        }

        // Party torch: two overlapping lights for a non-linear profile.
        // Inner (PartyTorch): bright, tight range — hot core close to the player.
        // Outer (PartyTorchFill): dim, wide range — gentle ambient bleed.
        // Both start at intensity 0; party_torch_system activates them.
        // Indoor rooms are ~500–3500 units; outdoor is open terrain up to fog distance.
        let (torch_range, fill_range) = if is_indoor {
            (1500.0_f32, 7000.0_f32)
        } else {
            (3500.0_f32, 7000.0_f32)
        };
        parent.spawn((
            Name::new("party_torch"),
            PartyTorch,
            PointLight {
                color: Color::srgb(1.0, 0.82, 0.45),
                intensity: 0.0,
                range: torch_range,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, -60.0, 0.0),
        ));
        parent.spawn((
            Name::new("party_torch_fill"),
            PartyTorchFill,
            PointLight {
                color: Color::srgb(0.85, 0.65, 0.30),
                intensity: 0.0,
                range: fill_range,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, -60.0, 0.0),
        ));
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

fn toggle_run_mode(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<PlayerKeyBindings>,
    mut world_state: ResMut<crate::game::world_state::WorldState>,
) {
    if keys.just_pressed(key_bindings.toggle_run) {
        world_state.player.is_running = !world_state.player.is_running;
        info!("Run mode: {}", if world_state.player.is_running { "ON" } else { "OFF" });
    }
}

fn toggle_fly_mode(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<PlayerKeyBindings>,
    gamepads: Query<&Gamepad>,
    physics_query: Query<&PlayerPhysics, With<Player>>,
    mut world_state: ResMut<crate::game::world_state::WorldState>,
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

fn toggle_mouse_look(keys: Res<ButtonInput<KeyCode>>, cfg: Res<GameConfig>, mut mouse_look: ResMut<MouseLookEnabled>) {
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
    door_colliders: Option<&crate::game::blv::DoorColliders>,
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

    let base_speed = if world_state.player.fly_mode {
        settings.fly_speed
    } else if world_state.player.is_running {
        settings.run_speed
    } else {
        settings.speed
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
                if key == key_bindings.rotate_left || key == KeyCode::KeyA {
                    transform.rotate_y(turn_speed * time.delta_secs());
                } else if key == key_bindings.rotate_right || key == KeyCode::KeyD {
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
        if keys.pressed(key_bindings.move_forward) || keys.pressed(KeyCode::ArrowUp) {
            movement += forward_flat;
        }
        if keys.pressed(key_bindings.move_backward) || keys.pressed(KeyCode::ArrowDown) {
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
                let dest = from + movement;
                transform.translation = move_with_substeps(
                    from,
                    dest,
                    settings.collision_radius,
                    settings.eye_height,
                    colliders.as_deref(),
                    door_colliders.as_deref(),
                );
            } else {
                let from = transform.translation;
                let mut dest = from + movement;

                // Terrain slope check — block movement if slope angle > ~35 degrees.
                // Skip entirely when the player is on BSP floor geometry (e.g. inside a castle):
                // the terrain heightmap doesn't match the building floor and would wrongly block stairs.
                let on_bsp_floor = colliders
                    .as_ref()
                    .and_then(|c| {
                        let feet_y = from.y - settings.eye_height;
                        c.floor_height_at(from.x, from.z, feet_y, MAX_STEP_UP)
                    })
                    .is_some();
                if !on_bsp_floor && let Some(ref hm) = height_map {
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
                let can_enter_water =
                    water_walking.as_ref().is_some_and(|w| w.0) || world_state.player.fly_mode || !physics.on_ground;
                if !can_enter_water
                    && let Some(ref wm) = water_map
                    && wm.is_water_at(dest.x, dest.z)
                    && !wm.is_water_at(from.x, from.z)
                {
                    let feet_y = from.y - settings.eye_height;
                    let on_bridge = colliders
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
                    colliders.as_deref(),
                    door_colliders.as_deref(),
                );
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
    if let Ok(mut cursor_options) = cursor_query.single_mut()
        && keys.just_pressed(key_bindings.toggle_grab_cursor)
    {
        toggle_grab_cursor(&mut cursor_options);
    }
}

/// Show/hide the party torch based on context:
/// - Indoor: always on (dungeons have no daylight).
/// - Outdoor: on when dark (before 6am or after 6pm), off during daytime.
/// Inner light: bright hot core close to the player.
const TORCH_INTENSITY: f32 = 200_000_000.0;
/// Outer fill: dim wide bleed so the dungeon fades rather than hard-cuts to black.
const TORCH_FILL_INTENSITY: f32 = 80_000_000.0;

fn party_torch_system(
    indoor: Option<Res<PreparedIndoorWorld>>,
    game_time: Option<Res<crate::game::game_time::GameTime>>,
    mut torch_query: Query<&mut PointLight, With<PartyTorch>>,
    mut fill_query: Query<&mut PointLight, (With<PartyTorchFill>, Without<PartyTorch>)>,
) {
    let is_indoor = indoor.is_some();
    let lit = if is_indoor {
        true
    } else if let Some(gt) = game_time {
        let t = gt.time_of_day();
        !(0.25..=0.75).contains(&t)
    } else {
        false
    };
    let (target, fill_target) = if lit {
        (TORCH_INTENSITY, TORCH_FILL_INTENSITY)
    } else {
        (0.0, 0.0)
    };
    for mut light in &mut torch_query {
        if (light.intensity - target).abs() > 1.0 {
            light.intensity = target;
        }
    }
    for mut light in &mut fill_query {
        if (light.intensity - fill_target).abs() > 1.0 {
            light.intensity = fill_target;
        }
    }
}
