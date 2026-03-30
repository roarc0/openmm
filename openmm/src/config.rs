use std::path::PathBuf;

use bevy::log::warn;
use bevy::prelude::Resource;
use clap::Parser;
use serde::{Deserialize, Serialize};

const CONFIG_PATH: &str = "openmm.toml";

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

    /// Log level: error, warn, info, debug, trace
    #[arg(long)]
    log_level: Option<String>,

    /// Enable debug HUD and tools
    #[arg(long)]
    debug: Option<bool>,

    /// Enable developer console (Tab key)
    #[arg(long)]
    console: Option<bool>,

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

    /// Anti-aliasing mode: "msaa4" (default), "msaa2", "msaa8", "taa", "fxaa", "smaa", "off"
    #[arg(long)]
    antialiasing: Option<String>,

    /// Enable bloom glow effect
    #[arg(long)]
    bloom: Option<bool>,

    /// Bloom intensity (0.0 - 1.0)
    #[arg(long)]
    bloom_intensity: Option<f32>,

    /// Enable screen-space ambient occlusion
    #[arg(long)]
    ssao: Option<bool>,

    /// Tonemapping mode: "none", "reinhard", "aces", "agx" (default), "blender_filmic"
    #[arg(long)]
    tonemapping: Option<String>,

    /// Enable sun shadows
    #[arg(long)]
    shadows: Option<bool>,

    /// Enable depth of field
    #[arg(long)]
    depth_of_field: Option<bool>,

    /// Enable motion blur
    #[arg(long)]
    motion_blur: Option<bool>,

    /// Exposure compensation (-2.0 to 2.0)
    #[arg(long)]
    exposure: Option<f32>,

    /// Path to config file
    /// Path to config file (default: ./openmm.toml, fallback: target/openmm.toml)
    #[arg(long)]
    config: Option<PathBuf>,
}

/// Deserialized from openmm.toml.
#[derive(Deserialize, Debug, Default)]
struct ConfigFile {
    map: Option<String>,
    log_level: Option<String>,
    skip_intro: Option<bool>,
    debug: Option<bool>,
    console: Option<bool>,
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
    bloom: Option<bool>,
    bloom_intensity: Option<f32>,
    ssao: Option<bool>,
    tonemapping: Option<String>,
    shadows: Option<bool>,
    depth_of_field: Option<bool>,
    motion_blur: Option<bool>,
    exposure: Option<f32>,
}

/// Resolved game configuration — available as a Bevy resource.
#[derive(Resource, Debug, Clone, Serialize)]
pub struct GameConfig {
    /// Path to the config file (not serialized).
    #[serde(skip)]
    pub config_path: PathBuf,
    pub map: Option<String>,
    /// Log level: "error", "warn", "info", "debug", "trace"
    pub log_level: String,
    pub skip_intro: bool,
    pub debug: bool,
    /// Enable developer console (Tab key)
    pub console: bool,
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
    /// Anti-aliasing: "msaa4", "msaa2", "msaa8", "taa" (default), "fxaa", "smaa", "off"
    pub antialiasing: String,
    /// Bloom glow on bright pixels
    pub bloom: bool,
    /// Bloom intensity (0.0 - 1.0)
    pub bloom_intensity: f32,
    /// Screen-space ambient occlusion (contact shadows)
    pub ssao: bool,
    /// Tonemapping: "none", "reinhard", "aces", "agx", "blender_filmic"
    pub tonemapping: String,
    /// Sun shadow mapping
    pub shadows: bool,
    /// Depth of field blur
    pub depth_of_field: bool,
    /// Depth of field focal distance
    pub depth_of_field_distance: f32,
    /// Motion blur
    pub motion_blur: bool,
    /// Exposure compensation
    pub exposure: f32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            config_path: PathBuf::from(CONFIG_PATH),
            map: None,
            log_level: "info".into(),
            skip_intro: false,
            debug: false,
            console: true,
            wireframe: false,
            width: 2880,
            height: 2160,
            aspect_ratio: "".into(),
            window_mode: "windowed".into(),
            vsync: "auto".into(),
            fps_cap: 60,
            draw_distance: 16000.0,
            fog_start: 12000.0,
            fog_end: 28000.0,
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
            antialiasing: "msaa4".into(),
            bloom: false,
            bloom_intensity: 0.1,
            ssao: false,
            tonemapping: "agx".into(),
            shadows: true,
            depth_of_field: false,
            depth_of_field_distance: 30.0,
            motion_blur: false,
            exposure: 0.0,
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
    fn resolve_config_path(explicit: Option<PathBuf>) -> PathBuf {
        explicit.unwrap_or_else(|| PathBuf::from(CONFIG_PATH))
    }

