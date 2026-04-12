use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::{
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
};

pub mod chart;
pub mod common;
pub mod cpu;
pub mod fps;
pub mod player;

use crate::GameState;
use crate::config::GameConfig;
use crate::game::InGame;
use crate::game::world::is_outdoor;

use self::chart::*;
use self::common::*;
use self::cpu::*;
use self::fps::*;
use self::player::*;

pub struct DebugHudPlugin;

impl Plugin for DebugHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<super::DebugKeyBindings>()
            .init_resource::<FpsHistory>()
            .init_resource::<HudThrottle>()
            .init_resource::<super::cpu_usage::CpuStats>()
            .add_plugins((WireframePlugin::default(), FrameTimeDiagnosticsPlugin::default()))
            .add_systems(
                Update,
                (
                    super::debug_input,
                    tick_fps_history,
                    super::cpu_usage::update_cpu_stats_system,
                    update_fps_text,
                    update_cpu_text,
                    update_map_info_text,
                    update_player_mode_text,
                    update_position_text,
                    update_tile_text.run_if(is_outdoor),
                    update_chart_labels,
                    update_fps_chart,
                    super::debug_log,
                )
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(
                Update,
                (
                    super::quicksave,
                    super::draw_play_area,
                    super::draw_events,
                    super::debug_screenshot,
                )
                    .run_if(in_state(GameState::Game)),
            )
            .add_systems(OnEnter(GameState::Game), debug_setup);
    }
}

fn debug_setup(
    mut commands: Commands,
    mut wireframe_config: ResMut<WireframeConfig>,
    mut world_state: ResMut<crate::game::world::WorldState>,
    cfg: Res<GameConfig>,
    ui_assets: Res<crate::game::ui_assets::UiAssets>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    wireframe_config.global = cfg.wireframe;
    wireframe_config.default_color = Color::srgba(0.8, 0.2, 0.6, 0.35);
    world_state.debug.show_play_area = cfg.debug;
    world_state.debug.show_events = cfg.debug;

    let (vp_left, vp_top) = windows
        .single()
        .map(|w| {
            let (l, t, _, _) = crate::game::viewport::viewport_rect(w, &cfg, &ui_assets);
            (l, t)
        })
        .unwrap_or((0.0, 0.0));
    let dbg_left = vp_left + 20.0;
    let dbg_top = vp_top + 32.0;

    let hud_visibility = if cfg.debug {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(dbg_left + 80.0),
                top: Val::Px(dbg_top),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            hud_visibility,
            InGame,
            DebugHud,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("FPS: --"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                FpsText,
            ));
            parent.spawn((
                Text::new(" CPU: --%"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                CpuText,
            ));
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.3, 0.6, 1.0)),
                MapNameSpan,
            ));
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.7, 0.2)),
                ModeSpan,
            ));
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
                PosSpan,
            ));
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                TileSpan,
            ));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(dbg_left + 80.0),
                top: Val::Px(dbg_top + 25.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            hud_visibility,
            InGame,
            DebugHud,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                ChartMaxLabel,
            ));
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

#[derive(Component)]
pub struct DebugHud;
