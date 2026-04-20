//! Element spawning: images, text, crosshair, and music.

use bevy::prelude::*;

use super::bindings::{ArrowBinding, CompassBinding, CroppedImage, MinimapBinding, SkyScrollBinding, TapBinding};
use super::fonts::GameFonts;
use super::property_source::DynamicTexture;
use super::runtime::{
    FrameAnimation, HiddenByDefault, HoverOverlay, RuntimeElement, RuntimeText, ScreenCrosshair, ScreenLayer,
    ScreenMusic,
};
use super::video::spawn_video_element;
use super::{
    ImageElement, REF_H, REF_W, ScreenElement, TextElement, load_texture_with_transparency, resolve_image_size,
};
use crate::assets::GameAssets;
use crate::game::ui::UiState;
use crate::screens::ui_assets::UiAssets;
use crate::system::config::GameConfig;

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
            spawn_text_element(commands, txt, index, screen_id, layer_tag);
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

    // Crop mode: native texture dimensions for inner image, explicit size for clip container.
    // Must be after load_texture_with_transparency — that populates ui_assets.dimensions().
    let native_size = if img.crop && img.size.0 > 0.0 && img.size.1 > 0.0 {
        img.texture_for_state("default").and_then(|name| {
            let bare = name
                .strip_prefix("icons/")
                .unwrap_or_else(|| name.split('/').next_back().unwrap_or(name));
            ui_assets
                .dimensions(bare)
                .or_else(|| ui_assets.dimensions(name))
                .map(|(tw, th)| (tw as f32, th as f32))
        })
    } else {
        None
    };

    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(img.position.0 / REF_W * 100.0),
        top: Val::Percent(img.position.1 / REF_H * 100.0),
        width: Val::Percent(w / REF_W * 100.0),
        height: Val::Percent(h / REF_H * 100.0),
        overflow: if native_size.is_some() {
            Overflow::clip()
        } else {
            Overflow::visible()
        },
        ..default()
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

    let clicked_state = img.states.get("clicked");
    let clicked_handle = clicked_state.and_then(|state| {
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
    let clicked_anim_data = clicked_state.and_then(|state| {
        state.animation.as_ref().map(|anim| {
            let handles = load_animation_handles(anim, &img.transparent_color, ui_assets, game_assets, images, cfg);
            (handles, anim.fps)
        })
    });

    let hover_state = img.states.get("hover").or_else(|| img.states.get("hover_texture"));
    let hover_texture_handle = hover_state.and_then(|state| {
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
    let hover_anim_data = hover_state.and_then(|state| {
        state.animation.as_ref().map(|anim| {
            let handles = load_animation_handles(anim, &img.transparent_color, ui_assets, game_assets, images, cfg);
            (handles, anim.fps, anim.ping_pong)
        })
    });

    let binding = img.bindings.get("source").map(|s| s.as_str());
    let auto_sky_crop = binding == Some("sky_scroll") && img.crop_w <= 0.0 && img.crop_h <= 0.0;

    // Cropped image: spawn as clip container + scrollable inner image.
    // sky_scroll auto-enables this path using the authored image size as viewport.
    let has_crop = (img.crop_w > 0.0 && img.crop_h > 0.0) || auto_sky_crop;
    if has_crop {
        let crop_w = if img.crop_w > 0.0 {
            img.crop_w
        } else if img.size.0 > 0.0 {
            img.size.0
        } else {
            w
        };
        let crop_h = if img.crop_h > 0.0 {
            img.crop_h
        } else if img.size.1 > 0.0 {
            img.size.1
        } else {
            h
        };

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
                let tile_w_pct = if w > crop_w { w / crop_w * 100.0 } else { 100.0 };
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

                if binding == Some("sky_scroll") {
                    let mut inner = clip.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Percent(tile_w_pct * 2.0),
                            height: inner_h,
                            ..default()
                        },
                        CroppedImage { crop_w, crop_h },
                        SkyScrollBinding,
                    ));

                    if let Some(handle) = image_handle {
                        inner.with_children(|parent| {
                            let tile_node = Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(0.0),
                                width: Val::Percent(50.0),
                                height: Val::Percent(100.0),
                                ..default()
                            };
                            parent.spawn((
                                Node {
                                    left: Val::Percent(0.0),
                                    ..tile_node.clone()
                                },
                                ImageNode::new(handle.clone()),
                            ));
                            parent.spawn((
                                Node {
                                    left: Val::Percent(50.0),
                                    ..tile_node
                                },
                                ImageNode::new(handle),
                            ));
                        });
                    }
                } else {
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

                    if let Some(anim) = load_animation(img, ui_assets, game_assets, images, cfg) {
                        inner.insert(anim);
                    }
                }
            });
        return;
    }

    let has_interaction = hover_handle.is_some()
        || hover_texture_handle.is_some()
        || hover_anim_data.is_some()
        || clicked_handle.is_some()
        || !img.on_click.is_empty()
        || !img.on_hover.is_empty();
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
        // Crop mode: outer node clips, inner child holds texture at native size.
        if let Some((tw, th)) = native_size {
            let mut entity = commands.spawn((node, z, marker, layer_tag.clone(), initial_vis));
            if has_interaction {
                entity.insert((Button, BackgroundColor(Color::NONE)));
            }
            if img.hidden {
                entity.insert(HiddenByDefault);
            }
            entity.with_children(|parent| {
                let mut inner = parent.spawn((
                    ImageNode::new(handle.clone()),
                    Node {
                        width: Val::Percent(tw / w * 100.0),
                        height: Val::Percent(th / h * 100.0),
                        ..default()
                    },
                ));
                if let Some(clicked) = &clicked_handle {
                    inner.insert(super::runtime::ClickedTexture {
                        clicked: clicked.clone(),
                        default: Some(handle.clone()),
                    });
                }
                if let Some((handles, fps)) = clicked_anim_data {
                    inner.insert(super::runtime::ClickedAnimation {
                        handles,
                        fps,
                        default: Some(handle.clone()),
                        elapsed: 0.0,
                        current_frame: 0,
                    });
                }
                if let Some(hover) = &hover_texture_handle {
                    inner.insert(super::runtime::HoverTexture {
                        hover: hover.clone(),
                        default: Some(handle.clone()),
                    });
                }
                if let Some((handles, fps, ping_pong)) = hover_anim_data {
                    info!(
                        "attaching hover animation to child of '{}' (ping_pong={})",
                        img.id, ping_pong
                    );
                    inner.insert(super::runtime::HoverAnimation {
                        handles,
                        fps,
                        default: Some(handle.clone()),
                        elapsed: 0.0,
                        current_frame: 0,
                        ping_pong,
                    });
                }
                if let Some(anim) = load_animation(img, ui_assets, game_assets, images, cfg) {
                    inner.insert(anim);
                }
            });
        } else {
            let mut entity = commands.spawn((
                ImageNode::new(handle.clone()),
                node,
                z,
                marker,
                layer_tag.clone(),
                initial_vis,
            ));
            // Dynamic texture: template contains ${...} placeholders resolved each frame.
            if default_tex.contains("${") {
                entity.insert(DynamicTexture {
                    template: default_tex.clone(),
                    transparent_color: img.transparent_color.clone(),
                    last_resolved: String::new(),
                });
            }
            if has_interaction {
                entity.insert((Button, BackgroundColor(Color::NONE)));
            }
            if let Some(clicked) = clicked_handle {
                entity.insert(super::runtime::ClickedTexture {
                    clicked,
                    default: Some(handle.clone()),
                });
            }
            if let Some((handles, fps)) = clicked_anim_data {
                entity.insert(super::runtime::ClickedAnimation {
                    handles,
                    fps,
                    default: Some(handle.clone()),
                    elapsed: 0.0,
                    current_frame: 0,
                });
            }
            if let Some(hover) = hover_texture_handle {
                entity.insert(super::runtime::HoverTexture {
                    hover,
                    default: Some(handle.clone()),
                });
            }
            if let Some((handles, fps, ping_pong)) = hover_anim_data {
                entity.insert(super::runtime::HoverAnimation {
                    handles,
                    fps,
                    default: Some(handle.clone()),
                    elapsed: 0.0,
                    current_frame: 0,
                    ping_pong,
                });
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
            if let Some(anim) = load_animation(img, ui_assets, game_assets, images, cfg) {
                entity.insert(anim);
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
        }
    } else {
        let mut entity = commands.spawn((ImageNode::default(), node, z, marker, layer_tag.clone(), initial_vis));
        // Dynamic texture: template contains ${...} placeholders resolved each frame.
        if default_tex.contains("${") {
            entity.insert(DynamicTexture {
                template: default_tex.clone(),
                transparent_color: img.transparent_color.clone(),
                last_resolved: String::new(),
            });
        }
        if has_interaction {
            entity.insert((Button, BackgroundColor(Color::NONE)));
        }
        if let Some(clicked) = clicked_handle {
            entity.insert(super::runtime::ClickedTexture { clicked, default: None });
        }
        if let Some((handles, fps)) = clicked_anim_data {
            entity.insert(super::runtime::ClickedAnimation {
                handles,
                fps,
                default: None,
                elapsed: 0.0,
                current_frame: 0,
            });
        }
        if let Some(hover) = hover_texture_handle {
            entity.insert(super::runtime::HoverTexture { hover, default: None });
        }
        if let Some((handles, fps, ping_pong)) = hover_anim_data {
            entity.insert(super::runtime::HoverAnimation {
                handles,
                fps,
                default: None,
                elapsed: 0.0,
                current_frame: 0,
                ping_pong,
            });
        }
        if img.hidden {
            entity.insert(HiddenByDefault);
        }
        if let Some(anim) = load_animation(img, ui_assets, game_assets, images, cfg) {
            entity.insert(anim);
        }
    }
}

