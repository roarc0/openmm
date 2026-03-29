use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::{PresentMode, Window, WindowMode, WindowResolution};

use crate::APP_NAME;
use crate::config::GameConfig;

pub struct BevyConfigPlugin;

impl Plugin for BevyConfigPlugin {
    fn build(&self, app: &mut App) {
        let cfg = app.world().resource::<GameConfig>().clone();

        let present_mode = if cfg.fps_cap == 0 {
            PresentMode::Immediate
        } else {
            match cfg.vsync.as_str() {
                "fast" => PresentMode::Mailbox,
                "off" => PresentMode::Immediate,
                _ => PresentMode::AutoVsync,
            }
        };

        let window_mode = match cfg.window_mode.as_str() {
            "borderless" => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            "fullscreen" => WindowMode::Fullscreen(MonitorSelection::Current, bevy::window::VideoModeSelection::Current),
            _ => WindowMode::Windowed,
        };

        let resolution = WindowResolution::new(cfg.width, cfg.height);

        let default_plugins = DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: APP_NAME.into(),
                present_mode,
                mode: window_mode,
                resolution,
                resize_constraints: bevy::window::WindowResizeConstraints {
                    min_width: 640.0,
                    min_height: 480.0,
                    ..default()
                },
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        });

        app.add_plugins((default_plugins, FrameTimeDiagnosticsPlugin::default()));
    }
}

/// Returns the MSAA component to add to the 3D camera based on config.
pub fn camera_msaa(cfg: &GameConfig) -> Msaa {
    match cfg.antialiasing.as_str() {
        "msaa2" => Msaa::Sample2,
        "msaa4" => Msaa::Sample4,
        "msaa8" => Msaa::Sample8,
        "off" => Msaa::Off,
        // FXAA/SMAA are post-process, disable MSAA for them
        "fxaa" | "smaa" => Msaa::Off,
        _ => Msaa::Sample4,
    }
}

/// Returns an optional FXAA component for the 3D camera.
pub fn camera_fxaa(cfg: &GameConfig) -> Option<bevy::anti_alias::fxaa::Fxaa> {
    if cfg.antialiasing == "fxaa" {
        Some(bevy::anti_alias::fxaa::Fxaa::default())
    } else {
        None
    }
}

/// Returns an optional SMAA component for the 3D camera.
pub fn camera_smaa(cfg: &GameConfig) -> Option<bevy::anti_alias::smaa::Smaa> {
    if cfg.antialiasing == "smaa" {
        Some(bevy::anti_alias::smaa::Smaa::default())
    } else {
        None
    }
}

/// Returns an optional TAA component for the 3D camera.
pub fn camera_taa(cfg: &GameConfig) -> Option<bevy::anti_alias::taa::TemporalAntiAliasing> {
    if cfg.antialiasing == "taa" {
        Some(bevy::anti_alias::taa::TemporalAntiAliasing::default())
    } else {
        None
    }
}
