//! Editor panel — screen properties, element editing, and guide lines.
//!
//! Layout:
//!   Screen section — ID, sound, on_load, + Add Text
//!   Guides section — collapsible guide line manager
//!   Element section — all properties of the selected element

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

use super::canvas::EditorScreen;
use super::canvas::Selection;
use super::guides::Guides;
use super::io::EditorConfig;
use crate::screens::ElementState;
use crate::screens::ScreenElement;

pub fn editor_panel_ui(
    mut contexts: EguiContexts,
    mut editor: ResMut<EditorScreen>,
    selection: Res<Selection>,
    mut guides: ResMut<Guides>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let cfg = EditorConfig::load();
    let default_pos = cfg.editor_pos.unwrap_or([430.0, 10.0]);

    let resp = egui::Window::new("Editor")
        .default_pos(egui::pos2(default_pos[0], default_pos[1]))
        .resizable(true)
        .default_width(240.0)
        .collapsible(true)
        .default_open(false)
        .show(ctx, |ui| {
            // ━━ Screen ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
            ui.heading("Screen");

            let mut screen_id = editor.screen.id.clone();
            ui.horizontal(|ui| {
                ui.label("ID:");
                if ui.text_edit_singleline(&mut screen_id).changed() {
                    editor.screen.id = screen_id;
                    editor.dirty = true;
                }
            });

            let mut sound_id = editor.screen.sound.id().to_string();
            ui.horizontal(|ui| {
                ui.label("Sound:");
                if ui.text_edit_singleline(&mut sound_id).changed() {
                    let old_start = editor.screen.sound.start_sec();
                    let old_loop = editor.screen.sound.looping();
                    if sound_id.is_empty() {
                        editor.screen.sound = crate::screens::Sound::None;
                    } else if old_start > 0.0 || !old_loop {
                        editor.screen.sound = crate::screens::Sound::Sound {
                            id: sound_id,
                            start_sec: old_start,
                            looping: old_loop,
                        };
                    } else {
                        editor.screen.sound = crate::screens::Sound::Id(sound_id);
                    }
                    editor.dirty = true;
                }
            });

            let mut start_sec = editor.screen.sound.start_sec();
            let mut looping = editor.screen.sound.looping();
            let mut sound_changed = false;
            ui.horizontal(|ui| {
                ui.label("Start (s):");
                if ui
                    .add(
                        egui::DragValue::new(&mut start_sec)
                            .speed(0.1)
                            .clamp_range(0.0..=3600.0),
                    )
                    .changed()
                {
                    sound_changed = true;
                }
                if ui.checkbox(&mut looping, "Loop").changed() {
                    sound_changed = true;
                }
            });
            if sound_changed {
                editor.screen.sound = crate::screens::Sound::Sound {
                    id: editor.screen.sound.id().to_string(),
                    start_sec,
                    looping,
                };
                editor.dirty = true;
            }

            ui.collapsing("On Load", |ui| {
                let mut actions = editor.screen.on_load.clone();
                let mut dirty = false;
                action_list_editor(ui, &mut actions, &mut dirty);
                if dirty {
                    editor.screen.on_load = actions;
                    editor.dirty = true;
                }
            });

            if ui.small_button("+ Add Text").clicked() {
                let max_z = editor.screen.elements.iter().map(|e| e.z()).max().unwrap_or(0);
                editor
                    .screen
                    .elements
                    .push(ScreenElement::Text(crate::screens::TextElement {
                        id: "new_text".to_string(),
                        position: (crate::screens::REF_W / 2.0, crate::screens::REF_H / 2.0),
                        size: (200.0, 12.0),
                        z: max_z + 1,
                        hidden: false,
                        source: "footer_text".to_string(),
                        font: "smallnum".to_string(),
                        color: "white".to_string(),
                        align: "center".to_string(),
                    }));
                editor.dirty = true;
            }

            // ━━ Guides ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
            ui.separator();
            ui.collapsing("Guides", |ui| {
                super::guides::guides_section(ui, &mut guides);
            });

            // ━━ Element ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
            ui.separator();

            let Some(sel_idx) = selection.index else {
                ui.weak("No element selected.");
                return;
            };
            if sel_idx >= editor.screen.elements.len() {
                ui.weak("Selection out of range.");
                return;
            }

            let kind = match &editor.screen.elements[sel_idx] {
                ScreenElement::Image(_) => "Image",
                ScreenElement::Video(_) => "Video",
                ScreenElement::Text(_) => "Text",
            };
            ui.heading(format!("Element ({})", kind));

            // -- Common fields --
            let mut elem_id = editor.screen.elements[sel_idx].id().to_string();
            let mut pos = editor.screen.elements[sel_idx].position();
            let mut size = editor.screen.elements[sel_idx].size();
            let mut z = editor.screen.elements[sel_idx].z();

            ui.horizontal(|ui| {
                ui.label("ID:");
                if ui.text_edit_singleline(&mut elem_id).changed() {
                    match &mut editor.screen.elements[sel_idx] {
                        ScreenElement::Image(img) => img.id = elem_id.clone(),
                        ScreenElement::Video(vid) => vid.id = elem_id.clone(),
                        ScreenElement::Text(txt) => txt.id = elem_id.clone(),
                    }
                    editor.dirty = true;
                }
            });

            let mut pos_changed = false;
            ui.horizontal(|ui| {
                ui.label("Pos:");
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

            ui.horizontal(|ui| {
                ui.label("Z:");
                if ui.add(egui::DragValue::new(&mut z).speed(1.0)).changed() {
                    editor.screen.elements[sel_idx].set_z(z);
                    editor.dirty = true;
                }
            });

            // -- Image fields --
            if let Some(_img) = editor.screen.elements[sel_idx].as_image() {
                ui.separator();

                // Default texture
                let mut tex = editor.screen.elements[sel_idx]
                    .as_image()
                    .unwrap()
                    .states
                    .get("default")
                    .map(|s| s.texture.clone())
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("Texture:");
                    if ui.text_edit_singleline(&mut tex).changed() {
                        if let Some(state) = editor.screen.elements[sel_idx]
                            .as_image_mut()
                            .unwrap()
                            .states
                            .get_mut("default")
                        {
                            state.texture = tex;
                            editor.dirty = true;
                        }
                    }
                });

                // Clicked texture
                let mut clicked_tex = editor.screen.elements[sel_idx]
                    .as_image()
                    .unwrap()
                    .states
                    .get("clicked")
                    .map(|s| s.texture.clone())
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("Clicked:");
                    if ui.text_edit_singleline(&mut clicked_tex).changed() {
                        let img = editor.screen.elements[sel_idx].as_image_mut().unwrap();
                        if clicked_tex.is_empty() {
                            img.states.remove("clicked");
                        } else {
                            img.states
                                .entry("clicked".to_string())
                                .and_modify(|s| s.texture = clicked_tex.clone())
                                .or_insert(ElementState {
                                    texture: clicked_tex,
                                    condition: String::new(),
                                    transparent_color: String::new(),
                                });
                        }
                        editor.dirty = true;
                    }
                });

                // States (advanced)
                let state_keys: Vec<String> = editor.screen.elements[sel_idx]
                    .as_image()
                    .unwrap()
                    .states
                    .keys()
                    .cloned()
                    .collect();
                ui.collapsing("All States", |ui| {
                    let mut to_remove: Option<String> = None;
                    for key in &state_keys {
                        let mut tex = editor.screen.elements[sel_idx]
                            .as_image()
                            .unwrap()
                            .states
                            .get(key)
                            .map(|s| s.texture.clone())
                            .unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label(key.as_str());
                            if ui.text_edit_singleline(&mut tex).changed() {
                                if let Some(state) = editor.screen.elements[sel_idx]
                                    .as_image_mut()
                                    .unwrap()
                                    .states
                                    .get_mut(key)
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
                            .as_image_mut()
                            .unwrap()
                            .states
                            .remove(&k);
                        editor.dirty = true;
                    }
                    if ui.small_button("+ Add State").clicked() {
                        let img = editor.screen.elements[sel_idx].as_image_mut().unwrap();
                        let new_key = format!("state{}", img.states.len());
                        img.states.insert(
                            new_key,
                            ElementState {
                                texture: String::new(),
                                condition: String::new(),
                                transparent_color: String::new(),
                            },
                        );
                        editor.dirty = true;
                    }
                });

                // Actions
                let on_click = editor.screen.elements[sel_idx].as_image().unwrap().on_click.clone();
                let on_hover = editor.screen.elements[sel_idx].as_image().unwrap().on_hover.clone();
                ui.collapsing("On Click", |ui| {
                    let mut actions = on_click;
                    let mut dirty = false;
                    action_list_editor(ui, &mut actions, &mut dirty);
                    if dirty {
                        editor.screen.elements[sel_idx].as_image_mut().unwrap().on_click = actions;
                        editor.dirty = true;
                    }
                });
                ui.collapsing("On Hover", |ui| {
                    let mut actions = on_hover;
                    let mut dirty = false;
                    action_list_editor(ui, &mut actions, &mut dirty);
                    if dirty {
                        editor.screen.elements[sel_idx].as_image_mut().unwrap().on_hover = actions;
                        editor.dirty = true;
                    }
                });
            }

            // -- Video fields --
            if let Some(_vid) = editor.screen.elements[sel_idx].as_video() {
                ui.separator();

                let mut video_name = editor.screen.elements[sel_idx].as_video().unwrap().video.clone();
                ui.horizontal(|ui| {
                    ui.label("Video:");
                    if ui.text_edit_singleline(&mut video_name).changed() {
                        editor.screen.elements[sel_idx].as_video_mut().unwrap().video = video_name;
                        editor.dirty = true;
                    }
                });
                if !editor.screen.elements[sel_idx].as_video().unwrap().video.is_empty() {
                    let mut vid_loop = editor.screen.elements[sel_idx].as_video().unwrap().looping;
                    let mut skippable = editor.screen.elements[sel_idx].as_video().unwrap().skippable;
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut vid_loop, "Loop").changed() {
                            changed = true;
                        }
                        if ui.checkbox(&mut skippable, "Skippable").changed() {
                            changed = true;
                        }
                    });
                    if changed {
                        let vid = editor.screen.elements[sel_idx].as_video_mut().unwrap();
                        vid.looping = vid_loop;
                        vid.skippable = skippable;
                        editor.dirty = true;
                    }
                }

                let on_end = editor.screen.elements[sel_idx].as_video().unwrap().on_end.clone();
                ui.collapsing("On End", |ui| {
                    let mut actions = on_end;
                    let mut dirty = false;
                    action_list_editor(ui, &mut actions, &mut dirty);
                    if dirty {
                        editor.screen.elements[sel_idx].as_video_mut().unwrap().on_end = actions;
                        editor.dirty = true;
                    }
                });
            }

            // -- Text fields --
            if let Some(txt) = editor.screen.elements[sel_idx].as_text() {
                ui.separator();

                let mut source = txt.source.clone();
                let mut font = txt.font.clone();
                let mut color = txt.color.clone();
                let mut align = txt.align.clone();
                let mut changed = false;

                ui.horizontal(|ui| {
                    ui.label("Source:");
                    egui::ComboBox::from_id_salt("txt_source")
                        .selected_text(&source)
                        .show_ui(ui, |ui| {
                            for &s in crate::screens::TEXT_SOURCES {
                                if ui.selectable_label(source == s, s).clicked() {
                                    source = s.to_string();
                                    changed = true;
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Font:");
                    if ui.text_edit_singleline(&mut font).changed() {
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Color:");
                    egui::ComboBox::from_id_salt("txt_color")
                        .selected_text(&color)
                        .show_ui(ui, |ui| {
                            for &c in crate::screens::TEXT_COLORS {
                                if ui.selectable_label(color == c, c).clicked() {
                                    color = c.to_string();
                                    changed = true;
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Align:");
                    egui::ComboBox::from_id_salt("txt_align")
                        .selected_text(&align)
                        .show_ui(ui, |ui| {
                            for &a in crate::screens::TEXT_ALIGNS {
                                if ui.selectable_label(align == a, a).clicked() {
                                    align = a.to_string();
                                    changed = true;
                                }
                            }
                        });
                });

                if changed {
                    let t = editor.screen.elements[sel_idx].as_text_mut().unwrap();
                    t.source = source;
                    t.font = font;
                    t.color = color;
                    t.align = align;
                    editor.dirty = true;
                }
            }

            // -- Debug --
            ui.separator();
            let elem = &editor.screen.elements[sel_idx];
            let (w, h) = elem.size();
            let (px, py) = elem.position();
            ui.weak(format!("{}[{w:.0},{h:.0}]@({px:.0},{py:.0})", elem.id()));
        });

    // Save window position when dragged.
    if let Some(inner) = resp {
        let pos = inner.response.rect.min;
        let new_pos = [pos.x, pos.y];
        let mut cfg = EditorConfig::load();
        if cfg.editor_pos != Some(new_pos) {
            cfg.editor_pos = Some(new_pos);
            cfg.save();
        }
    }
}

/// Reusable action list editor (add/remove/edit string actions).
fn action_list_editor(ui: &mut egui::Ui, actions: &mut Vec<String>, dirty: &mut bool) {
    let mut to_remove: Option<usize> = None;
    for i in 0..actions.len() {
        let mut action = actions[i].clone();
        ui.horizontal(|ui| {
            if ui.text_edit_singleline(&mut action).changed() {
                actions[i] = action;
                *dirty = true;
            }
            if ui.small_button("\u{2715}").clicked() {
                to_remove = Some(i);
            }
        });
    }
    if let Some(i) = to_remove {
        actions.remove(i);
        *dirty = true;
    }
    if ui.small_button("+ Add").clicked() {
        actions.push(String::new());
        *dirty = true;
    }
}
