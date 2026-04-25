//! Save slot helpers: directory paths, new-game template, slot naming.

use bevy::prelude::*;
use openmm_data::save::file::{SaveFile, list_saves};
use std::error::Error;
use std::path::PathBuf;

use crate::screens::PropertySource;

/// Directory where OpenMM-specific save files are stored.
pub fn local_saves_dir() -> PathBuf {
    PathBuf::from("data/Saves")
}

/// Directory where original MM6 save files are stored.
pub fn original_saves_dir() -> Option<PathBuf> {
    let data_path = PathBuf::from(openmm_data::get_data_path());
    // get_data_path returns the data/ directory. Saves are typically in the root alongside it.
    let root = data_path.parent()?;
    let candidate = root.join("Saves");
    if candidate.is_dir() {
        return Some(candidate);
    }
    let candidate = root.join("saves");
    if candidate.is_dir() {
        return Some(candidate);
    }
    None
}

/// Path to the new-game template LOD (ships with MM6 data).
pub fn new_game_template() -> PathBuf {
    PathBuf::from(openmm_data::get_data_path()).join("new.lod")
}

/// Create a fresh new-game save by copying the template to autosave1.mm6.
/// Returns the path to the created save file.
pub fn create_new_game_save() -> Result<PathBuf, Box<dyn Error>> {
    let src = new_game_template();
    let dir = local_saves_dir();
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join("autosave.mm6");
    std::fs::copy(&src, &dest)?;
    Ok(dest)
}

/// Full path for a named save slot, searching multiple locations.
/// Prioritizes local saves in data/Saves/.
pub fn slot_path(slot: &str) -> PathBuf {
    let filename = format!("{slot}.mm6");
    let local = local_saves_dir().join(&filename);
    if local.exists() {
        return local;
    }
    if let Some(orig) = original_saves_dir() {
        let orig_path = orig.join(&filename);
        if orig_path.exists() {
            return orig_path;
        }
    }
    // Fallback to local path if not found anywhere (so we create it there if needed)
    local
}

#[derive(Resource, Default)]
pub struct SaveManager {
    pub saves: Vec<SaveFile>,
    pub headers: Vec<openmm_data::save::header::SaveHeader>,
    pub offset: usize,
    pub absolute_selected: Option<usize>,
}

impl SaveManager {
    pub fn refresh(&mut self) {
        let mut all_saves = list_saves(local_saves_dir());
        if let Some(orig) = original_saves_dir() {
            let orig_saves = list_saves(orig);
            // Append original saves, but avoid duplicates if slot name is same
            for s in orig_saves {
                if !all_saves.iter().any(|existing| existing.slot == s.slot) {
                    all_saves.push(s);
                }
            }
        }
        self.saves = all_saves;
        // Re-sort the combined list to ensure consistent ordering across directories
        self.saves.sort_by_key(|s| {
            let slot = &s.slot;
            match slot.as_str() {
                s if s.starts_with("autosave") => (0, s.to_string()),
                s if s.starts_with("quiksave") => (1, s.to_string()),
                s => (2, s.to_string()),
            }
        });
        self.headers = self.saves.iter().map(|s| s.header()).collect();
    }

    pub fn scroll_up(&mut self) {
        if self.offset > 0 {
            self.offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if self.saves.len() > 7 && self.offset < self.saves.len() - 7 {
            self.offset += 1;
        }
    }

    pub fn get_slot_text(&self, index: usize) -> String {
        let actual_idx = self.offset + index;
        if actual_idx < self.headers.len() {
            let name = self.headers[actual_idx].save_name.clone();
            if name.is_empty() {
                self.saves[actual_idx].slot.clone()
            } else {
                name
            }
        } else {
            String::new()
        }
    }

    pub fn get_selected_save_name(&self) -> Option<String> {
        let idx = self.absolute_selected?;
        self.saves.get(idx).map(|s| s.slot.clone())
    }
}

struct SaveSlotSource {
    slots: [String; 7],
    selected_preview: Option<String>,
    selected_location: String,
    selected_time: String,
    absolute_selected: Option<usize>,
    offset: usize,
}

impl PropertySource for SaveSlotSource {
    fn source_name(&self) -> &str {
        "saveslot"
    }

    fn resolve(&self, path: &str) -> Option<String> {
        match path {
            "preview" => self.selected_preview.clone(),
            "location" => Some(self.selected_location.clone()),
            "time" => Some(self.selected_time.clone()),
            _ => {
                // Handle saveslot[idx] and saveslot[idx].color
                let (idx_str, sub) = if let Some((idx_part, sub_part)) = path.split_once('.') {
                    (idx_part, Some(sub_part))
                } else {
                    (path, None)
                };

                let idx: usize = if idx_str.starts_with('[') && idx_str.ends_with(']') {
                    idx_str[1..idx_str.len() - 1].parse().ok()?
                } else {
                    idx_str.parse().ok()?
                };

                if sub == Some("color") {
                    let actual_idx = self.offset + idx;
                    if self.absolute_selected == Some(actual_idx) {
                        return Some("green".to_string());
                    } else {
                        return Some("white".to_string());
                    }
                }

                self.slots.get(idx).cloned()
            }
        }
    }
}

pub fn update_save_registry(
    save_manager: Res<SaveManager>,
    game_assets: Res<crate::assets::GameAssets>,
    mut registry: ResMut<crate::screens::PropertyRegistry>,
) {
    let mut slots = [
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    ];
    for i in 0..7 {
        slots[i] = save_manager.get_slot_text(i);
    }

    let mut selected_preview = None;
    let mut selected_location = String::new();
    let mut selected_time = String::new();

    if let Some(idx) = save_manager.absolute_selected {
        let actual_idx = idx;
        if let Some(save) = save_manager.saves.get(actual_idx) {
            let header = &save_manager.headers[actual_idx];
            
            selected_preview = Some(format!("saveslot:preview:{}", save.slot));

            let map_name = &header.map_name;
            let mut resolved_name = None;

            // Try lookup with extension if missing
            for ext in ["", ".odm", ".blv"] {
                let candidate = format!("{}{}", map_name, ext);
                if let Some(info) = game_assets.data().mapstats.get(&candidate) {
                    resolved_name = Some(info.name.clone());
                    break;
                }
            }

            selected_location = resolved_name.unwrap_or_else(|| {
                if header.save_name.is_empty() {
                    header.map_name.clone()
                } else {
                    header.save_name.clone()
                }
            });

            // MM6 ticks are 128 per second. 1 minute = 60s = 7680 ticks.
            let total_minutes = (header.playing_time / 7680) as u64;
            selected_time = openmm_data::utils::time::format(total_minutes);
        }
    }

    registry.register(Box::new(SaveSlotSource {
        slots,
        selected_preview,
        selected_location,
        selected_time,
        absolute_selected: save_manager.absolute_selected,
        offset: save_manager.offset,
    }));
}