    pub fn load() -> Self {
        let cli = Cli::parse();
        let config_path = Self::resolve_config_path(cli.config);

        if !config_path.exists() {
            let defaults = GameConfig::default();
            match toml::to_string_pretty(&defaults) {
                Ok(contents) => {
                    if let Some(parent) = config_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    match std::fs::write(&config_path, &contents) {
                        Ok(()) => eprintln!("info: wrote default config to {}", config_path.display()),
                        Err(e) => eprintln!("warning: failed to write default config to {}: {e}", config_path.display()),
                    }
                }
                Err(e) => eprintln!("warning: failed to serialize default config: {e}"),
            }
        }

        let file_cfg = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|contents| {
                toml::from_str::<ConfigFile>(&contents)
                    .inspect_err(|e| eprintln!("warning: failed to parse {}: {e}", config_path.display()))
                    .ok()
            })
            .unwrap_or_default();

        let d = GameConfig::default();

        let resolved = GameConfig {
            config_path: config_path.clone(),
            map: cli.map.or(file_cfg.map).or(d.map),
            log_level: resolve!(cli.log_level, file_cfg.log_level, d.log_level),
            skip_intro: resolve!(cli.skip_intro, file_cfg.skip_intro, d.skip_intro),
            debug: resolve!(cli.debug, file_cfg.debug, d.debug),
            console: resolve!(cli.console, file_cfg.console, d.console),
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
            bloom: resolve!(cli.bloom, file_cfg.bloom, d.bloom),
            bloom_intensity: resolve!(cli.bloom_intensity, file_cfg.bloom_intensity, d.bloom_intensity),
            ssao: resolve!(cli.ssao, file_cfg.ssao, d.ssao),
            tonemapping: resolve!(cli.tonemapping, file_cfg.tonemapping, d.tonemapping),
            shadows: resolve!(cli.shadows, file_cfg.shadows, d.shadows),
            depth_of_field: resolve!(cli.depth_of_field, file_cfg.depth_of_field, d.depth_of_field),
            depth_of_field_distance: d.depth_of_field_distance,
            motion_blur: resolve!(cli.motion_blur, file_cfg.motion_blur, d.motion_blur),
            exposure: resolve!(cli.exposure, file_cfg.exposure, d.exposure),
        };
        resolved.validate();
        resolved
    }

    /// Save current config to disk.
    pub fn save(&self) -> Result<(), String> {
        let contents = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        if let Some(parent) = self.config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&self.config_path, contents)
            .map_err(|e| format!("Failed to write {}: {e}", self.config_path.display()))
    }
}

impl GameConfig {
    fn validate(&self) {
        // Validate string enum fields
        if !matches!(self.window_mode.as_str(), "windowed" | "borderless" | "fullscreen") {
            warn!("Unknown window_mode '{}' — using 'windowed'", self.window_mode);
        }
        if !matches!(self.vsync.as_str(), "auto" | "fast" | "off") {
            warn!("Unknown vsync '{}' — using 'auto'", self.vsync);
        }
        if !matches!(self.hud_filtering.as_str(), "nearest" | "linear") {
            warn!("Unknown hud_filtering '{}' — using 'nearest'", self.hud_filtering);
        }
        if !matches!(self.antialiasing.as_str(), "msaa2" | "msaa4" | "msaa8" | "fxaa" | "smaa" | "taa" | "off") {
            warn!("Unknown antialiasing '{}' — using 'msaa4'", self.antialiasing);
        }
        if !matches!(self.tonemapping.as_str(), "none" | "reinhard" | "aces" | "blender_filmic" | "agx") {
            warn!("Unknown tonemapping '{}' — using 'agx'", self.tonemapping);
        }
        // Warn about incompatible combos
        if self.bloom && self.tonemapping == "none" {
            warn!("Bloom requires tonemapping — forcing AgX (set tonemapping to avoid this)");
        }
        if self.ssao && matches!(self.antialiasing.as_str(), "msaa2" | "msaa4" | "msaa8") {
            warn!("SSAO requires Msaa::Off — MSAA will be disabled (use fxaa/smaa/taa instead)");
        }
    }
}
