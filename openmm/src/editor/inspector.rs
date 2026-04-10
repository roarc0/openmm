//! egui inspector panel: edit selected element properties.

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

use super::canvas::EditorScreen;
use super::canvas::Selection;
use super::format::ElementState;
use crate::screens::ScreenElement;

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

            let mut music = editor.screen.bg_music.clone();
            ui.horizontal(|ui| {
                ui.label("Music:");
                if ui.text_edit_singleline(&mut music).changed() {
                    editor.screen.bg_music = music;
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

            // Clone shared fields to avoid holding the mutable borrow across closures.
            let mut elem_id = editor.screen.elements[sel_idx].id().to_string();
            let mut pos = editor.screen.elements[sel_idx].position();
            let mut size = editor.screen.elements[sel_idx].size();
            let mut z = editor.screen.elements[sel_idx].z();
            let is_image = editor.screen.elements[sel_idx].as_image().is_some();
            let is_video = editor.screen.elements[sel_idx].as_video().is_some();
            let state_keys: Vec<String> = if is_image {
                editor.screen.elements[sel_idx].as_image().unwrap().states.keys().cloned().collect()
            } else {
                Vec::new()
            };
            let actions_count = editor.screen.elements[sel_idx].on_click().len();
            let hover_count = editor.screen.elements[sel_idx].on_hover().len();

            // ID
            ui.horizontal(|ui| {
                ui.label("ID:");
                if ui.text_edit_singleline(&mut elem_id).changed() {
                    match &mut editor.screen.elements[sel_idx] {
                        ScreenElement::Image(img) => img.id = elem_id.clone(),
                        ScreenElement::Video(vid) => vid.id = elem_id.clone(),
                    }
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
                editor.screen.elements[sel_idx].set_position(pos);
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
                editor.screen.elements[sel_idx].set_size(size);
                editor.dirty = true;
            }

            // Z
            ui.horizontal(|ui| {
                ui.label("Z:");
                if ui.add(egui::DragValue::new(&mut z).prefix("z: ").speed(1.0)).changed() {
                    editor.screen.elements[sel_idx].set_z(z);
                    editor.dirty = true;
                }
            });

            // Video (only for Video variant)
            if is_video {
                let mut video_name = editor.screen.elements[sel_idx]
                    .as_video().unwrap().video.clone();
                ui.horizontal(|ui| {
                    ui.label("Video:");
                    if ui.text_edit_singleline(&mut video_name).changed() {
                        editor.screen.elements[sel_idx]
                            .as_video_mut().unwrap().video = video_name;
                        editor.dirty = true;
                    }
                });
                if !editor.screen.elements[sel_idx].as_video().unwrap().video.is_empty() {
                    let mut looping = editor.screen.elements[sel_idx].as_video().unwrap().looping;
                    let mut skippable = editor.screen.elements[sel_idx].as_video().unwrap().skippable;
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut looping, "Loop").changed() {
                            changed = true;
                        }
                        if ui.checkbox(&mut skippable, "Skippable").changed() {
                            changed = true;
                        }
                    });
                    if changed {
                        let vid = editor.screen.elements[sel_idx].as_video_mut().unwrap();
                        vid.looping = looping;
                        vid.skippable = skippable;
                        editor.dirty = true;
                    }
                }
            }

            ui.separator();

            // ── States (image-only) ─────────────────────────────────
            if is_image {
                ui.collapsing("States", |ui| {
                    let mut to_remove: Option<String> = None;
                    let mut to_add = false;

                    for key in &state_keys {
                        let mut tex = editor.screen.elements[sel_idx]
                            .as_image().unwrap()
                            .states
                            .get(key)
                            .map(|s| s.texture.clone())
                            .unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label(key.as_str());
                            if ui.text_edit_singleline(&mut tex).changed() {
                                if let Some(state) = editor.screen.elements[sel_idx]
                                    .as_image_mut().unwrap().states.get_mut(key)
                                {
                                    state.texture = tex;
                                    editor.dirty = true;
                                }
                            }
                            if ui.small_button("\u{2715}").clicked() {
                                to_remove = Some(key.clone());
                            }
                        });
                    }
                    if let Some(k) = to_remove {
                        editor.screen.elements[sel_idx]
                            .as_image_mut().unwrap().states.remove(&k);
                        editor.dirty = true;
                    }
                    if ui.small_button("+ Add state").clicked() {
                        to_add = true;
                    }
                    if to_add {
                        let img = editor.screen.elements[sel_idx].as_image_mut().unwrap();
                        let new_key = format!("state{}", img.states.len());
                        img.states.insert(
                            new_key,
                            ElementState {
                                texture: String::new(),
                                condition: String::new(),
                            },
                        );
                        editor.dirty = true;
                    }
                });
            }

            // ── Actions (image-only) ─────────────────────────────────
            if is_image {
                ui.collapsing("on_click actions", |ui| {
                    let mut to_remove: Option<usize> = None;
                    for i in 0..actions_count {
                        let mut action = editor.screen.elements[sel_idx].as_image().unwrap().on_click[i].clone();
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(&mut action).changed() {
                                editor.screen.elements[sel_idx].as_image_mut().unwrap().on_click[i] = action;
                                editor.dirty = true;
                            }
                            if ui.small_button("\u{2715}").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = to_remove {
                        editor.screen.elements[sel_idx].as_image_mut().unwrap().on_click.remove(i);
                        editor.dirty = true;
                    }
                    if ui.small_button("+ Add action").clicked() {
                        editor.screen.elements[sel_idx].as_image_mut().unwrap().on_click.push(String::new());
                        editor.dirty = true;
                    }
                });

                ui.collapsing("on_hover actions", |ui| {
                    let mut to_remove: Option<usize> = None;
                    for i in 0..hover_count {
                        let mut action = editor.screen.elements[sel_idx].as_image().unwrap().on_hover[i].clone();
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(&mut action).changed() {
                                editor.screen.elements[sel_idx].as_image_mut().unwrap().on_hover[i] = action;
                                editor.dirty = true;
                            }
                            if ui.small_button("\u{2715}").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = to_remove {
                        editor.screen.elements[sel_idx].as_image_mut().unwrap().on_hover.remove(i);
                        editor.dirty = true;
                    }
                    if ui.small_button("+ Add action").clicked() {
                        editor.screen.elements[sel_idx].as_image_mut().unwrap().on_hover.push(String::new());
                        editor.dirty = true;
                    }
                });
            }

            // ── on_end (video-only) ─────────────────────────────────
            if is_video {
                ui.collapsing("on_end actions", |ui| {
                    let end_count = editor.screen.elements[sel_idx].as_video().unwrap().on_end.len();
                    let mut to_remove: Option<usize> = None;
                    for i in 0..end_count {
                        let mut action = editor.screen.elements[sel_idx].as_video().unwrap().on_end[i].clone();
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(&mut action).changed() {
                                editor.screen.elements[sel_idx].as_video_mut().unwrap().on_end[i] = action;
                                editor.dirty = true;
                            }
                            if ui.small_button("\u{2715}").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = to_remove {
                        editor.screen.elements[sel_idx].as_video_mut().unwrap().on_end.remove(i);
                        editor.dirty = true;
                    }
                    if ui.small_button("+ Add action").clicked() {
                        editor.screen.elements[sel_idx].as_video_mut().unwrap().on_end.push(String::new());
                        editor.dirty = true;
                    }
                });
            }

            ui.separator();

            // Debug label.
            let elem = &editor.screen.elements[sel_idx];
            let (w, h) = elem.size();
            let (px, py) = elem.position();
            ui.small(format!("{}[{w:.0},{h:.0}]@({px:.0},{py:.0})", elem.id()));
        });
}
