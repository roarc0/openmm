//! Screen file loading and texture utilities shared by editor and runtime.

use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

use super::Screen;
use super::ui_assets::UiAssets;
use crate::assets::GameAssets;
use crate::system::config::GameConfig;

/// Reference resolution for MM6 UI screens.
pub const REF_W: f32 = 640.0;
pub const REF_H: f32 = 480.0;

const SCREENS_DIR: &str = "openmm/assets/screens";

/// Color key transparency options.
pub const TRANSPARENCY_OPTIONS: &[&str] = &["", "black", "cyan", "lime", "red", "magenta", "blue"];

pub fn screen_path(id: &str) -> PathBuf {
    Path::new(SCREENS_DIR).join(format!("{}.ron", id))
}

pub fn load_screen(id: &str) -> Result<Screen, String> {
    let path = screen_path(id);
    let contents = fs::read_to_string(&path).map_err(|e| format!("Read error {}: {e}", path.display()))?;
    ron::from_str(&contents).map_err(|e| format!("RON parse error {}: {e}", path.display()))
}

pub fn save_screen(screen: &Screen) -> Result<(), String> {
    let _ = fs::create_dir_all(SCREENS_DIR);
    let path = screen_path(&screen.id);
    let ron_str = ron::ser::to_string_pretty(screen, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialize error: {e}"))?;
    fs::write(&path, &ron_str).map_err(|e| format!("Write error {}: {e}", path.display()))?;
    bevy::log::info!("saved screen to {}", path.display());
    Ok(())
}

pub fn list_screens() -> Vec<String> {
    let dir = Path::new(SCREENS_DIR);
    if !dir.exists() {
        return Vec::new();
    }
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".ron").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// Resolve image element size: explicit size > texture dimensions > 32x32 fallback.
pub fn resolve_image_size(elem: &super::ImageElement, ui_assets: &UiAssets) -> (f32, f32) {
    let (w, h) = elem.size;
    if w > 0.0 && h > 0.0 {
        return (w, h);
    }
    elem.texture_for_state("default")
        .and_then(|name| ui_assets.dimensions(name))
        .map(|(w, h)| (w as f32, h as f32))
        .unwrap_or((32.0, 32.0))
}

/// Load a texture with optional color-key transparency applied.
pub fn load_texture_with_transparency(
    tex_name: &str,
    transparent_color: &str,
    ui_assets: &mut UiAssets,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    cfg: &GameConfig,
) -> Option<Handle<Image>> {
    // Strip `icons/` prefix — UiAssets.get_or_load already prepends it via lod.icon().
    let bare = tex_name
        .strip_prefix("icons/")
        .unwrap_or_else(|| tex_name.split('/').next_back().unwrap_or(tex_name));
    if transparent_color.is_empty() {
        return ui_assets
            .get_or_load(bare, game_assets, images, cfg)
            .or_else(|| ui_assets.get_or_load(tex_name, game_assets, images, cfg));
    }
    let cache_key = format!("{}@t_{}", bare, transparent_color);
    let source = if ui_assets.get_or_load(bare, game_assets, images, cfg).is_some() {
        bare
    } else if ui_assets.get_or_load(tex_name, game_assets, images, cfg).is_some() {
        tex_name
    } else {
        bare
    };
    let tc = transparent_color.to_string();
    ui_assets.get_or_load_transformed(source, &cache_key, game_assets, images, cfg, move |img| {
        // Tight color-key matching — check raw pixel values before filtering.
        // Threshold of 8 accommodates minor palette rounding while rejecting
        // visually distinct colours that the old loose ranges (30/200) caught.
        super::ui_assets::make_transparent_where(img, |r, g, b| match tc.as_str() {
            "black" => r < 8 && g < 8 && b < 8,
            "cyan" => r < 8 && g > 247 && b > 247,
            "lime" => r < 8 && g > 247 && b < 8,
            "red" => r > 247 && g < 8 && b < 8,
            "magenta" => r > 247 && g < 8 && b > 247,
            "blue" => r < 8 && g < 8 && b > 247,
            _ => false,
        });
    })
}
