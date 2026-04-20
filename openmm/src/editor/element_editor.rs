//! Element editor window — all properties for the selected element.
//!
//! Common fields (ID, pos, size, z) + type-specific fields (texture, text, video).

use bevy_inspector_egui::bevy_egui::egui;
use openmm_data::dsounds::DSounds;

use super::canvas::{EditorScreen, Selection};
use crate::game::optional::OptionalWrite;
use crate::screens::{ElementState, ScreenElement, TRANSPARENCY_OPTIONS};

/// Draw the element editor window for the selected element.
pub fn draw_element_editor(
    ctx: &egui::Context,
    editor: &mut EditorScreen,
    selection: &mut Selection,
    cfg: &mut super::io::EditorConfig,
    game_assets: &crate::assets::GameAssets,
    ui_sound: &mut Option<bevy::ecs::message::MessageWriter<crate::game::sound::effects::PlayUiSoundEvent>>,
) {
    let Some(sel) = selection.index else { return };
    if sel >= editor.screen.elements.len() {
        return;
    }

    let kind = match &editor.screen.elements[sel] {
        ScreenElement::Image(_) => "Image",
        ScreenElement::Video(_) => "Video",
        ScreenElement::Text(_) => "Text",
    };
    let elem_id = editor.screen.elements[sel].id().to_string();

    let mut open = true;
    let win_id = egui::Id::new("edt_editor");
    let mut win = egui::Window::new(format!("{} — {}", kind, elem_id))
        .id(win_id)
        .resizable(true)
        .collapsible(true)
        .default_width(320.0)
        .open(&mut open);
    if let Some([x, y]) = cfg.edt_pos {
        win = win.default_pos(egui::pos2(x, y));
    }
    win.show(ctx, |ui| {
        draw_common_fields(ui, editor, sel);
        ui.separator();

        match &editor.screen.elements[sel] {
            ScreenElement::Image(_) => {
                draw_image_textures(ui, editor, sel, game_assets, selection, ui_sound);
                draw_image_actions(ui, editor, sel);
                draw_image_bindings(ui, editor, sel);
            }
            ScreenElement::Video(_) => {
                draw_video_properties(ui, editor, sel);
                draw_video_actions(ui, editor, sel);
            }
            ScreenElement::Text(_) => {
                draw_text_fields(ui, editor, sel);
                draw_text_actions(ui, editor, sel);
            }
        }
    });
    if !open {
        selection.edt_open = false;
    }
    // Save position on drag.
    if let Some(rect) = ctx.memory(|m: &egui::Memory| m.area_rect(win_id)) {
        let pos = rect.left_top();
        let new_pos = [pos.x, pos.y];
        if cfg.edt_pos != Some(new_pos) {
            cfg.edt_pos = Some(new_pos);
            cfg.mark_dirty();
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
                        ui.label("Texture:");
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
                        ui.label("Condition:");
                        if ui.text_edit_singleline(&mut cond).changed() {
                            if let Some(state) = editor.screen.elements[sel].as_image_mut().unwrap().states.get_mut(key)
                            {
                                state.condition = cond;
                                editor.dirty = true;
                            }
                        }
                    });

                    // Animation fields
                    let img = editor.screen.elements[sel].as_image().unwrap();
                    let mut anim =
                        img.states
                            .get(key)
                            .and_then(|s| s.animation.clone())
                            .unwrap_or(crate::screens::Animation {
                                pattern: String::new(),
                                frames: 0,
                                start_frame: 1,
                                fps: 10.0,
                                ping_pong: false,
                            });
                    let mut anim_changed = false;

                    ui.collapsing("Animation", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Frames:");
                            if ui
                                .add(egui::DragValue::new(&mut anim.frames).speed(1.0).range(0..=256))
                                .changed()
                            {
                                anim_changed = true;
                            }
                            ui.label("FPS:");
                            if ui
                                .add(egui::DragValue::new(&mut anim.fps).speed(0.1).range(0.1..=60.0))
                                .changed()
                            {
                                anim_changed = true;
                            }
                            ui.label("Start:");
                            if ui
                                .add(egui::DragValue::new(&mut anim.start_frame).range(0..=256))
                                .changed()
                            {
                                anim_changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Pattern:");
                            if ui
                                .add(egui::TextEdit::singleline(&mut anim.pattern).hint_text("icons/name%02d"))
                                .changed()
                            {
                                anim_changed = true;
                            }
                            if ui.checkbox(&mut anim.ping_pong, "Ping Pong").changed() {
                                anim_changed = true;
                            }
                        });
                    });

                    if anim_changed {
                        if let Some(state) = editor.screen.elements[sel].as_image_mut().unwrap().states.get_mut(key) {
                            if anim.frames > 0 {
                                state.animation = Some(anim);
                            } else {
                                state.animation = None;
                            }
                            editor.dirty = true;
                        }
                    }
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
                        animation: None,
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

