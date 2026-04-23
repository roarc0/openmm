use bevy::{input::ButtonInput, pbr::wireframe::WireframeConfig, prelude::*};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use std::f32::consts::TAU;

use crate::game::player::PlayerCamera;
use crate::game::spawn::WorldObstacle;
pub mod console;
pub mod cpu_usage;
pub mod hud;
#[cfg(feature = "perf_log")]
pub mod perf_log;

use crate::game::player::Player;
use crate::system::config::GameConfig;
use crate::system::save::GameSave;
use openmm_data::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

/// Dedicated render layer for debug gizmos so they render only in the 3D player camera.
pub const DEBUG_GIZMO_RENDER_LAYER: usize = 31;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugKeyBindings>().add_plugins(hud::DebugHudPlugin);

        #[cfg(feature = "perf_log")]
        app.add_plugins(perf_log::PerfLogPlugin);

        if app.world().resource::<GameConfig>().world_inspector {
            app.add_plugins(EguiPlugin::default())
                .add_plugins(WorldInspectorPlugin::default());
        }
    }
}

#[derive(Resource)]
pub struct DebugKeyBindings {
    pub toggle_wireframe: KeyCode,
    pub toggle_play_area: KeyCode,
    pub toggle_debug_wireframes: KeyCode,
}

impl Default for DebugKeyBindings {
    fn default() -> Self {
        Self {
            toggle_wireframe: KeyCode::BracketRight,
            toggle_play_area: KeyCode::BracketLeft,
            toggle_debug_wireframes: KeyCode::Backslash,
        }
    }
}

pub fn debug_input(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<DebugKeyBindings>,
    mut world_state: ResMut<crate::game::state::WorldState>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keys.just_pressed(key_bindings.toggle_wireframe) {
        wireframe_config.global = !wireframe_config.global;
    } else if keys.just_pressed(key_bindings.toggle_play_area) {
        world_state.debug.show_play_area = !world_state.debug.show_play_area;
    } else if keys.just_pressed(key_bindings.toggle_debug_wireframes) {
        world_state.debug.show_wireframes = !world_state.debug.show_wireframes;
    }
}

/// Distance + direction check for debug wireframes. Skips entities far away or behind camera.
fn debug_visible(cam_pos: Vec3, cam_fwd: Vec3, pos: Vec3) -> bool {
    const MAX_DIST_SQ: f32 = 5000.0 * 5000.0;
    let to = pos - cam_pos;
    // Distance cull.
    if to.length_squared() > MAX_DIST_SQ {
        return false;
    }
    // Wide frustum check — dot > -0.3 allows ~107° half-angle (catches periphery).
    let horiz = Vec3::new(to.x, 0.0, to.z);
    let fwd_horiz = Vec3::new(cam_fwd.x, 0.0, cam_fwd.z);
    if horiz.length_squared() > 1.0 && fwd_horiz.length_squared() > 0.01 {
        let dot = horiz.normalize().dot(fwd_horiz.normalize());
        if dot < -0.3 {
            return false;
        }
    }
    true
}

pub fn draw_play_area(world_state: Res<crate::game::state::WorldState>, mut gizmos: Gizmos) {
    if !world_state.debug.show_play_area {
        return;
    }
    let half = ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.0;
    let y = 120.0;

    gizmos.line(
        Vec3::new(-half, y, -half),
        Vec3::new(half, y, -half),
        Color::srgb(1.0, 0.0, 0.0),
    );
    gizmos.line(
        Vec3::new(-half, y, half),
        Vec3::new(half, y, half),
        Color::srgb(0.0, 1.0, 0.0),
    );
    gizmos.line(
        Vec3::new(half, y, -half),
        Vec3::new(half, y, half),
        Color::srgb(0.0, 0.0, 1.0),
    );
    gizmos.line(
        Vec3::new(-half, y, -half),
        Vec3::new(-half, y, half),
        Color::srgb(1.0, 0.0, 1.0),
    );
}

