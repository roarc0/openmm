use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    input::{ButtonInput, common_conditions::input_toggle_active},
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
};
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

use lod::dtile::Tileset;
use lod::odm::{ODM_PLAY_SIZE, ODM_TILE_SCALE};

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::player::Player;
use crate::save::GameSave;
use crate::states::loading::PreparedWorld;

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

fn debug_setup(
    mut commands: Commands,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut world_state: ResMut<crate::game::world_state::WorldState>,
    cfg: Res<GameConfig>,
    ui_assets: Res<crate::ui_assets::UiAssets>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    wireframe_config.global = cfg.wireframe;
    wireframe_config.default_color = Color::srgba(0.8, 0.2, 0.6, 0.35);
    world_state.debug.show_play_area = cfg.debug;
    world_state.debug.show_events = cfg.debug;

    // Compute play area offset so debug UI sits inside the 3D viewport
    let (vp_left, vp_top) = windows
        .single()
        .ok()
        .map(|w| {
            let (l, t, _, _) = crate::game::hud::viewport_rect(w, &cfg, &ui_assets);
            (l, t)
        })
        .unwrap_or((0.0, 0.0));
    let dbg_left = vp_left + 20.0;
    let dbg_top = vp_top + 10.0;

    let hud_visibility = if cfg.debug {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    // HUD text — positioned inside the game viewport (below border3, right of border4)
    commands
        .spawn((
            Text::new("FPS: --"),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(dbg_left + 80.0),
                top: Val::Px(dbg_top),
                ..default()
            },
            hud_visibility,
            FpsText,
            InGame,
            DebugHud,
        ))
        .with_child((
            TextSpan::new(""),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::srgb(0.3, 0.6, 1.0)), // blue — map name
            MapNameSpan,
        ))
        .with_child((
            TextSpan::new(""),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.7, 0.2)), // orange — fly/walk mode
            ModeSpan,
        ))
        .with_child((
            TextSpan::new(""),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::srgb(0.85, 0.85, 0.85)), // light gray — coordinates
            PosSpan,
        ))
        .with_child((
            TextSpan::new(""),
            TextFont {
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::WHITE),
            TileSpan,
        ));

    // FPS chart — bars with min/max labels
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(dbg_left + 80.0),
                top: Val::Px(dbg_top + 60.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            hud_visibility,
            InGame,
            DebugHud,
            FpsChart,
        ))
        .with_children(|parent| {
            // Max label at top
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                ChartMaxLabel,
            ));
            // Bar container
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FlexEnd,
                        height: Val::Px(FPS_CHART_HEIGHT),
                        column_gap: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
                ))
                .with_children(|bars| {
                    for i in 0..FPS_CHART_WIDTH {
                        bars.spawn((
                            Node {
                                width: Val::Px(FPS_CHART_BAR_W),
                                height: Val::Px(0.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.2, 1.0, 0.2)),
                            FpsChartBar(i),
                        ));
                    }
                });
            // Min label at bottom
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                ChartMinLabel,
            ));
        });
}

/// Marker for the debug HUD container.
#[derive(Component)]
pub struct DebugHud;

fn debug_input(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<DebugKeyBindings>,
    mut world_state: ResMut<crate::game::world_state::WorldState>,
    mut wireframe_config: ResMut<WireframeConfig>,
) {
    if keys.just_pressed(key_bindings.toggle_wireframe) {
        wireframe_config.global = !wireframe_config.global;
    } else if keys.just_pressed(key_bindings.toggle_play_area) {
        world_state.debug.show_play_area = !world_state.debug.show_play_area;
    }
}

/// Draw play area boundary lines: North=red, South=green, East=blue, West=magenta.
fn draw_play_area(world_state: Res<crate::game::world_state::WorldState>, mut gizmos: Gizmos) {
    if !world_state.debug.show_play_area {
        return;
    }
    let half = ODM_TILE_SCALE * ODM_PLAY_SIZE as f32 / 2.0;
    let y = 120.0;

    // North edge (positive Z in MM6 → negative Z in Bevy): red
    gizmos.line(
        Vec3::new(-half, y, -half),
        Vec3::new(half, y, -half),
        Color::srgb(1.0, 0.0, 0.0),
    );
    // South edge: green
    gizmos.line(
        Vec3::new(-half, y, half),
        Vec3::new(half, y, half),
        Color::srgb(0.0, 1.0, 0.0),
    );
    // East edge: blue
    gizmos.line(
        Vec3::new(half, y, -half),
        Vec3::new(half, y, half),
        Color::srgb(0.0, 0.0, 1.0),
    );
    // West edge: magenta
    gizmos.line(
        Vec3::new(-half, y, -half),
        Vec3::new(-half, y, half),
        Color::srgb(1.0, 0.0, 1.0),
    );
}