/// Common element fields: ID, position, size, z-order.
fn draw_common_fields(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let mut elem_id = editor.screen.elements[sel].id().to_string();
    let mut pos = editor.screen.elements[sel].position();
    let mut size = editor.screen.elements[sel].size();
    let mut z = editor.screen.elements[sel].z();

    let mut pos_changed = false;
    let mut size_changed = false;
    let mut z_changed = false;
    let mut crop_changed = false;

    ui.horizontal_wrapped(|ui| {
        ui.label("ID:");
        if ui.text_edit_singleline(&mut elem_id).changed() {
            match &mut editor.screen.elements[sel] {
                ScreenElement::Image(img) => img.id = elem_id.clone(),
                ScreenElement::Video(vid) => vid.id = elem_id.clone(),
                ScreenElement::Text(txt) => txt.id = elem_id.clone(),
            }
            editor.dirty = true;
        }

        ui.separator();
        ui.label("Pos:");
        pos_changed |= ui
            .add(egui::DragValue::new(&mut pos.0).prefix("x:").speed(1.0))
            .changed();
        pos_changed |= ui
            .add(egui::DragValue::new(&mut pos.1).prefix("y:").speed(1.0))
            .changed();

        ui.separator();
        ui.label("Size:");
        size_changed |= ui
            .add(egui::DragValue::new(&mut size.0).prefix("w:").speed(1.0))
            .changed();
        size_changed |= ui
            .add(egui::DragValue::new(&mut size.1).prefix("h:").speed(1.0))
            .changed();

        ui.separator();
        ui.label("Z:");
        z_changed |= ui.add(egui::DragValue::new(&mut z).speed(1.0)).changed();

        if let Some(img) = editor.screen.elements[sel].as_image_mut()
            && img.size.0 > 0.0
            && img.size.1 > 0.0
        {
            ui.separator();
            crop_changed |= ui.checkbox(&mut img.crop, "Crop").changed();
        }
    });

    if pos_changed {
        editor.screen.elements[sel].set_position(pos);
        editor.dirty = true;
    }
    if size_changed {
        editor.screen.elements[sel].set_size(size);
        editor.dirty = true;
    }
    if z_changed {
        editor.screen.elements[sel].set_z(z);
        editor.dirty = true;
    }
    if crop_changed {
        editor.dirty = true;
    }
}

