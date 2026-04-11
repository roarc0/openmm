//! Canvas rendering: element spawning, selection, drag-move, z-order, debug labels.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui, input::EguiWantsInput};

use super::io;
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::hud::UiAssets;
use crate::screens::{
    REF_H, REF_W, Screen, ScreenElement, TRANSPARENCY_OPTIONS,
    load_texture_with_transparency, resolve_image_size,
};

/// Resolve element size: delegates to `resolve_image_size` for images, falls back to explicit size for videos.
fn resolve_elem_size(elem: &ScreenElement, ui_assets: &UiAssets) -> (f32, f32) {
    if let Some(img) = elem.as_image() {
        resolve_image_size(img, ui_assets)
    } else {
        let (w, h) = elem.size();
        if w > 0.0 && h > 0.0 { (w, h) } else { (32.0, 32.0) }
    }
}

/// Generate a small checkerboard texture for the editor canvas background.
fn generate_checkerboard(
    images: &mut Assets<Image>,
    cell_size: u32,
    color_a: [u8; 4],
    color_b: [u8; 4],
) -> Handle<Image> {
    let size = cell_size * 2;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let color = if ((x / cell_size) + (y / cell_size)) % 2 == 0 {
                color_a
            } else {
                color_b
            };
            data[idx..idx + 4].copy_from_slice(&color);
        }
    }
    let mut img = Image::new(
        bevy::render::render_resource::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = bevy::image::ImageSampler::nearest();
    images.add(img)
}

/// Generate a crosshatch pattern texture for video placeholders.
fn generate_stripes(images: &mut Assets<Image>) -> Handle<Image> {
    let size = 128u32;
    let cell = 32u32;
    let bg: [u8; 4] = [40, 40, 45, 255];
    let line: [u8; 4] = [70, 70, 80, 255];
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let on_grid = x % cell == 0 || y % cell == 0;
            let color = if on_grid { line } else { bg };
            data[idx..idx + 4].copy_from_slice(&color);
        }
    }
    let mut img = Image::new(
        bevy::render::render_resource::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = bevy::image::ImageSampler::nearest();
    images.add(img)
}

/// Runtime state of the screen being edited.
#[derive(Resource)]
pub struct EditorScreen {
    pub screen: Screen,
    pub dirty: bool,
}

/// Marker component on each spawned element node.
#[derive(Component)]
pub struct CanvasElement {
    pub index: usize,
}

/// Marker component for the background image node.
#[derive(Component)]
pub struct CanvasBackground;

/// Per-element editor-only state (not saved to RON).
#[derive(Resource, Default)]
pub struct ElementEditorState {
    /// Hidden element indices (texture hidden, gizmo remains).
    pub hidden: std::collections::HashSet<usize>,
    /// Locked element IDs (by element id string, persisted across sessions).
    pub locked: std::collections::HashSet<String>,
}

/// Current selection state.
#[derive(Resource, Default)]
pub struct Selection {
    pub index: Option<usize>,
    pub drag_offset: Option<Vec2>,
    /// Whether the event editor window is open for the selected element.
    pub edt_open: bool,
    /// Whether the variant editor window is open for the selected element.
    pub var_open: bool,
    /// Which state to preview on canvas (None = default).
    pub preview_state: Option<String>,
}

// ─── Rebuild canvas ─────────────────────────────────────────────────────────