pub(super) fn load_animation(
    img: &super::ImageElement,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
) -> Option<FrameAnimation> {
    let anim = img.animation.as_ref()?;
    let handles = load_animation_handles(anim, &img.transparent_color, ui_assets, game_assets, images, cfg);
    if handles.is_empty() {
        return None;
    }
    Some(FrameAnimation {
        handles,
        fps: anim.fps,
        elapsed: 0.0,
        current_frame: 0,
    })
}

pub(super) fn load_animation_handles(
    anim: &super::Animation,
    transparent_color: &str,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
) -> Vec<Handle<Image>> {
    let mut handles = Vec::with_capacity(anim.frames as usize);
    for i in anim.start_frame..(anim.start_frame + anim.frames) {
        let name = format_animation_frame(&anim.pattern, i);
        match load_texture_with_transparency(&name, transparent_color, ui_assets, game_assets, images, cfg) {
            Some(handle) => handles.push(handle),
            None => {
                bevy::log::warn!("animation frame '{}' not found", name);
            }
        }
    }
    handles
}

/// Expand a printf-style pattern like `"icons/Watwalk%02d"` with a frame number.
/// Supports `%d`, `%0Nd` (zero-padded to N digits).
fn format_animation_frame(pattern: &str, frame: u32) -> String {
    // Find %d or %0Nd
    if let Some(pos) = pattern.find('%') {
        let rest = &pattern[pos + 1..];
        if rest.starts_with('d') {
            return format!("{}{}{}", &pattern[..pos], frame, &rest[1..]);
        }
        if rest.starts_with('c') {
            let ch = (b'a' + (frame.saturating_sub(1) % 26) as u8) as char;
            return format!("{}{}{}", &pattern[..pos], ch, &rest[1..]);
        }
        if rest.starts_with('C') {
            let ch = (b'A' + (frame.saturating_sub(1) % 26) as u8) as char;
            return format!("{}{}{}", &pattern[..pos], ch, &rest[1..]);
        }
        // %02d, %03d, etc.
        if let Some(d_pos) = rest.find('d') {
            let width_str = &rest[..d_pos];
            if let Ok(width) = width_str.trim_start_matches('0').parse::<usize>() {
                return format!("{}{:0>width$}{}", &pattern[..pos], frame, &rest[d_pos + 1..]);
            }
        }
    }
    // Fallback: append frame number
    format!("{}{}", pattern, frame)
}

