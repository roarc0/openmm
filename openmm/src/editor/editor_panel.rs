//! Editor panel — screen-level properties only.
//!
//! Layout:
//!   Screen section — ID, sound, on_load, + Add Text / + Add Image
//!   Guides section — collapsible guide line manager
//!
//! Element editing lives in `element_editor.rs` (per-element Edit window).

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

use super::canvas::EditorScreen;
use super::guides::Guides;
use super::io::EditorConfig;
use crate::game::controls::{get_key_name, parse_key_code};

pub fn editor_panel_ui(
    mut contexts: EguiContexts,
    mut editor: ResMut<EditorScreen>,
    mut guides: ResMut<Guides>,
    mut cfg: ResMut<EditorConfig>,
    mut new_key_name: Local<String>,
    mut new_key_action: Local<String>,
    mut key_feedback: Local<String>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

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

            ui.horizontal(|ui| {
                ui.label("Kind:");
                let current = format!("{:?}", editor.screen.kind);
                egui::ComboBox::from_id_salt("screen_kind")
                    .selected_text(&current)
                    .show_ui(ui, |ui| {
                        use crate::screens::ScreenKind;
                        for kind in [ScreenKind::Base, ScreenKind::Hud, ScreenKind::Modal] {
                            if ui
                                .selectable_label(editor.screen.kind == kind, format!("{:?}", kind))
                                .clicked()
                            {
                                editor.screen.kind = kind;
                                editor.dirty = true;
                            }
                        }
                    });
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
                    .add(egui::DragValue::new(&mut start_sec).speed(0.1).range(0.0..=3600.0))
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

            ui.collapsing("On Close", |ui| {
                let mut actions = editor.screen.on_close.clone();
                let mut dirty = false;
                action_list_editor(ui, &mut actions, &mut dirty);
                if dirty {
                    editor.screen.on_close = actions;
                    editor.dirty = true;
                }
            });

            ui.collapsing("Keys", |ui| {
                let mut keys_to_remove: Option<String> = None;
                let mut keys_to_rename: Option<(String, String)> = None;

                ui.small("Use canonical names (Escape, Enter, Up, F1..F11, A..Z, 0..9). Aliases like Esc/Return are accepted.");

                ui.horizontal_wrapped(|ui| {
                    ui.small("Quick add:");
                    for quick in ["Escape", "Enter", "Space", "Tab", "Up", "Down", "Left", "Right"] {
                        if ui.small_button(quick).clicked() {
                            if !editor.screen.keys.contains_key(quick) {
                                editor.screen.keys.insert(quick.to_string(), vec![String::new()]);
                                editor.dirty = true;
                                *key_feedback = format!("Added key '{}'", quick);
                            } else {
                                *key_feedback = format!("Key '{}' already exists", quick);
                            }
                        }
                    }
                });

                if !key_feedback.is_empty() {
                    ui.label(egui::RichText::new(key_feedback.as_str()).color(egui::Color32::from_rgb(220, 220, 120)));
                }
                ui.separator();

                let keys_list: Vec<String> = editor.screen.keys.keys().cloned().collect();
                for key in keys_list {
                    let mut actions = editor.screen.keys.get(&key).unwrap().clone();
                    let mut actions_dirty = false;

                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let mut current_key = key.clone();
                            let response = ui.text_edit_singleline(&mut current_key);
                            let commit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if commit {
                                keys_to_rename = Some((key.clone(), current_key));
                            }

                            if ui.small_button("\u{2715}").on_hover_text("Remove Shortcut").clicked() {
                                keys_to_remove = Some(key.clone());
                            }
                        });

                        ui.small("Actions for this key:");
                        action_list_editor(ui, &mut actions, &mut actions_dirty);
                        if actions_dirty {
                            editor.screen.keys.insert(key, actions);
                            editor.dirty = true;
                        }
                    });
                }

                if let Some(k) = keys_to_remove {
                    editor.screen.keys.remove(&k);
                    editor.dirty = true;
                }
                if let Some((old, new)) = keys_to_rename {
                    if let Some(canonical) = canonical_key_name(&new) {
                        if old != canonical
                            && !editor.screen.keys.contains_key(&canonical)
                            && let Some(actions) = editor.screen.keys.remove(&old)
                        {
                            editor.screen.keys.insert(canonical.clone(), actions);
                            editor.dirty = true;
                            *key_feedback = format!("Renamed '{}' -> '{}'", old, canonical);
                        }
                    } else {
                        *key_feedback = format!("Unknown key '{}'. Try Escape, Enter, arrows, F1..F11, A..Z, 0..9", new.trim());
                    }
                }

                ui.label("Add key with first action:");
                ui.horizontal(|ui| {
                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let key_commit = ui
                        .add(egui::TextEdit::singleline(&mut *new_key_name).hint_text("Key (e.g. Escape)"))
                        .lost_focus()
                        && enter_pressed;
                    let action_commit = ui
                        .add(egui::TextEdit::singleline(&mut *new_key_action).hint_text("Action (e.g. LoadScreen(\"menu\"))"))
                        .lost_focus()
                        && enter_pressed;

                    if key_commit || action_commit || ui.button("+ Add Key").clicked() {
                        add_key_binding(&mut editor, &mut new_key_name, &mut new_key_action, &mut key_feedback);
                    }
                });
            });

            if ui.small_button("+ Add Text").clicked() {
                let max_z = editor.screen.max_z();
                editor
                    .screen
                    .elements
                    .push(crate::screens::ScreenElement::Text(crate::screens::TextElement {
                        id: "new_text".to_string(),
                        position: (crate::screens::REF_W / 2.0, crate::screens::REF_H / 2.0),
                        size: (200.0, 12.0),
                        z: max_z + 1,
                        hidden: false,
                        source: String::new(),
                        value: "New Text".to_string(),
                        font: "smallnum".to_string(),
                        font_size: 14.0,
                        color: "white".to_string(),
                        hover_color: None,
                        align: "center".to_string(),
                        on_click: vec![],
                        on_hover: vec![],
                    }));
                editor.dirty = true;
            }

            // ━━ Guides ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
            ui.separator();
            ui.collapsing("Guides", |ui| {
                super::guides::guides_section(ui, &mut guides, &mut cfg);
            });
        });

    // Save window position when dragged.
    if let Some(inner) = resp {
        let pos = inner.response.rect.min;
        let new_pos = [pos.x, pos.y];
        if cfg.editor_pos != Some(new_pos) {
            cfg.editor_pos = Some(new_pos);
            cfg.mark_dirty();
        }
    }
}

