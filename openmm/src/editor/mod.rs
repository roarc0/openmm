mod browser;
pub mod canvas;
mod format;
mod inspector;
mod io;

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::input::EguiWantsInput;
use bevy_inspector_egui::bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

use crate::GameState;

pub use format::Screen;

/// Marker for all editor entities — despawned on editor exit.
#[derive(Component)]
struct InEditor;

/// Whether all egui UI is visible. Esc toggles.
#[derive(Resource)]
pub struct UiVisible(pub bool);

impl Default for UiVisible {
    fn default() -> Self {
        Self(true)
    }
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        app.init_resource::<canvas::Selection>()
            .init_resource::<browser::BrowserState>()
            .init_resource::<UiVisible>()
            .init_resource::<canvas::OverlayAction>()
            .add_systems(OnEnter(GameState::Editor), editor_setup)
            .add_systems(
                Update,
                (
                    canvas::rebuild_canvas,
                    (canvas::selection_system, canvas::drag_system).chain(),
                    canvas::sync_element_positions,
                    canvas::z_order_system,
                    canvas::arrow_nudge_system,
                    canvas::delete_system,
                    canvas::tab_cycle_system,
                    canvas::save_shortcut_system,
                    canvas::apply_overlay_actions,
                    browser::init_browser,
                    browser::toggle_browser,
                    toggle_ui,
                )
                    .run_if(in_state(GameState::Editor)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (
                    browser::browser_ui,
                    inspector::inspector_ui,
                    editor_toolbar,
                    canvas::draw_overlays,
                )
                    .run_if(in_state(GameState::Editor))
                    .run_if(|vis: Res<UiVisible>| vis.0),
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
    let screen = io::load_last_screen();
    let name = screen.id.clone();
    commands.insert_resource(canvas::EditorScreen { screen, dirty: false });
    info!("screen editor started — editing '{name}'");
}

/// Esc toggles all egui UI visibility.
fn toggle_ui(keys: Res<ButtonInput<KeyCode>>, mut visible: ResMut<UiVisible>, egui_input: Option<Res<EguiWantsInput>>) {
    if egui_input.is_some_and(|e| e.wants_keyboard_input()) {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        visible.0 = !visible.0;
    }
}

/// Top toolbar: name indicator, New/Open/Save buttons, help text.
fn editor_toolbar(mut contexts: EguiContexts, mut editor: ResMut<canvas::EditorScreen>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::TopBottomPanel::top("editor_toolbar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let dirty_mark = if editor.dirty { "*" } else { "" };
            ui.strong(format!("Screen: {}{}", editor.screen.id, dirty_mark));

            ui.separator();

            if ui.button("New").clicked() {
                editor.screen = Screen::new("untitled");
                editor.dirty = false;
                io::set_last_screen("untitled");
            }

            let screens = io::list_screens();
            if !screens.is_empty() {
                egui::ComboBox::from_label("Open")
                    .selected_text(&editor.screen.id)
                    .show_ui(ui, |ui| {
                        for name in &screens {
                            if ui.selectable_label(false, name).clicked() {
                                match io::load_screen(name) {
                                    Ok(s) => {
                                        editor.screen = s;
                                        editor.dirty = false;
                                        io::set_last_screen(name);
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

            ui.weak("Tab: cycle | F2: browser | Esc: hide UI | Drag: move | Scroll: z | Del: remove");
        });
    });
}
