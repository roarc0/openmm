mod browser;
mod canvas;
mod format;
mod inspector;
mod io;

use crate::GameState;
use bevy::prelude::*;

/// Marker for all editor entities — despawned on editor exit.
#[derive(Component)]
struct InEditor;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Editor), editor_setup);
    }
}

fn editor_setup(mut commands: Commands) {
    // 2D camera for editor canvas
    commands.spawn((Name::new("editor_camera"), Camera2d, InEditor));
    info!("Screen editor started");
}
