//! egui bitmap browser panel: LOD folder navigation with search and click-to-place.

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

use super::canvas::EditorScreen;
use openmm_data::Archive;
use openmm_data::assets::SmkArchive;

use crate::screens::{ImageElement, REF_H, REF_W, ScreenElement, VideoElement};
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::hud::UiAssets;

/// Filter to LOD image archives and VID video archives (exclude sound archives).
fn browsable_archive_names(assets: &openmm_data::Assets, all: Vec<String>) -> Vec<String> {
    all.into_iter()
        .filter(|name| {
            assets.files_in(name).is_some_and(|f| !f.is_empty())
        })
        .filter(|name| {
            let lower = name.to_lowercase();
            !lower.contains("snd")
        })
        .collect()
}

/// True if the archive name looks like a VID/SMK video archive.
fn is_video_archive(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("anims") || lower.contains("vid") || lower.contains("smk")
}

/// One archive and its file list.
struct LodFolder {
    name: String,
    files: Vec<String>,
    is_video: bool,
}

/// Browser panel state.
#[derive(Resource, Default)]
pub struct BrowserState {
    pub open: bool,
    pub search: String,
    folders: Vec<LodFolder>,
    current_folder: Option<usize>,
    filtered: Vec<String>,
    default_pos: Option<[f32; 2]>,
    /// Currently hovered file for preview.
    pub hovered_file: Option<String>,
    /// Cached preview egui texture id.
    preview_tex: Option<(String, egui::TextureId, (u32, u32))>,
    initialized: bool,
}

/// Load LOD archive structure once and restore browser state from config.
pub fn init_browser(game_assets: Res<GameAssets>, mut browser: ResMut<BrowserState>) {
    if browser.initialized {
        return;
    }
    browser.initialized = true;

    let assets = game_assets.assets();
    let mut archive_names = assets.archives();
    archive_names.sort();

    let lod_archives: Vec<String> = browsable_archive_names(&assets, archive_names);

    for archive in &lod_archives {
        let mut files = assets.files_in(archive).unwrap_or_default();
        files.sort();
        let prefixed: Vec<String> = files.into_iter().map(|f| format!("{}/{}", archive, f)).collect();
        browser.folders.push(LodFolder {
            name: archive.clone(),
            files: prefixed,
            is_video: false,
        });
    }

    // Discover VID archives in the Anims sibling directory.
    let data_path = openmm_data::get_data_path();
    let base = std::path::Path::new(&data_path);
    let parent = base.parent().unwrap_or(base);
    if let Some(anims_dir) = openmm_data::utils::find_path_case_insensitive(parent, "Anims") {
        for vid_name in &["Anims1.vid", "Anims2.vid"] {
            if let Some(path) = openmm_data::utils::find_path_case_insensitive(&anims_dir, vid_name) {
                if let Ok(vid) = SmkArchive::open(&path) {
                    let mut files: Vec<String> = vid.list_files().iter().map(|e| e.name.clone()).collect();
                    files.sort();
                    let folder_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or(vid_name).to_string();
                    let prefixed: Vec<String> = files.into_iter().map(|f| format!("{}/{}", folder_name, f)).collect();
                    browser.folders.push(LodFolder {
                        name: folder_name,
                        files: prefixed,
                        is_video: true,
                    });
                }
            }
        }
    }

    browser.filtered.clear();

    let cfg = super::io::EditorConfig::load();
    browser.open = cfg.browser_open;
    browser.default_pos = cfg.browser_pos;
}

/// F2 toggles the browser open/closed.
pub fn toggle_browser(keys: Res<ButtonInput<KeyCode>>, mut browser: ResMut<BrowserState>) {
    if keys.just_pressed(KeyCode::F2) {
        browser.open = !browser.open;
        let mut cfg = super::io::EditorConfig::load();
        cfg.browser_open = browser.open;
        cfg.save();
    }
}