/// Rebuild canvas entities when structure changes (element count, background).
pub fn rebuild_canvas(
    mut commands: Commands,
    editor: Res<EditorScreen>,
    mut ui_assets: ResMut<UiAssets>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
    old_bg: Query<Entity, With<CanvasBackground>>,
    old_elems: Query<Entity, With<CanvasElement>>,
    mut last_fingerprint: Local<u64>,
) {
    // Fingerprint: element count + background + all transparency settings.
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    editor.screen.elements.len().hash(&mut hasher);
    for elem in &editor.screen.elements {
        match elem {
            ScreenElement::Image(img) => {
                img.transparent_color.hash(&mut hasher);
                img.texture_for_state("default").hash(&mut hasher);
            }
            ScreenElement::Video(vid) => {
                vid.video.hash(&mut hasher);
            }
            ScreenElement::Text(txt) => {
                txt.source.hash(&mut hasher);
                txt.font.hash(&mut hasher);
            }
        }
    }
    let fp = hasher.finish();
    if *last_fingerprint == fp {
        return;
    }
    *last_fingerprint = fp;

    for e in old_bg.iter().chain(old_elems.iter()) {
        commands.entity(e).despawn();
    }

    // Checkerboard background (always present behind everything).
    let checker_handle = generate_checkerboard(&mut images, 16, [40, 40, 40, 255], [50, 50, 50, 255]);
    let mut checker_img = ImageNode::new(checker_handle);
    checker_img.image_mode = bevy::ui::widget::NodeImageMode::Tiled {
        tile_x: true,
        tile_y: true,
        stretch_value: 1.0,
    };
    commands.spawn((
        Name::new("canvas_checker"),
        checker_img,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(0.0),
            top: Val::Percent(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        Pickable::IGNORE,
        ZIndex(-2),
        CanvasBackground,
    ));

    // Elements.
    for (i, elem) in editor.screen.elements.iter().enumerate() {
        spawn_element(&mut commands, &mut ui_assets, &game_assets, &mut images, &cfg, elem, i);
    }
}

fn spawn_element(
    commands: &mut Commands,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
    elem: &ScreenElement,
    index: usize,
) {
    let (w, h) = resolve_elem_size(elem, ui_assets);
    let pos = elem.position();

    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(pos.0 / REF_W * 100.0),
        top: Val::Percent(pos.1 / REF_H * 100.0),
        width: Val::Percent(w / REF_W * 100.0),
        height: Val::Percent(h / REF_H * 100.0),
        ..default()
    };

    let label = Name::new(format!("canvas_elem_{}", elem.id()));
    let marker = CanvasElement { index };
    let z = ZIndex(elem.z());

    if elem.as_text().is_some() {
        // Text placeholder: semi-transparent dark background.
        commands.spawn((
            label,
            BackgroundColor(Color::srgba(0.1, 0.1, 0.3, 0.6)),
            node,
            z,
            marker,
            Pickable::IGNORE,
        ));
    } else if elem.as_video().is_some() {
        // Video placeholder: horizontal black/white stripes.
        let stripe_handle = generate_stripes(images);
        let mut stripe_img = ImageNode::new(stripe_handle);
        stripe_img.image_mode = bevy::ui::widget::NodeImageMode::Tiled {
            tile_x: true,
            tile_y: true,
            stretch_value: 1.0,
        };
        commands.spawn((label, stripe_img, node, z, marker, Pickable::IGNORE));
    } else if let Some(img) = elem.as_image() {
        let tex_name = img.texture_for_state("default").unwrap_or("").to_string();
        let maybe_handle = if !tex_name.is_empty() {
            load_texture_with_transparency(&tex_name, &img.transparent_color, ui_assets, game_assets, images, cfg)
        } else {
            None
        };
        if let Some(handle) = maybe_handle {
            commands.spawn((label, ImageNode::new(handle), node, z, marker, Pickable::IGNORE));
        } else if img.bindings.get("source").is_some() {
            // Bound element (minimap, etc.) — texture loaded at runtime, show transparent placeholder.
            commands.spawn((
                label,
                BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 0.4)),
                node,
                z,
                marker,
                Pickable::IGNORE,
            ));
        } else {
            warn!("editor: failed to load texture '{}' (transparent_color='{}') — showing magenta", tex_name, img.transparent_color);
            commands.spawn((
                label,
                BackgroundColor(Color::srgba(1.0, 0.0, 1.0, 0.8)),
                node,
                z,
                marker,
                Pickable::IGNORE,
            ));
        }
    }
}

// ─── Debug overlays (egui painter) ──────────────────────────────────────────

/// Draw selection borders and labels via egui painter. Runs in EguiPrimaryContextPass.
/// Pending action from overlay buttons, applied next frame.
#[derive(Resource, Default)]
pub struct OverlayAction {
    pub action: Option<OverlayCmd>,
}

pub enum OverlayCmd {
    BringToTop(usize),
    SendToBottom(usize),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
    ToggleVisibility(usize),
    ToggleLock(usize),
}

