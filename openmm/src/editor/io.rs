//! Load/save .screen.ron files from data/screens/ and editor config.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::format::Screen;

const SCREENS_DIR: &str = "data/screens";
const EDITOR_CONFIG_PATH: &str = "openmm-editor.toml";

/// Persisted editor settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EditorConfig {
    /// Last screen that was open, auto-loaded on startup.
    pub last_screen: Option<String>,
    /// Whether the bitmap browser was open.
    #[serde(default)]
    pub browser_open: bool,
}

impl EditorConfig {
    pub fn load() -> Self {
        fs::read_to_string(EDITOR_CONFIG_PATH)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = fs::write(EDITOR_CONFIG_PATH, s);
        }
    }
}

fn ensure_dir() {
    let _ = fs::create_dir_all(SCREENS_DIR);
}

pub fn screen_path(id: &str) -> PathBuf {
    Path::new(SCREENS_DIR).join(format!("{}.screen.ron", id))
}

pub fn save_screen(screen: &Screen) -> Result<(), String> {
    ensure_dir();
    let path = screen_path(&screen.id);
    let ron_str = ron::ser::to_string_pretty(screen, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialize error: {e}"))?;
    fs::write(&path, &ron_str).map_err(|e| format!("Write error {}: {e}", path.display()))?;
    // Remember last screen in editor config.
    let mut cfg = EditorConfig::load();
    cfg.last_screen = Some(screen.id.clone());
    cfg.save();
    bevy::log::info!("saved screen to {}", path.display());
    Ok(())
}

/// Update the last_screen field in the editor config.
pub fn set_last_screen(id: &str) {
    let mut cfg = EditorConfig::load();
    cfg.last_screen = Some(id.to_string());
    cfg.save();
}

/// Load the last-edited screen from editor config, or return a blank screen.
pub fn load_last_screen() -> Screen {
    let cfg = EditorConfig::load();
    cfg.last_screen
        .as_deref()
        .and_then(|id| load_screen(id).ok())
        .unwrap_or_else(|| Screen::new("untitled"))
}

pub fn load_screen(id: &str) -> Result<Screen, String> {
    let path = screen_path(id);
    let contents = fs::read_to_string(&path).map_err(|e| format!("Read error {}: {e}", path.display()))?;
    ron::from_str(&contents).map_err(|e| format!("RON parse error {}: {e}", path.display()))
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
            name.strip_suffix(".screen.ron").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::format::Screen;

    #[test]
    fn save_and_load_round_trip() {
        let screen = Screen::new("test_io_roundtrip");
        save_screen(&screen).unwrap();
        let loaded = load_screen("test_io_roundtrip").unwrap();
        assert_eq!(loaded.id, "test_io_roundtrip");
        let _ = std::fs::remove_file(screen_path("test_io_roundtrip"));
    }
}
