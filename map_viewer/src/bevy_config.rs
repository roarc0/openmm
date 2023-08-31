use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{PresentMode, Window};

use crate::APP_NAME;

pub struct BevyConfigPlugin;

impl Plugin for BevyConfigPlugin {
    fn build(&self, app: &mut App) {
        let default_plugins = DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: APP_NAME.into(),
                present_mode: PresentMode::AutoVsync,
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        });

        app.insert_resource(Msaa::Sample4)
            .add_plugins((default_plugins, FrameTimeDiagnosticsPlugin));
    }
}
