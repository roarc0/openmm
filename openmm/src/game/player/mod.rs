//! Player systems — spawn, physics, camera, torch, and key bindings.
mod input;
pub(crate) mod physics;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use openmm_data::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::collision::sample_terrain_height;
use crate::save::GameSave;
use crate::states::loading::{PreparedIndoorWorld, PreparedWorld};

use input::{cursor_grab, player_look, player_movement, toggle_fly_mode, toggle_mouse_look, toggle_run_mode};

// --- Constants ---

const WALK_SPEED: f32 = 640.0;
const RUN_SPEED: f32 = WALK_SPEED * 1.5;
const FLY_SPEED: f32 = 1920.0;
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

/// System set label for player input systems (movement, look, cursor).
/// Used by other systems to order themselves after player input is processed.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct PlayerInputSet;

/// Runtime toggle for mouse look — initialized from config, toggled by CapsLock.
#[derive(Resource)]
pub struct MouseLookEnabled(pub bool);

/// Runtime mouse sensitivity multiplier — adjusted with Home/End keys.
#[derive(Resource)]
pub struct MouseSensitivity {
    pub x: f32,
    pub y: f32,
}

/// Runtime walk/run/fly speed multiplier — adjusted via the console `speed` command.
#[derive(Resource)]
pub struct SpeedMultiplier(pub f32);

impl Default for SpeedMultiplier {
    fn default() -> Self {
        Self(1.0)
    }
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
            .init_resource::<SpeedMultiplier>()
            .insert_resource(MouseLookEnabled(true))
            .insert_resource(MouseSensitivity {
                x: cfg.mouse_sensitivity_x,
                y: cfg.mouse_sensitivity_y,
            })
            .add_systems(OnEnter(GameState::Game), (spawn_player, grab_cursor_on_enter))
            // `PlayerInputSet` bundles every per-frame input/movement system
            // so other systems can order themselves with `.after(PlayerInputSet)`.
            // Within the set, toggles (run/fly/mouse-look) must run before the
            // movement/look systems that read them — other members are
            // independent and allowed to run in parallel.
            .add_systems(
                Update,
                (toggle_run_mode, toggle_fly_mode, toggle_mouse_look)
                    .before(player_movement)
                    .before(player_look)
                    .in_set(PlayerInputSet)
                    .run_if(in_state(GameState::Game))
                    .run_if(crate::game::world::ui_state::game_input_active),
            )
            .add_systems(
                Update,
                (player_movement, player_look, cursor_grab, log_gamepads)
                    .in_set(PlayerInputSet)
                    .run_if(in_state(GameState::Game))
                    .run_if(crate::game::world::ui_state::game_input_active),
            )
            // Torch visibility follows GameTime, which freezes while UiMode ≠ World.
            // Gate the system so it doesn't flicker the torch on/off during dialogues.
            .add_systems(
                Update,
                party_torch_system
                    .run_if(in_state(GameState::Game))
                    .run_if(|ui: Res<crate::game::world::ui_state::UiState>| ui.mode == crate::game::world::ui_state::UiMode::World),
            )
            .add_systems(
                PostUpdate,
                sync_player_to_world_state.run_if(in_state(GameState::Game)),
            );
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
            crate::engine::camera_msaa(&cfg),
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
        if let Some(fxaa) = crate::engine::camera_fxaa(&cfg) {
            cam.insert(fxaa);
        }
        if let Some(smaa) = crate::engine::camera_smaa(&cfg) {
            cam.insert(smaa);
        }
        if let Some(taa) = crate::engine::camera_taa(&cfg) {
            cam.insert(taa);
        }
        if let Some(bloom) = crate::engine::camera_bloom(&cfg) {
            cam.insert(bloom);
        }
        // Depth prepass is required by SSAO and DoF (both sample the depth buffer).
        // Normal prepass is only needed by SSAO.
        if cfg.ssao || cfg.depth_of_field {
            cam.insert(bevy::core_pipeline::prepass::DepthPrepass);
        }
        if cfg.ssao {
            cam.insert(bevy::core_pipeline::prepass::NormalPrepass);
        }
        if let Some(ssao) = crate::engine::camera_ssao(&cfg) {
            cam.insert(ssao);
        }
        if let Some(motion_blur) = crate::engine::camera_motion_blur(&cfg) {
            cam.insert(motion_blur);
        }
        if let Some(dof) = crate::engine::camera_dof(&cfg) {
            cam.insert(dof);
        }
        // Always apply tonemapping + exposure for consistent physically-based rendering.
        // Bloom requires real tonemapping (not "none") — force AgX if needed.
        let tonemapping = if cfg.bloom && cfg.tonemapping == "none" {
            bevy::core_pipeline::tonemapping::Tonemapping::AgX
        } else {
            crate::engine::camera_tonemapping(&cfg)
        };
        cam.insert(tonemapping);
        cam.insert(crate::engine::camera_exposure(&cfg));
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

// --- Torch ---

/// Show/hide the party torch based on context:
/// - Indoor: always on (dungeons have no daylight).
/// - Outdoor: on when dark (before 6am or after 6pm), off during daytime.
/// Inner light: bright hot core close to the player.
const TORCH_INTENSITY: f32 = 200_000_000.0;
/// Outer fill: dim wide bleed so the dungeon fades rather than hard-cuts to black.
const TORCH_FILL_INTENSITY: f32 = 80_000_000.0;

fn party_torch_system(
    indoor: Option<Res<PreparedIndoorWorld>>,
    game_time: Option<Res<crate::game::world::GameTime>>,
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

/// Copy Player entity transform → WorldState every frame (PostUpdate).
fn sync_player_to_world_state(
    mut world_state: ResMut<crate::game::world::WorldState>,
    player_query: Query<&Transform, With<Player>>,
) {
    if let Ok(transform) = player_query.single() {
        world_state.player.position = transform.translation;
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        world_state.player.yaw = yaw;
    }
}
