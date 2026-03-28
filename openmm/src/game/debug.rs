use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    input::{common_conditions::input_toggle_active, ButtonInput},
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use lod::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::odm::OdmName;
use crate::game::player::Player;
use crate::save::{GameSave, PlayerState, MapState};
use crate::states::loading::LoadRequest;

#[derive(Resource)]
struct DebugConfig {
    show_play_area: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            show_play_area: true,
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

/// Tracks the current map for dev map switching.
#[derive(Resource)]
pub struct CurrentMapName(pub OdmName);

impl Default for CurrentMapName {
    fn default() -> Self {
        Self(OdmName::default())
    }
}

fn debug_setup(
    mut commands: Commands,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut debug_config: ResMut<DebugConfig>,
    cfg: Res<GameConfig>,
) {
    wireframe_config.global = cfg.wireframe;
    debug_config.show_play_area = cfg.show_play_area;

    let hud_visibility = if cfg.debug {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    commands
        .spawn((
            Text::new("FPS: --"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::WHITE),
            hud_visibility,
            FpsText,
            InGame,
            DebugHud,
        ))
        .with_child((
            TextSpan::new("\nPOS: --"),
            TextFont { font_size: 24.0, ..default() },
            TextColor(Color::WHITE),
            PosSpan,
        ));
}

/// Marker for the debug HUD container.
#[derive(Component)]
struct DebugHud;

fn debug_input(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<DebugKeyBindings>,
    mut dev_config: ResMut<DebugConfig>,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut hud_query: Query<&mut Visibility, With<DebugHud>>,
) {
    if keys.just_pressed(key_bindings.toggle_wireframe) {
        wireframe_config.global = !wireframe_config.global;
    } else if keys.just_pressed(key_bindings.toggle_play_area) {
        dev_config.show_play_area = !dev_config.show_play_area;
    }
    if keys.just_pressed(KeyCode::F11) {
        for mut vis in hud_query.iter_mut() {
            *vis = match *vis {
                Visibility::Hidden => Visibility::Inherited,
                _ => Visibility::Hidden,
            };
        }
    }
}

/// Draw play area boundary lines: North=red, South=green, East=blue, West=magenta.
fn draw_play_area(config: Res<DebugConfig>, mut gizmos: Gizmos) {
    if !config.show_play_area {
        return;
    }
    let half = ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.0;
    let y = 120.0;

    // North edge (positive Z in MM6 → negative Z in Bevy): red
    gizmos.line(Vec3::new(-half, y, -half), Vec3::new(half, y, -half), Color::srgb(1.0, 0.0, 0.0));
    // South edge: green
    gizmos.line(Vec3::new(-half, y, half), Vec3::new(half, y, half), Color::srgb(0.0, 1.0, 0.0));
    // East edge: blue
    gizmos.line(Vec3::new(half, y, -half), Vec3::new(half, y, half), Color::srgb(0.0, 0.0, 1.0));
    // West edge: magenta
    gizmos.line(Vec3::new(-half, y, -half), Vec3::new(-half, y, half), Color::srgb(1.0, 0.0, 1.0));
}

/// Debug map switching with H/J/K/L keys.
fn debug_change_map(
    keys: Res<ButtonInput<KeyCode>>,
    mut current_map: ResMut<CurrentMapName>,
    mut commands: Commands,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let new_map = if keys.just_pressed(KeyCode::KeyJ) {
        current_map.0.go_north()
    } else if keys.just_pressed(KeyCode::KeyH) {
        current_map.0.go_west()
    } else if keys.just_pressed(KeyCode::KeyK) {
        current_map.0.go_south()
    } else if keys.just_pressed(KeyCode::KeyL) {
        current_map.0.go_east()
    } else {
        None
    };

    if let Some(new_map) = new_map {
        info!("Dev: changing map to {}", &new_map);
        commands.insert_resource(LoadRequest {
            map_name: new_map.clone(),
        });
        current_map.0 = new_map;
        game_state.set(GameState::Loading);
    }
}

/// FPS history for chart and percentile calculations.
const FPS_HISTORY_SIZE: usize = 120;
const FPS_CHART_WIDTH: usize = 40;

#[derive(Resource)]
struct FpsHistory {
    samples: Vec<f64>,
}

impl Default for FpsHistory {
    fn default() -> Self {
        Self { samples: Vec::with_capacity(FPS_HISTORY_SIZE) }
    }
}

impl FpsHistory {
    fn push(&mut self, fps: f64) {
        if self.samples.len() >= FPS_HISTORY_SIZE {
            self.samples.remove(0);
        }
        self.samples.push(fps);
    }

    /// 1% low: average of the lowest 1% of samples (min 1 sample).
    fn percentile_low(&self, pct: f32) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = ((sorted.len() as f32 * pct / 100.0).ceil() as usize).max(1);
        sorted[..count].iter().sum::<f64>() / count as f64
    }

    /// Build a text-based scrolling chart. Each column is one sample,
    /// characters represent vertical bars at different heights.
    fn chart(&self) -> String {
        let width = FPS_CHART_WIDTH.min(self.samples.len());
        if width == 0 { return String::new(); }
        let start = self.samples.len() - width;
        let slice = &self.samples[start..];
        let max_fps = 120.0_f64;
        let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        let mut chart = String::with_capacity(width);
        for &fps in slice {
            let ratio = (fps / max_fps).clamp(0.0, 1.0);
            let idx = (ratio * (bar_chars.len() - 1) as f64) as usize;
            chart.push(bar_chars[idx]);
        }
        chart
    }
}

#[derive(Component)]
pub struct FpsText;

/// Marker for the position text span (child of FpsText).
#[derive(Component)]
struct PosSpan;

fn update_hud_text(
    diagnostics: Res<DiagnosticsStore>,
    player_query: Query<&Transform, With<Player>>,
    mut fps_history: ResMut<FpsHistory>,
    mut fps_query: Query<(&mut Text, &mut TextColor), With<FpsText>>,
    mut pos_query: Query<(&mut TextSpan, &mut TextColor), (With<PosSpan>, Without<FpsText>)>,
) {
    let fps_val = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed());