// ── Text elements ───────────────────────────────────────────────────────────

pub(super) fn spawn_text_element(
    commands: &mut Commands,
    txt: &TextElement,
    index: usize,
    screen_id: &str,
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

    let marker = RuntimeElement {
        screen_id: screen_id.to_string(),
        index,
        element_id: txt.id.clone(),
    };

    let mut entity = commands.spawn((
        ImageNode::new(Handle::default()),
        node,
        ZIndex(txt.z),
        Visibility::Hidden,
        layer_tag.clone(),
        marker,
        RuntimeText {
            source: txt.source.clone(),
            value: txt.value.clone(),
            color_expr: txt.color.clone(),
            font: txt.font.clone(),
            font_size: txt.font_size,
            color: txt.color_rgba(),
            base_color: txt.color_rgba(),
            hover_color: txt.hover_rgba(),
            align: txt.align.clone(),
            bounds: (txt.position.0, txt.position.1, txt.size.0, txt.size.1),
            last_text: "\x00".to_string(), // sentinel — forces first update
            last_color: txt.color_rgba(),
        },
    ));

    if !txt.on_click.is_empty() || !txt.on_hover.is_empty() || txt.hover_color.is_some() {
        entity.insert((Button, Interaction::None, BackgroundColor(Color::NONE)));
    }
}

