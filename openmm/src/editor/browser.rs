//! egui bitmap browser panel: search LOD icons, click-to-place.

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, egui};

use super::canvas::{EditorScreen, REF_H, REF_W};
use super::format::ScreenElement;
use crate::assets::GameAssets;
use crate::config::GameConfig;
use crate::game::hud::UiAssets;

/// Browser panel state.
#[derive(Resource, Default)]
pub struct BrowserState {
    pub open: bool,
    pub search: String,
    pub all_icons: Vec<String>,
    pub filtered: Vec<String>,
    initialized: bool,
}

/// Load icon names from LOD once. Called every frame but guards with `initialized`.
pub fn init_browser(game_assets: Res<GameAssets>, mut browser: ResMut<BrowserState>) {
    if browser.initialized {
        return;
    }
    browser.initialized = true;

    let mut names: Vec<String> = game_assets.assets().files_in("icons").unwrap_or_default();

    // Also include bitmaps with a prefix so users can search "bitmaps/".
    let bitmaps: Vec<String> = game_assets
        .assets()
        .files_in("bitmaps")
        .unwrap_or_default()
        .into_iter()
        .map(|n| format!("bitmaps/{n}"))
        .collect();

    names.extend(bitmaps);
    names.sort();

    let filtered = names.clone();
    browser.all_icons = names;
    browser.filtered = filtered;
}

/// Tab key toggles the browser open/closed.
pub fn toggle_browser(keys: Res<ButtonInput<KeyCode>>, mut browser: ResMut<BrowserState>) {
    if keys.just_pressed(KeyCode::Tab) {
        browser.open = !browser.open;
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
    mut ready: Local<bool>,
) {
    if !browser.open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };
    if !*ready {
        *ready = true;
        return;
    }
    egui::Window::new("Bitmap Browser")
        .resizable(true)
        .default_width(260.0)
        .show(ctx, |ui| {
            // Search input.
            let prev = browser.search.clone();
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut browser.search);
            });

            // Re-filter when search string changes.
            if browser.search != prev {
                let needle = browser.search.to_lowercase();
                browser.filtered = browser
                    .all_icons
                    .iter()
                    .filter(|n| n.to_lowercase().contains(&needle))
                    .cloned()
                    .collect();
            }

            ui.label(format!("{} results", browser.filtered.len()));
            ui.separator();

            // Show up to 200 results in a scrollable list.
            egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                let show: Vec<String> = browser.filtered.iter().take(200).cloned().collect();
                for name in show {
                    if ui.button(&name).clicked() {
                        place_element(&name, &mut editor, &mut ui_assets, &game_assets, &mut images, &cfg);
                    }
                }
            });
        });
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
    // Resolve the icon name — strip "bitmaps/" prefix for the lookup key.
    let icon_name = name.strip_prefix("bitmaps/").unwrap_or(name);

    let mut elem = ScreenElement::new(
        format!("elem_{}", editor.screen.elements.len()),
        icon_name,
        (REF_W / 2.0, REF_H / 2.0),
    );

    // Try to get dimensions from the loaded texture.
    if let Some(handle) = ui_assets.get_or_load(icon_name, game_assets, images, cfg) {
        // Ensure dimensions are cached before calling dimensions().
        let _ = handle;
    }
    if let Some((w, h)) = ui_assets.dimensions(icon_name) {
        elem.size = Some((w as f32, h as f32));
    }

    editor.screen.elements.push(elem);
    editor.dirty = true;
}
