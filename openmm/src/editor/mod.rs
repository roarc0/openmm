mod browser;
pub mod canvas;
mod clipboard;
mod editor_panel;
mod element_editor;
mod guides;
mod input;
pub mod io;
mod overlay;

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::input::EguiWantsInput;
use bevy_inspector_egui::bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext, egui};

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::sound::SoundManager;
use crate::game::sound::effects::EffectsPlugin;
use crate::screens::Screen;
use crate::screens::ui_assets::UiAssets;

/// Run condition: true when egui wants keyboard input (user typing in a text field).
fn egui_wants_keyboard(egui_input: Option<Res<EguiWantsInput>>) -> bool {
    egui_input.is_some_and(|e| e.wants_keyboard_input())
}

/// Marker for all editor entities — despawned on editor exit.
#[derive(Component)]
pub(crate) struct InEditor;

/// Whether all egui UI is visible. Esc toggles.
#[derive(Resource)]
pub struct UiVisible(pub bool);

impl Default for UiVisible {
    fn default() -> Self {
        Self(true)
    }
}

/// Primary egui context entities that existed before entering editor mode.
#[derive(Resource, Default)]
struct PreviousPrimaryEguiContexts(Vec<Entity>);

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.add_plugins(EffectsPlugin);

        // Load config once and keep as resource.
        let cfg = io::EditorConfig::load_from_disk();
        let screen = io::load_last_screen(&cfg);
        info!("screen editor — editing '{}'", screen.id);

        app.init_resource::<canvas::Selection>()
            .init_resource::<browser::BrowserState>()
            .init_resource::<UiVisible>()
            .init_resource::<PreviousPrimaryEguiContexts>()
            .init_resource::<overlay::OverlayAction>()
            .insert_resource({
                let original_id = Some(screen.id.clone());
                canvas::EditorScreen {
                    screen,
                    dirty: false,
                    original_id,
                }
            })
            .init_resource::<canvas::ElementEditorState>()
            .init_resource::<clipboard::Clipboard>()
            .insert_resource(guides::Guides::from_config(&cfg))
            .insert_resource(cfg)
            .add_systems(
                OnExit(GameState::Editor),
                (crate::despawn_all::<InEditor>, restore_primary_egui_contexts).chain(),
            )
            .add_systems(OnEnter(GameState::Editor), editor_setup)
            .add_systems(OnEnter(GameState::Editor), init_editor_sound_manager)
            .add_systems(
                Update,
                (
                    canvas::rebuild_canvas,
                    (input::selection_system, input::drag_system).chain(),
                    canvas::sync_element_positions,
                    overlay::apply_overlay_actions,
                    input::save_shortcut_system,
                    browser::init_browser,
                    io::flush_config,
                    // Keyboard systems — disabled when typing in egui text fields.
                    (
                        input::z_order_system,
                        input::arrow_nudge_system,
                        input::shortcut_system,
                        input::delete_system,
                        input::copy_paste_system,
                        input::tab_cycle_system,
                        browser::toggle_browser,
                        toggle_ui,
                    )
                        .run_if(not(egui_wants_keyboard)),
                )
                    .run_if(in_state(GameState::Editor)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (
                    browser::browser_ui,
                    editor_panel::editor_panel_ui,
                    editor_toolbar,
                    overlay::draw_overlays,
                )
                    .run_if(in_state(GameState::Editor))
                    .run_if(|vis: Res<UiVisible>| vis.0),
            );
    }
}

fn editor_setup(
    mut commands: Commands,
    mut ui_visible: ResMut<UiVisible>,
    mut ui_assets: ResMut<UiAssets>,
    mut previous_primary: ResMut<PreviousPrimaryEguiContexts>,
    primary_contexts: Query<Entity, With<PrimaryEguiContext>>,
) {
    ui_visible.0 = true;
    ui_assets.clear_cache();
    previous_primary.0.clear();
    for entity in &primary_contexts {
        previous_primary.0.push(entity);
        commands.entity(entity).remove::<PrimaryEguiContext>();
    }

    commands.spawn((
        Name::new("editor_camera"),
        Camera2d,
        PrimaryEguiContext,
        bevy::picking::Pickable::IGNORE,
        InEditor,
    ));
}

fn restore_primary_egui_contexts(
    mut commands: Commands,
    mut previous_primary: ResMut<PreviousPrimaryEguiContexts>,
    alive: Query<Entity>,
) {
    for entity in previous_primary.0.drain(..) {
        if alive.get(entity).is_ok() {
            commands.entity(entity).insert(PrimaryEguiContext);
        }
    }
}

fn init_editor_sound_manager(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    existing: Option<Res<SoundManager>>,
) {
    if existing.is_some() {
        return;
    }

    let Some(manager) = SoundManager::from_game_assets(&game_assets) else {
        warn!("editor: audio resources missing — click sound preview disabled");
        return;
    };

    info!("editor sound preview initialized");
    commands.insert_resource(manager);
}

/// Esc toggles all egui UI visibility.
fn toggle_ui(keys: Res<ButtonInput<KeyCode>>, mut visible: ResMut<UiVisible>, time: Res<Time>) {
    // Skip first second to avoid spurious Esc from window manager.
    if time.elapsed_secs() < 1.0 {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        visible.0 = !visible.0;
        info!("UI visible: {}", visible.0);
    }
}

/// Top toolbar: name indicator, New/Open/Save buttons, help text.
fn editor_toolbar(
    mut contexts: EguiContexts,
    mut editor: ResMut<canvas::EditorScreen>,
    mut editor_state: ResMut<canvas::ElementEditorState>,
    mut cfg: ResMut<io::EditorConfig>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("Toolbar")
        .id(egui::Id::new("editor_toolbar"))
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(4.0, 4.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let dirty_mark = if editor.dirty { "*" } else { "" };
                ui.strong(format!("Screen: {}{}", editor.screen.id, dirty_mark));

                ui.separator();

                if ui.button("New").clicked() {
                    editor.screen = Screen::new("untitled");
                    editor.dirty = false;
                    editor.original_id = None;
                    editor_state.hidden.clear();
                    io::set_last_screen(&mut cfg, "untitled");
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
                                            editor.original_id = Some(name.to_string());
                                            editor_state.hidden.clear();
                                            io::set_last_screen(&mut cfg, name);
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
                    input::save_editor_screen(&mut editor);
                }

                ui.separator();

                ui.weak("Tab: cycle | F2: browser | Esc: hide UI | Drag: move | Scroll: z | Del: remove");
            });
        });
}
