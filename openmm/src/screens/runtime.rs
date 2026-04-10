//! Screen runtime: renders a .ron screen definition as Bevy UI.
//! Activated via `--screens=<id>` CLI flag.

use bevy::prelude::*;

use super::{REF_H, REF_W, Screen, ScreenElement, load_screen, load_texture_with_transparency, resolve_size};
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::hud::UiAssets;
use crate::GameState;

/// Plugin that drives the screen runtime when `--screens` is set.
pub struct ScreenRuntimePlugin;

impl Plugin for ScreenRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Menu), screen_setup)
            .add_systems(OnExit(GameState::Menu), screen_teardown)
            .add_systems(
                Update,
                (screen_hover, screen_click, click_flash_tick, handle_load_screen)
                    .run_if(in_state(GameState::Menu)),
            );
    }
}

// ── Components & resources ──────────────────────────────────────────────────

#[derive(Component)]
struct OnScreenRuntime;

#[derive(Component)]
struct RuntimeElement {
    index: usize,
}

#[derive(Component)]
struct HoverOverlay;

#[derive(Component)]
struct ScreenMusic;

/// Hides element briefly on click, then fires pending actions.
#[derive(Component)]
struct ClickFlash {
    timer: Timer,
    pending_actions: Vec<String>,
}

#[derive(Resource)]
struct RuntimeScreen {
    screen: Screen,
}

/// Queued screen transition — processed by `handle_load_screen`.
#[derive(Resource)]
struct PendingLoadScreen {
    screen_id: String,
}

// ── Setup & teardown ────────────────────────────────────────────────────────

fn screen_setup(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    pending: Option<Res<PendingLoadScreen>>,
) {
    let screen_id = if let Some(ref p) = pending {
        p.screen_id.clone()
    } else if let Some(ref id) = cfg.screens {
        id.clone()
    } else {
        return;
    };
    commands.remove_resource::<PendingLoadScreen>();

    let screen = match load_screen(&screen_id) {
        Ok(s) => s,
        Err(e) => {
            error!("failed to load screen '{}': {}", screen_id, e);
            return;
        }
    };

    info!("screen runtime: loaded '{}' ({} elements)", screen.id, screen.elements.len());

    commands.spawn((Camera2d, OnScreenRuntime));

    // Background music.
    if !screen.bg_music.is_empty() {
        spawn_screen_music(&mut commands, &mut audio_sources, &screen.bg_music, &cfg);
    }

    for (i, elem) in screen.elements.iter().enumerate() {
        spawn_runtime_element(&mut commands, &mut ui_assets, &game_assets, &mut images, &cfg, elem, i);
    }

    commands.insert_resource(RuntimeScreen { screen });
}

fn spawn_screen_music(
    commands: &mut Commands,
    audio_sources: &mut Assets<AudioSource>,
    track: &str,
    cfg: &GameConfig,
) {
    let data_path = openmm_data::get_data_path();
    let base_dir = std::path::Path::new(&data_path)
        .parent()
        .unwrap_or(std::path::Path::new(&data_path));
    let track_name = format!("Music/{}.mp3", track);
    let music_path = openmm_data::find_path_case_insensitive(base_dir, &track_name);

    if let Some(path) = music_path {
        if let Ok(bytes) = std::fs::read(&path) {
            let handle = audio_sources.add(AudioSource { bytes: bytes.into() });
            commands.spawn((
                AudioPlayer(handle),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::Linear(cfg.music_volume),
                    ..default()
                },
                ScreenMusic,
                OnScreenRuntime,
            ));
            info!("screen music: playing '{}' from {:?}", track, path);
        } else {
            warn!("screen music: failed to read {:?}", path);
        }
    } else {
        warn!("screen music: '{}' not found (searched {:?})", track_name, base_dir);
    }
}

fn screen_teardown(mut commands: Commands, entities: Query<Entity, With<OnScreenRuntime>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<RuntimeScreen>();
}

/// Despawn all runtime entities in-place (for screen-to-screen transitions without state change).
fn despawn_runtime(commands: &mut Commands, entities: &Query<Entity, With<OnScreenRuntime>>) {
    for entity in entities.iter() {
        commands.entity(entity).despawn();
    }
}

// ── Element spawning ────────────────────────────────────────────────────────

fn spawn_runtime_element(
    commands: &mut Commands,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
    elem: &ScreenElement,
    index: usize,
) {
    let (w, h) = resolve_size(elem, ui_assets);

    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(elem.position.0 / REF_W * 100.0),
        top: Val::Percent(elem.position.1 / REF_H * 100.0),
        width: Val::Percent(w / REF_W * 100.0),
        height: Val::Percent(h / REF_H * 100.0),
        ..default()
    };

    let default_tex = elem.texture_for_state("default").unwrap_or("").to_string();
    let default_handle = if !default_tex.is_empty() {
        load_texture_with_transparency(&default_tex, &elem.transparent_color, ui_assets, game_assets, images, cfg)
    } else {
        None
    };

    let hover_handle = elem.states.get("hover").and_then(|state| {
        if state.texture.is_empty() {
            None
        } else {
            load_texture_with_transparency(&state.texture, &elem.transparent_color, ui_assets, game_assets, images, cfg)
        }
    });

    let has_interaction = hover_handle.is_some() || !elem.on_click.is_empty() || !elem.on_hover.is_empty();
    let z = ZIndex(elem.z);
    let marker = RuntimeElement { index };

    if let Some(handle) = default_handle {
        let mut entity = commands.spawn((ImageNode::new(handle), node, z, marker, OnScreenRuntime));
        if has_interaction {
            entity.insert((Button, BackgroundColor(Color::NONE)));
        }
        if let Some(h_handle) = hover_handle {
            entity.with_children(|parent| {
                parent.spawn((
                    ImageNode::new(h_handle),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    Visibility::Hidden,
                    HoverOverlay,
                ));
            });
        }
    } else {
        let mut entity = commands.spawn((node, z, marker, OnScreenRuntime));
        if has_interaction {
            entity.insert((Button, BackgroundColor(Color::NONE)));
        }
    }
}

