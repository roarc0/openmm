//! Screen runtime: renders .ron screen definitions as composable Bevy UI layers.
//! Activated via `--screens=<id>` CLI flag.
//!
//! Multiple screens can be visible simultaneously (e.g. HUD + building UI).
//! - `LoadScreen("x")` — replaces ALL screens with a single new one
//! - `ShowScreen("x")` — adds a screen layer on top of existing ones
//! - `HideScreen("x")` — removes a specific screen layer

use std::collections::HashMap;

use bevy::prelude::*;

use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use openmm_data::Archive;
use openmm_data::assets::{SmkArchive as Vid, SmkDecoder};

use super::{
    ImageElement, REF_H, REF_W, Screen, ScreenElement, TextElement, VideoElement, load_screen,
    load_texture_with_transparency, resolve_image_size,
};
use crate::GameState;
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::fonts::GameFonts;
use crate::game::hud::{FooterText, UiAssets};

pub struct ScreenRuntimePlugin;

impl Plugin for ScreenRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenLayers>()
            // Menu state: load "menu" screen.
            .add_systems(OnEnter(GameState::Menu), menu_screen_setup)
            .add_systems(OnExit(GameState::Menu), screen_teardown)
            // Game state: load "ingame" screen as HUD overlay (no extra camera).
            .add_systems(OnEnter(GameState::Game), game_screen_setup)
            .add_systems(OnExit(GameState::Game), screen_teardown)
            // Interaction systems run in both Menu and Game states.
            .add_systems(
                Update,
                (
                    screen_hover,
                    hover_actions,
                    pulse_hover,
                    pulse_animate,
                    screen_click,
                    screen_keys,
                    video_tick,
                    text_update,
                    click_flash_tick,
                    process_pending_actions,
                )
                    .run_if(in_state(GameState::Menu).or(in_state(GameState::Game))),
            );
    }
}

// ── Components & resources ──────────────────────────────────────────────────

/// Tags an entity as belonging to a specific screen layer.
#[derive(Component, Clone)]
struct ScreenLayer(String);

/// Maps a Bevy entity to a screen element index within its layer.
#[derive(Component)]
struct RuntimeElement {
    screen_id: String,
    index: usize,
}

#[derive(Component)]
struct HoverOverlay;

#[derive(Component)]
struct ScreenMusic(String);

#[derive(Component)]
struct ClickFlash {
    timer: Timer,
    pending_actions: Vec<String>,
}

/// Marks a text element with its data source binding.
#[derive(Component)]
struct RuntimeText {
    source: String,
    font: String,
    color: [u8; 4],
    align: String,
    /// Bounding box in reference pixels: (x, y, w, h).
    bounds: (f32, f32, f32, f32),
    /// Last rendered text — skip re-render if unchanged.
    last_text: String,
}

/// Element starts hidden (from `hidden: true` in RON). Restored to Hidden on unhover.
#[derive(Component)]
struct HiddenByDefault;

/// Runtime state for an inline SMK video.
#[derive(Component)]
struct InlineVideo {
    decoder: SmkDecoder,
    image_handle: Handle<Image>,
    frame_timer: f32,
    spf: f32,
    looping: bool,
    skippable: bool,
    on_end: Vec<String>,
    smk_bytes: Vec<u8>,
    finished: bool,
}

/// Element has PulseSprite() in on_hover — eligible for pulse animation.
#[derive(Component)]
struct Pulsable;

/// Currently pulsing (hover active). Accumulates time for sine wave.
#[derive(Component)]
struct Pulsing {
    elapsed: f32,
}

/// All active screen layers, keyed by screen id.
#[derive(Resource, Default)]
struct ScreenLayers {
    screens: HashMap<String, Screen>,
}

/// Queued actions from click handlers, processed next frame.
#[derive(Resource, Default)]
struct PendingActions {
    actions: Vec<String>,
}

// ── Setup & teardown ────────────────────────────────────────────────────────

/// Menu state: spawn Camera2d + load "menu" screen.
fn menu_screen_setup(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut layers: ResMut<ScreenLayers>,
) {
    if !cfg.screens {
        return;
    }

    commands.spawn((Camera2d, ScreenLayer("__camera__".into())));

    show_screen(
        "logo",
        &mut commands,
        &mut layers,
        &mut ui_assets,
        &game_assets,
        &mut images,
        &mut audio_sources,
        &cfg,
    );
}

