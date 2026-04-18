//! Overlay drawing: selection borders, labels, status stamps, element toolbar.
//! Also handles deferred overlay commands (z-order, remove, lock, visibility).

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

use super::canvas::{CanvasElement, EditorScreen, ElementEditorState, Selection, resolve_elem_size};
use crate::screens::ui_assets::UiAssets;
use crate::screens::{REF_H, REF_W};

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
    guides: Res<super::guides::Guides>,
    mut cfg: ResMut<super::io::EditorConfig>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let Ok(window) = windows.single() else { return };
    let win_w = window.width();
    let win_h = window.height();

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("canvas_overlays"),
    ));

    // Draw guide lines first (behind selection overlays).
    super::guides::draw_guides(&painter, &guides, win_w, win_h);

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
            "{}{}\n[{},{}]@({},{})\nz={}",
            elem.id(),
            flags,
            w as i32,
            h as i32,
            pos.0 as i32,
            pos.1 as i32,
            elem.z(),
        );
        if is_selected {
            painter.text(
                rect.left_top() + egui::vec2(3.0, 2.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(12.0),
                text_color,
            );
        }

        // Status stamps at bottom-right inside the element.
        draw_status_stamps(&painter, elem, &editor, rect);
    }

    // Toolbar buttons for selected element.
    draw_element_toolbar(
        ctx,
        &editor,
        &mut selection,
        &ui_assets,
        &mut overlay_action,
        &editor_state,
        win_w,
        win_h,
    );

    // Element editor window.
    if selection.edt_open {
        super::element_editor::draw_element_editor(ctx, &mut editor, &mut selection, &mut cfg);
    }
    // Variant editor window.
    if selection.var_open {
        super::element_editor::draw_variant_editor(ctx, &mut editor, &mut selection);
    }
}

/// Draw status stamps (LCK, EVT, VAR) at bottom-right of an element.
fn draw_status_stamps(
    painter: &egui::Painter,
    elem: &crate::screens::ScreenElement,
    editor: &EditorScreen,
    rect: egui::Rect,
) {
    let is_locked = editor.screen.editor.locked.contains(&elem.id().to_string());
    let evt_count = elem.on_click().len() + elem.on_hover().len() + elem.as_image().map_or(0, |img| img.bindings.len());
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
    if stamps.is_empty() {
        return;
    }
    let text = stamps.join(" ");
    let font = egui::FontId::monospace(13.0);
    let galley = painter.layout_no_wrap(text.clone(), font.clone(), egui::Color32::WHITE);
    let text_size = galley.size();
    let pad = egui::vec2(4.0, 2.0);
    let text_pos =
        rect.right_bottom() - egui::vec2(text_size.x + pad.x * 2.0, text_size.y + pad.y * 2.0) + egui::vec2(-2.0, -2.0);
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

/// Draw toolbar buttons for the selected element.
fn draw_element_toolbar(
    ctx: &egui::Context,
    editor: &EditorScreen,
    selection: &mut Selection,
    ui_assets: &UiAssets,
    overlay_action: &mut OverlayAction,
    editor_state: &ElementEditorState,
    win_w: f32,
    win_h: f32,
) {
    let Some(sel) = selection.index else { return };
    let Some(elem) = editor.screen.elements.get(sel) else {
        return;
    };

    let (_, h) = resolve_elem_size(elem, ui_assets);
    let pos = elem.position();
    let sx = pos.0 / REF_W * win_w;
    let sy = pos.1 / REF_H * win_h;
    let sh = h / REF_H * win_h;

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
                let is_locked = editor.screen.editor.locked.contains(&elem.id().to_string());
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
                let id = editor.screen.elements[idx].id().to_string();
                editor.screen.elements.remove(idx);
                editor.dirty = true;
                selection.index = None;
                editor_state.hidden.remove(&idx);
                editor.screen.editor.locked.retain(|locked_id| locked_id != &id);
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
                if let Some(pos) = editor.screen.editor.locked.iter().position(|l| l == &id) {
                    editor.screen.editor.locked.remove(pos);
                } else {
                    editor.screen.editor.locked.push(id);
                }
                editor.dirty = true;
            }
        }
        OverlayCmd::BringToTop(idx) => {
            let new_z = editor.screen.max_z() + 1;
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
