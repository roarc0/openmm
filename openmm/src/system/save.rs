use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

const SAVE_DIR: &str = "data/saves";

/// Persistent quest progress: quest bits, autonotes, gold, food.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SavedProgress {
    pub quest_bits: Vec<i32>,
    pub autonotes: Vec<i32>,
    pub gold: i32,
    pub food: i32,
}

/// Persistent game state that can be saved and loaded.
#[derive(Serialize, Deserialize, Clone, Debug, Resource)]
pub struct GameSave {
    pub version: u32,
    pub map: MapState,
    pub player: PlayerState,
    pub progress: SavedProgress,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapState {
    pub map_x: char,
    pub map_y: char,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerState {
    pub position: [f32; 3],
    pub yaw: f32,
}

impl Default for GameSave {
    fn default() -> Self {
        Self {
            version: 1,
            map: MapState { map_x: 'e', map_y: '3' },
            player: PlayerState {
                position: [-10178.0, 340.0, 11206.0],
                yaw: -38.7_f32.to_radians(),
            },
            progress: SavedProgress {
                quest_bits: Vec::new(),
                autonotes: Vec::new(),
                gold: 200,
                food: 7,
            },
        }
    }
}

impl GameSave {
    fn save_dir() -> PathBuf {
        PathBuf::from(SAVE_DIR)
    }

    fn slot_path(slot: &str) -> PathBuf {
        Self::save_dir().join(format!("{}.json", slot))
    }

    /// Save to a named slot.
    pub fn save(&self, slot: &str) -> Result<(), Box<dyn std::error::Error>> {
        let dir = Self::save_dir();
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(Self::slot_path(slot), json)?;
        Ok(())
    }

    /// Load from a named slot, or return None if it doesn't exist.
    pub fn load(slot: &str) -> Option<Self> {
        let data = fs::read_to_string(Self::slot_path(slot)).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Load the autosave slot, or fall back to defaults.
    pub fn load_or_default() -> Self {
        Self::load("autosave").unwrap_or_default()
    }

    /// Save to the autosave slot.
    pub fn autosave(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save("autosave")
    }
}
