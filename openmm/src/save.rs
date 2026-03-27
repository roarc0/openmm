use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

const SAVE_DIR: &str = "target/saves";
const QUICKSAVE_FILE: &str = "quicksave.json";

/// Persistent game state that can be saved and loaded.
#[derive(Serialize, Deserialize, Clone, Debug, Resource)]
pub struct SaveData {
    pub version: u32,
    pub map: MapSave,
    pub player: PlayerSave,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapSave {
    /// Map grid coordinate, e.g. "e3"
    pub map_x: char,
    pub map_y: char,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerSave {
    pub position: [f32; 3],
    pub yaw: f32,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: 1,
            map: MapSave {
                map_x: 'e',
                map_y: '3',
            },
            player: PlayerSave {
                position: [-10178.0, 340.0, 11206.0],
                yaw: -38.7_f32.to_radians(),
            },
        }
    }
}

impl SaveData {
    fn save_dir() -> PathBuf {
        PathBuf::from(SAVE_DIR)
    }

    fn quicksave_path() -> PathBuf {
        Self::save_dir().join(QUICKSAVE_FILE)
    }

    /// Save to the quicksave file.
    pub fn quicksave(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dir = Self::save_dir();
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(Self::quicksave_path(), json)?;
        Ok(())
    }

    /// Load from the quicksave file, or return None if it doesn't exist.
    pub fn quickload() -> Option<Self> {
        let path = Self::quicksave_path();
        let data = fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Load quicksave or fall back to defaults.
    pub fn load_or_default() -> Self {
        Self::quickload().unwrap_or_default()
    }
}