pub fn draw_events(
    world_state: Res<crate::game::state::WorldState>,
    mut gizmos: Gizmos,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    clickable_faces: Option<Res<crate::game::interaction::clickable::Faces>>,
    decorations: Query<&crate::game::interaction::DecorationInfo>,
) {
    if !world_state.debug.show_wireframes {
        return;
    }
    let Ok(cam_tf) = camera_query.single() else {
        return;
    };
    let cam_pos = cam_tf.translation();
    let cam_fwd = cam_tf.forward().as_vec3();

    if let Some(ref faces) = clickable_faces {
        for face in faces.faces.iter().filter(|f| f.event_id > 0) {
            if face.vertices.len() < 2 {
                continue;
            }
            let center: Vec3 = face.vertices.iter().copied().sum::<Vec3>() / face.vertices.len() as f32;
            if !debug_visible(cam_pos, cam_fwd, center) {
                continue;
            }
            let color = Color::srgb(0.0, 1.0, 0.0);
            for i in 0..face.vertices.len() {
                gizmos.line(face.vertices[i], face.vertices[(i + 1) % face.vertices.len()], color);
            }
            let s = 15.0;
            gizmos.line(center - Vec3::X * s, center + Vec3::X * s, color);
            gizmos.line(center - Vec3::Y * s, center + Vec3::Y * s, color);
            gizmos.line(center - Vec3::Z * s, center + Vec3::Z * s, color);
        }
    }

    for info in decorations.iter().filter(|i| i.event_id > 0) {
        let pos = info.position;
        if !debug_visible(cam_pos, cam_fwd, pos) {
            continue;
        }
        let color = Color::srgb(1.0, 1.0, 0.0);
        let top = pos + Vec3::Y * 200.0;
        let s = 40.0;
        gizmos.line(top + Vec3::X * s, top + Vec3::Z * s, color);
        gizmos.line(top + Vec3::Z * s, top - Vec3::X * s, color);
        gizmos.line(top - Vec3::X * s, top - Vec3::Z * s, color);
        gizmos.line(top - Vec3::Z * s, top + Vec3::X * s, color);
        gizmos.line(pos, top, color);
    }
}

