use std::path::PathBuf;

use bevy::prelude::Resource;
use clap::Parser;
use serde::{Deserialize, Serialize};

const CONFIG_PATH: &str = "target/openmm.toml";

/// Command-line arguments — override config file values.
#[derive(Parser, Debug)]
#[command(name = "openmm", about = "Open-source Might and Magic VI engine")]
struct Cli {
    /// Load directly into a map (e.g. "oute3" or "outb2")
    #[arg(long)]
    map: Option<String>,

    /// Skip splash and menu, go straight to game
    #[arg(long)]
    skip_intro: Option<bool>,

    /// Enable debug HUD and tools
    #[arg(long)]
    debug: Option<bool>,

    /// Enable wireframe rendering
    #[arg(long)]
    wireframe: Option<bool>,

    /// Window width
    #[arg(long)]
    width: Option<u32>,

    /// Window height
    #[arg(long)]
    height: Option<u32>,

    /// Aspect ratio constraint, e.g. "4:3" — adds black bars on wider displays
    #[arg(long)]
    aspect_ratio: Option<String>,

    /// Window mode: windowed, borderless, fullscreen
    #[arg(long)]
    window_mode: Option<String>,

    /// VSync mode: auto, fast, off
    #[arg(long)]
    vsync: Option<String>,

    /// FPS cap: 0 = unlimited
    #[arg(long)]
    fps_cap: Option<u32>,

    /// Entity draw/culling distance in world units
    #[arg(long)]
    draw_distance: Option<f32>,

    /// Fog start distance
    #[arg(long)]
    fog_start: Option<f32>,

    /// Fog end distance
    #[arg(long)]
    fog_end: Option<f32>,

    /// Music volume 0.0 - 1.0
    #[arg(long)]
    music_volume: Option<f32>,

    /// SFX volume 0.0 - 1.0
    #[arg(long)]
    sfx_volume: Option<f32>,

    /// Always run instead of walk
    #[arg(long)]
    always_run: Option<bool>,

    /// Enable mouse look
    #[arg(long)]
    mouse_look: Option<bool>,

    /// Mouse sensitivity X axis
    #[arg(long)]
    mouse_sensitivity_x: Option<f32>,

    /// Mouse sensitivity Y axis
    #[arg(long)]
    mouse_sensitivity_y: Option<f32>,

    /// Always strafe with left/right keys
    #[arg(long)]
    always_strafe: Option<bool>,

    /// Keyboard turn speed
    #[arg(long)]
    turn_speed: Option<f32>,

    /// Mouse look controls vertical flight
    #[arg(long)]
    mouse_look_fly: Option<bool>,

    /// Mouse wheel controls flight altitude
    #[arg(long)]
    mouse_wheel_fly: Option<bool>,

    /// CapsLock toggles mouse look
    #[arg(long)]
    capslock_toggle_mouse_look: Option<bool>,

    /// HUD texture filtering: "nearest" (crisp pixels) or "linear" (smooth)
    #[arg(long)]
    hud_filtering: Option<String>,

    /// Anti-aliasing mode: "msaa4" (default), "msaa2", "msaa8", "taa", "fxaa", "off"
    #[arg(long)]
    antialiasing: Option<String>,

    /// Path to config file
    #[arg(long, default_value = CONFIG_PATH)]
    config: PathBuf,
}

/// Deserialized from openmm.toml.
#[derive(Deserialize, Debug, Default)]
struct ConfigFile {
    map: Option<String>,
    skip_intro: Option<bool>,
    debug: Option<bool>,
    wireframe: Option<bool>,
    width: Option<u32>,
    height: Option<u32>,
    aspect_ratio: Option<String>,
    window_mode: Option<String>,
    vsync: Option<String>,
    fps_cap: Option<u32>,
    draw_distance: Option<f32>,
    fog_start: Option<f32>,
    fog_end: Option<f32>,
    music_volume: Option<f32>,
    sfx_volume: Option<f32>,
    always_run: Option<bool>,
    mouse_look: Option<bool>,
    mouse_sensitivity_x: Option<f32>,
    mouse_sensitivity_y: Option<f32>,
    always_strafe: Option<bool>,
    turn_speed: Option<f32>,
    mouse_look_fly: Option<bool>,
    mouse_wheel_fly: Option<bool>,
    capslock_toggle_mouse_look: Option<bool>,
    hud_filtering: Option<String>,
    antialiasing: Option<String>,
}

