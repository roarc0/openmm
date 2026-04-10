//! Editor-specific IO: config persistence, lock state, last screen.
//! Screen loading/saving delegated to crate::screens.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// Re-export shared functions so existing editor code doesn't break.
pub use crate::screens::{Screen, list_screens, load_screen, save_screen, screen_path};

const SCREENS_DIR: &str = "openmm/assets";
const EDITOR_CONFIG_PATH: &str = "openmm-editor.toml";

/// Persisted editor settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EditorConfig {
    pub last_screen: Option<String>,
    #[serde(default)]
    pub browser_open: bool,
    #[serde(default)]
    pub browser_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub edt_pos: Option<[f32; 2]>,
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

/// Update the last_screen field in the editor config.
pub fn set_last_screen(id: &str) {
    let mut cfg = EditorConfig::load();
    cfg.last_screen = Some(id.to_string());
    cfg.save();
}

/// Load the last-edited screen from editor config, or return a blank screen.
pub fn load_last_screen() -> Screen {
    let cfg = EditorConfig::load();
    if let Some(id) = &cfg.last_screen {
        match load_screen(id) {
            Ok(screen) => return screen,
            Err(e) => bevy::log::warn!("failed to load last screen '{}': {}", id, e),
        }
    }
    Screen::new("untitled")
}

/// Save screen and update last_screen in editor config.
pub fn save_screen_with_config(screen: &Screen) -> Result<(), String> {
    save_screen(screen)?;
    let mut cfg = EditorConfig::load();
    cfg.last_screen = Some(screen.id.clone());
    cfg.save();
    Ok(())
}

/// Per-element editor-only properties, stored alongside the screen RON.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScreenEditorData {
    #[serde(default)]
    pub locked: Vec<String>,
}

fn editor_data_path(screen_id: &str) -> PathBuf {
    Path::new(SCREENS_DIR).join(format!("{}.editor.ron", screen_id))
}

pub fn save_editor_data(screen_id: &str, data: &ScreenEditorData) {
    let _ = fs::create_dir_all(SCREENS_DIR);
    let path = editor_data_path(screen_id);
    if let Ok(s) = ron::ser::to_string_pretty(data, ron::ser::PrettyConfig::default()) {
        let _ = fs::write(&path, s);
    }
}

pub fn load_editor_data(screen_id: &str) -> ScreenEditorData {
    let path = editor_data_path(screen_id);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| ron::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_locks(screen_id: &str, locked: &std::collections::HashSet<String>) {
    let data = ScreenEditorData {
        locked: locked.iter().cloned().collect(),
    };
    save_editor_data(screen_id, &data);
}

pub fn load_locks(screen_id: &str) -> std::collections::HashSet<String> {
    load_editor_data(screen_id).locked.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_load_round_trip() {
        let screen = Screen::new("test_io_roundtrip");
        save_screen(&screen).unwrap();
        let loaded = load_screen("test_io_roundtrip").unwrap();
        assert_eq!(loaded.id, "test_io_roundtrip");
        let _ = std::fs::remove_file(screen_path("test_io_roundtrip"));
    }
}