/// Draw actor collision cylinders and indoor door collision geometry.
/// Only draws entities within range and in front of camera.
pub fn draw_colliders(
    world_state: Res<crate::game::state::WorldState>,
    mut gizmos: Gizmos,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    actors: Query<(&Transform, &crate::game::actors::Actor)>,
    decorations: Query<(&Transform, &WorldObstacle), Without<crate::game::actors::Actor>>,
    door_colliders: Option<Res<crate::game::map::indoor::DoorColliders>>,
) {
    if !world_state.debug.show_wireframes {
        return;
    }
    let Ok(cam_tf) = camera_query.single() else {
        return;
    };
    let cam_pos = cam_tf.translation();
    let cam_fwd = cam_tf.forward().as_vec3();

    const ACTOR_BODY_HEIGHT: f32 = 140.0;
    const ACTOR_GIZMO_Y_OFFSET: f32 = 24.0;
    const RING_SEGMENTS: usize = 12;

    let draw_ring = |gizmos: &mut Gizmos, center: Vec3, radius: f32, y: f32, color: Color| {
        for i in 0..RING_SEGMENTS {
            let t0 = i as f32 / RING_SEGMENTS as f32 * TAU;
            let t1 = (i + 1) as f32 / RING_SEGMENTS as f32 * TAU;
            let p0 = Vec3::new(center.x + t0.cos() * radius, y, center.z + t0.sin() * radius);
            let p1 = Vec3::new(center.x + t1.cos() * radius, y, center.z + t1.sin() * radius);
            gizmos.line(p0, p1, color);
        }
    };

    for (tf, actor) in &actors {
        let center = tf.translation;
        if !debug_visible(cam_pos, cam_fwd, center) {
            continue;
        }

        let y_top = center.y + ACTOR_GIZMO_Y_OFFSET;
        let y_bottom = center.y - ACTOR_BODY_HEIGHT + ACTOR_GIZMO_Y_OFFSET;
        let radius = actor.collision_radius;
        let color = if actor.hostile {
            Color::srgb(1.0, 0.3, 0.1)
        } else {
            Color::srgb(0.2, 0.55, 1.0)
        };

        draw_ring(&mut gizmos, center, radius, y_top, color);
        draw_ring(&mut gizmos, center, radius, y_bottom, color);

        for angle in [0.0_f32, TAU * 0.25, TAU * 0.5, TAU * 0.75] {
            let x = center.x + angle.cos() * radius;
            let z = center.z + angle.sin() * radius;
            gizmos.line(Vec3::new(x, y_bottom, z), Vec3::new(x, y_top, z), color);
        }
    }

    let dec_color = Color::srgb(1.0, 0.5, 0.0);
    for (tf, obs) in &decorations {
        let center = tf.translation;
        if !debug_visible(cam_pos, cam_fwd, center) {
            continue;
        }
        let y_bottom = center.y - ACTOR_BODY_HEIGHT * 0.5;
        let y_top = center.y + ACTOR_BODY_HEIGHT * 0.5;
        draw_ring(&mut gizmos, center, obs.radius, y_top, dec_color);
        draw_ring(&mut gizmos, center, obs.radius, y_bottom, dec_color);
        for angle in [0.0_f32, TAU * 0.25, TAU * 0.5, TAU * 0.75] {
            let x = center.x + angle.cos() * obs.radius;
            let z = center.z + angle.sin() * obs.radius;
            gizmos.line(Vec3::new(x, y_bottom, z), Vec3::new(x, y_top, z), dec_color);
        }
    }

    let Some(door_colliders) = door_colliders else {
        return;
    };

    for wall in &door_colliders.walls {
        let wall_center = Vec3::new(
            (wall.min_x + wall.max_x) * 0.5,
            (wall.min_y + wall.max_y) * 0.5,
            (wall.min_z + wall.max_z) * 0.5,
        );
        if !debug_visible(cam_pos, cam_fwd, wall_center) {
            continue;
        }
        let color = Color::srgb(0.15, 0.75, 1.0);
        let corners_low = [
            Vec3::new(wall.min_x, wall.min_y, wall.min_z),
            Vec3::new(wall.max_x, wall.min_y, wall.min_z),
            Vec3::new(wall.max_x, wall.min_y, wall.max_z),
            Vec3::new(wall.min_x, wall.min_y, wall.max_z),
        ];
        let corners_high = [
            Vec3::new(wall.min_x, wall.max_y, wall.min_z),
            Vec3::new(wall.max_x, wall.max_y, wall.min_z),
            Vec3::new(wall.max_x, wall.max_y, wall.max_z),
            Vec3::new(wall.min_x, wall.max_y, wall.max_z),
        ];

        for i in 0..4 {
            let next = (i + 1) % 4;
            gizmos.line(corners_low[i], corners_low[next], color);
            gizmos.line(corners_high[i], corners_high[next], color);
            gizmos.line(corners_low[i], corners_high[i], color);
        }
    }

    for ceil in &door_colliders.dynamic_ceilings {
        let center = (ceil.v0 + ceil.v1 + ceil.v2) / 3.0;
        if !debug_visible(cam_pos, cam_fwd, center) {
            continue;
        }
        let color = Color::srgb(0.0, 0.9, 0.55);
        gizmos.line(ceil.v0, ceil.v1, color);
        gizmos.line(ceil.v1, ceil.v2, color);
        gizmos.line(ceil.v2, ceil.v0, color);
    }
}

pub fn debug_log(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    mut last_pos: Local<Option<(f32, f32, f32, f32, f32)>>,
    player_query: Query<&Transform, With<Player>>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(3.0, TimerMode::Repeating));
    timer.tick(time.delta());
    if timer.just_finished()
        && let Ok(transform) = player_query.single()
    {
        let (yaw, pitch, _): (f32, f32, f32) = transform.rotation.to_euler(EulerRot::YXZ);
        let cur = (
            (transform.translation.x * 0.1).round(),
            (transform.translation.y * 0.1).round(),
            (transform.translation.z * 0.1).round(),
            (yaw.to_degrees() * 10.0).round(),
            (pitch.to_degrees() * 10.0).round(),
        );
        if last_pos.as_ref() == Some(&cur) {
            return;
        }
        *last_pos = Some(cur);
        info!(
            "pos=({:.0}, {:.0}, {:.0}) yaw={:.1}deg pitch={:.1}deg",
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
            yaw.to_degrees(),
            pitch.to_degrees()
        );
    }
}

pub fn quicksave(
    keys: Res<ButtonInput<KeyCode>>,
    world_state: Res<crate::game::state::WorldState>,
    mut save_data: ResMut<GameSave>,
) {
    if keys.just_pressed(KeyCode::F3) {
        world_state.write_to_save(&mut save_data);
        match save_data.autosave() {
            Ok(()) => info!("Saved to data/saves/autosave.json"),
            Err(e) => error!("Failed to save: {}", e),
        }
    }
}

pub fn debug_screenshot(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::F11) {
        let path = format!(
            "./data/screenshots/screenshot_{}.png",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
    }
}