/// Game state: spawn UI camera + load "ingame" screen as HUD overlay.
fn game_screen_setup(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut layers: ResMut<ScreenLayers>,
) {
    if !cfg.screens {
        return;
    }

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
    );
}

fn screen_teardown(
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

fn show_screen(
    screen_id: &str,
    commands: &mut Commands,
    layers: &mut ScreenLayers,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    audio_sources: &mut Assets<AudioSource>,
    cfg: &GameConfig,
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

    if !screen.bg_music.is_empty() {
        spawn_screen_music(commands, audio_sources, &screen.bg_music, screen_id, cfg);
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
        commands.insert_resource(PendingActions {
            actions: screen.on_load.clone(),
        });
    }

    layers.screens.insert(screen_id.to_string(), screen);
}

fn hide_screen(
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

fn load_screen_replace_all(
    screen_id: &str,
    commands: &mut Commands,
    layers: &mut ScreenLayers,
    entities: &Query<(Entity, &ScreenLayer)>,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    audio_sources: &mut Assets<AudioSource>,
    cfg: &GameConfig,
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
    );
}

// ── Music ───────────────────────────────────────────────────────────────────

fn spawn_screen_music(
    commands: &mut Commands,
    audio_sources: &mut Assets<AudioSource>,
    track: &str,
    screen_id: &str,
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
                ScreenMusic(screen_id.to_string()),
                ScreenLayer(screen_id.to_string()),
            ));
            info!("screen music: playing '{}' from {:?}", track, path);
        } else {
            warn!("screen music: failed to read {:?}", path);
        }
    } else {
        warn!("screen music: '{}' not found (searched {:?})", track_name, base_dir);
    }
}

// ── Video spawning ──────────────────────────────────────────────────────────

/// Build a minimal WAV from raw PCM bytes.
fn build_wav(pcm: &[u8], channels: u8, sample_rate: u32, bitdepth: u8) -> Vec<u8> {
    let data_len = pcm.len() as u32;
    let block_align = (channels as u32) * (bitdepth as u32 / 8);
    let byte_rate = sample_rate * block_align;
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&(channels as u16).to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&(block_align as u16).to_le_bytes());
    wav.extend_from_slice(&(bitdepth as u16).to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm);
    wav
}

/// Load SMK bytes from Anims1.vid / Anims2.vid.
fn load_smk_bytes(name: &str) -> Option<Vec<u8>> {
    let data_path = openmm_data::get_data_path();
    let base = std::path::Path::new(&data_path);
    let parent = base.parent().unwrap_or(base);
    let anims_dir = openmm_data::utils::find_path_case_insensitive(parent, "Anims")?;

    ["Anims1.vid", "Anims2.vid"].iter().find_map(|fname| {
        let path = openmm_data::utils::find_path_case_insensitive(&anims_dir, fname)?;
        let vid = Vid::open(&path).ok()?;
        vid.get_file(name)
    })
}

