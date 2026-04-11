use bevy::{input::ButtonInput, pbr::wireframe::WireframeConfig, prelude::*};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

pub mod console;
pub mod cpu_usage;
pub mod hud;
#[cfg(feature = "perf_log")]
pub mod perf_log;

use crate::config::GameConfig;
use crate::game::player::Player;
use crate::save::GameSave;
use openmm_data::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

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
}

impl Default for DebugKeyBindings {
    fn default() -> Self {
        Self {
            toggle_wireframe: KeyCode::BracketRight,
            toggle_play_area: KeyCode::BracketLeft,
        }
    }
}

pub fn debug_input(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<DebugKeyBindings>,
    mut world_state: ResMut<crate::game::world::WorldState>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keys.just_pressed(key_bindings.toggle_wireframe) {
        wireframe_config.global = !wireframe_config.global;
    } else if keys.just_pressed(key_bindings.toggle_play_area) {
        world_state.debug.show_play_area = !world_state.debug.show_play_area;
    }
}

pub fn draw_play_area(world_state: Res<crate::game::world::WorldState>, mut gizmos: Gizmos) {
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
    world_state: Res<crate::game::world::WorldState>,
    mut gizmos: Gizmos,
    clickable_faces: Option<Res<crate::game::interaction::clickable::Faces>>,
    decorations: Query<&crate::game::interaction::DecorationInfo>,
) {
    if !world_state.debug.show_events {
        return;
    }

    if let Some(ref faces) = clickable_faces {
        for face in &faces.faces {
            if face.vertices.len() < 2 {
                continue;
            }
            let color = Color::srgb(0.0, 1.0, 0.0);
            for i in 0..face.vertices.len() {
                gizmos.line(face.vertices[i], face.vertices[(i + 1) % face.vertices.len()], color);
            }
            let center: Vec3 = face.vertices.iter().copied().sum::<Vec3>() / face.vertices.len() as f32;
            let s = 15.0;
            gizmos.line(center - Vec3::X * s, center + Vec3::X * s, color);
            gizmos.line(center - Vec3::Y * s, center + Vec3::Y * s, color);
            gizmos.line(center - Vec3::Z * s, center + Vec3::Z * s, color);
        }
    }

    for info in decorations.iter() {
        let pos = info.position;
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
    world_state: Res<crate::game::world::WorldState>,
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
    if keys.just_pressed(KeyCode::F12) {
        let path = format!(
            "./data/screenshots/screenshot_{}.png",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
            .observe(bevy::render::view::screenshot::save_to_disk(path));
    }
}
