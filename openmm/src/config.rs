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

    /// Enable debug HUD and tools by default
    #[arg(long)]
    debug: Option<bool>,

    /// Enable wireframe rendering
    #[arg(long)]
    wireframe: Option<bool>,

    /// Show play area boundary lines
    #[arg(long)]
    show_play_area: Option<bool>,

    /// Window VSync mode: auto, fast, off
    #[arg(long)]
    vsync: Option<String>,

    /// Start in fullscreen
    #[arg(long)]
    fullscreen: Option<bool>,

    /// FPS cap: 0 = unlimited, 30/60/120/etc
    #[arg(long)]
    fps_cap: Option<u32>,

    /// Auto-move the player forward (dev testing for lazy loading)
    #[arg(long)]
    auto_move: Option<bool>,

    /// Entity draw/culling distance in world units (default 10000)
    #[arg(long)]
    draw_distance: Option<f32>,

    /// Fog start distance (default 8000)
    #[arg(long)]
    fog_start: Option<f32>,

    /// Fog end distance (default 22000)
    #[arg(long)]
    fog_end: Option<f32>,

    /// Music volume 0.0 - 1.0 (0 = muted)
    #[arg(long)]
    music_volume: Option<f32>,

    /// Environment/SFX volume 0.0 - 1.0 (0 = muted)
    #[arg(long)]
    sfx_volume: Option<f32>,

    /// Path to config file (default: target/openmm.toml)
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
    show_play_area: Option<bool>,
    vsync: Option<String>,
    fullscreen: Option<bool>,
    fps_cap: Option<u32>,
    auto_move: Option<bool>,
    draw_distance: Option<f32>,
    fog_start: Option<f32>,
    fog_end: Option<f32>,
    music_volume: Option<f32>,
    sfx_volume: Option<f32>,
}

/// Resolved game configuration — available as a Bevy resource.
#[derive(Resource, Debug, Clone, Serialize)]
pub struct GameConfig {
    /// Map to load directly into (e.g. "oute3"), bypasses save data
    pub map: Option<String>,
    /// Skip splash/menu and go straight to loading
    pub skip_intro: bool,
    /// Debug HUD and tools enabled
    pub debug: bool,
    /// Start with wireframe on
    pub wireframe: bool,
    /// Show play area boundary lines
    pub show_play_area: bool,
    /// VSync mode: "auto", "fast", "off"
    pub vsync: String,
    /// Start in fullscreen
    pub fullscreen: bool,
    /// FPS cap: 0 = unlimited
    pub fps_cap: u32,
    /// Auto-move the player forward (dev testing)
    pub auto_move: bool,
    /// Entity draw/culling distance in world units
    pub draw_distance: f32,
    /// Fog start distance (fully clear before this)
    pub fog_start: f32,
    /// Fog end distance (fully opaque after this)
    pub fog_end: f32,
    /// Music volume (0.0 - 1.0, 0 = muted)
    pub music_volume: f32,
    /// Environment/SFX volume (0.0 - 1.0, 0 = muted)
    pub sfx_volume: f32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            map: None,
            skip_intro: false,
            debug: true,
            wireframe: false,
            show_play_area: true,
            vsync: "auto".into(),
            fullscreen: false,
            fps_cap: 60,
            auto_move: false,
            draw_distance: 10000.0,
            fog_start: 8000.0,
            fog_end: 22000.0,
            music_volume: 1.0,
            sfx_volume: 1.0,
        }
    }
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

        let defaults = GameConfig::default();

        GameConfig {
            map: cli.map.or(file_cfg.map).or(defaults.map),
            skip_intro: cli.skip_intro
                .or(file_cfg.skip_intro)
                .unwrap_or(defaults.skip_intro),
            debug: cli.debug
                .or(file_cfg.debug)
                .unwrap_or(defaults.debug),
            wireframe: cli.wireframe
                .or(file_cfg.wireframe)
                .unwrap_or(defaults.wireframe),
            show_play_area: cli.show_play_area
                .or(file_cfg.show_play_area)
                .unwrap_or(defaults.show_play_area),
            vsync: cli.vsync
                .or(file_cfg.vsync)
                .unwrap_or(defaults.vsync),
            fullscreen: cli.fullscreen
                .or(file_cfg.fullscreen)
                .unwrap_or(defaults.fullscreen),
            fps_cap: cli.fps_cap
                .or(file_cfg.fps_cap)
                .unwrap_or(defaults.fps_cap),
            auto_move: cli.auto_move
                .or(file_cfg.auto_move)
                .unwrap_or(defaults.auto_move),
            draw_distance: cli.draw_distance
                .or(file_cfg.draw_distance)
                .unwrap_or(defaults.draw_distance),
            fog_start: cli.fog_start
                .or(file_cfg.fog_start)
                .unwrap_or(defaults.fog_start),
            fog_end: cli.fog_end
                .or(file_cfg.fog_end)
                .unwrap_or(defaults.fog_end),
            music_volume: cli.music_volume
                .or(file_cfg.music_volume)
                .unwrap_or(defaults.music_volume),
            sfx_volume: cli.sfx_volume
                .or(file_cfg.sfx_volume)
                .unwrap_or(defaults.sfx_volume),
        }
    }
}