/// Draw selection borders, labels, and z-order toolbar via egui. Runs in EguiPrimaryContextPass.
pub fn draw_overlays(
    mut contexts: EguiContexts,
    mut editor: ResMut<EditorScreen>,
    mut selection: ResMut<Selection>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_assets: Res<UiAssets>,
    mut overlay_action: ResMut<OverlayAction>,
    editor_state: Res<ElementEditorState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let Ok(window) = windows.single() else { return };
    let win_w = window.width();
    let win_h = window.height();

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("canvas_overlays"),
    ));

    for (i, elem) in editor.screen.elements.iter().enumerate() {
        let (w, h) = resolve_elem_size(elem, &ui_assets);
        let pos = elem.position();

        let sx = pos.0 / REF_W * win_w;
        let sy = pos.1 / REF_H * win_h;
        let sw = w / REF_W * win_w;
        let sh = h / REF_H * win_h;

        let rect = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(sw, sh));

        let is_selected = selection.index == Some(i);
        let is_video = elem.as_video().is_some();
        let is_text = elem.as_text().is_some();
        let (stroke, text_color) = if is_selected {
            (
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 0, 255)),
                egui::Color32::from_rgb(255, 0, 255),
            )
        } else if is_video {
            (
                egui::Stroke::new(1.5, egui::Color32::from_rgb(255, 60, 60)),
                egui::Color32::from_rgb(255, 60, 60),
            )
        } else if is_text {
            (
                egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 180, 255)),
                egui::Color32::from_rgb(100, 180, 255),
            )
        } else {
            (
                egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 255, 0)),
                egui::Color32::from_rgb(0, 255, 0),
            )
        };

        painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);

        let is_hidden = editor_state.hidden.contains(&i);

        // Label: id[w,h]@(x,y) z=N [TYPE] [H]
        let mut flags = String::new();
        if is_video {
            flags.push_str(" [VID]");
        }
        if is_text {
            flags.push_str(" [TXT]");
        }
        if is_hidden {
            flags.push_str(" [H]");
        }
        let label = format!(
            "{}[{},{}]@({},{}) z={}{}",
            elem.id(), w as i32, h as i32, pos.0 as i32, pos.1 as i32, elem.z(), flags,
        );
        painter.text(
            rect.left_top() + egui::vec2(3.0, 2.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(13.0),
            text_color,
        );

        // Status stamps at bottom-right inside the element.
        let is_locked = editor_state.locked.contains(elem.id());
        let evt_count = elem.on_click().len() + elem.on_hover().len()
            + elem.as_image().map_or(0, |img| img.bindings.len());
        let mut stamps: Vec<String> = Vec::new();
        if is_locked {
            stamps.push("LCK".into());
        }
        if evt_count > 0 {
            stamps.push(format!("EVT({})", evt_count));
        }
        if let Some(img) = elem.as_image() {
            if img.states.len() > 1 {
                stamps.push(format!("VAR({})", img.states.len()));
            }
        }
        if !stamps.is_empty() {
            let text = stamps.join(" ");
            // Draw background pill for readability.
            let font = egui::FontId::monospace(13.0);
            let galley = painter.layout_no_wrap(text.clone(), font.clone(), egui::Color32::WHITE);
            let text_size = galley.size();
            let pad = egui::vec2(4.0, 2.0);
            let text_pos = rect.right_bottom() - egui::vec2(text_size.x + pad.x * 2.0, text_size.y + pad.y * 2.0)
                + egui::vec2(-2.0, -2.0);
            let bg_rect = egui::Rect::from_min_size(text_pos, text_size + pad * 2.0);
            painter.rect_filled(bg_rect, 3.0, egui::Color32::from_rgba_unmultiplied(120, 0, 200, 180));
            painter.text(
                bg_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font,
                egui::Color32::WHITE,
            );
        }
    }

    // Toolbar buttons for selected element — positioned below the element.
    if let Some(sel) = selection.index {
        if let Some(elem) = editor.screen.elements.get(sel) {
            let (w, h) = resolve_elem_size(elem, &ui_assets);
            let pos = elem.position();
            let sx = pos.0 / REF_W * win_w;
            let sy = pos.1 / REF_H * win_h;
            let sh = h / REF_H * win_h;
            let _sw = w / REF_W * win_w;

            // Place toolbar inside the element, at the bottom.
            let toolbar_y = (sy + sh - 20.0).max(sy);
            let toolbar_x = sx + 2.0;

            egui::Area::new(egui::Id::new("elem_toolbar"))
                .fixed_pos(egui::pos2(toolbar_x, toolbar_y))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.style_mut().spacing.item_spacing = egui::vec2(2.0, 0.0);
                        let btn = |ui: &mut egui::Ui, text: &str| -> bool { ui.small_button(text).clicked() };
                        if btn(ui, "Top") {
                            overlay_action.action = Some(OverlayCmd::BringToTop(sel));
                        }
                        if btn(ui, "Up") {
                            overlay_action.action = Some(OverlayCmd::MoveUp(sel));
                        }
                        if btn(ui, "Dn") {
                            overlay_action.action = Some(OverlayCmd::MoveDown(sel));
                        }
                        if btn(ui, "Bot") {
                            overlay_action.action = Some(OverlayCmd::SendToBottom(sel));
                        }
                        let vis_label = if editor_state.hidden.contains(&sel) {
                            "Show"
                        } else {
                            "Vis"
                        };
                        if btn(ui, vis_label) {
                            overlay_action.action = Some(OverlayCmd::ToggleVisibility(sel));
                        }
                        let is_locked = editor_state.locked.contains(elem.id());
                        if btn(ui, if is_locked { "Unlk" } else { "Lock" }) {
                            overlay_action.action = Some(OverlayCmd::ToggleLock(sel));
                        }
                        if btn(ui, "Edt") {
                            selection.edt_open = !selection.edt_open;
                        }
                        let var_count = elem.as_image().map_or(0, |img| img.states.len());
                        let var_label = if var_count > 1 {
                            format!("Var({})", var_count)
                        } else {
                            "Var".to_string()
                        };
                        if btn(ui, &var_label) {
                            selection.var_open = !selection.var_open;
                        }
                        if btn(ui, "X") {
                            overlay_action.action = Some(OverlayCmd::Remove(sel));
                        }
                        ui.weak(format!("z={}", elem.z()));
                    });
                });
        }
    }

    // Event editor window — one at a time, for the selected element.
    if selection.edt_open {
        if let Some(sel) = selection.index {
            if sel < editor.screen.elements.len() {
                let elem_id = editor.screen.elements[sel].id().to_string();
                let mut open = true;
                let evt_id = egui::Id::new("edt_editor");
                let mut win = egui::Window::new("Edit")
                    .id(evt_id)
                    .resizable(true)
                    .collapsible(false)
                    .default_width(320.0)
                    .open(&mut open);
                // Restore position from config on first open.
                let cfg = super::io::EditorConfig::load();
                if let Some([x, y]) = cfg.edt_pos {
                    win = win.default_pos(egui::pos2(x, y));
                }
                win.show(ctx, |ui| {
                    ui.strong(&elem_id);
                    ui.separator();

                    // Video-specific properties.
                    if let Some(vid) = editor.screen.elements[sel].as_video() {
                        let mut video_name = vid.video.clone();
                        ui.horizontal(|ui| {
                            ui.label("Video:");
                            if ui.text_edit_singleline(&mut video_name).changed() {
                                editor.screen.elements[sel].as_video_mut().unwrap().video = video_name;
                                editor.dirty = true;
                            }
                        });

                        let mut vid_size = editor.screen.elements[sel].size();
                        let mut size_changed = false;
                        ui.horizontal(|ui| {
                            ui.label("Size:");
                            size_changed |= ui.add(egui::DragValue::new(&mut vid_size.0).prefix("w: ").speed(1.0)).changed();
                            size_changed |= ui.add(egui::DragValue::new(&mut vid_size.1).prefix("h: ").speed(1.0)).changed();
                        });
                        if size_changed {
                            editor.screen.elements[sel].set_size(vid_size);
                            editor.dirty = true;
                        }

                        let mut looping = editor.screen.elements[sel].as_video().unwrap().looping;
                        let mut skippable = editor.screen.elements[sel].as_video().unwrap().skippable;
                        let mut hidden = editor.screen.elements[sel].hidden();
                        let mut flags_changed = false;
                        ui.horizontal(|ui| {
                            flags_changed |= ui.checkbox(&mut looping, "Loop").changed();
                            flags_changed |= ui.checkbox(&mut skippable, "Skip").changed();
                            flags_changed |= ui.checkbox(&mut hidden, "Hidden").changed();
                        });
                        if flags_changed {
                            let v = editor.screen.elements[sel].as_video_mut().unwrap();
                            v.looping = looping;
                            v.skippable = skippable;
                            v.hidden = hidden;
                            editor.dirty = true;
                        }

                        ui.separator();
                    }

                    // Image-specific: transparency, bindings.
                    let is_image = editor.screen.elements[sel].as_image().is_some();

                    if is_image {
                        // Transparency color key.
                        let current_tc = editor.screen.elements[sel]
                            .as_image().unwrap().transparent_color.clone();
                        let tc_label = if current_tc.is_empty() { "none" } else { &current_tc };
                        ui.horizontal(|ui| {
                            ui.label("Transparent:");
                            egui::ComboBox::from_id_salt("edt_tc")
                                .selected_text(tc_label)
                                .show_ui(ui, |ui| {
                                    for &opt in TRANSPARENCY_OPTIONS {
                                        let label = if opt.is_empty() { "none" } else { opt };
                                        if ui.selectable_label(current_tc == opt, label).clicked() {
                                            editor.screen.elements[sel]
                                                .as_image_mut().unwrap().transparent_color = opt.to_string();
                                            editor.dirty = true;
                                        }
                                    }
                                });
                        });

                        ui.separator();
                    }

                    // on_click / on_hover / bindings (image-only)
                    if let Some(img_ref) = editor.screen.elements[sel].as_image() {
                        let click_count = img_ref.on_click.len();
                        let hover_count = img_ref.on_hover.len();

                        ui.heading("on_click");
                        let mut click_remove: Option<usize> = None;
                        for i in 0..click_count {
                            let mut action = editor.screen.elements[sel].as_image().unwrap().on_click[i].clone();
                            ui.horizontal(|ui| {
                                ui.label(format!("{}:", i));
                                if ui.text_edit_singleline(&mut action).changed() {
                                    editor.screen.elements[sel].as_image_mut().unwrap().on_click[i] = action;
                                    editor.dirty = true;
                                }
                                if ui.small_button("\u{2715}").clicked() {
                                    click_remove = Some(i);
                                }
                            });
                        }
                        if let Some(i) = click_remove {
                            editor.screen.elements[sel].as_image_mut().unwrap().on_click.remove(i);
                            editor.dirty = true;
                        }
                        if ui.small_button("+ Add on_click").clicked() {
                            editor.screen.elements[sel].as_image_mut().unwrap().on_click.push(String::new());
                            editor.dirty = true;
                        }

                        ui.separator();

                        ui.heading("on_hover");
                        let mut hover_remove: Option<usize> = None;
                        for i in 0..hover_count {
                            let mut action = editor.screen.elements[sel].as_image().unwrap().on_hover[i].clone();
                            ui.horizontal(|ui| {
                                ui.label(format!("{}:", i));
                                if ui.text_edit_singleline(&mut action).changed() {
                                    editor.screen.elements[sel].as_image_mut().unwrap().on_hover[i] = action;
                                    editor.dirty = true;
                                }
                                if ui.small_button("\u{2715}").clicked() {
                                    hover_remove = Some(i);
                                }
                            });
                        }
                        if let Some(i) = hover_remove {
                            editor.screen.elements[sel].as_image_mut().unwrap().on_hover.remove(i);
                            editor.dirty = true;
                        }
                        if ui.small_button("+ Add on_hover").clicked() {
                            editor.screen.elements[sel].as_image_mut().unwrap().on_hover.push(String::new());
                            editor.dirty = true;
                        }
                    }

                    // on_end (video-only)
                    if let Some(vid_ref) = editor.screen.elements[sel].as_video() {
                        ui.heading("on_end");
                        let end_count = vid_ref.on_end.len();
                        let mut end_remove: Option<usize> = None;
                        for i in 0..end_count {
                            let mut action = editor.screen.elements[sel].as_video().unwrap().on_end[i].clone();
                            ui.horizontal(|ui| {
                                ui.label(format!("{}:", i));
                                if ui.text_edit_singleline(&mut action).changed() {
                                    editor.screen.elements[sel].as_video_mut().unwrap().on_end[i] = action;
                                    editor.dirty = true;
                                }
                                if ui.small_button("\u{2715}").clicked() {
                                    end_remove = Some(i);
                                }
                            });
                        }
                        if let Some(i) = end_remove {
                            editor.screen.elements[sel].as_video_mut().unwrap().on_end.remove(i);
                            editor.dirty = true;
                        }
                        if ui.small_button("+ Add on_end").clicked() {
                            editor.screen.elements[sel].as_video_mut().unwrap().on_end.push(String::new());
                            editor.dirty = true;
                        }
                    }

                    // bindings (image-only)
                    if is_image {
                        ui.separator();
                        ui.heading("bindings");
                        ui.small("property \u{2192} variable (e.g. scroll_x \u{2192} player.compass_yaw)");
                        let bind_keys: Vec<String> = editor.screen.elements[sel]
                            .as_image().unwrap().bindings.keys().cloned().collect();
                        let mut bind_remove: Option<String> = None;
                        for key in &bind_keys {
                            let mut val = editor.screen.elements[sel]
                                .as_image().unwrap()
                                .bindings
                                .get(key)
                                .cloned()
                                .unwrap_or_default();
                            ui.horizontal(|ui| {
                                ui.label(format!("{}:", key));
                                if ui.text_edit_singleline(&mut val).changed() {
                                    editor.screen.elements[sel]
                                        .as_image_mut().unwrap()
                                        .bindings.insert(key.clone(), val);
                                    editor.dirty = true;
                                }
                                if ui.small_button("\u{2715}").clicked() {
                                    bind_remove = Some(key.clone());
                                }
                            });
                        }
                        if let Some(k) = bind_remove {
                            editor.screen.elements[sel]
                                .as_image_mut().unwrap().bindings.remove(&k);
                            editor.dirty = true;
                        }
                        let mut add_binding: Option<&str> = None;
                        ui.horizontal(|ui| {
                            if ui.small_button("+ texture").clicked() {
                                add_binding = Some("texture");
                            }
                            if ui.small_button("+ text").clicked() {
                                add_binding = Some("text");
                            }
                            if ui.small_button("+ scroll_x").clicked() {
                                add_binding = Some("scroll_x");
                            }
                            if ui.small_button("+ scroll_y").clicked() {
                                add_binding = Some("scroll_y");
                            }
                            if ui.small_button("+ visible").clicked() {
                                add_binding = Some("visible");
                            }
                        });
                        if let Some(key) = add_binding {
                            editor.screen.elements[sel]
                                .as_image_mut().unwrap()
                                .bindings.entry(key.to_string()).or_default();
                            editor.dirty = true;
                        }
                    }
                });
                if !open {
                    selection.edt_open = false;
                }
                // Save position on drag.
                if let Some(rect) = ctx.memory(|m: &egui::Memory| m.area_rect(evt_id)) {
                    let pos = rect.left_top();
                    let mut cfg = super::io::EditorConfig::load();
                    let new_pos = [pos.x, pos.y];
                    if cfg.edt_pos != Some(new_pos) {
                        cfg.edt_pos = Some(new_pos);
                        cfg.save();
                    }
                }
            }
        }
    }
    // Variant editor window.
    if selection.var_open {
        if let Some(sel) = selection.index {
            if sel < editor.screen.elements.len() && editor.screen.elements[sel].as_image().is_some() {
                let _elem_id = editor.screen.elements[sel].id().to_string();
                let mut open = true;
                egui::Window::new("Variants")
                    .id(egui::Id::new("var_editor"))
                    .resizable(true)
                    .collapsible(false)
                    .default_width(350.0)
                    .open(&mut open)
                    .show(ctx, |ui| {
                        let state_keys: Vec<String> = editor.screen.elements[sel]
                            .as_image().unwrap().states.keys().cloned().collect();
                        let mut to_remove: Option<String> = None;

                        for key in &state_keys {
                            let is_previewing = selection.preview_state.as_deref() == Some(key.as_str());
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    let prefix = if is_previewing { ">> " } else { "" };
                                    if ui.button(format!("{}[{}]", prefix, key)).clicked() {
                                        selection.preview_state = if is_previewing { None } else { Some(key.clone()) };
                                    }
                                    if key != "default" && ui.small_button("\u{2715}").clicked() {
                                        to_remove = Some(key.clone());
                                    }
                                });

                                let img = editor.screen.elements[sel].as_image().unwrap();
                                let mut tex = img.states
                                    .get(key)
                                    .map(|s| s.texture.clone())
                                    .unwrap_or_default();
                                ui.horizontal(|ui| {
                                    ui.label("texture:");
                                    if ui.text_edit_singleline(&mut tex).changed() {
                                        if let Some(state) = editor.screen.elements[sel]
                                            .as_image_mut().unwrap().states.get_mut(key)
                                        {
                                            state.texture = tex;
                                            editor.dirty = true;
                                        }
                                    }
                                });

                                let img = editor.screen.elements[sel].as_image().unwrap();
                                let mut cond = img.states
                                    .get(key)
                                    .map(|s| s.condition.clone())
                                    .unwrap_or_default();
                                ui.horizontal(|ui| {
                                    ui.label("condition:");
                                    if ui.text_edit_singleline(&mut cond).changed() {
                                        if let Some(state) = editor.screen.elements[sel]
                                            .as_image_mut().unwrap().states.get_mut(key)
                                        {
                                            state.condition = cond;
                                            editor.dirty = true;
                                        }
                                    }
                                });
                            });
                        }

                        if let Some(k) = to_remove {
                            editor.screen.elements[sel]
                                .as_image_mut().unwrap().states.remove(&k);
                            if selection.preview_state.as_deref() == Some(&k) {
                                selection.preview_state = None;
                            }
                            editor.dirty = true;
                        }

                        if ui.button("+ Add variant").clicked() {
                            let img = editor.screen.elements[sel].as_image_mut().unwrap();
                            let name = format!("state_{}", img.states.len());
                            img.states.insert(
                                name,
                                super::format::ElementState {
                                    texture: String::new(),
                                    condition: String::new(),
                                },
                            );
                            editor.dirty = true;
                        }
                    });
                if !open {
                    selection.var_open = false;
                    selection.preview_state = None;
                }
            }
        }
    }
}

