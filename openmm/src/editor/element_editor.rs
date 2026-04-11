//! Event editor and variant editor windows extracted from canvas overlays.

use bevy_inspector_egui::bevy_egui::egui;

use super::canvas::{EditorScreen, Selection};
use super::format::ElementState;
use crate::screens::TRANSPARENCY_OPTIONS;

/// Draw the event editor window for the selected element.
/// Returns true if the window was closed this frame.
pub fn draw_event_editor(ctx: &egui::Context, editor: &mut EditorScreen, selection: &mut Selection) {
    let Some(sel) = selection.index else { return };
    if sel >= editor.screen.elements.len() {
        return;
    }

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

        draw_video_properties(ui, editor, sel);
        draw_image_transparency(ui, editor, sel);
        draw_image_actions(ui, editor, sel);
        draw_video_actions(ui, editor, sel);
        draw_image_bindings(ui, editor, sel);
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

/// Draw the variant (states) editor window for the selected image element.
pub fn draw_variant_editor(ctx: &egui::Context, editor: &mut EditorScreen, selection: &mut Selection) {
    let Some(sel) = selection.index else { return };
    if sel >= editor.screen.elements.len() || editor.screen.elements[sel].as_image().is_none() {
        return;
    }

    let mut open = true;
    egui::Window::new("Variants")
        .id(egui::Id::new("var_editor"))
        .resizable(true)
        .collapsible(false)
        .default_width(350.0)
        .open(&mut open)
        .show(ctx, |ui| {
            let state_keys: Vec<String> = editor.screen.elements[sel]
                .as_image()
                .unwrap()
                .states
                .keys()
                .cloned()
                .collect();
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
                    let mut tex = img.states.get(key).map(|s| s.texture.clone()).unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label("texture:");
                        if ui.text_edit_singleline(&mut tex).changed() {
                            if let Some(state) = editor.screen.elements[sel].as_image_mut().unwrap().states.get_mut(key)
                            {
                                state.texture = tex;
                                editor.dirty = true;
                            }
                        }
                    });

                    let img = editor.screen.elements[sel].as_image().unwrap();
                    let mut cond = img.states.get(key).map(|s| s.condition.clone()).unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label("condition:");
                        if ui.text_edit_singleline(&mut cond).changed() {
                            if let Some(state) = editor.screen.elements[sel].as_image_mut().unwrap().states.get_mut(key)
                            {
                                state.condition = cond;
                                editor.dirty = true;
                            }
                        }
                    });
                });
            }

            if let Some(k) = to_remove {
                editor.screen.elements[sel].as_image_mut().unwrap().states.remove(&k);
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
                    ElementState {
                        texture: String::new(),
                        condition: String::new(),
                        transparent_color: String::new(),
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

// ── Private helpers ──────────────────────────────────────────────────────────

fn draw_video_properties(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let Some(vid) = editor.screen.elements[sel].as_video() else {
        return;
    };

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
        size_changed |= ui
            .add(egui::DragValue::new(&mut vid_size.0).prefix("w: ").speed(1.0))
            .changed();
        size_changed |= ui
            .add(egui::DragValue::new(&mut vid_size.1).prefix("h: ").speed(1.0))
            .changed();
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

fn draw_image_transparency(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let Some(img) = editor.screen.elements[sel].as_image() else {
        return;
    };

    let current_tc = img.transparent_color.clone();
    let tc_label = if current_tc.is_empty() { "none" } else { &current_tc };
    ui.horizontal(|ui| {
        ui.label("Transparent:");
        egui::ComboBox::from_id_salt("edt_tc")
            .selected_text(tc_label)
            .show_ui(ui, |ui| {
                for &opt in TRANSPARENCY_OPTIONS {
                    let label = if opt.is_empty() { "none" } else { opt };
                    if ui.selectable_label(current_tc == opt, label).clicked() {
                        editor.screen.elements[sel].as_image_mut().unwrap().transparent_color = opt.to_string();
                        editor.dirty = true;
                    }
                }
            });
    });

    ui.separator();
}

fn draw_image_actions(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let Some(img_ref) = editor.screen.elements[sel].as_image() else {
        return;
    };
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
        editor.screen.elements[sel]
            .as_image_mut()
            .unwrap()
            .on_click
            .push(String::new());
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
        editor.screen.elements[sel]
            .as_image_mut()
            .unwrap()
            .on_hover
            .push(String::new());
        editor.dirty = true;
    }
}

fn draw_video_actions(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let Some(vid_ref) = editor.screen.elements[sel].as_video() else {
        return;
    };

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
        editor.screen.elements[sel]
            .as_video_mut()
            .unwrap()
            .on_end
            .push(String::new());
        editor.dirty = true;
    }
}

fn draw_image_bindings(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    if editor.screen.elements[sel].as_image().is_none() {
        return;
    }

    ui.separator();
    ui.heading("bindings");
    ui.small("property \u{2192} variable (e.g. scroll_x \u{2192} player.compass_yaw)");
    let bind_keys: Vec<String> = editor.screen.elements[sel]
        .as_image()
        .unwrap()
        .bindings
        .keys()
        .cloned()
        .collect();
    let mut bind_remove: Option<String> = None;
    for key in &bind_keys {
        let mut val = editor.screen.elements[sel]
            .as_image()
            .unwrap()
            .bindings
            .get(key)
            .cloned()
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(format!("{}:", key));
            if ui.text_edit_singleline(&mut val).changed() {
                editor.screen.elements[sel]
                    .as_image_mut()
                    .unwrap()
                    .bindings
                    .insert(key.clone(), val);
                editor.dirty = true;
            }
            if ui.small_button("\u{2715}").clicked() {
                bind_remove = Some(key.clone());
            }
        });
    }
    if let Some(k) = bind_remove {
        editor.screen.elements[sel].as_image_mut().unwrap().bindings.remove(&k);
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
            .as_image_mut()
            .unwrap()
            .bindings
            .entry(key.to_string())
            .or_default();
        editor.dirty = true;
    }
}
