use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{PresentMode, Window, WindowMode};

use crate::APP_NAME;
use crate::config::GameConfig;

pub struct BevyConfigPlugin;

impl Plugin for BevyConfigPlugin {
    fn build(&self, app: &mut App) {
        let cfg = app.world().resource::<GameConfig>().clone();

        // fps_cap=0 means unlimited (no vsync), otherwise use vsync.
        // The vsync config is overridden by fps_cap=0.
        let present_mode = if cfg.fps_cap == 0 {
            PresentMode::Immediate
        } else {
            match cfg.vsync.as_str() {
                "fast" => PresentMode::Mailbox,
                "off" => PresentMode::Immediate,
                _ => PresentMode::AutoVsync,
            }
        };

        let window_mode = if cfg.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };

        let default_plugins = DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: APP_NAME.into(),
                present_mode,
                mode: window_mode,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        });

        app.add_plugins((default_plugins, FrameTimeDiagnosticsPlugin::default()))
            .add_systems(Startup, maximize_window);
    }
}

fn maximize_window(mut windows: Query<&mut Window>) {
    for mut window in &mut windows {
        window.set_maximized(true);
    }
}