/// Draw wireframe outlines for clickable/interactive faces, buildings, and decorations.
fn draw_events(
    world_state: Res<crate::game::world_state::WorldState>,
    mut gizmos: Gizmos,
    clickable_faces: Option<Res<crate::game::blv::ClickableFaces>>,
    buildings: Query<(&crate::game::interaction::BuildingInfo, &GlobalTransform)>,
    decorations: Query<&crate::game::interaction::DecorationInfo>,
) {
    if !world_state.debug.show_events {
        return;
    }

    // Indoor: draw clickable face outlines (bright green, thicker cross)
    if let Some(ref faces) = clickable_faces {
        for face in &faces.faces {
            if face.vertices.len() < 2 {
                continue;
            }
            let color = Color::srgb(0.0, 1.0, 0.0);
            for i in 0..face.vertices.len() {
                let a = face.vertices[i];
                let b = face.vertices[(i + 1) % face.vertices.len()];
                gizmos.line(a, b, color);
            }
            let center: Vec3 = face.vertices.iter().copied().sum::<Vec3>() / face.vertices.len() as f32;
            let s = 15.0;
            gizmos.line(center - Vec3::X * s, center + Vec3::X * s, color);
            gizmos.line(center - Vec3::Y * s, center + Vec3::Y * s, color);
            gizmos.line(center - Vec3::Z * s, center + Vec3::Z * s, color);
        }
    }

    // Outdoor: draw building interaction markers at info.position (BSP model center)
    for (info, _gt) in buildings.iter() {
        let pos = info.position;
        let color = Color::srgb(0.0, 1.0, 1.0);
        // Diamond above building roof + vertical line down to position
        let top = pos + Vec3::Y * 400.0;
        let s = 80.0;
        gizmos.line(top + Vec3::X * s, top + Vec3::Z * s, color);
        gizmos.line(top + Vec3::Z * s, top - Vec3::X * s, color);
        gizmos.line(top - Vec3::X * s, top - Vec3::Z * s, color);
        gizmos.line(top - Vec3::Z * s, top + Vec3::X * s, color);
        gizmos.line(pos, top, color);
    }

    // Outdoor: draw decoration event markers (yellow diamond + vertical line, same style as buildings)
    for info in decorations.iter() {
        let pos = info.position;
        let color = Color::srgb(1.0, 1.0, 0.0);
        // Diamond above decoration + vertical line down to position
        let top = pos + Vec3::Y * 200.0;
        let s = 40.0;
        gizmos.line(top + Vec3::X * s, top + Vec3::Z * s, color);
        gizmos.line(top + Vec3::Z * s, top - Vec3::X * s, color);
        gizmos.line(top - Vec3::X * s, top - Vec3::Z * s, color);
        gizmos.line(top - Vec3::Z * s, top + Vec3::X * s, color);
        gizmos.line(pos, top, color);
    }
}

/// FPS history for chart and percentile calculations.
const FPS_HISTORY_SIZE: usize = 120;
const FPS_CHART_WIDTH: usize = 60;
const FPS_CHART_BAR_W: f32 = 3.0;
const FPS_CHART_HEIGHT: f32 = 50.0;
const FPS_AVG_WINDOW: usize = 30;
/// Only push a new sample every N frames to slow the chart scroll.
const FPS_SAMPLE_INTERVAL: usize = 15;

#[derive(Resource)]
struct FpsHistory {
    samples: Vec<f64>,
    frame_counter: usize,
    accumulator: f64,
    accum_count: usize,
}

impl Default for FpsHistory {
    fn default() -> Self {
        Self {
            samples: Vec::with_capacity(FPS_HISTORY_SIZE),
            frame_counter: 0,
            accumulator: 0.0,
            accum_count: 0,
        }
    }
}

impl FpsHistory {
    /// Accumulate frames and push an averaged sample every N frames.
    fn tick(&mut self, fps: f64) {
        self.accumulator += fps;
        self.accum_count += 1;
        self.frame_counter += 1;
        if self.frame_counter >= FPS_SAMPLE_INTERVAL {
            let avg = self.accumulator / self.accum_count as f64;
            if self.samples.len() >= FPS_HISTORY_SIZE {
                self.samples.remove(0);
            }
            self.samples.push(avg);
            self.frame_counter = 0;
            self.accumulator = 0.0;
            self.accum_count = 0;
        }
    }

    fn averaged(&self) -> f64 {
        let n = self.samples.len().min(FPS_AVG_WINDOW);
        if n == 0 {
            return 0.0;
        }
        self.samples[self.samples.len() - n..].iter().sum::<f64>() / n as f64
    }