/// Resolved game configuration — available as a Bevy resource.
#[derive(Resource, Debug, Clone, Serialize)]
pub struct GameConfig {
    pub map: Option<String>,
    pub skip_intro: bool,
    pub debug: bool,
    pub wireframe: bool,
    /// Window width (MM6: width)
    pub width: u32,
    /// Window height (MM6: height)
    pub height: u32,
    /// Aspect ratio constraint, e.g. "4:3" — adds letterbox bars on wider displays
    pub aspect_ratio: String,
    /// Window mode: "windowed", "borderless", "fullscreen"
    pub window_mode: String,
    /// VSync mode: "auto", "fast", "off"
    pub vsync: String,
    pub fps_cap: u32,
    pub draw_distance: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    /// Always run (MM6: AlwaysRun)
    pub always_run: bool,
    /// Mouse look enabled (MM6: MouseLook)
    pub mouse_look: bool,
    /// Mouse sensitivity X (MM6: MouseSensitivityX)
    pub mouse_sensitivity_x: f32,
    /// Mouse sensitivity Y (MM6: MouseSensitivityY)
    pub mouse_sensitivity_y: f32,
    /// Always strafe with left/right (MM6: AlwaysStrafe)
    pub always_strafe: bool,
    /// Keyboard turn speed (MM6: TurnSpeedNormal)
    pub turn_speed: f32,
    /// Mouse look controls vertical flight (MM6: MouseLookFly)
    pub mouse_look_fly: bool,
    /// Mouse wheel controls flight altitude (MM6: MouseWheelFly)
    pub mouse_wheel_fly: bool,
    /// CapsLock toggles mouse look on/off (MM6: CapsLockToggleMouseLook)
    pub capslock_toggle_mouse_look: bool,
    /// HUD texture filtering mode: "nearest" (crisp pixels) or "linear" (smooth)
    pub hud_filtering: String,
    /// Anti-aliasing: "msaa4" (default), "msaa2", "msaa8", "taa", "fxaa", "off"
    pub antialiasing: String,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            map: None,
            skip_intro: false,
            debug: true,
            wireframe: false,
            width: 2880,
            height: 2160,
            aspect_ratio: "4:3".into(),
            window_mode: "windowed".into(),
            vsync: "auto".into(),
            fps_cap: 60,
            draw_distance: 10000.0,
            fog_start: 8000.0,
            fog_end: 22000.0,
            music_volume: 1.0,
            sfx_volume: 1.0,
            always_run: true,
            mouse_look: true,
            mouse_sensitivity_x: 1.0,
            mouse_sensitivity_y: 1.0,
            always_strafe: false,
            turn_speed: 100.0,
            mouse_look_fly: true,
            mouse_wheel_fly: true,
            capslock_toggle_mouse_look: true,
            hud_filtering: "nearest".into(),
            antialiasing: "taa".into(),
        }
    }
}

macro_rules! resolve {
    ($cli:expr, $file:expr, $default:expr) => {
        $cli.or($file).unwrap_or($default)
    };
}

impl GameConfig {
    /// Load config from file, then apply CLI overrides.
    /// If the config file doesn't exist, writes a default one.
    pub fn load() -> Self {
        let cli = Cli::parse();

        if !cli.config.exists() {
            let defaults = GameConfig::default();
            match toml::to_string_pretty(&defaults) {
                Ok(contents) => {
                    if let Some(parent) = cli.config.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    match std::fs::write(&cli.config, &contents) {
                        Ok(()) => eprintln!("info: wrote default config to {}", cli.config.display()),
                        Err(e) => eprintln!("warning: failed to write default config to {}: {e}", cli.config.display()),
                    }
                }
                Err(e) => eprintln!("warning: failed to serialize default config: {e}"),
            }
        }

        let file_cfg = std::fs::read_to_string(&cli.config)
            .ok()
            .and_then(|contents| {
                toml::from_str::<ConfigFile>(&contents)
                    .inspect_err(|e| eprintln!("warning: failed to parse {}: {e}", cli.config.display()))
                    .ok()
            })
            .unwrap_or_default();

        let d = GameConfig::default();

        GameConfig {
            map: cli.map.or(file_cfg.map).or(d.map),
            skip_intro: resolve!(cli.skip_intro, file_cfg.skip_intro, d.skip_intro),
            debug: resolve!(cli.debug, file_cfg.debug, d.debug),
            wireframe: resolve!(cli.wireframe, file_cfg.wireframe, d.wireframe),
            width: resolve!(cli.width, file_cfg.width, d.width),
            height: resolve!(cli.height, file_cfg.height, d.height),
            aspect_ratio: resolve!(cli.aspect_ratio, file_cfg.aspect_ratio, d.aspect_ratio),
            window_mode: resolve!(cli.window_mode, file_cfg.window_mode, d.window_mode),
            vsync: resolve!(cli.vsync, file_cfg.vsync, d.vsync),
            fps_cap: resolve!(cli.fps_cap, file_cfg.fps_cap, d.fps_cap),
            draw_distance: resolve!(cli.draw_distance, file_cfg.draw_distance, d.draw_distance),
            fog_start: resolve!(cli.fog_start, file_cfg.fog_start, d.fog_start),
            fog_end: resolve!(cli.fog_end, file_cfg.fog_end, d.fog_end),
            music_volume: resolve!(cli.music_volume, file_cfg.music_volume, d.music_volume),
            sfx_volume: resolve!(cli.sfx_volume, file_cfg.sfx_volume, d.sfx_volume),
            always_run: resolve!(cli.always_run, file_cfg.always_run, d.always_run),
            mouse_look: resolve!(cli.mouse_look, file_cfg.mouse_look, d.mouse_look),
            mouse_sensitivity_x: resolve!(cli.mouse_sensitivity_x, file_cfg.mouse_sensitivity_x, d.mouse_sensitivity_x),
            mouse_sensitivity_y: resolve!(cli.mouse_sensitivity_y, file_cfg.mouse_sensitivity_y, d.mouse_sensitivity_y),
            always_strafe: resolve!(cli.always_strafe, file_cfg.always_strafe, d.always_strafe),
            turn_speed: resolve!(cli.turn_speed, file_cfg.turn_speed, d.turn_speed),
            mouse_look_fly: resolve!(cli.mouse_look_fly, file_cfg.mouse_look_fly, d.mouse_look_fly),
            mouse_wheel_fly: resolve!(cli.mouse_wheel_fly, file_cfg.mouse_wheel_fly, d.mouse_wheel_fly),
            capslock_toggle_mouse_look: resolve!(cli.capslock_toggle_mouse_look, file_cfg.capslock_toggle_mouse_look, d.capslock_toggle_mouse_look),
            hud_filtering: resolve!(cli.hud_filtering, file_cfg.hud_filtering, d.hud_filtering),
            antialiasing: resolve!(cli.antialiasing, file_cfg.antialiasing, d.antialiasing),
        }
    }
}
