//! Editor-specific IO: config persistence, lock state, last screen.
//! Screen loading/saving delegated to crate::screens.
//!
//! `EditorConfig` is loaded from disk once at startup and kept in memory.
//! Mutations go through helper methods that mark it dirty; `flush_config`
//! writes to disk only when needed (once per frame at most).

use std::fs;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub use crate::screens::{Screen, delete_screen, list_screens, load_screen, save_screen};

const EDITOR_CONFIG_PATH: &str = "openmm-editor.toml";

/// Persisted editor settings — kept as a Bevy Resource, flushed to disk lazily.
#[derive(Debug, Clone, Resource, Serialize, Deserialize)]
pub struct EditorConfig {
    pub last_screen: Option<String>,
    #[serde(default)]
    pub browser_open: bool,
    #[serde(default)]
    pub browser_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub edt_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub editor_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub guides: Vec<super::guides::GuideLine>,
    /// Dirty flag — true when in-memory state differs from disk.
    #[serde(skip)]
    pub dirty: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            last_screen: None,
            browser_open: false,
            browser_pos: None,
            edt_pos: None,
            editor_pos: None,
            guides: Vec::new(),
            dirty: false,
        }
    }
}

impl EditorConfig {
    /// Load config from disk (used once at startup).
    pub fn load_from_disk() -> Self {
        let mut cfg: Self = fs::read_to_string(EDITOR_CONFIG_PATH)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        cfg.dirty = false;
        cfg
    }

    /// Mark config as needing a disk write.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Write to disk if dirty, then clear the flag.
    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = fs::write(EDITOR_CONFIG_PATH, s);
        }
        self.dirty = false;
    }
}

/// System: flush config to disk once per frame (only if dirty).
pub fn flush_config(mut cfg: ResMut<EditorConfig>) {
    cfg.flush();
}

/// Update the last_screen field in the editor config.
pub fn set_last_screen(cfg: &mut EditorConfig, id: &str) {
    cfg.last_screen = Some(id.to_string());
    cfg.mark_dirty();
}

/// Load the last-edited screen from editor config, or return a blank screen.
pub fn load_last_screen(cfg: &EditorConfig) -> Screen {
    if let Some(id) = &cfg.last_screen {
        match load_screen(id) {
            Ok(screen) => return screen,
            Err(e) => bevy::log::warn!("failed to load last screen '{}': {}", id, e),
        }
    }
    Screen::new("untitled")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::screen_path;

    #[test]
    fn save_and_load_round_trip() {
        let screen = Screen::new("test_io_roundtrip");
        save_screen(&screen).unwrap();
        let loaded = load_screen("test_io_roundtrip").unwrap();
        assert_eq!(loaded.id, "test_io_roundtrip");
        let _ = std::fs::remove_file(screen_path("test_io_roundtrip"));
    }
}