fn draw_click_sound_suggestions(
    ui: &mut egui::Ui,
    current: &str,
    dsounds: Option<&DSounds>,
    selection: &mut Selection,
    ui_sound: &mut Option<bevy::ecs::message::MessageWriter<crate::game::sound::effects::PlayUiSoundEvent>>,
) -> Option<String> {
    let dsounds = dsounds?;

    let query = current.trim();
    if query.is_empty() || query.starts_with('#') {
        return None;
    }

    let query_lower = query.to_ascii_lowercase();
    let mut selected: Option<String> = None;
    let mut shown = 0usize;
    let mut hovered_sound_id: Option<u32> = None;
    let mut clicked_sound_id: Option<u32> = None;

    ui.small("Click Sound matches:");
    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
        for info in &dsounds.items {
            if shown >= 10 {
                break;
            }

            let Some(name) = info.name() else {
                continue;
            };

            if !name.to_ascii_lowercase().contains(&query_lower) {
                continue;
            }

            shown += 1;
            ui.horizontal(|ui| {
                let resp = ui.selectable_label(false, &name);
                if resp.clicked() {
                    selected = Some(name.clone());
                    clicked_sound_id = Some(info.sound_id);
                }
                if resp.hovered() {
                    hovered_sound_id = Some(info.sound_id);
                }
                ui.small(format!("#{}", info.sound_id));
            });
        }
    });

    // Click should always play, even if this row was already hovered previously.
    if let Some(sound_id) = clicked_sound_id {
        ui_sound.try_write(crate::game::sound::effects::PlayUiSoundEvent { sound_id });
        selection.click_sound_preview_id = Some(sound_id);
    }

    if let Some(sound_id) = hovered_sound_id {
        if selection.click_sound_preview_id != Some(sound_id) {
            ui_sound.try_write(crate::game::sound::effects::PlayUiSoundEvent { sound_id });
            selection.click_sound_preview_id = Some(sound_id);
        }
    } else {
        selection.click_sound_preview_id = None;
    }

    if shown == 0 {
        ui.small("No matching sounds");
        selection.click_sound_preview_id = None;
    }

    selected
}

