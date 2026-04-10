mod browser;
pub mod canvas;
mod format;
mod inspector;
mod io;

use bevy::prelude::*;

use crate::GameState;

pub use format::{Screen, ScreenElement};

/// Marker for all editor entities — despawned on editor exit.
#[derive(Component)]
struct InEditor;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<canvas::Selection>()
            .add_systems(OnEnter(GameState::Editor), editor_setup)
            .add_systems(
                Update,
                canvas::rebuild_canvas
                    .run_if(resource_changed::<canvas::EditorScreen>)
                    .run_if(in_state(GameState::Editor)),
            )
            .add_systems(
                Update,
                (
                    canvas::selection_system,
                    canvas::drag_system,
                    canvas::sync_element_positions,
                    canvas::update_labels,
                    canvas::z_order_system,
                    canvas::delete_system,
                    canvas::save_shortcut_system,
                )
                    .run_if(in_state(GameState::Editor)),
            );
    }
}

fn editor_setup(mut commands: Commands) {
    commands.spawn((Name::new("editor_camera"), Camera2d, InEditor));
    let screen = canvas::EditorScreen {
        screen: Screen::new("untitled"),
        dirty: false,
    };
    commands.insert_resource(screen);
    info!("screen editor started — Tab to browse bitmaps, Ctrl+S to save");
}