/// Apply pending overlay actions (runs in Update, after egui pass).
pub fn apply_overlay_actions(
    mut action: ResMut<OverlayAction>,
    mut editor: ResMut<EditorScreen>,
    mut selection: ResMut<Selection>,
    mut elem_q: Query<(&CanvasElement, &mut ZIndex)>,
    mut editor_state: ResMut<ElementEditorState>,
) {
    let Some(cmd) = action.action.take() else { return };
    match cmd {
        OverlayCmd::Remove(idx) => {
            if idx < editor.screen.elements.len() {
                editor.screen.elements.remove(idx);
                editor.dirty = true;
                selection.index = None;
                editor_state.hidden.remove(&idx);
            }
        }
        OverlayCmd::ToggleVisibility(idx) => {
            if !editor_state.hidden.remove(&idx) {
                editor_state.hidden.insert(idx);
            }
        }
        OverlayCmd::ToggleLock(idx) => {
            if let Some(elem) = editor.screen.elements.get(idx) {
                let id = elem.id().to_string();
                if !editor_state.locked.remove(&id) {
                    editor_state.locked.insert(id);
                }
                // Persist lock state.
                super::io::save_locks(&mut editor.screen, &editor_state.locked);
            }
        }
        OverlayCmd::BringToTop(idx) => {
            let new_z = editor.screen.elements.iter().map(|e| e.z()).max().unwrap_or(0) + 1;
            set_z(&mut editor, idx, new_z, &mut elem_q);
        }
        OverlayCmd::SendToBottom(idx) => {
            let new_z = editor.screen.elements.iter().map(|e| e.z()).min().unwrap_or(0) - 1;
            set_z(&mut editor, idx, new_z, &mut elem_q);
        }
        OverlayCmd::MoveUp(idx) => {
            let new_z = editor.screen.elements.get(idx).map(|e| e.z() + 1).unwrap_or(0);
            set_z(&mut editor, idx, new_z, &mut elem_q);
        }
        OverlayCmd::MoveDown(idx) => {
            let new_z = editor.screen.elements.get(idx).map(|e| e.z() - 1).unwrap_or(0);
            set_z(&mut editor, idx, new_z, &mut elem_q);
        }
    }
}