    fn percentile_low(&self, pct: f32) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = ((sorted.len() as f32 * pct / 100.0).ceil() as usize).max(1);
        sorted[..count].iter().sum::<f64>() / count as f64
    }

    /// Min/max of the visible chart window.
    fn chart_min_max(&self) -> (f64, f64) {
        let width = FPS_CHART_WIDTH.min(self.samples.len());
        if width == 0 {
            return (0.0, 60.0);
        }
        let start = self.samples.len() - width;
        let slice = &self.samples[start..];
        let min = slice.iter().copied().fold(f64::MAX, f64::min);
        let max = slice.iter().copied().fold(0.0_f64, f64::max);
        (min, max)
    }
}

/// Distinct color per terrain tileset.
fn tileset_color(ts: Tileset) -> Color {
    match ts {
        Tileset::Grass => Color::srgb(0.3, 0.9, 0.3),
        Tileset::Snow => Color::srgb(0.9, 0.9, 1.0),
        Tileset::Desert => Color::srgb(1.0, 0.85, 0.4),
        Tileset::Volcanic => Color::srgb(0.8, 0.3, 0.2),
        Tileset::Dirt => Color::srgb(0.7, 0.5, 0.3),
        Tileset::Water => Color::srgb(0.3, 0.5, 1.0),
        Tileset::CrackedSwamp => Color::srgb(0.5, 0.5, 0.2),
        Tileset::Swamp => Color::srgb(0.4, 0.6, 0.2),
        Tileset::Road => Color::srgb(0.7, 0.7, 0.7),
    }
}

/// Color for an FPS value: green > 55, yellow > 30, red below.
fn fps_color(fps: f64) -> Color {
    if fps >= 55.0 {
        Color::srgb(0.2, 1.0, 0.2)
    } else if fps >= 30.0 {
        Color::srgb(1.0, 0.9, 0.1)
    } else {
        Color::srgb(1.0, 0.2, 0.2)
    }
}

#[derive(Component)]
pub struct FpsText;

#[derive(Component)]
struct MapNameSpan;

#[derive(Component)]
struct ModeSpan;

#[derive(Component)]
struct PosSpan;

#[derive(Component)]
struct TileSpan;

/// Marker for the FPS chart container.
#[derive(Component)]
struct FpsChart;

/// Marker for individual chart bars.
#[derive(Component)]
struct FpsChartBar(usize);

#[derive(Component)]
struct ChartMaxLabel;

#[derive(Component)]
struct ChartMinLabel;

