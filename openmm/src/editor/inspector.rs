//! egui inspector panel: edit selected element properties.

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

use super::canvas::EditorScreen;
use super::canvas::Selection;
use super::format::ElementState;

/// Draw the inspector egui window (always visible, anchored top-right).
pub fn inspector_ui(mut contexts: EguiContexts, mut editor: ResMut<EditorScreen>, selection: Res<Selection>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("Inspector")
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-10.0, 10.0))
        .resizable(true)
        .default_width(240.0)
        .collapsible(true)
        .default_open(false)
        .show(ctx, |ui| {
            // ── Screen-level ─────────────────────────────────────────
            ui.heading("Screen");

            let mut screen_id = editor.screen.id.clone();
            ui.horizontal(|ui| {
                ui.label("ID:");
                if ui.text_edit_singleline(&mut screen_id).changed() {
                    editor.screen.id = screen_id;
                    editor.dirty = true;
                }
            });

            let mut bg = editor.screen.background.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Background:");
                if ui.text_edit_singleline(&mut bg).changed() {
                    editor.screen.background = if bg.is_empty() { None } else { Some(bg.clone()) };
                    editor.dirty = true;
                }
            });

            ui.separator();

            // ── Element ──────────────────────────────────────────────
            let Some(sel_idx) = selection.index else {
                ui.label("No element selected.");
                return;
            };
            if sel_idx >= editor.screen.elements.len() {
                ui.label("Selection out of range.");
                return;
            }

            ui.heading("Element");

            // Clone fields to avoid holding the mutable borrow across closures.
            let mut elem_id = editor.screen.elements[sel_idx].id.clone();
            let mut pos = editor.screen.elements[sel_idx].position;
            let mut size = editor.screen.elements[sel_idx].size;
            let mut z = editor.screen.elements[sel_idx].z;
            let state_keys: Vec<String> = editor.screen.elements[sel_idx].states.keys().cloned().collect();
            let actions_count = editor.screen.elements[sel_idx].on_click.len();

            // ID
            ui.horizontal(|ui| {
                ui.label("ID:");
                if ui.text_edit_singleline(&mut elem_id).changed() {
                    editor.screen.elements[sel_idx].id = elem_id.clone();
                    editor.dirty = true;
                }
            });

            // Position
            let mut pos_changed = false;
            ui.horizontal(|ui| {
                ui.label("Position:");
                pos_changed |= ui
                    .add(egui::DragValue::new(&mut pos.0).prefix("x: ").speed(1.0))
                    .changed();
                pos_changed |= ui
                    .add(egui::DragValue::new(&mut pos.1).prefix("y: ").speed(1.0))
                    .changed();
            });
            if pos_changed {
                editor.screen.elements[sel_idx].position = pos;
                editor.dirty = true;
            }

            // Size
            let mut size_changed = false;
            ui.horizontal(|ui| {
                ui.label("Size:");
                size_changed |= ui
                    .add(egui::DragValue::new(&mut size.0).prefix("w: ").speed(1.0))
                    .changed();
                size_changed |= ui
                    .add(egui::DragValue::new(&mut size.1).prefix("h: ").speed(1.0))
                    .changed();
            });
            if size_changed {
                editor.screen.elements[sel_idx].size = size;
                editor.dirty = true;
            }

            // Z
            ui.horizontal(|ui| {
                ui.label("Z:");
                if ui.add(egui::DragValue::new(&mut z).prefix("z: ").speed(1.0)).changed() {
                    editor.screen.elements[sel_idx].z = z;
                    editor.dirty = true;
                }
            });

            // Hover only
            let mut hover_only = editor.screen.elements[sel_idx].hover_only;
            if ui.checkbox(&mut hover_only, "Hover only").changed() {
                editor.screen.elements[sel_idx].hover_only = hover_only;
                editor.dirty = true;
            }

            ui.separator();

            // ── States ───────────────────────────────────────────────
            ui.collapsing("States", |ui| {
                let mut to_remove: Option<String> = None;
                let mut to_add = false;

                for key in &state_keys {
                    let mut tex = editor.screen.elements[sel_idx]
                        .states
                        .get(key)
                        .map(|s| s.texture.clone())
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(key.as_str());
                        if ui.text_edit_singleline(&mut tex).changed() {
                            if let Some(state) = editor.screen.elements[sel_idx].states.get_mut(key) {
                                state.texture = tex;
                                editor.dirty = true;
                            }
                        }
                        if ui.small_button("✕").clicked() {
                            to_remove = Some(key.clone());
                        }
                    });
                }
                if let Some(k) = to_remove {
                    editor.screen.elements[sel_idx].states.remove(&k);
                    editor.dirty = true;
                }
                if ui.small_button("+ Add state").clicked() {
                    to_add = true;
                }
                if to_add {
                    let new_key = format!("state{}", editor.screen.elements[sel_idx].states.len());
                    editor.screen.elements[sel_idx]
                        .states
                        .insert(new_key, ElementState { texture: String::new() });
                    editor.dirty = true;
                }
            });

            // ── Actions ──────────────────────────────────────────────
            ui.collapsing("on_click actions", |ui| {
                let mut to_remove: Option<usize> = None;
                let mut to_add = false;

                for i in 0..actions_count {
                    let mut action = editor.screen.elements[sel_idx].on_click[i].clone();
                    ui.horizontal(|ui| {
                        if ui.text_edit_singleline(&mut action).changed() {
                            editor.screen.elements[sel_idx].on_click[i] = action;
                            editor.dirty = true;
                        }
                        if ui.small_button("✕").clicked() {
                            to_remove = Some(i);
                        }
                    });
                }
                if let Some(i) = to_remove {
                    editor.screen.elements[sel_idx].on_click.remove(i);
                    editor.dirty = true;
                }
                if ui.small_button("+ Add action").clicked() {
                    to_add = true;
                }
                if to_add {
                    editor.screen.elements[sel_idx].on_click.push(String::new());
                    editor.dirty = true;
                }
            });

            ui.separator();

            // Debug label.
            let elem = &editor.screen.elements[sel_idx];
            let (w, h) = elem.size;
            let (px, py) = elem.position;
            ui.small(format!("{}[{w:.0},{h:.0}]@({px:.0},{py:.0})", elem.id));
        });
}