fn spawn_video_element(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    audio_sources: &mut Assets<AudioSource>,
    vid: &VideoElement,
    layer_tag: &ScreenLayer,
) {
    let Some(bytes) = load_smk_bytes(&vid.video) else {
        warn!(
            "video element '{}': '{}' not found in Anims VID archives",
            vid.id, vid.video
        );
        return;
    };

    let mut decoder = match SmkDecoder::new(bytes.clone()) {
        Ok(d) => d,
        Err(e) => {
            warn!("video element '{}': failed to decode '{}': {e}", vid.id, vid.video);
            return;
        }
    };

    let native_w = decoder.width;
    let native_h = decoder.height;
    let spf = if decoder.fps > 0.0 {
        1.0 / decoder.fps
    } else {
        1.0 / 15.0
    };

    if let Some(audio_info) = decoder.audio
        && let Ok(mut audio_dec) = SmkDecoder::new(bytes.clone())
    {
        let mut pcm: Vec<u8> = Vec::new();
        while audio_dec.next_frame().is_some() {
            pcm.extend_from_slice(&audio_dec.decode_current_audio());
        }
        if !pcm.is_empty() {
            let wav = build_wav(&pcm, audio_info.channels, audio_info.rate, audio_info.bitdepth);
            let handle = audio_sources.add(AudioSource { bytes: wav.into() });
            let mode = if vid.looping {
                bevy::audio::PlaybackMode::Loop
            } else {
                bevy::audio::PlaybackMode::Despawn
            };
            commands.spawn((
                AudioPlayer(handle),
                PlaybackSettings { mode, ..default() },
                layer_tag.clone(),
            ));
        }
    }

    let mut image = Image::new_fill(
        Extent3d {
            width: native_w,
            height: native_h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    if let Some(rgba) = decoder.next_frame() {
        image.data = Some(rgba);
    }
    let image_handle = images.add(image);

    let (w, h) = if vid.size.0 > 0.0 && vid.size.1 > 0.0 {
        vid.size
    } else {
        (native_w as f32, native_h as f32)
    };

    let initial_vis = if vid.hidden {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    commands.spawn((
        ImageNode::new(image_handle.clone()),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(vid.position.0 / REF_W * 100.0),
            top: Val::Percent(vid.position.1 / REF_H * 100.0),
            width: Val::Percent(w / REF_W * 100.0),
            height: Val::Percent(h / REF_H * 100.0),
            ..default()
        },
        ZIndex(vid.z),
        initial_vis,
        layer_tag.clone(),
        InlineVideo {
            decoder,
            image_handle,
            frame_timer: 0.0,
            spf,
            looping: vid.looping,
            skippable: vid.skippable,
            on_end: vid.on_end.clone(),
            smk_bytes: bytes,
            finished: false,
        },
    ));

    info!(
        "video element '{}': '{}' ({}x{}, {:.1}fps, loop={})",
        vid.id,
        vid.video,
        native_w,
        native_h,
        1.0 / spf,
        vid.looping
    );
}

/// Advance inline video frames and dispatch on_end actions.
fn video_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut InlineVideo)>,
    mut images: ResMut<Assets<Image>>,
    keys: Res<ButtonInput<KeyCode>>,
    pending: Option<Res<PendingActions>>,
) {
    for (entity, mut vid) in &mut query {
        if vid.finished {
            continue;
        }

        // Skip check.
        if vid.skippable && keys.just_pressed(KeyCode::Escape) {
            vid.finished = true;
            if !vid.on_end.is_empty() && pending.is_none() {
                commands.insert_resource(PendingActions {
                    actions: vid.on_end.clone(),
                });
            }
            commands.entity(entity).despawn();
            continue;
        }

        vid.frame_timer += time.delta_secs();
        if vid.frame_timer < vid.spf {
            continue;
        }
        vid.frame_timer -= vid.spf;

        match vid.decoder.next_frame() {
            Some(rgba) => {
                if let Some(img) = images.get_mut(&vid.image_handle) {
                    img.data = Some(rgba);
                }
            }
            None => {
                if vid.looping {
                    // Restart decoder from beginning.
                    if let Ok(new_dec) = SmkDecoder::new(vid.smk_bytes.clone()) {
                        vid.decoder = new_dec;
                        vid.frame_timer = 0.0;
                    } else {
                        vid.finished = true;
                    }
                } else {
                    vid.finished = true;
                    if !vid.on_end.is_empty() && pending.is_none() {
                        commands.insert_resource(PendingActions {
                            actions: vid.on_end.clone(),
                        });
                    }
                    commands.entity(entity).despawn();
                }
            }
        }
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
    screen_id: &str,
    layer_tag: &ScreenLayer,
    audio_sources: &mut Assets<AudioSource>,
) {
    match elem {
        ScreenElement::Image(img) => {
            spawn_image_element(
                commands,
                ui_assets,
                game_assets,
                images,
                cfg,
                img,
                index,
                screen_id,
                layer_tag,
            );
        }
        ScreenElement::Video(vid) => {
            spawn_video_element(commands, images, audio_sources, vid, layer_tag);
        }
        ScreenElement::Text(txt) => {
            spawn_text_element(commands, txt, layer_tag);
        }
    }
}

fn spawn_image_element(
    commands: &mut Commands,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
    img: &ImageElement,
    index: usize,
    screen_id: &str,
    layer_tag: &ScreenLayer,
) {
    let (w, h) = resolve_image_size(img, ui_assets);

    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(img.position.0 / REF_W * 100.0),
        top: Val::Percent(img.position.1 / REF_H * 100.0),
        width: Val::Percent(w / REF_W * 100.0),
        height: Val::Percent(h / REF_H * 100.0),
        ..default()
    };

    let default_tex = img.texture_for_state("default").unwrap_or("").to_string();
    let default_handle = if !default_tex.is_empty() {
        load_texture_with_transparency(
            &default_tex,
            &img.transparent_color,
            ui_assets,
            game_assets,
            images,
            cfg,
        )
    } else {
        None
    };

    let hover_handle = img.states.get("hover").and_then(|state| {
        if state.texture.is_empty() {
            None
        } else {
            load_texture_with_transparency(
                &state.texture,
                &img.transparent_color,
                ui_assets,
                game_assets,
                images,
                cfg,
            )
        }
    });

    let has_interaction = hover_handle.is_some() || !img.on_click.is_empty() || !img.on_hover.is_empty();
    let has_pulse = img.on_hover.iter().any(|a| a.trim() == "PulseSprite()");
    let z = ZIndex(img.z);
    let marker = RuntimeElement {
        screen_id: screen_id.to_string(),
        index,
    };
    let initial_vis = if img.hidden {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };

    if let Some(handle) = default_handle {
        let mut entity = commands.spawn((ImageNode::new(handle), node, z, marker, layer_tag.clone(), initial_vis));
        if has_interaction {
            entity.insert((Button, BackgroundColor(Color::NONE)));
        }
        if has_pulse {
            entity.insert(Pulsable);
        }
        if img.hidden {
            entity.insert(HiddenByDefault);
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
        let mut entity = commands.spawn((node, z, marker, layer_tag.clone(), initial_vis));
        if has_interaction {
            entity.insert((Button, BackgroundColor(Color::NONE)));
        }
        if has_pulse {
            entity.insert(Pulsable);
        }
        if img.hidden {
            entity.insert(HiddenByDefault);
        }
    }
}

// ── Text elements ───────────────────────────────────────────────────────────

fn spawn_text_element(
    commands: &mut Commands,
    txt: &TextElement,
    layer_tag: &ScreenLayer,
) {
    // Text starts hidden. Width/position set dynamically by text_update.
    let node = Node {
        position_type: PositionType::Absolute,
        top: Val::Percent(txt.position.1 / REF_H * 100.0),
        // Store the reference height as percent; text_update converts to Px for rendering.
        height: Val::Percent(txt.size.1 / REF_H * 100.0),
        width: Val::Auto,
        ..default()
    };

    commands.spawn((
        ImageNode::new(Handle::default()),
        node,
        ZIndex(txt.z),
        Visibility::Hidden,
        layer_tag.clone(),
        RuntimeText {
            source: txt.source.clone(),
            font: txt.font.clone(),
            color: txt.color_rgba(),
            align: txt.align.clone(),
            bounds: (txt.position.0, txt.position.1, txt.size.0, txt.size.1),
            last_text: "\x00".to_string(), // sentinel — forces first update
        },
    ));
}

/// Read data sources, re-render text when content changes, reposition every frame.
fn text_update(
    footer: Res<FooterText>,
    world_state: Option<Res<crate::game::world::WorldState>>,
    game_fonts: Res<GameFonts>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut query: Query<(&mut RuntimeText, &mut ImageNode, &mut Visibility, &mut Node)>,
) {
    let Ok(window) = windows.single() else { return };
    let win_w = window.width();
    let win_h = window.height();
    let sx = win_w / REF_W;
    let sy = win_h / REF_H;

    for (mut rt, mut img_node, mut vis, mut node) in &mut query {
        let current = match rt.source.as_str() {
            "footer_text" => footer.text().to_string(),
            "gold" => world_state.as_ref().map_or(String::new(), |ws| ws.game_vars.gold.to_string()),
            "food" => world_state.as_ref().map_or(String::new(), |ws| ws.game_vars.food.to_string()),
            _ => String::new(),
        };

        // Re-render only when text changed.
        if current != rt.last_text {
            rt.last_text = current.clone();

            if current.is_empty() {
                *vis = Visibility::Hidden;
                continue;
            }

            if let Some(handle) = game_fonts.render(&current, &rt.font, rt.color, &mut images) {
                img_node.image = handle;
                *vis = Visibility::Inherited;
            } else {
                *vis = Visibility::Hidden;
                continue;
            }
        }

        // Skip positioning if hidden.
        if rt.last_text.is_empty() {
            continue;
        }

        // Bounding box in screen pixels.
        let (bx, by, bw, bh) = rt.bounds;
        let box_x = bx * sx;
        let box_w = bw * sx;
        let display_h = bh * sy;

        // Compute rendered text width in screen pixels.
        let text_px_w = game_fonts.measure(&rt.last_text, &rt.font) as f32;
        let display_w = if let Some(font) = game_fonts.get(&rt.font) {
            text_px_w * (display_h / font.height as f32)
        } else {
            text_px_w * sx
        };

        // Position text within bounding box based on alignment.
        node.width = Val::Auto;
        node.height = Val::Px(display_h);
        node.top = Val::Px(by * sy);
        node.left = Val::Auto;
        node.right = Val::Auto;

        match rt.align.as_str() {
            "right" => {
                // Right edge of text at right edge of bounding box.
                let right_edge = box_x + box_w;
                node.left = Val::Px(right_edge - display_w);
            }
            "center" => {
                // Center text within bounding box.
                let center_x = box_x + box_w / 2.0;
                node.left = Val::Px(center_x - display_w / 2.0);
            }
            _ => {
                node.left = Val::Px(box_x);
            }
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
                *vis = if show {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

/// Dispatch on_hover actions when hover starts (excluding PulseSprite, handled separately).
fn hover_actions(
    mut commands: Commands,
    query: Query<(&Interaction, &RuntimeElement), Changed<Interaction>>,
    layers: Res<ScreenLayers>,
    pending: Option<Res<PendingActions>>,
) {
    // Don't queue hover actions while another action batch is pending.
    if pending.is_some() {
        return;
    }
    for (interaction, rt_elem) in &query {
        if *interaction != Interaction::Hovered {
            continue;
        }
        let Some(screen) = layers.screens.get(&rt_elem.screen_id) else {
            continue;
        };
        let hover_actions: Vec<String> = screen.elements[rt_elem.index]
            .on_hover()
            .iter()
            .filter(|a| a.trim() != "PulseSprite()")
            .cloned()
            .collect();
        if !hover_actions.is_empty() {
            commands.insert_resource(PendingActions { actions: hover_actions });
        }
    }
}

/// Start/stop pulsing on hover for Pulsable elements.
fn pulse_hover(
    mut commands: Commands,
    query: Query<(Entity, &Interaction, Has<HiddenByDefault>), (Changed<Interaction>, With<Pulsable>)>,
    pulsing_query: Query<&Pulsing>,
    mut image_query: Query<&mut ImageNode>,
) {
    for (entity, interaction, hidden_default) in &query {
        let hovering = matches!(interaction, Interaction::Hovered | Interaction::Pressed);
        if hovering && !pulsing_query.contains(entity) {
            commands
                .entity(entity)
                .insert((Pulsing { elapsed: 0.0 }, Visibility::Inherited));
        } else if !hovering && pulsing_query.contains(entity) {
            commands.entity(entity).remove::<Pulsing>();
            if let Ok(mut img) = image_query.get_mut(entity) {
                img.color = img.color.with_alpha(1.0);
            }
            if hidden_default {
                commands.entity(entity).insert(Visibility::Hidden);
            }
        }
    }
}

/// Animate alpha on pulsing elements: smooth 0→1→0 each second via sine wave.
fn pulse_animate(time: Res<Time>, mut query: Query<(&mut Pulsing, &mut ImageNode)>) {
    for (mut pulse, mut img) in &mut query {
        pulse.elapsed += time.delta_secs();
        // sin gives -1..1, remap to 0..1. Full cycle = 1 second (2π per second).
        let alpha = (pulse.elapsed * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        img.color = img.color.with_alpha(alpha);
    }
}

/// On click: hide element briefly, then fire actions after the flash.
fn screen_click(
    mut commands: Commands,
    query: Query<(Entity, &Interaction, &RuntimeElement), (Changed<Interaction>, With<Button>)>,
    layers: Res<ScreenLayers>,
    flash_query: Query<&ClickFlash>,
) {
    for (entity, interaction, rt_elem) in &query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if flash_query.contains(entity) {
            continue;
        }
        let Some(screen) = layers.screens.get(&rt_elem.screen_id) else {
            continue;
        };
        let elem = &screen.elements[rt_elem.index];
        if elem.on_click().is_empty() {
            continue;
        }

        info!("screen click [{}/{}]", rt_elem.screen_id, elem.id());
        commands.entity(entity).insert((
            Visibility::Hidden,
            ClickFlash {
                timer: Timer::from_seconds(0.15, TimerMode::Once),
                pending_actions: elem.on_click().to_vec(),
            },
        ));
    }
}

/// Check keyboard shortcuts defined in all active screens.
fn screen_keys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    layers: Res<ScreenLayers>,
    pending: Option<Res<PendingActions>>,
) {
    // Don't queue keys while another action batch is pending.
    if pending.is_some() {
        return;
    }
    for screen in layers.screens.values() {
        for (key_name, actions) in &screen.keys {
            if let Some(code) = parse_key_code(key_name) {
                if keys.just_pressed(code) {
                    info!("screen key [{}]: {}", screen.id, key_name);
                    commands.insert_resource(PendingActions {
                        actions: actions.clone(),
                    });
                    return; // one key per frame
                }
            }
        }
    }
}

/// Map a key name string to a Bevy KeyCode.
fn parse_key_code(name: &str) -> Option<KeyCode> {
    Some(match name {
        "Escape" => KeyCode::Escape,
        "Return" | "Enter" => KeyCode::Enter,
        "Space" => KeyCode::Space,
        "Tab" => KeyCode::Tab,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Up" => KeyCode::ArrowUp,
        "Down" => KeyCode::ArrowDown,
        "Left" => KeyCode::ArrowLeft,
        "Right" => KeyCode::ArrowRight,
        // Letters
        "A" => KeyCode::KeyA,
        "B" => KeyCode::KeyB,
        "C" => KeyCode::KeyC,
        "D" => KeyCode::KeyD,
        "E" => KeyCode::KeyE,
        "F" => KeyCode::KeyF,
        "G" => KeyCode::KeyG,
        "H" => KeyCode::KeyH,
        "I" => KeyCode::KeyI,
        "J" => KeyCode::KeyJ,
        "K" => KeyCode::KeyK,
        "L" => KeyCode::KeyL,
        "M" => KeyCode::KeyM,
        "N" => KeyCode::KeyN,
        "O" => KeyCode::KeyO,
        "P" => KeyCode::KeyP,
        "Q" => KeyCode::KeyQ,
        "R" => KeyCode::KeyR,
        "S" => KeyCode::KeyS,
        "T" => KeyCode::KeyT,
        "U" => KeyCode::KeyU,
        "V" => KeyCode::KeyV,
        "W" => KeyCode::KeyW,
        "X" => KeyCode::KeyX,
        "Y" => KeyCode::KeyY,
        "Z" => KeyCode::KeyZ,
        // Numbers
        "0" => KeyCode::Digit0,
        "1" => KeyCode::Digit1,
        "2" => KeyCode::Digit2,
        "3" => KeyCode::Digit3,
        "4" => KeyCode::Digit4,
        "5" => KeyCode::Digit5,
        "6" => KeyCode::Digit6,
        "7" => KeyCode::Digit7,
        "8" => KeyCode::Digit8,
        "9" => KeyCode::Digit9,
        // Function keys
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F7" => KeyCode::F7,
        "F8" => KeyCode::F8,
        "F9" => KeyCode::F9,
        "F10" => KeyCode::F10,
        "F11" => KeyCode::F11,
        "F12" => KeyCode::F12,
        other => {
            warn!("unknown key name: '{}'", other);
            return None;
        }
    })
}

/// Tick flash timers — collect actions into PendingActions for deferred processing.
fn click_flash_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ClickFlash, &mut Visibility)>,
) {
    for (entity, mut flash, mut vis) in &mut query {
        flash.timer.tick(time.delta());
        if !flash.timer.just_finished() {
            continue;
        }

        *vis = Visibility::Inherited;
        let actions: Vec<String> = flash.pending_actions.drain(..).collect();
        commands.entity(entity).remove::<ClickFlash>();

        if !actions.is_empty() {
            commands.insert_resource(PendingActions { actions });
        }
    }
}

/// Process queued actions with full system access (commands, layers, entities, exit).
/// Uses the scripting executor for Compare/Else/End control flow.
fn process_pending_actions(
    mut commands: Commands,
    pending: Option<Res<PendingActions>>,
    mut layers: ResMut<ScreenLayers>,
    layer_entities: Query<(Entity, &ScreenLayer)>,
    cfg: Res<GameConfig>,
    game_assets: Res<GameAssets>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut exit_writer: bevy::ecs::message::MessageWriter<bevy::app::AppExit>,
    world_state: Option<Res<crate::game::world::WorldState>>,
    mut event_queue: Option<ResMut<crate::game::world::scripting::EventQueue>>,
    _time: Res<Time>,
) {
    use super::scripting::Action;

    let Some(pending) = pending else { return };
    let action_strings = pending.actions.clone();
    commands.remove_resource::<PendingActions>();

    // Build script context from available resources.
    let default_vars = crate::game::world::state::GameVariables::default();
    let vars = world_state.as_ref().map(|ws| &ws.game_vars).unwrap_or(&default_vars);
    let config_flags = build_config_flags(&cfg);
    let ctx = super::scripting::ScriptContext { vars, config_flags: &config_flags };

    let actions = super::scripting::execute_actions(&action_strings, &ctx);

    for action in actions {
        match action {
            Action::Quit => {
                info!("action: Quit");
                exit_writer.write(bevy::app::AppExit::Success);
            }
            Action::NewGame => {
                info!("action: NewGame");
                commands.set_state(GameState::Loading);
            }
            Action::LoadScreen(id) => {
                info!("action: LoadScreen(\"{}\")", id);
                load_screen_replace_all(
                    &id,
                    &mut commands,
                    &mut layers,
                    &layer_entities,
                    &mut ui_assets,
                    &game_assets,
                    &mut images,
                    &mut audio_sources,
                    &cfg,
                );
            }
            Action::ShowScreen(id) => {
                info!("action: ShowScreen(\"{}\")", id);
                show_screen(
                    &id,
                    &mut commands,
                    &mut layers,
                    &mut ui_assets,
                    &game_assets,
                    &mut images,
                    &mut audio_sources,
                    &cfg,
                );
            }
            Action::HideScreen(id) => {
                info!("action: HideScreen(\"{}\")", id);
                hide_screen(&id, &mut commands, &mut layers, &layer_entities);
            }
            Action::ShowSprite(_) => {
                info!("action: ShowSprite (not yet wired)");
            }
            Action::HideSprite(_) => {
                info!("action: HideSprite (not yet wired)");
            }
            Action::PulseSprite => {} // handled at spawn time
            Action::EvtProxy(evt_str) => {
                if let Some(ref mut eq) = event_queue {
                    proxy_evt_action(&evt_str, eq);
                }
            }
            Action::Unknown(s) => {
                warn!("unknown screen action: '{}'", s);
            }
            Action::Compare(_) | Action::Else | Action::End => {} // consumed by execute_actions
        }
    }
}

/// Build config flags set from GameConfig for condition evaluation.
fn build_config_flags(cfg: &GameConfig) -> std::collections::HashSet<String> {
    let mut flags = std::collections::HashSet::new();
    if cfg.skip_intro { flags.insert("skip_intro".into()); }
    if cfg.debug { flags.insert("debug".into()); }
    if cfg.console { flags.insert("console".into()); }
    flags
}

/// Proxy an `evt:` action string to the EVT EventQueue.
fn proxy_evt_action(
    evt_str: &str,
    event_queue: &mut crate::game::world::scripting::EventQueue,
) {
    use openmm_data::evt::GameEvent;

    let s = evt_str.trim();

    // PlaySound(id)
    if let Some(rest) = s.strip_prefix("PlaySound(").and_then(|r| r.strip_suffix(')')) {
        if let Ok(id) = rest.trim().parse::<u32>() {
            event_queue.push_single(GameEvent::PlaySound { sound_id: id });
            return;
        }
    }

    // Hint("text")
    if let Some(text) = super::scripting::parse_string_arg(s, "Hint") {
        event_queue.push_single(GameEvent::Hint {
            str_id: 0,
            text: text.to_string(),
        });
        return;
    }

    // StatusText("text")
    if let Some(text) = super::scripting::parse_string_arg(s, "StatusText") {
        event_queue.push_single(GameEvent::StatusText {
            str_id: 0,
            text: text.to_string(),
        });
        return;
    }

    warn!("evt: unknown proxy action: '{}'", s);
}
