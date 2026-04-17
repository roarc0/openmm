//! Element spawning: images, text, crosshair, and music.

use bevy::prelude::*;

use super::bindings::{ArrowBinding, CompassBinding, CroppedImage, MinimapBinding, TapBinding};
use super::runtime::{
    HiddenByDefault, HoverOverlay, Pulsable, RuntimeElement, RuntimeText, ScreenCrosshair, ScreenLayer, ScreenMusic,
};
use super::video::spawn_video_element;
use super::{
    ImageElement, REF_H, REF_W, ScreenElement, TextElement, load_texture_with_transparency, resolve_image_size,
};
use crate::assets::GameAssets;
use crate::config::GameConfig;
use super::fonts::GameFonts;
use crate::game::state::ui_state::UiState;
use crate::screens::ui_assets::UiAssets;

// ── Element spawning ────────────────────────────────────────────────────────

pub(super) fn spawn_runtime_element(
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
            spawn_video_element(commands, images, audio_sources, vid, layer_tag, game_assets);
        }
        ScreenElement::Text(txt) => {
            spawn_text_element(commands, txt, layer_tag);
        }
    }
}

pub(super) fn spawn_image_element(
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

    // Cropped image: spawn as clip container + scrollable inner image.
    let has_crop = img.crop_w > 0.0 && img.crop_h > 0.0;
    if has_crop {
        let crop_w = img.crop_w;
        let crop_h = img.crop_h;
        let binding = img.bindings.get("source").map(|s| s.as_str());

        // For minimap, texture is loaded at runtime — use a placeholder.
        let image_handle = if binding == Some("minimap") {
            None // loaded dynamically by minimap_scroll
        } else {
            default_handle.clone()
        };

        let clip_node = Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(img.position.0 / REF_W * 100.0),
            top: Val::Percent(img.position.1 / REF_H * 100.0),
            width: Val::Percent(crop_w / REF_W * 100.0),
            height: Val::Percent(crop_h / REF_H * 100.0),
            overflow: Overflow::clip(),
            ..default()
        };
        let marker = RuntimeElement {
            screen_id: screen_id.to_string(),
            index,
            element_id: img.id.clone(),
        };
        commands
            .spawn((clip_node, ZIndex(img.z), marker, layer_tag.clone()))
            .with_children(|clip| {
                // Inner image fills the crop area by default (bindings scroll it).
                let inner_w = if w > crop_w {
                    Val::Percent(w / crop_w * 100.0)
                } else {
                    Val::Percent(100.0)
                };
                let inner_h = if h > crop_h {
                    Val::Percent(h / crop_h * 100.0)
                } else {
                    Val::Percent(100.0)
                };

                let mut inner = clip.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: inner_w,
                        height: inner_h,
                        ..default()
                    },
                    CroppedImage { crop_w, crop_h },
                ));

                if let Some(handle) = image_handle {
                    inner.insert(ImageNode::new(handle));
                } else if binding == Some("minimap") {
                    // Minimap gets a default ImageNode — texture set by minimap_scroll.
                    inner.insert(ImageNode::default());
                }

                match binding {
                    Some("compass") => {
                        inner.insert(CompassBinding);
                    }
                    Some("minimap") => {
                        inner.insert(MinimapBinding { zoom: 3.0 });
                    }
                    _ => {}
                }
            });
        return;
    }

    let has_interaction = hover_handle.is_some() || !img.on_click.is_empty() || !img.on_hover.is_empty();
    let has_pulse = img.on_hover.iter().any(|a| a.trim() == "PulseSprite()");
    let z = ZIndex(img.z);
    let marker = RuntimeElement {
        screen_id: screen_id.to_string(),
        index,
        element_id: img.id.clone(),
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
        match img.bindings.get("source").map(|s| s.as_str()) {
            Some("arrow") => {
                entity.insert(ArrowBinding);
            }
            Some("tap") => {
                entity.insert(TapBinding);
            }
            Some("loading") => {
                let frame = img
                    .bindings
                    .get("frame")
                    .and_then(|f| f.parse::<u32>().ok())
                    .unwrap_or(0);
                entity.insert(super::bindings::LoadingFrameBinding { frame });
            }
            _ => {}
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

pub(super) fn spawn_text_element(commands: &mut Commands, txt: &TextElement, layer_tag: &ScreenLayer) {
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
pub(super) fn text_update(
    ui: Res<UiState>,
    world_state: Option<Res<crate::game::state::WorldState>>,
    loading_step: Option<Res<crate::prepare::loading::LoadingStep>>,
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
            "footer_text" => ui.footer.text().to_string(),
            "gold" => world_state
                .as_ref()
                .map_or(String::new(), |ws| ws.game_vars.gold.to_string()),
            "food" => world_state
                .as_ref()
                .map_or(String::new(), |ws| ws.game_vars.food.to_string()),
            "loading_step" => loading_step.as_ref().map_or(String::new(), |s| s.label().to_string()),
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
        // Use set_if_neq to avoid triggering UI layout recalculation when values haven't changed.
        let target_left = match rt.align.as_str() {
            "right" => {
                let right_edge = box_x + box_w;
                Val::Px(right_edge - display_w)
            }
            "center" => {
                let center_x = box_x + box_w / 2.0;
                Val::Px(center_x - display_w / 2.0)
            }
            _ => Val::Px(box_x),
        };
        let target_top = Val::Px(by * sy);
        let target_h = Val::Px(display_h);

        if node.width != Val::Auto {
            node.width = Val::Auto;
        }
        if node.height != target_h {
            node.height = target_h;
        }
        if node.top != target_top {
            node.top = target_top;
        }
        if node.left != target_left {
            node.left = target_left;
        }
        if node.right != Val::Auto {
            node.right = Val::Auto;
        }
    }
}

// ── Crosshair ──────────────────────────────────────────────────────────────

/// Spawn a crosshair overlay for the ingame screen.
pub(super) fn spawn_screen_crosshair(commands: &mut Commands, layer_tag: &ScreenLayer) {
    let color = BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.9));
    commands
        .spawn((
            Name::new("screen_crosshair"),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(50),
            ScreenCrosshair,
            layer_tag.clone(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("crosshair_h"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(9.0),
                    width: Val::Px(20.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                color,
            ));
            parent.spawn((
                Name::new("crosshair_v"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(9.0),
                    top: Val::Px(0.0),
                    width: Val::Px(2.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                color,
            ));
        });
}

/// Position crosshair at viewport center, show only when cursor is grabbed.
pub(super) fn update_screen_crosshair(
    windows: Query<(&Window, &bevy::window::CursorOptions), With<bevy::window::PrimaryWindow>>,
    cfg: Res<GameConfig>,
    ui_assets: Res<UiAssets>,
    mut query: Query<(&mut Node, &mut Visibility), With<ScreenCrosshair>>,
) {
    let Ok((window, cursor)) = windows.single() else { return };
    let (vp_left, vp_top, vp_w, vp_h) = crate::game::rendering::viewport::viewport_rect(window, &cfg, &ui_assets);
    let cx = vp_left + vp_w / 2.0;
    let cy = vp_top + vp_h / 2.0;
    let cursor_free = matches!(cursor.grab_mode, bevy::window::CursorGrabMode::None);

    let target_left = Val::Px(cx - 10.0);
    let target_top = Val::Px(cy - 10.0);
    let target_vis = if cfg.crosshair && !cursor_free {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for (mut node, mut vis) in query.iter_mut() {
        if node.left != target_left {
            node.left = target_left;
        }
        if node.top != target_top {
            node.top = target_top;
        }
        vis.set_if_neq(target_vis);
    }
}

// ── Music ───────────────────────────────────────────────────────────────────

pub(super) fn spawn_screen_music(
    commands: &mut Commands,
    audio_sources: &mut Assets<AudioSource>,
    sound: &super::Sound,
    screen_id: &str,
    cfg: &GameConfig,
    game_assets: &GameAssets,
) {
    let track = sound.id();
    let start_sec = sound.start_sec();
    let looping = sound.looping();
    let bytes = if let Some(b) = game_assets.music_bytes(track) {
        b
    } else {
        warn!("screen music: track '{}' not found", track);
        return;
    };

    let handle = audio_sources.add(AudioSource { bytes: bytes.into() });
    commands.spawn((
        AudioPlayer(handle),
        PlaybackSettings {
            mode: if looping {
                bevy::audio::PlaybackMode::Loop
            } else {
                bevy::audio::PlaybackMode::Despawn
            },
            volume: bevy::audio::Volume::Linear(cfg.music_volume),
            start_position: if start_sec > 0.0 {
                Some(std::time::Duration::from_secs_f32(start_sec))
            } else {
                None
            },
            ..default()
        },
        ScreenMusic(screen_id.to_string()),
        ScreenLayer(screen_id.to_string()),
    ));
    info!("screen music: playing track '{}'", track);
}
