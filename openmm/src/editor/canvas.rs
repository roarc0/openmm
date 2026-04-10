//! Canvas rendering: element spawning, selection, drag-move, z-order, debug labels.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

use super::format::{Screen, ScreenElement};
use super::io;
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::hud::UiAssets;

/// Reference resolution (MM6 UI coordinate space).
pub const REF_W: f32 = 640.0;
pub const REF_H: f32 = 480.0;

/// Runtime state of the screen being edited.
#[derive(Resource)]
pub struct EditorScreen {
    pub screen: Screen,
    /// Set to true when the screen has unsaved changes.
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

/// Current selection state.
#[derive(Resource, Default)]
pub struct Selection {
    pub index: Option<usize>,
    pub drag_offset: Option<Vec2>,
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
    mut last_count: Local<usize>,
    mut last_bg: Local<Option<String>>,
) {
    let current_count = editor.screen.elements.len();
    let current_bg = editor.screen.background.clone();

    if *last_count == current_count && *last_bg == current_bg {
        return;
    }
    *last_count = current_count;
    *last_bg = current_bg.clone();

    for e in old_bg.iter().chain(old_elems.iter()) {
        commands.entity(e).despawn();
    }

    // Background.
    if let Some(ref bg_name) = editor.screen.background {
        if let Some(handle) = ui_assets.get_or_load(bg_name, &game_assets, &mut images, &cfg) {
            commands.spawn((
                Name::new("canvas_background"),
                ImageNode::new(handle),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Percent(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Pickable::IGNORE,
                ZIndex(-1),
                CanvasBackground,
            ));
        }
    }

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
    let (w, h) = elem.size.unwrap_or_else(|| {
        elem.texture_for_state("default")
            .and_then(|name| ui_assets.dimensions(name))
            .map(|(w, h)| (w as f32, h as f32))
            .unwrap_or((32.0, 32.0))
    });

    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(elem.position.0 / REF_W * 100.0),
        top: Val::Percent(elem.position.1 / REF_H * 100.0),
        width: Val::Percent(w / REF_W * 100.0),
        height: Val::Percent(h / REF_H * 100.0),
        ..default()
    };

    let tex_name = elem.texture_for_state("default").unwrap_or("").to_string();
    let maybe_handle = if !tex_name.is_empty() {
        ui_assets.get_or_load(&tex_name, game_assets, images, cfg)
    } else {
        None
    };

    let label = Name::new(format!("canvas_elem_{}", elem.id));
    let marker = CanvasElement { index };
    let z = ZIndex(elem.z);

    if let Some(handle) = maybe_handle {
        commands.spawn((label, ImageNode::new(handle), node, z, marker, Pickable::IGNORE));
    } else {
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

// ─── Debug overlays (egui painter) ──────────────────────────────────────────

/// Draw selection borders and labels via egui painter. Runs in EguiPrimaryContextPass.
pub fn draw_overlays(
    mut contexts: EguiContexts,
    editor: Res<EditorScreen>,
    selection: Res<Selection>,
    windows: Query<&Window, With<PrimaryWindow>>,
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
        let (w, h) = elem.size.unwrap_or((32.0, 32.0));

        // Convert reference coords to screen coords.
        let sx = elem.position.0 / REF_W * win_w;
        let sy = elem.position.1 / REF_H * win_h;
        let sw = w / REF_W * win_w;
        let sh = h / REF_H * win_h;

        let rect = egui::Rect::from_min_size(egui::pos2(sx, sy), egui::vec2(sw, sh));

        let is_selected = selection.index == Some(i);
        let (stroke, text_color) = if is_selected {
            (
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 0, 255)),
                egui::Color32::from_rgb(255, 0, 255),
            )
        } else {
            (
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(128, 128, 128, 100)),
                egui::Color32::from_rgba_unmultiplied(200, 200, 200, 150),
            )
        };

        // Border drawn inside the sprite rect.
        painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);

        // Label inside the sprite, top-left corner.
        let label = format!(
            "{}[{},{}]@({},{})",
            elem.id, w as i32, h as i32, elem.position.0 as i32, elem.position.1 as i32,
        );
        painter.text(
            rect.left_top() + egui::vec2(3.0, 2.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(13.0),
            text_color,
        );
    }
}

// ─── Selection ──────────────────────────────────────────────────────────────

/// On left click, selects the topmost element under the cursor.
pub fn selection_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    editor: Res<EditorScreen>,
    mut selection: ResMut<Selection>,
) {
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
        let (w, h) = elem.size.unwrap_or((32.0, 32.0));
        let (ex, ey) = elem.position;
        if cx >= ex && cx <= ex + w && cy >= ey && cy <= ey + h {
            if best.map_or(true, |(_, bz)| elem.z > bz) {
                best = Some((i, elem.z));
            }
        }
    }

    selection.index = best.map(|(idx, _)| idx);
    selection.drag_offset = None;
}

// ─── Drag ───────────────────────────────────────────────────────────────────

/// Drag selected element with mouse.
pub fn drag_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut selection: ResMut<Selection>,
    mut editor: ResMut<EditorScreen>,
) {
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let win_w = window.width();
    let win_h = window.height();

    let cursor_ref = Vec2::new(cursor.x / win_w * REF_W, cursor.y / win_h * REF_H);
    let Some(sel_idx) = selection.index else { return };

    if mouse.just_pressed(MouseButton::Left) {
        if let Some(elem) = editor.screen.elements.get(sel_idx) {
            let elem_pos = Vec2::new(elem.position.0, elem.position.1);
            selection.drag_offset = Some(cursor_ref - elem_pos);
        }
    }

    if mouse.pressed(MouseButton::Left) {
        if let Some(offset) = selection.drag_offset {
            let new_pos = cursor_ref - offset;
            if let Some(elem) = editor.screen.elements.get_mut(sel_idx) {
                let (w, h) = elem.size.unwrap_or((32.0, 32.0));
                let x = new_pos.x.round().clamp(0.0, (REF_W - w).max(0.0));
                let y = new_pos.y.round().clamp(0.0, (REF_H - h).max(0.0));
                elem.position = (x, y);
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
pub fn sync_element_positions(editor: Res<EditorScreen>, mut elem_q: Query<(&CanvasElement, &mut Node)>) {
    for (ce, mut node) in &mut elem_q {
        let Some(elem) = editor.screen.elements.get(ce.index) else {
            continue;
        };
        node.left = Val::Percent(elem.position.0 / REF_W * 100.0);
        node.top = Val::Percent(elem.position.1 / REF_H * 100.0);
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
        elem.z += delta;
        Some(elem.z)
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

// ─── Delete ────────────────────────────────────────────────────────────────

/// Delete/Backspace removes the selected element.
pub fn delete_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut editor: ResMut<EditorScreen>,
) {
    if !keys.just_pressed(KeyCode::Delete) && !keys.just_pressed(KeyCode::Backspace) {
        return;
    }
    let Some(idx) = selection.index else { return };
    if idx < editor.screen.elements.len() {
        editor.screen.elements.remove(idx);
        editor.dirty = true;
        selection.index = None;
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
