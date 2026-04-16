//! Screen lifecycle: setup, teardown, show/hide/load, and preloading.

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

use super::runtime::{ScreenActions, ScreenLayer, ScreenLayers};
use super::{Screen, ScreenElement, load_screen};
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::optional::OptionalWrite;
use crate::game::ui_assets::UiAssets;

use super::elements::{spawn_runtime_element, spawn_screen_crosshair, spawn_screen_music};

/// Menu state: spawn Camera2d + load "menu" screen.
pub(super) fn menu_screen_setup(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut layers: ResMut<ScreenLayers>,
    mut actions: Option<MessageWriter<ScreenActions>>,
) {
    commands.spawn((Camera2d, ScreenLayer("__camera__".into())));

    show_screen(
        "3dologo",
        &mut commands,
        &mut layers,
        &mut ui_assets,
        &game_assets,
        &mut images,
        &mut audio_sources,
        &cfg,
        &mut actions,
    );
}

/// Loading state: spawn Camera2d + load "loading" screen.
pub(super) fn loading_screen_setup(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut layers: ResMut<ScreenLayers>,
    mut actions: Option<MessageWriter<ScreenActions>>,
) {
    commands.spawn((Camera2d, ScreenLayer("__camera__".into())));

    show_screen(
        "loading",
        &mut commands,
        &mut layers,
        &mut ui_assets,
        &game_assets,
        &mut images,
        &mut audio_sources,
        &cfg,
        &mut actions,
    );
}

/// Game state: spawn UI camera + load "ingame" screen as HUD overlay.
pub(super) fn game_screen_setup(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut layers: ResMut<ScreenLayers>,
    mut actions: Option<MessageWriter<ScreenActions>>,
) {
    // UI camera renders on top of the 3D scene (order=1, no clear).
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        bevy::ui::IsDefaultUiCamera,
        ScreenLayer("__camera__".into()),
    ));

    show_screen(
        "ingame",
        &mut commands,
        &mut layers,
        &mut ui_assets,
        &game_assets,
        &mut images,
        &mut audio_sources,
        &cfg,
        &mut actions,
    );
}

pub(super) fn screen_teardown(
    mut commands: Commands,
    entities: Query<Entity, With<ScreenLayer>>,
    mut layers: ResMut<ScreenLayers>,
) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    layers.screens.clear();
}

// ── Show / Hide / Load ──────────────────────────────────────────────────────

pub(super) fn show_screen(
    screen_id: &str,
    commands: &mut Commands,
    layers: &mut ScreenLayers,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    audio_sources: &mut Assets<AudioSource>,
    cfg: &GameConfig,
    actions: &mut Option<MessageWriter<ScreenActions>>,
) {
    if layers.screens.contains_key(screen_id) {
        warn!("ShowScreen: '{}' already visible", screen_id);
        return;
    }

    let screen = match load_screen(screen_id) {
        Ok(s) => s,
        Err(e) => {
            error!("ShowScreen: failed to load '{}': {}", screen_id, e);
            return;
        }
    };

    info!("ShowScreen: '{}' ({} elements)", screen.id, screen.elements.len());

    if !screen.sound.is_empty() {
        spawn_screen_music(commands, audio_sources, &screen.sound, screen_id, cfg, game_assets);
    }

    let layer_tag = ScreenLayer(screen_id.to_string());
    for (i, elem) in screen.elements.iter().enumerate() {
        spawn_runtime_element(
            commands,
            ui_assets,
            game_assets,
            images,
            cfg,
            elem,
            i,
            screen_id,
            &layer_tag,
            audio_sources,
        );
    }

    // Queue on_load actions if present.
    if !screen.on_load.is_empty() {
        actions.try_write(ScreenActions {
            actions: screen.on_load.clone(),
        });
    }

    // Spawn crosshair for the ingame screen.
    if screen_id == "ingame" {
        spawn_screen_crosshair(commands, &layer_tag);
    }

    // Preload assets for screens reachable from this one.
    preload_next_screens(&screen, game_assets);

    layers.screens.insert(screen_id.to_string(), screen);
}

pub(super) fn hide_screen(
    screen_id: &str,
    commands: &mut Commands,
    layers: &mut ScreenLayers,
    entities: &Query<(Entity, &ScreenLayer)>,
) {
    if layers.screens.remove(screen_id).is_none() {
        warn!("HideScreen: '{}' not visible", screen_id);
        return;
    }

    info!("HideScreen: '{}'", screen_id);
    for (entity, layer) in entities.iter() {
        if layer.0 == screen_id {
            commands.entity(entity).despawn();
        }
    }
}

pub(super) fn load_screen_replace_all(
    screen_id: &str,
    commands: &mut Commands,
    layers: &mut ScreenLayers,
    entities: &Query<(Entity, &ScreenLayer)>,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    audio_sources: &mut Assets<AudioSource>,
    cfg: &GameConfig,
    actions: &mut Option<MessageWriter<ScreenActions>>,
) {
    // Despawn everything except the camera.
    for (entity, layer) in entities.iter() {
        if layer.0 != "__camera__" {
            commands.entity(entity).despawn();
        }
    }
    layers.screens.clear();

    show_screen(
        screen_id,
        commands,
        layers,
        ui_assets,
        game_assets,
        images,
        audio_sources,
        cfg,
        actions,
    );
}

// ── Preloading ──────────────────────────────────────────────────────────────

/// Scan a screen's actions for `LoadScreen("x")` references and eagerly cache
/// the target screen's decoded audio and music in the `MediaCache`.
///
/// Raw video bytes are served by `GameAssets` (VID archives loaded at startup).
/// Only loads assets not already present in the cache; safe to call repeatedly.
pub(super) fn preload_next_screens(screen: &Screen, game_assets: &GameAssets) {
    let mut next_ids: Vec<String> = Vec::new();

    // Collect LoadScreen targets from all action lists in this screen.
    let mut collect = |actions: &[String]| {
        for a in actions {
            if let Some(id) = super::scripting::parse_string_arg(a.trim(), "LoadScreen") {
                next_ids.push(id.to_string());
            }
        }
    };

    // on_load actions.
    collect(&screen.on_load);

    // Element-level actions.
    for elem in &screen.elements {
        match elem {
            ScreenElement::Video(vid) => collect(&vid.on_end),
            ScreenElement::Image(img) => {
                collect(&img.on_click);
                collect(&img.on_hover);
            }
            ScreenElement::Text(_) => {}
        }
    }

    // Keyboard shortcut actions.
    for actions in screen.keys.values() {
        collect(actions);
    }

    // Deduplicate.
    next_ids.sort();
    next_ids.dedup();

    // Preload each target screen's assets.
    for next_id in &next_ids {
        // Parse the screen RON (fast).
        if let Ok(next_screen) = load_screen(next_id) {
            // Pre-decode audio for upcoming videos.
            for elem in &next_screen.elements {
                if let ScreenElement::Video(vid) = elem {
                    game_assets.preload_smk_audio(&vid.video);
                }
            }

            // Preload upcoming music.
            if !next_screen.sound.is_empty() {
                game_assets.preload_music(next_screen.sound.id());
            }
        }
    }
}
