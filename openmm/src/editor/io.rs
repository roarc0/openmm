//! Editor-specific IO: config persistence, lock state, last screen.
//! Screen loading/saving delegated to crate::screens.

use std::fs;

use serde::{Deserialize, Serialize};

// Re-export shared functions so existing editor code doesn't break.
pub use crate::screens::{Screen, list_screens, load_screen, save_screen, screen_path};

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

/// Save lock state into the screen's editor section and persist the whole screen.
pub fn save_locks(screen: &mut Screen, locked: &std::collections::HashSet<String>) {
    screen.editor.locked = locked.iter().cloned().collect();
    let _ = save_screen(screen);
}

/// Load lock state from a screen's editor section.
pub fn load_locks(screen: &Screen) -> std::collections::HashSet<String> {
    screen.editor.locked.iter().cloned().collect()
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