fn set_z(editor: &mut EditorScreen, idx: usize, new_z: i32, elem_q: &mut Query<(&CanvasElement, &mut ZIndex)>) {
    let clamped = new_z.max(0);
    if let Some(elem) = editor.screen.elements.get_mut(idx) {
        elem.set_z(clamped);
    }
    editor.dirty = true;
    for (ce, mut z) in elem_q.iter_mut() {
        if ce.index == idx {
            *z = ZIndex(new_z);
        }
    }
}

// ─── Selection ──────────────────────────────────────────────────────────────

/// On left click, selects the topmost element under the cursor.
pub fn selection_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    editor: Res<EditorScreen>,
    mut selection: ResMut<Selection>,
    ui_assets: Res<UiAssets>,
    egui_input: Option<Res<EguiWantsInput>>,
) {
    // Don't change selection when clicking on egui windows (browser, evt editor, etc.)
    if egui_input.is_some_and(|e| e.wants_pointer_input()) {
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let win_w = window.width();
    let win_h = window.height();

    // Convert cursor to reference coords.
    let cx = cursor.x / win_w * REF_W;
    let cy = cursor.y / win_h * REF_H;

    // Hit test against elements (last = topmost due to z-order).
    let mut best: Option<(usize, i32)> = None;
    for (i, elem) in editor.screen.elements.iter().enumerate() {
        let (w, h) = resolve_elem_size(elem, &ui_assets);
        let (ex, ey) = elem.position();
        if cx >= ex && cx <= ex + w && cy >= ey && cy <= ey + h {
            if best.map_or(true, |(_, bz)| elem.z() > bz) {
                best = Some((i, elem.z()));
            }
        }
    }

    let new_sel = best.map(|(idx, _)| idx);
    if new_sel != selection.index {
        selection.edt_open = false;
        selection.var_open = false;
        selection.preview_state = None;
    }
    selection.index = new_sel;
    selection.drag_offset = None;
}

// ─── Drag ───────────────────────────────────────────────────────────────────

/// Drag selected element with mouse.
pub fn drag_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut selection: ResMut<Selection>,
    mut editor: ResMut<EditorScreen>,
    ui_assets: Res<UiAssets>,
    editor_state: Res<ElementEditorState>,
    egui_input: Option<Res<EguiWantsInput>>,
) {
    // Don't drag canvas elements while interacting with egui windows.
    if egui_input.is_some_and(|e| e.wants_pointer_input()) {
        selection.drag_offset = None;
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let win_w = window.width();
    let win_h = window.height();

    let cursor_ref = Vec2::new(cursor.x / win_w * REF_W, cursor.y / win_h * REF_H);
    let Some(sel_idx) = selection.index else { return };
    // Locked elements can't be dragged.
    if editor
        .screen
        .elements
        .get(sel_idx)
        .is_some_and(|e| editor_state.locked.contains(e.id()))
    {
        return;
    }

    if mouse.just_pressed(MouseButton::Left) {
        if let Some(elem) = editor.screen.elements.get(sel_idx) {
            let pos = elem.position();
            let elem_pos = Vec2::new(pos.0, pos.1);
            selection.drag_offset = Some(cursor_ref - elem_pos);
        }
    }

    if mouse.pressed(MouseButton::Left) {
        if let Some(offset) = selection.drag_offset {
            let new_pos = cursor_ref - offset;
            if let Some(elem) = editor.screen.elements.get_mut(sel_idx) {
                let (w, h) = resolve_elem_size(elem, &ui_assets);
                let x = new_pos.x.round().clamp(0.0, (REF_W - w).max(0.0));
                let y = new_pos.y.round().clamp(0.0, (REF_H - h).max(0.0));
                elem.set_position((x, y));
                editor.dirty = true;
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        selection.drag_offset = None;
    }
}

// ─── Sync positions ────────────────────────────────────────────────────────

/// Syncs Bevy `Node` positions from `EditorScreen` data every frame.
pub fn sync_element_positions(
    editor: Res<EditorScreen>,
    editor_state: Res<ElementEditorState>,
    selection: Res<Selection>,
    mut ui_assets: ResMut<UiAssets>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
    mut elem_q: Query<(&CanvasElement, &mut Node, &mut Visibility, Option<&mut ImageNode>)>,
) {
    for (ce, mut node, mut vis, image_node) in &mut elem_q {
        let Some(elem) = editor.screen.elements.get(ce.index) else {
            continue;
        };
        let pos = elem.position();
        let (w, h) = resolve_elem_size(elem, &ui_assets);
        node.left = Val::Percent(pos.0 / REF_W * 100.0);
        node.top = Val::Percent(pos.1 / REF_H * 100.0);
        node.width = Val::Percent(w / REF_W * 100.0);
        node.height = Val::Percent(h / REF_H * 100.0);
        *vis = if editor_state.hidden.contains(&ce.index) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };

        // Swap texture for preview state (only on selected image elements).
        if selection.index == Some(ce.index) {
            if let Some(ref state_name) = selection.preview_state {
                if let Some(img_elem) = elem.as_image() {
                    if let Some(mut img) = image_node {
                        let tex_name = img_elem.states.get(state_name).map(|s| s.texture.as_str()).unwrap_or("");
                        if let Some(handle) = load_texture_with_transparency(
                            tex_name,
                            &img_elem.transparent_color,
                            &mut ui_assets,
                            &game_assets,
                            &mut images,
                            &cfg,
                        ) {
                            img.image = handle;
                        }
                    }
                }
            }
        }
    }
}

// ─── Z-order ───────────────────────────────────────────────────────────────

/// Scroll wheel on selected element increments/decrements z.
pub fn z_order_system(
    scroll: Res<AccumulatedMouseScroll>,
    selection: Res<Selection>,
    mut editor: ResMut<EditorScreen>,
    mut elem_q: Query<(&CanvasElement, &mut ZIndex)>,
) {
    let Some(sel_idx) = selection.index else { return };

    let delta = scroll.delta.y;
    if delta == 0.0 {
        return;
    }
    let delta = if delta > 0.0 { 1i32 } else { -1 };

    let new_z = if let Some(elem) = editor.screen.elements.get_mut(sel_idx) {
        let z = (elem.z() + delta).max(0);
        elem.set_z(z);
        Some(z)
    } else {
        None
    };
    if let Some(z_val) = new_z {
        editor.dirty = true;
        for (ce, mut z) in &mut elem_q {
            if ce.index == sel_idx {
                *z = ZIndex(z_val);
            }
        }
    }
}

// ─── Arrow nudge ──────────────────────────────────────────────────────────

/// Arrow keys move the selected element by 1 reference pixel.
pub fn arrow_nudge_system(
    keys: Res<ButtonInput<KeyCode>>,
    selection: Res<Selection>,
    mut editor: ResMut<EditorScreen>,
    ui_assets: Res<UiAssets>,
    editor_state: Res<ElementEditorState>,
) {
    let Some(sel_idx) = selection.index else { return };
    if editor
        .screen
        .elements
        .get(sel_idx)
        .is_some_and(|e| editor_state.locked.contains(e.id()))
    {
        return;
    }
    let mut dx: f32 = 0.0;
    let mut dy: f32 = 0.0;
    if keys.just_pressed(KeyCode::ArrowLeft) {
        dx -= 1.0;
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        dx += 1.0;
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        dy -= 1.0;
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        dy += 1.0;
    }
    if dx == 0.0 && dy == 0.0 {
        return;
    }

    if let Some(elem) = editor.screen.elements.get_mut(sel_idx) {
        let (w, h) = resolve_elem_size(elem, &ui_assets);
        let pos = elem.position();
        let x = (pos.0 + dx).clamp(0.0, (REF_W - w).max(0.0));
        let y = (pos.1 + dy).clamp(0.0, (REF_H - h).max(0.0));
        elem.set_position((x, y));
        editor.dirty = true;
    }
}

// ─── Delete ────────────────────────────────────────────────────────────────

/// Delete/Backspace removes the selected element.
pub fn delete_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut editor: ResMut<EditorScreen>,
    editor_state: Res<ElementEditorState>,
) {
    if !keys.just_pressed(KeyCode::Delete) && !keys.just_pressed(KeyCode::Backspace) {
        return;
    }
    let Some(idx) = selection.index else { return };
    if editor
        .screen
        .elements
        .get(idx)
        .is_some_and(|e| editor_state.locked.contains(e.id()))
    {
        return;
    }
    if idx < editor.screen.elements.len() {
        editor.screen.elements.remove(idx);
        editor.dirty = true;
        selection.index = None;
    }
}

// ─── Keyboard shortcuts for z-order and visibility ────────────────────

/// T=top, U=up, D=down, B=bottom, V=toggle visibility on selected element.
pub fn z_shortcut_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut overlay_action: ResMut<OverlayAction>,
) {
    let Some(sel) = selection.index else { return };
    if keys.just_pressed(KeyCode::KeyE) {
        selection.edt_open = !selection.edt_open;
        return;
    }
    if keys.just_pressed(KeyCode::KeyW) {
        selection.var_open = !selection.var_open;
        if !selection.var_open {
            selection.preview_state = None;
        }
        return;
    }
    if overlay_action.action.is_some() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        overlay_action.action = Some(OverlayCmd::BringToTop(sel));
    } else if keys.just_pressed(KeyCode::KeyU) {
        overlay_action.action = Some(OverlayCmd::MoveUp(sel));
    } else if keys.just_pressed(KeyCode::KeyD) {
        overlay_action.action = Some(OverlayCmd::MoveDown(sel));
    } else if keys.just_pressed(KeyCode::KeyB) {
        overlay_action.action = Some(OverlayCmd::SendToBottom(sel));
    } else if keys.just_pressed(KeyCode::KeyV) {
        overlay_action.action = Some(OverlayCmd::ToggleVisibility(sel));
    } else if keys.just_pressed(KeyCode::KeyL) {
        overlay_action.action = Some(OverlayCmd::ToggleLock(sel));
    }
}

// ─── Tab cycle ─────────────────────────────────────────────────────────

/// Tab cycles forward through elements, Shift+Tab cycles backward.
pub fn tab_cycle_system(keys: Res<ButtonInput<KeyCode>>, editor: Res<EditorScreen>, mut selection: ResMut<Selection>) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let count = editor.screen.elements.len();
    if count == 0 {
        return;
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    selection.index = Some(match selection.index {
        Some(i) if shift => {
            if i == 0 {
                count - 1
            } else {
                i - 1
            }
        }
        Some(i) => (i + 1) % count,
        None => 0,
    });
    selection.drag_offset = None;
}

// ─── Save ──────────────────────────────────────────────────────────────────

/// Ctrl+S saves the current screen to disk.
pub fn save_shortcut_system(keys: Res<ButtonInput<KeyCode>>, mut editor: ResMut<EditorScreen>) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        match io::save_screen(&editor.screen) {
            Ok(()) => {
                editor.dirty = false;
                info!("screen '{}' saved", editor.screen.id);
            }
            Err(e) => error!("save failed: {e}"),
        }
    }
}