/// Image texture fields: default texture, clicked texture, all states.
fn draw_image_textures(
    ui: &mut egui::Ui,
    editor: &mut EditorScreen,
    sel: usize,
    game_assets: &crate::assets::GameAssets,
    selection: &mut Selection,
    ui_sound: &mut Option<bevy::ecs::message::MessageWriter<crate::game::sound::effects::PlayUiSoundEvent>>,
) {
    let Some(_img) = editor.screen.elements[sel].as_image() else {
        return;
    };

    ui.heading("Base");

    // Default texture (may not exist — e.g. image only has a clicked state)
    let mut tex = editor.screen.elements[sel]
        .as_image()
        .unwrap()
        .states
        .get("default")
        .map(|s| s.texture.clone())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Default Texture:");
        if ui.text_edit_singleline(&mut tex).changed() {
            let img = editor.screen.elements[sel].as_image_mut().unwrap();
            if tex.is_empty() {
                img.states.remove("default");
            } else {
                img.states
                    .entry("default".to_string())
                    .and_modify(|s| s.texture = tex.clone())
                    .or_insert(ElementState {
                        texture: tex,
                        condition: String::new(),
                        transparent_color: String::new(),
                        animation: None,
                    });
            }
            editor.dirty = true;
        }
    });

    draw_image_transparency(ui, editor, sel);

    // --- Base Animation ---
    let mut anim = editor.screen.elements[sel]
        .as_image()
        .unwrap()
        .animation
        .clone()
        .unwrap_or_else(|| crate::screens::Animation {
            frames: 0,
            pattern: String::new(),
            start_frame: 1,
            fps: 10.0,
            ping_pong: false,
        });
    let mut anim_changed = false;

    egui::CollapsingHeader::new(format!(
        "Base Animation ({})",
        if anim.frames > 0 { "Active" } else { "None" }
    ))
    .id_salt("base_anim_collapsing")
    .show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("Frames:");
            if ui.add(egui::DragValue::new(&mut anim.frames).range(0..=256)).changed() {
                anim_changed = true;
            }
            ui.label(" (0=off)");
        });

        if anim.frames > 0 {
            ui.horizontal(|ui| {
                ui.label("Pattern:");
                if ui
                    .add(egui::TextEdit::singleline(&mut anim.pattern).hint_text("icons/name%02d"))
                    .changed()
                {
                    anim_changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("FPS:");
                if ui.add(egui::DragValue::new(&mut anim.fps).range(0.1..=60.0)).changed() {
                    anim_changed = true;
                }
                ui.label("Start:");
                if ui
                    .add(egui::DragValue::new(&mut anim.start_frame).range(0..=256))
                    .changed()
                {
                    anim_changed = true;
                }
                if ui.checkbox(&mut anim.ping_pong, "Ping Pong").changed() {
                    anim_changed = true;
                }
            });
        }
    });

    if anim_changed {
        let img = editor.screen.elements[sel].as_image_mut().unwrap();
        if anim.frames == 0 {
            img.animation = None;
        } else {
            img.animation = Some(anim);
        }
        editor.dirty = true;
    }

    ui.separator();
    ui.heading("Hover");

    // Hover texture
    let mut hover_tex = editor.screen.elements[sel]
        .as_image()
        .unwrap()
        .states
        .get("hover_texture")
        .map(|s| s.texture.clone())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Hover Texture:");
        if ui.text_edit_singleline(&mut hover_tex).changed() {
            let img = editor.screen.elements[sel].as_image_mut().unwrap();
            if hover_tex.is_empty() {
                img.states.remove("hover_texture");
            } else {
                img.states
                    .entry("hover_texture".to_string())
                    .and_modify(|s| s.texture = hover_tex.clone())
                    .or_insert(ElementState {
                        texture: hover_tex,
                        condition: String::new(),
                        transparent_color: String::new(),
                        animation: None,
                    });
            }
            editor.dirty = true;
        }
    });

    // Hover animation
    let mut has_anim = editor.screen.elements[sel]
        .as_image()
        .unwrap()
        .states
        .get("hover_texture")
        .and_then(|s| s.animation.as_ref())
        .is_some();
    if ui.checkbox(&mut has_anim, "Hover Animation").changed() {
        let img = editor.screen.elements[sel].as_image_mut().unwrap();
        let state = img.states.entry("hover_texture".to_string()).or_insert(ElementState {
            texture: String::new(),
            condition: String::new(),
            transparent_color: String::new(),
            animation: None,
        });
        if has_anim {
            state.animation = Some(crate::screens::Animation {
                pattern: "icons/Watwalk%02d".into(),
                frames: 3,
                start_frame: 1,
                fps: 8.0,
                ping_pong: false,
            });
        } else {
            state.animation = None;
        }
        editor.dirty = true;
    }

    if has_anim {
        ui.indent("hover_anim", |ui| {
            let img = editor.screen.elements[sel].as_image_mut().unwrap();
            let state = img.states.get_mut("hover_texture").unwrap();
            let anim = state.animation.as_mut().unwrap();
            ui.horizontal(|ui| {
                ui.label("Pattern:");
                if ui.text_edit_singleline(&mut anim.pattern).changed() {
                    editor.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Frames:");
                if ui.add(egui::DragValue::new(&mut anim.frames).range(1..=100)).changed() {
                    editor.dirty = true;
                }
                ui.label("FPS:");
                if ui.add(egui::DragValue::new(&mut anim.fps).range(0.1..=60.0)).changed() {
                    editor.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                if ui.checkbox(&mut anim.ping_pong, "Ping Pong").changed() {
                    editor.dirty = true;
                }
            });
        });
    }

    ui.separator();
    ui.heading("Click");

    // Clicked texture
    let mut clicked_tex = editor.screen.elements[sel]
        .as_image()
        .unwrap()
        .states
        .get("clicked")
        .map(|s| s.texture.clone())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Clicked Texture:");
        if ui.text_edit_singleline(&mut clicked_tex).changed() {
            let img = editor.screen.elements[sel].as_image_mut().unwrap();
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
                        animation: None,
                    });
            }
            editor.dirty = true;
        }
    });

    ui.separator();
    ui.heading("Sound");
    if let Some(img) = editor.screen.elements[sel].as_image_mut() {
        ui.horizontal(|ui| {
            ui.label("Click Sound:");
            if ui.text_edit_singleline(&mut img.click_sound).changed() {
                editor.dirty = true;
            }
        });
        ui.small("Use name (ClickStart) or #id (e.g. #42)");

        if let Some(selected) =
            draw_click_sound_suggestions(ui, &img.click_sound, game_assets.dsounds(), selection, ui_sound)
        {
            img.click_sound = selected;
            editor.dirty = true;
        }
    }

    ui.separator();
    ui.heading("Advanced");

    // Raw custom states (advanced)
    let state_keys: Vec<String> = editor.screen.elements[sel]
        .as_image()
        .unwrap()
        .states
        .keys()
        .filter(|k| {
            let k = k.as_str();
            k != "default" && k != "clicked" && k != "hover_texture"
        })
        .cloned()
        .collect();
    ui.collapsing("Custom States", |ui| {
        let mut to_remove: Option<String> = None;
        if state_keys.is_empty() {
            ui.small("No custom states");
        }
        for key in &state_keys {
            let mut tex = editor.screen.elements[sel]
                .as_image()
                .unwrap()
                .states
                .get(key)
                .map(|s| s.texture.clone())
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(key.as_str());
                if ui.text_edit_singleline(&mut tex).changed() {
                    if let Some(state) = editor.screen.elements[sel].as_image_mut().unwrap().states.get_mut(key) {
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
            editor.screen.elements[sel].as_image_mut().unwrap().states.remove(&k);
            editor.dirty = true;
        }
        if ui.small_button("+ Add State").clicked() {
            let img = editor.screen.elements[sel].as_image_mut().unwrap();
            let new_key = format!("state{}", img.states.len());
            img.states.insert(
                new_key,
                ElementState {
                    texture: String::new(),
                    condition: String::new(),
                    transparent_color: String::new(),
                    animation: None,
                },
            );
            editor.dirty = true;
        }
    });
}

/// Text element fields: source, font, color, alignment.
fn draw_text_fields(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let Some(txt) = editor.screen.elements[sel].as_text() else {
        return;
    };

    let mut source = txt.source.clone();
    let mut value = txt.value.clone();
    let mut font = txt.font.clone();
    let mut font_size = txt.font_size;
    let mut color = txt.color.clone();
    let mut align = txt.align.clone();
    let mut changed = false;

    ui.heading("Text");

    ui.horizontal(|ui| {
        ui.label("Value:");
        if ui.text_edit_multiline(&mut value).changed() {
            changed = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Source:");
        if ui.text_edit_singleline(&mut source).changed() {
            changed = true;
        }
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

    ui.separator();
    ui.heading("Style");

    ui.horizontal(|ui| {
        ui.label("Font:");
        if ui.text_edit_singleline(&mut font).changed() {
            changed = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("Font Size:");
        if ui
            .add(
                egui::DragValue::new(&mut font_size)
                    .speed(0.5)
                    .range(0.0..=256.0)
                    .suffix(" px (0=box)"),
            )
            .changed()
        {
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
        let t = editor.screen.elements[sel].as_text_mut().unwrap();
        t.source = source;
        t.value = value;
        t.font = font;
        t.font_size = font_size;
        t.color = color;
        t.align = align;
        editor.dirty = true;
    }

    let mut hover_color = editor.screen.elements[sel]
        .as_text()
        .unwrap()
        .hover_color
        .clone()
        .unwrap_or_else(|| "none".to_string());
    let mut hover_changed = false;

    ui.horizontal(|ui| {
        ui.label("Hover Color:");
        egui::ComboBox::from_id_salt("txt_hover_color")
            .selected_text(&hover_color)
            .show_ui(ui, |ui| {
                if ui.selectable_label(hover_color == "none", "none").clicked() {
                    hover_color = "none".to_string();
                    hover_changed = true;
                }
                for &c in crate::screens::TEXT_COLORS {
                    if ui.selectable_label(hover_color == c, c).clicked() {
                        hover_color = c.to_string();
                        hover_changed = true;
                    }
                }
            });
    });

    if hover_changed {
        let t = editor.screen.elements[sel].as_text_mut().unwrap();
        t.hover_color = if hover_color == "none" { None } else { Some(hover_color) };
        editor.dirty = true;
    }
}

fn draw_video_properties(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let Some(vid) = editor.screen.elements[sel].as_video() else {
        return;
    };

    ui.heading("Video");

    let mut video_name = vid.video.clone();
    ui.horizontal(|ui| {
        ui.label("Video:");
        if ui.text_edit_singleline(&mut video_name).changed() {
            editor.screen.elements[sel].as_video_mut().unwrap().video = video_name;
            editor.dirty = true;
        }
    });

    ui.separator();
    ui.heading("Playback");

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
}

fn draw_image_actions(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let Some(_) = editor.screen.elements[sel].as_image() else {
        return;
    };

    ui.heading("On Click");
    let mut clicks = editor.screen.elements[sel].as_image().unwrap().on_click.clone();
    let mut dirty = false;
    super::editor_panel::action_list_editor(ui, &mut clicks, &mut dirty);
    if dirty {
        editor.screen.elements[sel].as_image_mut().unwrap().on_click = clicks;
        editor.dirty = true;
    }

    ui.separator();

    ui.heading("On Hover");
    let mut hovers = editor.screen.elements[sel].as_image().unwrap().on_hover.clone();
    let mut dirty = false;
    super::editor_panel::action_list_editor(ui, &mut hovers, &mut dirty);
    if dirty {
        editor.screen.elements[sel].as_image_mut().unwrap().on_hover = hovers;
        editor.dirty = true;
    }
}

fn draw_video_actions(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    if editor.screen.elements[sel].as_video().is_none() {
        return;
    }

    ui.separator();
    ui.heading("Actions");
    ui.heading("On End");
    let mut ends = editor.screen.elements[sel].as_video().unwrap().on_end.clone();
    let mut dirty = false;
    super::editor_panel::action_list_editor(ui, &mut ends, &mut dirty);
    if dirty {
        editor.screen.elements[sel].as_video_mut().unwrap().on_end = ends;
        editor.dirty = true;
    }
}

fn draw_text_actions(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    let Some(_) = editor.screen.elements[sel].as_text() else {
        return;
    };

    ui.separator();
    ui.heading("Actions");
    ui.heading("On Click");
    let mut clicks = editor.screen.elements[sel].as_text().unwrap().on_click.clone();
    let mut dirty = false;
    super::editor_panel::action_list_editor(ui, &mut clicks, &mut dirty);
    if dirty {
        editor.screen.elements[sel].as_text_mut().unwrap().on_click = clicks;
        editor.dirty = true;
    }

    ui.separator();

    ui.heading("On Hover");
    let mut hovers = editor.screen.elements[sel].as_text().unwrap().on_hover.clone();
    let mut dirty = false;
    super::editor_panel::action_list_editor(ui, &mut hovers, &mut dirty);
    if dirty {
        editor.screen.elements[sel].as_text_mut().unwrap().on_hover = hovers;
        editor.dirty = true;
    }
}

fn draw_image_bindings(ui: &mut egui::Ui, editor: &mut EditorScreen, sel: usize) {
    if editor.screen.elements[sel].as_image().is_none() {
        return;
    }

    ui.separator();
    ui.heading("Bindings");
    ui.small(
        "binding key \u{2192} value (e.g. scroll_x \u{2192} player.compass_yaw, texture \u{2192} ${member0.portrait})",
    );
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
