mod browser;
pub mod canvas;
mod format;
mod inspector;
mod io;

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContexts, EguiPlugin, egui};

use crate::GameState;

pub use format::Screen;

/// Marker for all editor entities — despawned on editor exit.
#[derive(Component)]
struct InEditor;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        app.init_resource::<canvas::Selection>()
            .init_resource::<browser::BrowserState>()
            .add_systems(OnEnter(GameState::Editor), editor_setup)
            .add_systems(
                Update,
                (
                    canvas::rebuild_canvas,
                    canvas::selection_system,
                    canvas::drag_system,
                    canvas::sync_element_positions,
                    canvas::update_labels,
                    canvas::z_order_system,
                    canvas::delete_system,
                    canvas::save_shortcut_system,
                    browser::init_browser,
                    browser::toggle_browser,
                    browser::browser_ui,
                    inspector::inspector_ui,
                    editor_toolbar,
                )
                    .run_if(in_state(GameState::Editor)),
            );
    }
}

fn editor_setup(mut commands: Commands) {
    commands.spawn((
        Name::new("editor_camera"),
        Camera2d,
        bevy::picking::Pickable::IGNORE,
        InEditor,
    ));
    let screen = canvas::EditorScreen {
        screen: Screen::new("untitled"),
        dirty: false,
    };
    commands.insert_resource(screen);
    info!("screen editor started — Tab to browse bitmaps, Ctrl+S to save");
}

/// Top toolbar: name indicator, New/Open/Save buttons, help text.
fn editor_toolbar(mut contexts: EguiContexts, mut editor: ResMut<canvas::EditorScreen>, mut ready: Local<u32>) {
    if *ready < 2 {
        *ready += 1;
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::TopBottomPanel::top("editor_toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Screen name + dirty indicator.
            let dirty_mark = if editor.dirty { "*" } else { "" };
            ui.strong(format!("Screen: {}{}", editor.screen.id, dirty_mark));

            ui.separator();

            // New button.
            if ui.button("New").clicked() {
                editor.screen = Screen::new("untitled");
                editor.dirty = false;
            }

            // Open ComboBox — lists saved screens.
            let screens = io::list_screens();
            if !screens.is_empty() {
                egui::ComboBox::from_label("Open")
                    .selected_text("— pick screen —")
                    .show_ui(ui, |ui| {
                        for name in &screens {
                            if ui.selectable_label(false, name).clicked() {
                                match io::load_screen(name) {
                                    Ok(s) => {
                                        editor.screen = s;
                                        editor.dirty = false;
                                        info!("loaded screen '{name}'");
                                    }
                                    Err(e) => error!("load failed: {e}"),
                                }
                            }
                        }
                    });
            } else {
                ui.label("(no saved screens)");
            }

            // Save button.
            if ui.button("Save").clicked() {
                match io::save_screen(&editor.screen) {
                    Ok(()) => {
                        editor.dirty = false;
                        info!("screen '{}' saved", editor.screen.id);
                    }
                    Err(e) => error!("save failed: {e}"),
                }
            }

            ui.separator();

            ui.weak("Tab: browser | Click: select | Drag: move | Scroll: z-order | Del: remove");
        });
    });
}