// ── Interaction systems ─────────────────────────────────────────────────────

fn screen_hover(
    query: Query<(&Interaction, &Children), (Changed<Interaction>, With<RuntimeElement>)>,
    mut hover_query: Query<&mut Visibility, With<HoverOverlay>>,
) {
    for (interaction, children) in &query {
        let show = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        for child in children.iter() {
            if let Ok(mut vis) = hover_query.get_mut(child) {
                *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
    }
}

/// On click: hide element briefly, then fire actions after the flash.
fn screen_click(
    mut commands: Commands,
    query: Query<(Entity, &Interaction, &RuntimeElement), (Changed<Interaction>, With<Button>)>,
    runtime: Option<Res<RuntimeScreen>>,
    flash_query: Query<&ClickFlash>,
) {
    let Some(runtime) = runtime else { return };
    for (entity, interaction, rt_elem) in &query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if flash_query.contains(entity) {
            continue;
        }
        let elem = &runtime.screen.elements[rt_elem.index];
        if elem.on_click.is_empty() {
            continue;
        }

        info!("screen click [{}]", elem.id);
        commands.entity(entity).insert((
            Visibility::Hidden,
            ClickFlash {
                timer: Timer::from_seconds(0.15, TimerMode::Once),
                pending_actions: elem.on_click.clone(),
            },
        ));
    }
}

/// Tick flash timers — when done, re-show element and dispatch actions.
fn click_flash_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ClickFlash, &mut Visibility)>,
    mut exit_writer: bevy::ecs::message::MessageWriter<bevy::app::AppExit>,
) {
    for (entity, mut flash, mut vis) in &mut query {
        flash.timer.tick(time.delta());
        if !flash.timer.just_finished() {
            continue;
        }

        *vis = Visibility::Inherited;
        let actions: Vec<String> = flash.pending_actions.drain(..).collect();
        commands.entity(entity).remove::<ClickFlash>();

        for action in &actions {
            dispatch_action(action, &mut commands, &mut exit_writer);
        }
    }
}

/// Process `PendingLoadScreen` — despawn current screen, load new one.
fn handle_load_screen(
    mut commands: Commands,
    pending: Option<Res<PendingLoadScreen>>,
    entities: Query<Entity, With<OnScreenRuntime>>,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
) {
    let Some(pending) = pending else { return };
    let screen_id = pending.screen_id.clone();
    commands.remove_resource::<PendingLoadScreen>();
    commands.remove_resource::<RuntimeScreen>();

    // Despawn current screen.
    despawn_runtime(&mut commands, &entities);

    // Load new screen.
    let screen = match load_screen(&screen_id) {
        Ok(s) => s,
        Err(e) => {
            error!("LoadScreen: failed to load '{}': {}", screen_id, e);
            return;
        }
    };

    info!("LoadScreen: '{}' ({} elements)", screen.id, screen.elements.len());

    commands.spawn((Camera2d, OnScreenRuntime));

    if !screen.bg_music.is_empty() {
        spawn_screen_music(&mut commands, &mut audio_sources, &screen.bg_music, &cfg);
    }

    for (i, elem) in screen.elements.iter().enumerate() {
        spawn_runtime_element(&mut commands, &mut ui_assets, &game_assets, &mut images, &cfg, elem, i);
    }

    commands.insert_resource(RuntimeScreen { screen });
}

// ── Action dispatch ─────────────────────────────────────────────────────────

fn dispatch_action(
    action: &str,
    commands: &mut Commands,
    exit_writer: &mut bevy::ecs::message::MessageWriter<bevy::app::AppExit>,
) {
    let trimmed = action.trim();

    if trimmed == "Quit()" {
        info!("action: Quit()");
        exit_writer.write(bevy::app::AppExit::Success);
        return;
    }

    if trimmed == "NewGame()" {
        info!("action: NewGame()");
        commands.set_state(GameState::Loading);
        return;
    }

    if let Some(inner) = parse_string_arg(trimmed, "LoadScreen") {
        info!("action: LoadScreen(\"{}\")", inner);
        commands.insert_resource(PendingLoadScreen {
            screen_id: inner.to_string(),
        });
        return;
    }

    warn!("unknown screen action: '{}'", trimmed);
}

/// Extract string arg from `FuncName("value")`.
fn parse_string_arg<'a>(input: &'a str, func_name: &str) -> Option<&'a str> {
    let rest = input.strip_prefix(func_name)?.trim();
    let rest = rest.strip_prefix('(')?.strip_suffix(')')?;
    let rest = rest.trim();
    rest.strip_prefix('"')?.strip_suffix('"')
}
