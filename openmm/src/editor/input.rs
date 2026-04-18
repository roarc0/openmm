//! Editor input systems: selection, drag, z-order, arrow nudge, delete, shortcuts.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_inspector_egui::bevy_egui::input::EguiWantsInput;

use super::canvas::{CanvasElement, EditorScreen, Selection, resolve_elem_size};

use super::clipboard::Clipboard;
use super::overlay::{OverlayAction, OverlayCmd};
use super::io;

use crate::screens::ui_assets::UiAssets;
use crate::screens::{REF_H, REF_W};


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

    let cx = cursor.x / win_w * REF_W;
    let cy = cursor.y / win_h * REF_H;

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
        selection.edt_open = new_sel.is_some();
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
    egui_input: Option<Res<EguiWantsInput>>,
) {
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
        .is_some_and(|e| editor.screen.editor.locked.contains(&e.id().to_string()))
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
) {
    let Some(sel_idx) = selection.index else { return };
    if editor
        .screen
        .elements
        .get(sel_idx)
        .is_some_and(|e| editor.screen.editor.locked.contains(&e.id().to_string()))
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
) {
    if !keys.just_pressed(KeyCode::Delete) && !keys.just_pressed(KeyCode::Backspace) {
        return;
    }
    let Some(idx) = selection.index else { return };
    if editor
        .screen
        .elements
        .get(idx)
        .is_some_and(|e| editor.screen.editor.locked.contains(&e.id().to_string()))
    {
        return;
    }
    if idx < editor.screen.elements.len() {
        editor.screen.elements.remove(idx);
        editor.dirty = true;
        selection.index = None;
    }
}

// ─── Keyboard shortcuts ────────────────────────────────────────────────────

/// T=top, U=up, D=down, B=bottom, V=toggle visibility, L=lock, E=edit, W=variants.
pub fn shortcut_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut selection: ResMut<Selection>,
    mut overlay_action: ResMut<OverlayAction>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl {
        return;
    }

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

/// Save the current screen, handling renames and pruning locked elements.
pub fn save_editor_screen(editor: &mut EditorScreen) {
    editor.screen.prune_locked_elements();
    if let Some(old) = &editor.original_id {
        if *old != editor.screen.id {
            io::delete_screen(old);
        }
    }
    match io::save_screen(&editor.screen) {
        Ok(()) => {
            editor.dirty = false;
            editor.original_id = Some(editor.screen.id.clone());
            info!("screen '{}' saved", editor.screen.id);
        }
        Err(e) => error!("save failed: {e}"),
    }
}

/// Ctrl+S saves the current screen to disk.
pub fn save_shortcut_system(keys: Res<ButtonInput<KeyCode>>, mut editor: ResMut<EditorScreen>) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyS) {
        save_editor_screen(&mut editor);
    }
}

// ─── Copy Paste ───────────────────────────────────────────────────────────

/// Ctrl+C copies selected element, Ctrl+V pastes it at mouse position.
pub fn copy_paste_system(
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut selection: ResMut<Selection>,
    mut editor: ResMut<EditorScreen>,
    mut clipboard: ResMut<Clipboard>,
    ui_assets: Res<UiAssets>,
) {
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    // Copy
    if ctrl && keys.just_pressed(KeyCode::KeyC) {
        if let Some(idx) = selection.index {
            if let Some(elem) = editor.screen.elements.get(idx) {
                clipboard.copy(elem);
            }
        }
    }

    // Paste
    if ctrl && keys.just_pressed(KeyCode::KeyV) {
        let Ok(window) = windows.single() else { return };
        let Some(cursor) = window.cursor_position() else { return };
        let win_w = window.width();
        let win_h = window.height();
        let mouse_ref = Vec2::new(cursor.x / win_w * REF_W, cursor.y / win_h * REF_H);

        if let Some(new_idx) = clipboard.paste(&mut editor, mouse_ref, &ui_assets) {
            selection.index = Some(new_idx);
            selection.edt_open = true;
            selection.var_open = false;
            selection.drag_offset = None;
        }
    }
}