    if let Some(fps) = fps_val {
        fps_history.push(fps);
    }

    let low_1 = fps_history.percentile_low(1.0);
    let chart = fps_history.chart();

    let fps_str = fps_val
        .map(|v| format!("FPS: {v:.0}  1%low: {low_1:.0}\n{chart}"))
        .unwrap_or_else(|| "FPS: --".into());

    let fps_color = match fps_val {
        Some(v) if v >= 55.0 => Color::srgb(0.2, 1.0, 0.2),
        Some(v) if v >= 30.0 => Color::srgb(1.0, 0.9, 0.1),
        Some(_) => Color::srgb(1.0, 0.2, 0.2),
        None => Color::WHITE,
    };

    let pos_str = if let Ok(transform) = player_query.single() {
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        format!(
            "\nX:{:.0}  Y:{:.0}  Z:{:.0}  YAW:{:.0}°",
            transform.translation.x, transform.translation.y,
            transform.translation.z, yaw.to_degrees(),
        )
    } else {
        "\nPOS: --".into()
    };

    for (mut text, mut color) in &mut fps_query {
        **text = fps_str.clone();
        *color = TextColor(fps_color);
    }
    for (mut span, mut color) in &mut pos_query {
        **span = pos_str.clone();
        *color = TextColor(Color::WHITE);
    }
}

fn debug_log(
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    player_query: Query<&Transform, With<Player>>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(3.0, TimerMode::Repeating));
    timer.tick(time.delta());
    if timer.just_finished() {
        if let Ok(transform) = player_query.single() {
            let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
            info!(
                "pos=({:.0}, {:.0}, {:.0}) yaw={:.1}° pitch={:.1}°",
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
                yaw.to_degrees(),
                pitch.to_degrees(),
            );
        }
    }
}

fn quicksave(
    keys: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, With<Player>>,
    current_map: Res<CurrentMapName>,
    mut save_data: ResMut<GameSave>,
) {
    if keys.just_pressed(KeyCode::F3) {
        if let Ok(transform) = player_query.single() {
            let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
            save_data.player = PlayerState {
                position: [
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ],
                yaw,
            };
            save_data.map = MapState {
                map_x: current_map.0.x,
                map_y: current_map.0.y,
            };
            match save_data.autosave() {
                Ok(()) => info!("Saved to target/saves/autosave.json"),
                Err(e) => error!("Failed to quicksave: {}", e),
            }
        }
    }
}

pub struct DebugPlugin;
impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugKeyBindings>()
            .init_resource::<DebugConfig>()
            .init_resource::<CurrentMapName>()
            .init_resource::<FpsHistory>()
            .add_plugins((
                WireframePlugin::default(),
                LogDiagnosticsPlugin::default(),
                EguiPlugin::default(),
                WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            ))
            .add_systems(
                Update,
                (debug_input, update_hud_text, debug_change_map, debug_log, quicksave, draw_play_area)
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), debug_setup);
    }
}
