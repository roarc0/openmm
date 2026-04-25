//! Save slot helpers: directory paths, new-game template, slot naming.

use std::error::Error;
use std::path::PathBuf;

/// Directory where save files are stored.
pub fn saves_dir() -> PathBuf {
    PathBuf::from("data/saves")
}

/// Path to the new-game template LOD (ships with MM6 data).
pub fn new_game_template() -> PathBuf {
    PathBuf::from(openmm_data::get_data_path()).join("new.lod")
}

/// Create a fresh new-game save by copying the template to autosave1.mm6.
/// Returns the path to the created save file.
pub fn create_new_game_save() -> Result<PathBuf, Box<dyn Error>> {
    let src = new_game_template();
    let dir = saves_dir();
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join("autosave1.mm6");
    std::fs::copy(&src, &dest)?;
    Ok(dest)
}

/// Full path for a named save slot, e.g. `"save000"` -> `data/saves/save000.mm6`.
/// Also checks the MM6 `Saves/` directory (sibling of the data/LOD path) as fallback.
pub fn slot_path(slot: &str) -> PathBuf {
    let filename = format!("{slot}.mm6");

    // Check openmm saves dir first
    let openmm_path = saves_dir().join(&filename);
    if openmm_path.exists() {
        return openmm_path;
    }

    // Fallback: MM6 Saves/ directory (sibling of the LOD data path)
    let data_path = std::path::PathBuf::from(openmm_data::get_data_path());
    if let Some(parent) = data_path.parent() {
        let mm6_path = parent.join("Saves").join(&filename);
        if mm6_path.exists() {
            return mm6_path;
        }
    }

    // Return openmm path even if missing (caller gets a clear "not found" error)
    openmm_path
}
