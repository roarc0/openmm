//! Player systems — spawn, physics, camera, torch, and key bindings.
mod input;
pub(crate) mod party;
pub(crate) mod physics;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use openmm_data::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::GameState;
use crate::game::InGame;
use crate::game::map::collision::{BuildingColliders, probe_ground_height};
use crate::game::optional::OptionalWrite;
use crate::game::save::ActiveSave;
use crate::prepare::loading::{PreparedIndoorWorld, PreparedWorld};
use crate::system::config::GameConfig;

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
            move_forward: KeyCode::ArrowUp,
            move_backward: KeyCode::ArrowDown,
            strafe_left: KeyCode::ArrowLeft,
            strafe_right: KeyCode::ArrowRight,
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
                    .run_if(crate::game::ui::game_input_active),
            )
            .add_systems(
                Update,
                (player_movement, player_look, cursor_grab, log_gamepads)
                    .in_set(PlayerInputSet)
                    .run_if(in_state(GameState::Game))
                    .run_if(crate::game::ui::game_input_active),
            )
            // Torch visibility follows GameTime, which freezes while UiMode ≠ World.
            // Gate the system so it doesn't flicker the torch on/off during dialogues.
            .add_systems(
                Update,
                party_torch_system
                    .run_if(in_state(GameState::Game))
                    .run_if(crate::game::ui::is_world_mode),
            )
            .add_systems(PostUpdate, sync_player_to_world_state.run_if(in_state(GameState::Game)))
            .add_systems(
                Update,
                water_overlay_system
                    .after(PlayerInputSet)
                    .run_if(in_state(GameState::Game)),
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
    colliders: Option<Res<BuildingColliders>>,
    settings: Res<PlayerSettings>,
    cfg: Res<crate::system::config::GameConfig>,
    active_save: Res<ActiveSave>,
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
        let pos = active_save.spawn_position;
        let has_save_pos = pos.x != 0.0 || pos.y != 0.0 || pos.z != 0.0;
        if has_save_pos {
            let y =
                probe_ground_height(&prepared.map.height_map, colliders.as_deref(), pos.x, pos.z) + settings.eye_height;
            (pos.x, y, pos.z, active_save.spawn_yaw)
        } else {
            let party_start = prepared.start_points.iter().find(|sp| {
                sp.name.to_lowercase().contains("party start") || sp.name.to_lowercase().contains("party_start")
            });
            if let Some(sp) = party_start {
                let y = probe_ground_height(
                    &prepared.map.height_map,
                    colliders.as_deref(),
                    sp.position.x,
                    sp.position.z,
                ) + settings.eye_height;
                (sp.position.x, y, sp.position.z, sp.yaw)
            } else {
                (0.0, settings.eye_height, 0.0, 0.0)
            }
        }
    } else {
        (
            active_save.spawn_position.x,
            active_save.spawn_position.y,
            active_save.spawn_position.z,
            active_save.spawn_yaw,
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
            RenderLayers::from_layers(&[0, crate::screens::debug::DEBUG_GIZMO_RENDER_LAYER]),
            Camera3d::default(),
            SpatialListener::new(4.0),
            crate::game::rendering::engine::camera_msaa(&cfg),
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
        if let Some(fxaa) = crate::game::rendering::engine::camera_fxaa(&cfg) {
            cam.insert(fxaa);
        }
        if let Some(smaa) = crate::game::rendering::engine::camera_smaa(&cfg) {
            cam.insert(smaa);
        }
        if let Some(taa) = crate::game::rendering::engine::camera_taa(&cfg) {
            cam.insert(taa);
        }
        if let Some(bloom) = crate::game::rendering::engine::camera_bloom(&cfg) {
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
        if let Some(ssao) = crate::game::rendering::engine::camera_ssao(&cfg) {
            cam.insert(ssao);
        }
        if let Some(motion_blur) = crate::game::rendering::engine::camera_motion_blur(&cfg) {
            cam.insert(motion_blur);
        }
        if let Some(dof) = crate::game::rendering::engine::camera_dof(&cfg) {
            cam.insert(dof);
        }
        // Always apply tonemapping + exposure for consistent physically-based rendering.
        // Bloom requires real tonemapping (not "none") — force AgX if needed.
        let tonemapping = if cfg.bloom && cfg.tonemapping == "none" {
            bevy::core_pipeline::tonemapping::Tonemapping::AgX
        } else {
            crate::game::rendering::engine::camera_tonemapping(&cfg)
        };
        cam.insert(tonemapping);
        cam.insert(crate::game::rendering::engine::camera_exposure(&cfg));
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
    game_time: Option<Res<crate::game::state::GameTime>>,
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
    mut world_state: ResMut<crate::game::state::WorldState>,
    player_query: Query<&Transform, With<Player>>,
) {
    if let Ok(transform) = player_query.single() {
        world_state.player.position = transform.translation;
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        world_state.player.yaw = yaw;
    }
}

/// Show waterwalk overlay when player is on a pure water tile (no bridge above).
fn water_overlay_system(
    player_query: Query<(&Transform, &PlayerPhysics), With<Player>>,
    water_map: Option<Res<crate::game::map::collision::WaterMap>>,
    colliders: Option<Res<crate::game::map::collision::BuildingColliders>>,
    settings: Res<PlayerSettings>,
    mut actions: Option<bevy::ecs::message::MessageWriter<crate::screens::runtime::ScreenActions>>,
    mut showing: Local<bool>,
) {
    let Ok((transform, physics)) = player_query.single() else {
        return;
    };
    let Some(ref wm) = water_map else { return };

    let pos = transform.translation;
    let on_water_tile = wm.is_water_at(pos.x, pos.z);
    let feet_y = pos.y - settings.eye_height;
    let on_bridge = colliders
        .as_ref()
        .and_then(|c| c.floor_height_at(pos.x, pos.z, feet_y, crate::game::map::collision::MAX_STEP_UP))
        .is_some();

    let should_show = on_water_tile && physics.on_ground && !on_bridge;

    if should_show != *showing {
        *showing = should_show;
        let action = if should_show {
            "ShowScreen(\"waterwalk\")"
        } else {
            "HideScreen(\"waterwalk\")"
        };
        actions.try_write(crate::screens::runtime::ScreenActions {
            actions: vec![action.to_string()],
        });
    }
}