/// Read data sources, re-render text when content changes, reposition every frame.
pub(super) fn text_update(
    ui: Res<UiState>,
    world_state: Option<Res<crate::game::state::WorldState>>,
    npc_profile: Option<Res<crate::game::actors::npc_dialogue::NpcProfile>>,
    house_profile: Option<Res<crate::game::ui::HouseProfile>>,
    loading_step: Option<Res<crate::prepare::loading::LoadingStep>>,
    registry: Res<crate::screens::PropertyRegistry>,
    game_fonts: Res<GameFonts>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut query: Query<(
        &mut RuntimeText,
        &mut ImageNode,
        &mut Visibility,
        &mut Node,
        Option<&Interaction>,
    )>,
) {
    let Ok(window) = windows.single() else { return };
    let win_w = window.width();
    let win_h = window.height();
    let sx = win_w / REF_W;
    let sy = win_h / REF_H;

    for (mut rt, mut img_node, mut vis, mut node, interaction) in &mut query {
        let resolved_color = crate::screens::interpolate(&rt.color_expr, &registry);
        if !resolved_color.is_empty() {
            let rgba = crate::screens::TextElement::resolve_color(&resolved_color);
            rt.base_color = rgba;
            let is_hovered = matches!(interaction, Some(Interaction::Hovered) | Some(Interaction::Pressed));
            if !is_hovered && rt.color != rgba {
                rt.color = rgba;
            }
        }

        if rt.source == "ui.footer" {
            let footer_color = ui.footer.color();
            let rgba = crate::screens::TextElement::resolve_color(footer_color);
            rt.base_color = rgba;

            let is_hovered = matches!(interaction, Some(Interaction::Hovered) | Some(Interaction::Pressed));
            if !is_hovered && rt.color != rgba {
                rt.color = rgba;
            }
        }

        // "object.property" bindings — object name dispatches to the matching resource.
        // Core resources resolve inline (no clone needed).
        // Unknown objects fall through to the dynamic registry.
        let mut current = if let Some(dot) = rt.source.find('.') {
            let (src, path) = (&rt.source[..dot], &rt.source[dot + 1..]);
            match src {
                "player" => world_state
                    .as_ref()
                    .and_then(|ws| {
                        use crate::screens::PropertySource;
                        ws.resolve(path)
                    })
                    .unwrap_or_default(),
                "ui" => {
                    use crate::screens::PropertySource;
                    ui.resolve(path).unwrap_or_default()
                }
                "npc" => npc_profile
                    .as_ref()
                    .and_then(|p| {
                        use crate::screens::PropertySource;
                        p.resolve(path)
                    })
                    .unwrap_or_default(),
                "house" => house_profile
                    .as_ref()
                    .and_then(|p| {
                        use crate::screens::PropertySource;
                        p.resolve(path)
                    })
                    .unwrap_or_default(),
                "loading" => loading_step.as_ref().map_or(String::new(), |s| s.label().to_string()),
                // Unknown object → try dynamic registry
                _ => registry.resolve(&rt.source).unwrap_or_default(),
            }
        } else {
            // Unknown bare name: fall back to the element's static value.
            rt.value.clone()
        };

        if current.is_empty() && !rt.value.is_empty() {
            current = rt.value.clone();
        }

        current = crate::screens::interpolate(&current, &registry);

        // Re-render when text or color changed.
        if current != rt.last_text || rt.color != rt.last_color {
            rt.last_text = current.clone();
            rt.last_color = rt.color;

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
        let glyph_h_ref = if rt.font_size > 0.0 { rt.font_size } else { bh };
        let display_h = glyph_h_ref * sy;

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

/// Resolve `${...}` templates on image textures and swap the `ImageNode` handle.
/// Only entities tagged with [`DynamicTexture`] participate.
pub(super) fn dynamic_texture_update(
    registry: Res<crate::screens::PropertyRegistry>,
    game_assets: Res<GameAssets>,
    cfg: Res<GameConfig>,
    mut ui_assets: ResMut<UiAssets>,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<(&mut ImageNode, &mut DynamicTexture)>,
) {
    for (mut image_node, mut dyn_tex) in &mut query {
        let resolved = crate::screens::interpolate(&dyn_tex.template, &registry);
        if resolved.is_empty() || resolved == dyn_tex.last_resolved {
            continue;
        }
        if let Some(handle) = load_texture_with_transparency(
            &resolved,
            &dyn_tex.transparent_color,
            &mut ui_assets,
            &game_assets,
            &mut images,
            &cfg,
        ) {
            image_node.image = handle;
            dyn_tex.last_resolved = resolved;
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