fn canonical_key_name(raw: &str) -> Option<String> {
    let name = parse_key_code(raw).and_then(get_key_name)?;
    if name == "F12" {
        return None;
    }
    Some(name.to_string())
}

fn add_key_binding(
    editor: &mut EditorScreen,
    new_key_name: &mut String,
    new_key_action: &mut String,
    key_feedback: &mut String,
) {
    let raw = new_key_name.trim();
    if raw.is_empty() {
        return;
    }

    let Some(canonical) = canonical_key_name(raw) else {
        *key_feedback = format!("Unknown key '{}'. Try Escape, Enter, arrows, F1..F11, A..Z, 0..9", raw);
        return;
    };

    if editor.screen.keys.contains_key(&canonical) {
        *key_feedback = format!("Key '{}' already exists", canonical);
        return;
    }

    let first_action = new_key_action.trim();
    let actions = if first_action.is_empty() {
        vec![String::new()]
    } else {
        vec![first_action.to_string()]
    };

    editor.screen.keys.insert(canonical.clone(), actions);
    editor.dirty = true;
    *new_key_name = String::new();
    *new_key_action = String::new();
    *key_feedback = format!("Added key '{}'", canonical);
}

/// Reusable action list editor (add/remove/edit string actions).
pub fn action_list_editor(ui: &mut egui::Ui, actions: &mut Vec<String>, dirty: &mut bool) {
    let presets = [
        "LoadScreen(\"\")",
        "ShowScreen(\"\")",
        "HideScreen(\"\")",
        "ShowSprite(\"\")",
        "HideSprite(\"\")",
        "CloseWindow()",
        "PlaySoundNamed(\"\")",
        "Quit()",
        "EnterTurnBattle()",
        "NewGame()",
        "GreetingSound()",
        "SaveConfig(\"\", \"\")",
        "Compare(\"\")",
        "Else()",
        "End()",
        "evt:",
    ];

    let mut to_remove: Option<usize> = None;
    for i in 0..actions.len() {
        let mut action = actions[i].clone();
        ui.horizontal(|ui| {
            if ui
                .add(egui::TextEdit::singleline(&mut action).hint_text("Event/action, e.g. LoadScreen(\"menu\")"))
                .changed()
            {
                actions[i] = action.clone();
                *dirty = true;
            }

            egui::ComboBox::from_id_salt(ui.make_persistent_id(i))
                .width(16.0)
                .selected_text("")
                .show_ui(ui, |ui| {
                    for preset in presets {
                        if ui.selectable_label(false, preset).clicked() {
                            actions[i] = preset.to_string();
                            *dirty = true;
                        }
                    }
                });

            if ui.small_button("\u{2715}").clicked() {
                to_remove = Some(i);
            }
        });
    }
    if let Some(i) = to_remove {
        actions.remove(i);
        *dirty = true;
    }
    if ui.small_button("+ Add Action").clicked() {
        actions.push(String::new());
        *dirty = true;
    }
}
