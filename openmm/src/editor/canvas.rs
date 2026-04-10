//! Canvas rendering: element spawning, selection, drag-move, z-order, debug labels.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

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
    /// Offset from element top-left to cursor at drag start (in reference coords).
    pub drag_offset: Option<Vec2>,
}

// ─── Rebuild ─────────────────────────────────────────────────────────────────

/// Despawns and re-spawns all canvas entities when the element count or background changes.
///
/// Position-only changes during drag are handled by `sync_element_positions` to
/// avoid an expensive full rebuild every frame.
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

    // Only rebuild when structure changes — not on every position update.
    if *last_count == current_count && *last_bg == current_bg {
        return;
    }
    *last_count = current_count;
    *last_bg = current_bg.clone();

    // Despawn old entities.
    for e in old_bg.iter().chain(old_elems.iter()) {
        commands.entity(e).despawn();
    }

    // Spawn background.
    if let Some(ref bg_name) = editor.screen.background {
        let color = BackgroundColor(Color::srgba(0.1, 0.1, 0.1, 1.0));
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
                ZIndex(-1),
                CanvasBackground,
            ));
        } else {
            commands.spawn((
                Name::new("canvas_background"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.0),
                    top: Val::Percent(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                color,
                ZIndex(-1),
                CanvasBackground,
            ));
        }
    }

    // Spawn each element.
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
    let left = elem.position.0 / REF_W * 100.0;
    let top = elem.position.1 / REF_H * 100.0;

    let (width, height) = if let Some((w, h)) = elem.size {
        (Val::Percent(w / REF_W * 100.0), Val::Percent(h / REF_H * 100.0))
    } else {
        // Try to get natural size from the loaded texture.
        let tex_name = elem.texture_for_state("default").unwrap_or("");
        if let Some((w, h)) = ui_assets.dimensions(tex_name) {
            (
                Val::Percent(w as f32 / REF_W * 100.0),
                Val::Percent(h as f32 / REF_H * 100.0),
            )
        } else {
            (Val::Percent(5.0), Val::Percent(5.0))
        }
    };

    let node = Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(left),
        top: Val::Percent(top),
        width,
        height,
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
        commands.spawn((label, ImageNode::new(handle), node, z, marker));
    } else {
        // Magenta placeholder for missing textures.
        commands.spawn((
            label,
            BackgroundColor(Color::srgba(1.0, 0.0, 1.0, 0.8)),
            node,
            z,
            marker,
        ));
    }
}

// ─── Update labels (gizmo borders) ───────────────────────────────────────────

/// Draws gizmo rect borders around each element — yellow for selected, grey for unselected.
pub fn update_labels(
    mut gizmos: Gizmos,
    selection: Res<Selection>,
    elem_q: Query<(&GlobalTransform, &ComputedNode, &CanvasElement)>,
) {
    for (gt, cn, ce) in &elem_q {
        let pos = gt.translation().truncate();
        let size = cn.size();
        if size == Vec2::ZERO {
            continue;
        }
        let center = pos + Vec2::new(size.x * 0.5, -size.y * 0.5);
        let color = if selection.index == Some(ce.index) {
            Color::srgba(1.0, 1.0, 0.0, 1.0)
        } else {
            Color::srgba(0.4, 0.4, 0.4, 0.6)
        };
        gizmos.rect_2d(Isometry2d::from_translation(center), size, color);
    }
}

// ─── Selection ───────────────────────────────────────────────────────────────

/// On left click, selects the topmost element under the cursor.
pub fn selection_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    elem_q: Query<(&GlobalTransform, &ComputedNode, &CanvasElement)>,
    mut selection: ResMut<Selection>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };

    // Find topmost (highest ZIndex) element containing cursor.
    let mut best: Option<(usize, i32)> = None;
    for (gt, cn, ce) in &elem_q {
        let pos = gt.translation().truncate();
        let size = cn.size();
        // UI nodes: origin at top-left, y increases downward in screen space.
        let rect = Rect::from_corners(pos, pos + Vec2::new(size.x, size.y));
        if rect.contains(cursor) {
            let z = ce.index as i32; // use index as tiebreaker; z_order tracked separately
            if best.map_or(true, |(_, bz)| z > bz) {
                best = Some((ce.index, z));
            }
        }
    }

    selection.index = best.map(|(idx, _)| idx);
}

// ─── Drag ────────────────────────────────────────────────────────────────────

/// Records drag offset on press, updates element position while held, clears on release.
pub fn drag_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    elem_q: Query<(&GlobalTransform, &ComputedNode, &CanvasElement)>,
    mut selection: ResMut<Selection>,
    mut editor: ResMut<EditorScreen>,
) {
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let win_size = Vec2::new(window.width(), window.height());

    // Convert cursor screen pos to reference coords.
    let cursor_ref = Vec2::new(cursor.x / win_size.x * REF_W, cursor.y / win_size.y * REF_H);

    let Some(sel_idx) = selection.index else { return };

    if mouse.just_pressed(MouseButton::Left) {
        // Record offset between element position and cursor (in ref coords).
        for (gt, _cn, ce) in &elem_q {
            if ce.index != sel_idx {
                continue;
            }
            let pos = gt.translation().truncate();
            let elem_ref = Vec2::new(pos.x / win_size.x * REF_W, pos.y / win_size.y * REF_H);
            selection.drag_offset = Some(cursor_ref - elem_ref);
            break;
        }
    }

    if mouse.pressed(MouseButton::Left) {
        if let Some(offset) = selection.drag_offset {
            let new_pos = cursor_ref - offset;
            if let Some(elem) = editor.screen.elements.get_mut(sel_idx) {
                elem.position = (new_pos.x, new_pos.y);
                editor.dirty = true;
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        selection.drag_offset = None;
    }
}

// ─── Sync positions ───────────────────────────────────────────────────────────

/// Syncs Bevy `Node` positions from `EditorScreen` data every frame.
/// Handles position changes from drag and inspector edits without a full rebuild.
pub fn sync_element_positions(editor: Res<EditorScreen>, mut elem_q: Query<(&CanvasElement, &mut Node)>) {
    for (ce, mut node) in &mut elem_q {
        let Some(elem) = editor.screen.elements.get(ce.index) else {
            continue;
        };
        node.left = Val::Percent(elem.position.0 / REF_W * 100.0);
        node.top = Val::Percent(elem.position.1 / REF_H * 100.0);
    }
}

// ─── Z-order ─────────────────────────────────────────────────────────────────

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
        // Update the ZIndex component directly to avoid a full rebuild.
        for (ce, mut z) in &mut elem_q {
            if ce.index == sel_idx {
                *z = ZIndex(z_val);
            }
        }
    }
}

// ─── Delete ───────────────────────────────────────────────────────────────────

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
        // Rebuild triggered automatically via resource_changed.
    }
}

// ─── Save ────────────────────────────────────────────────────────────────────

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