/// Draw the browser egui window.
pub fn browser_ui(
    mut contexts: EguiContexts,
    mut browser: ResMut<BrowserState>,
    mut editor: ResMut<EditorScreen>,
    mut ui_assets: ResMut<UiAssets>,
    game_assets: Res<GameAssets>,
    mut images: ResMut<Assets<Image>>,
    cfg: Res<GameConfig>,
) {
    if !browser.open {
        return;
    }

    // Pre-load preview for the last hovered file (before borrowing ctx).
    if let Some(ref hovered) = browser.hovered_file {
        let need_load = browser
            .preview_tex
            .as_ref()
            .map_or(true, |(name, _, _)| name != hovered);
        if need_load {
            let bare = hovered.split('/').last().unwrap_or(hovered);
            let handle = ui_assets
                .get_or_load(hovered, &game_assets, &mut images, &cfg)
                .or_else(|| ui_assets.get_or_load(bare, &game_assets, &mut images, &cfg));
            if let Some(h) = handle {
                let dims = ui_assets
                    .dimensions(hovered)
                    .or_else(|| ui_assets.dimensions(bare))
                    .unwrap_or((0, 0));
                let tex_id = contexts.image_id(&h).unwrap_or_else(|| {
                    contexts.add_image(bevy_inspector_egui::bevy_egui::EguiTextureHandle::Weak(h.id()))
                });
                browser.preview_tex = Some((hovered.clone(), tex_id, dims));
            } else {
                browser.preview_tex = None;
            }
        }
    } else {
        browser.preview_tex = None;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };
    let mut win = egui::Window::new("LOD Browser")
        .id(egui::Id::new("lod_browser"))
        .resizable(true)
        .default_width(480.0);
    if let Some([x, y]) = browser.default_pos.take() {
        win = win.default_pos(egui::pos2(x, y));
    }
    let response = win.show(ctx, |ui| {
        match browser.current_folder {
            None => {
                ui.heading("LOD Archives");
                ui.separator();
                let folder_count = browser.folders.len();
                for i in 0..folder_count {
                    let tag = if browser.folders[i].is_video { " [VID]" } else { "" };
                    let label = format!(
                        "{}/  ({} files){}",
                        browser.folders[i].name,
                        browser.folders[i].files.len(),
                        tag,
                    );
                    if ui.button(&label).clicked() {
                        browser.current_folder = Some(i);
                        browser.search.clear();
                        browser.filtered = browser.folders[i].files.clone();
                    }
                }
            }
            Some(folder_idx) => {
                let folder_name = browser.folders[folder_idx].name.clone();
                let folder_is_video = browser.folders[folder_idx].is_video;

                ui.horizontal(|ui| {
                    if ui.button("<- Back").clicked() {
                        browser.current_folder = None;
                        browser.search.clear();
                        browser.filtered.clear();
                    }
                    ui.strong(&folder_name);
                });

                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.text_edit_singleline(&mut browser.search);
                });

                let needle = browser.search.to_lowercase();
                browser.filtered = if needle.is_empty() {
                    browser.folders[folder_idx].files.clone()
                } else {
                    browser.folders[folder_idx]
                        .files
                        .iter()
                        .filter(|n| n.to_lowercase().contains(&needle))
                        .cloned()
                        .collect()
                };

                ui.label(format!("{} files", browser.filtered.len()));
                ui.separator();

                // File list (left) + preview (right).
                let mut new_hover: Option<String> = None;
                ui.horizontal_top(|ui| {
                    // Left: scrollable file list.
                    ui.vertical(|ui| {
                        ui.set_min_width(200.0);
                        egui::ScrollArea::vertical()
                            .id_salt("browser_files")
                            .max_height(600.0)
                            .show(ui, |ui| {
                                for full_name in &browser.filtered {
                                    let short = full_name
                                        .strip_prefix(&format!("{}/", folder_name))
                                        .unwrap_or(full_name);
                                    let resp = ui.button(short);
                                    if resp.clicked() {
                                        if folder_is_video {
                                            place_video_element(short, &mut editor);
                                        } else {
                                            place_element(
                                                full_name,
                                                &mut editor,
                                                &mut ui_assets,
                                                &game_assets,
                                                &mut images,
                                                &cfg,
                                            );
                                        }
                                    }
                                    if resp.hovered() {
                                        new_hover = Some(full_name.clone());
                                    }
                                }
                            });
                    });

                    ui.separator();

                    // Right: preview of hovered file.
                    ui.vertical(|ui| {
                        ui.set_min_width(200.0);
                        if let Some((ref name, tex_id, (pw, ph))) = browser.preview_tex {
                            if new_hover.is_some() {
                                let short = name.split('/').last().unwrap_or(name);
                                ui.label(short);
                                ui.weak(format!("{}x{}", pw, ph));
                                if pw > 0 && ph > 0 {
                                    let scale = (200.0 / pw as f32).min(1.0);
                                    let w = pw as f32 * scale;
                                    let h = ph as f32 * scale;
                                    ui.image(egui::load::SizedTexture::new(tex_id, egui::vec2(w, h)));
                                }
                            }
                        } else {
                            ui.weak("Hover a file to preview");
                        }
                    });
                });
                browser.hovered_file = new_hover;
            }
        }
    });

    // Save window position when dragged.
    if let Some(inner) = response {
        let resp = inner.response;
        if resp.drag_stopped() {
            let id = egui::Id::new("lod_browser");
            if let Some(rect) = resp.ctx.memory(|m: &egui::Memory| m.area_rect(id)) {
                let pos = rect.left_top();
                let mut cfg = super::io::EditorConfig::load();
                cfg.browser_pos = Some([pos.x, pos.y]);
                cfg.save();
            }
        }
    }
}

/// Create a new element at canvas center and push it to the screen.
fn place_element(
    name: &str,
    editor: &mut EditorScreen,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
) {
    let max_z = editor.screen.elements.iter().map(|e| e.z()).max().unwrap_or(0);

    let mut img = ImageElement::new(name, name, (REF_W / 2.0, REF_H / 2.0));
    img.z = max_z + 1;

    let bare = name.split('/').last().unwrap_or(name);
    let handle = ui_assets
        .get_or_load(name, game_assets, images, cfg)
        .or_else(|| ui_assets.get_or_load(bare, game_assets, images, cfg));

    if handle.is_some() {
        let dims = ui_assets.dimensions(name).or_else(|| ui_assets.dimensions(bare));
        if let Some((w, h)) = dims {
            img.size = (w as f32, h as f32);
        }
    }

    editor.screen.elements.push(ScreenElement::Image(img));
    editor.dirty = true;
}

/// Create a new video element from a VID archive entry.
fn place_video_element(name: &str, editor: &mut EditorScreen) {
    let max_z = editor.screen.elements.iter().map(|e| e.z()).max().unwrap_or(0);
    // Strip extension if present (e.g. "3dologo.smk" -> "3dologo").
    let video_name = name.strip_suffix(".smk").or_else(|| name.strip_suffix(".SMK")).unwrap_or(name);

    let vid = VideoElement {
        id: format!("vid_{}", video_name),
        position: (REF_W / 2.0, REF_H / 2.0),
        size: (320.0, 240.0),
        z: max_z + 1,
        video: video_name.to_string(),
        hidden: false,
        looping: false,
        skippable: false,
        on_end: Vec::new(),
    };

    editor.screen.elements.push(ScreenElement::Video(vid));
    editor.dirty = true;
}
