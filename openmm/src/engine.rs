use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::{PresentMode, Window, WindowMode, WindowResolution};
use bevy_framepace::{FramepacePlugin, FramepaceSettings, Limiter};
use std::time::Duration;

use crate::APP_NAME;
use crate::config::GameConfig;
use crate::log::{log_level, log_level_name};

/// Converts an `fps_cap` config value into the framepace limiter strategy.
/// 0 disables limiting entirely.
fn limiter_from_cap(fps_cap: u32) -> Limiter {
    if fps_cap == 0 {
        Limiter::Off
    } else {
        Limiter::Manual(Duration::from_secs_f64(1.0 / fps_cap as f64))
    }
}

/// Syncs `FramepaceSettings` when `GameConfig::fps_cap` is mutated at runtime
/// (e.g. from the developer console). Tracks the previous cap so unrelated
/// config changes (volume, vsync, …) don't touch the framepace resource and
/// trigger downstream change-detection on its proxy.
fn sync_framepace_settings(
    cfg: Res<GameConfig>,
    mut settings: ResMut<FramepaceSettings>,
    mut last_cap: Local<Option<u32>>,
) {
    if *last_cap == Some(cfg.fps_cap) {
        return;
    }
    *last_cap = Some(cfg.fps_cap);
    settings.limiter = limiter_from_cap(cfg.fps_cap);
}

pub struct EngineConfigPlugin;

impl Plugin for EngineConfigPlugin {
    fn build(&self, app: &mut App) {
        let cfg = app.world().resource::<GameConfig>().clone();

        // Present mode is purely controlled by the vsync setting. The
        // `fps_cap` is enforced independently by `bevy_framepace` so it works
        // regardless of vsync mode (otherwise vsync=off + fps_cap=60 would
        // silently spin the CPU at thousands of fps).
        let present_mode = match cfg.vsync.as_str() {
            "fast" => PresentMode::Mailbox,
            "off" => PresentMode::Immediate,
            _ => PresentMode::AutoVsync,
        };

        let window_mode = match cfg.window_mode.as_str() {
            "borderless" => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            "fullscreen" => {
                WindowMode::Fullscreen(MonitorSelection::Current, bevy::window::VideoModeSelection::Current)
            }
            _ => WindowMode::Windowed,
        };

        let resolution = WindowResolution::new(cfg.width, cfg.height);

        // Build tracing filter: external crates → warn, our crates → configured level.
        // Format: "warn,openmm=info,lod=info"
        let level = log_level(&cfg);
        let level_str = log_level_name(level);
        let log_filter = format!("warn,openmm={level_str},lod={level_str}");

        let plugins = DefaultPlugins
            .set(WindowPlugin {
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
            })
            .set(LogPlugin {
                filter: log_filter,
                level,
                ..default()
            });

        let initial_framepace = FramepaceSettings {
            limiter: limiter_from_cap(cfg.fps_cap),
        };

        app.add_plugins(plugins)
            .add_plugins(FramepacePlugin)
            .insert_resource(initial_framepace)
            .add_systems(Update, sync_framepace_settings);
    }
}

/// Returns the MSAA component to add to the 3D camera based on config.
pub fn camera_msaa(cfg: &GameConfig) -> Msaa {
    let wants_msaa = matches!(cfg.antialiasing.as_str(), "msaa2" | "msaa4" | "msaa8");

    let needs_msaa_off = cfg.ssao || cfg.bloom;
    if needs_msaa_off && wants_msaa {
        warn!(
            "Post-processing (bloom/SSAO/DOF) requires Msaa::Off — disabling MSAA (antialiasing='{}' ignored)",
            cfg.antialiasing
        );
        return Msaa::Off;
    }
    if needs_msaa_off && !matches!(cfg.antialiasing.as_str(), "fxaa" | "smaa" | "taa" | "off") {
        warn!("Post-processing requires Msaa::Off — forcing Msaa::Off (set antialiasing to fxaa/smaa/taa)");
        return Msaa::Off;
    }

    match cfg.antialiasing.as_str() {
        "msaa2" => Msaa::Sample2,
        "msaa4" => Msaa::Sample4,
        "msaa8" => Msaa::Sample8,
        "off" => Msaa::Off,
        "fxaa" | "smaa" | "taa" => Msaa::Off,
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

/// Returns an optional Bloom component.
pub fn camera_bloom(cfg: &GameConfig) -> Option<bevy::post_process::bloom::Bloom> {
    if cfg.bloom {
        Some(bevy::post_process::bloom::Bloom {
            intensity: cfg.bloom_intensity,
            ..Default::default()
        })
    } else {
        None
    }
}

/// Returns the Tonemapping component from config.
pub fn camera_tonemapping(cfg: &GameConfig) -> bevy::core_pipeline::tonemapping::Tonemapping {
    use bevy::core_pipeline::tonemapping::Tonemapping;
    match cfg.tonemapping.as_str() {
        "none" => Tonemapping::None,
        "reinhard" => Tonemapping::Reinhard,
        "aces" => Tonemapping::AcesFitted,
        "blender_filmic" => Tonemapping::BlenderFilmic,
        _ => Tonemapping::AgX,
    }
}

/// Returns an optional SSAO component.
pub fn camera_ssao(cfg: &GameConfig) -> Option<bevy::pbr::ScreenSpaceAmbientOcclusion> {
    if cfg.ssao {
        Some(bevy::pbr::ScreenSpaceAmbientOcclusion::default())
    } else {
        None
    }
}

/// Returns an optional MotionBlur component.
pub fn camera_motion_blur(cfg: &GameConfig) -> Option<bevy::post_process::motion_blur::MotionBlur> {
    if cfg.motion_blur {
        Some(bevy::post_process::motion_blur::MotionBlur::default())
    } else {
        None
    }
}

/// Bevy's DoF is physically based: focal length is derived from `sensor_height`
/// (Bevy default 18.66mm Super 35) and FOV. MM6's world units are far larger than
/// real-world meters, so we override `sensor_height` to a value that produces
/// visible blur at MM6 scale. Not user-tunable — this is a unit-system constant.
const DOF_SENSOR_HEIGHT: f32 = 10.0;

/// Returns an optional DepthOfField component tuned for MM6's world unit scale.
pub fn camera_dof(cfg: &GameConfig) -> Option<bevy::post_process::dof::DepthOfField> {
    if cfg.depth_of_field {
        Some(bevy::post_process::dof::DepthOfField {
            focal_distance: cfg.depth_of_field_distance,
            aperture_f_stops: cfg.depth_of_field_aperture,
            sensor_height: DOF_SENSOR_HEIGHT,
            ..default()
        })
    } else {
        None
    }
}

/// Returns an Exposure component.
/// Base EV100 of 14.5 matches outdoor daylight with physically-based light values.
/// cfg.exposure adjusts relative to this base.
pub fn camera_exposure(cfg: &GameConfig) -> bevy::camera::Exposure {
    bevy::camera::Exposure {
        ev100: 9.7 + cfg.exposure,
    }
}