fn update_hud_text(
    diagnostics: Res<DiagnosticsStore>,
    player_query: Query<&Transform, With<Player>>,
    cfg: Res<GameConfig>,
    world_state: Res<crate::game::world_state::WorldState>,
    spawn_progress: Res<crate::game::odm::SpawnProgress>,
    prepared: Option<Res<PreparedWorld>>,
    mut fps_history: ResMut<FpsHistory>,
    mut fps_query: Query<(&mut Text, &mut TextColor), With<FpsText>>,
    mut map_name_query: Query<(&mut TextSpan, &mut TextColor), (With<MapNameSpan>, Without<FpsText>)>,
    mut mode_query: Query<(&mut TextSpan, &mut TextColor), (With<ModeSpan>, Without<FpsText>, Without<MapNameSpan>)>,
    mut pos_query: Query<
        (&mut TextSpan, &mut TextColor),
        (With<PosSpan>, Without<FpsText>, Without<ModeSpan>, Without<MapNameSpan>),
    >,
    mut tile_query: Query<
        (&mut TextSpan, &mut TextColor),
        (
            With<TileSpan>,
            Without<FpsText>,
            Without<PosSpan>,
            Without<ModeSpan>,
            Without<MapNameSpan>,
        ),
    >,
    mut bar_query: Query<(&FpsChartBar, &mut Node, &mut BackgroundColor)>,
    mut max_label: Query<&mut Text, (With<ChartMaxLabel>, Without<FpsText>)>,
    mut min_label: Query<&mut Text, (With<ChartMinLabel>, Without<FpsText>, Without<ChartMaxLabel>)>,
) {
    let fps_val = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed());

    if let Some(fps) = fps_val {
        fps_history.tick(fps);
    }

    let avg = fps_history.averaged();
    let low_1 = fps_history.percentile_low(1.0);
    let (chart_min, chart_max) = fps_history.chart_min_max();
    // Snap to nice round boundaries so the scale doesn't jitter.
    // Floor min down to nearest 10, ceil max up to nearest 20.
    let scale_min = ((chart_min / 10.0).floor() * 10.0).max(0.0);
    let scale_max = ((chart_max / 20.0).ceil() * 20.0).max(scale_min + 20.0);

    let fps_str = if avg > 0.0 {
        format!("FPS: {avg:.0} ({low_1:.0} min)")
    } else {
        "FPS: --".into()
    };

    let color = if avg > 0.0 { fps_color(avg) } else { Color::WHITE };

    let map_name = world_state.map.name.to_string().to_uppercase();

    let (mode_str, coords_str) = if let Ok(transform) = player_query.single() {
        let (yaw, _, _) = transform.rotation.to_euler(EulerRot::YXZ);
        let spawn_str = if cfg.debug && spawn_progress.total > 0 && spawn_progress.done < spawn_progress.total {
            format!("  SPAWN: {}/{}", spawn_progress.done, spawn_progress.total)
        } else {
            String::new()
        };
        let mode = if world_state.player.fly_mode { "  FLY" } else { "  WALK" };
        let coords = format!(
            "  X:{:.0}  Y:{:.0}  Z:{:.0}  YAW:{:.0}deg{}",
            transform.translation.x,
            transform.translation.y,
            transform.translation.z,
            yaw.to_degrees(),
            spawn_str,
        );
        (mode.to_string(), coords)
    } else {
        ("  WALK".to_string(), "  POS: --".to_string())
    };

    for (mut text, mut tc) in &mut fps_query {
        **text = fps_str.clone();
        *tc = TextColor(color);
    }
    for (mut span, mut tc) in &mut map_name_query {
        **span = format!("\n{}", map_name);
        *tc = TextColor(Color::srgb(0.3, 0.6, 1.0));
    }
    for (mut span, mut tc) in &mut mode_query {
        **span = mode_str.clone();
        *tc = TextColor(Color::srgb(1.0, 0.7, 0.2));
    }
    for (mut span, mut tc) in &mut pos_query {
        **span = coords_str.clone();
        *tc = TextColor(Color::srgb(0.85, 0.85, 0.85));
    }

    // Update tile type from terrain
    let tileset = player_query
        .single()
        .ok()
        .and_then(|tf| prepared.as_ref()?.terrain_at(tf.translation.x, tf.translation.z));
    for (mut span, mut tc) in &mut tile_query {
        if let Some(ts) = tileset {
            **span = format!("  {ts}");
            *tc = TextColor(tileset_color(ts));
        } else {
            **span = String::new();
        }
    }

    // Update chart bars with adaptive min/max scaling
    let width = FPS_CHART_WIDTH.min(fps_history.samples.len());
    let start = fps_history.samples.len().saturating_sub(FPS_CHART_WIDTH);
    let range = (scale_max - scale_min).max(1.0);
    for (bar, mut node, mut bg) in bar_query.iter_mut() {
        let idx = bar.0;
        if idx < width {
            let fps = fps_history.samples[start + idx];
            let ratio = ((fps - scale_min) / range).clamp(0.0, 1.0) as f32;
            node.height = Val::Px(ratio * FPS_CHART_HEIGHT);
            *bg = BackgroundColor(fps_color(fps));
        } else {
            node.height = Val::Px(0.0);
        }
    }

    // Update min/max labels
    for mut text in max_label.iter_mut() {
        **text = format!("{scale_max:.0}");
    }
    for mut text in min_label.iter_mut() {
        **text = format!("{scale_min:.0}");
    }
}

fn debug_log(
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
        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
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
            pitch.to_degrees(),
        );
    }
}

fn quicksave(
    keys: Res<ButtonInput<KeyCode>>,
    world_state: Res<crate::game::world_state::WorldState>,
    mut save_data: ResMut<GameSave>,
) {
    if keys.just_pressed(KeyCode::F3) {
        world_state.write_to_save(&mut save_data);
        match save_data.autosave() {
            Ok(()) => info!("Saved to target/saves/autosave.json"),
            Err(e) => error!("Failed to quicksave: {}", e),
        }
    }
}

fn debug_screenshot(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    if keys.just_pressed(KeyCode::F12) {
        let path = format!(
            "target/screenshot_{}.png",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        info!("Saving screenshot to {}", path);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}

pub struct DebugPlugin;
impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugKeyBindings>()
            .init_resource::<FpsHistory>()
            .add_plugins((
                WireframePlugin::default(),
                LogDiagnosticsPlugin::default(),
                EguiPlugin::default(),
                WorldInspectorPlugin::default().run_if(input_toggle_active(true, KeyCode::Escape)),
            ))
            .add_systems(
                Update,
                (
                    debug_input,
                    update_hud_text,
                    debug_log,
                    quicksave,
                    draw_play_area,
                    draw_events,
                    debug_screenshot,
                )
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), debug_setup);
    }
}
