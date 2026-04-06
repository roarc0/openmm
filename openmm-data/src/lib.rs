use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};
pub use openmm_archive::Archive;

use raw::lod::LodExt;

/// Serialise a parsed LOD structure back to its binary/text wire format.
pub trait LodSerialise {
    fn to_bytes(&self) -> Vec<u8>;
}

pub mod raw;
pub use raw::*;


// ── Game-engine API (decoded, game-ready assets) ──────────────────────────
pub mod game;

pub mod generator;
pub mod utils;
pub use utils::find_path_case_insensitive;

pub const ENV_OPENMM_6_PATH: &str = "OPENMM_6_PATH";

pub struct LodManager {
    lods: HashMap<String, raw::lod::Lod>,
    game_dir: PathBuf,
}

impl LodManager {
    pub fn new<P>(path: P) -> Result<Self, Box<dyn Error>>
    where
        P: AsRef<Path>,
    {
        let game_dir = path.as_ref().to_path_buf();
        let lod_files = Self::list_lod_files(&game_dir)?;
        let lod_map = Self::create_lod_file_map(lod_files)?;
        Ok(Self {
            lods: lod_map,
            game_dir,
        })
    }

    pub fn game_dir(&self) -> &Path {
        &self.game_dir
    }

    /// Access high-level, game-ready assets (sprites, bitmaps, icons, fonts).
    pub fn game(&self) -> game::GameLod<'_> {
        game::GameLod::new(self)
    }

    fn list_lod_files<P>(path: P) -> Result<Vec<PathBuf>, std::io::Error>
    where
        P: AsRef<Path>,
    {
        let mut lod_files = Vec::new();
        let entries = fs::read_dir(&path)?;

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            if let Some(name) = file_name.to_str()
                && name.to_lowercase().ends_with(".lod")
            {
                lod_files.push(Path::join(path.as_ref(), name));
            }
        }

        Ok(lod_files)
    }

    fn create_lod_file_map(lod_files: Vec<PathBuf>) -> Result<HashMap<String, Lod>, Box<dyn Error>> {
        let mut lod_file_map: HashMap<String, Lod> = HashMap::new();

        for path in lod_files.iter() {
            let lod = Lod::open(path)?;
            let key = path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_lowercase();
            lod_file_map.insert(key, lod);
        }

        Ok(lod_file_map)
    }

    pub fn try_get_bytes<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, Box<dyn Error>> {
        let (lod_name, file_name) = self.split_path(path.as_ref());
        if file_name.is_empty() {
            log::warn!("Attempted to fetch asset with empty filename from LOD '{}'", lod_name);
        }

        if let Some(lod) = self.lods.get(&lod_name.to_lowercase()) {
            if let Some(data) = lod.get_file(&file_name) {
                return Ok(data);
            }
        }
        Err(format!("File not found in LOD: {:?}", path.as_ref()).into())
    }

    pub fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let (lod_name, file_name) = self.split_path(path.as_ref());
        if let Some(lod) = self.lods.get(&lod_name.to_lowercase()) {
            return lod.contains(&file_name);
        }
        false
    }

    fn split_path(&self, path: &Path) -> (String, String) {
        let lod_name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        (lod_name, file_name)
    }

    pub fn get_decompressed<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, Box<dyn Error>> {
        let raw = self.try_get_bytes(path)?;
        Ok(match raw::lod_data::LodData::try_from(raw.as_slice()) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        })
    }

    pub fn palettes(&self) -> Result<raw::palette::Palettes, Box<dyn Error>> {
        let bitmaps_lod = self.lods.get("bitmaps").ok_or("expected to have bitmaps.lod")?;
        let palettes = raw::palette::Palettes::try_from(bitmaps_lod)?;
        Ok(palettes)
    }

    pub fn save_all<P: AsRef<Path>>(&self, output_dir: P) -> Result<(), Box<dyn Error>> {
        let output_dir = output_dir.as_ref();
        fs::create_dir_all(output_dir)?;
        let palettes = self.palettes()?;

        for (name, lod) in &self.lods {
            let archive_dir = output_dir.join(name);
            lod.save_all(&archive_dir, &palettes)?;
        }
        Ok(())
    }

    pub fn patch<P: AsRef<Path>>(&self, name: &str, output_path: P, overrides: &[(&str, Vec<u8>)]) -> Result<(), Box<dyn Error>> {
        let lod_path = self.game_dir.join(format!("{}.lod", name.to_uppercase()));
        crate::raw::lod::LodWriter::patch(&lod_path, output_path, overrides)
    }

    pub fn files_in(&self, lod_name: &str) -> Option<Vec<String>> {
        self.lods.get(&lod_name.to_lowercase()).map(|lod| {
            lod.list_files().iter().map(|e| e.name.clone()).collect()
        })
    }

    pub fn archives(&self) -> Vec<String> {
        self.lods.keys().cloned().collect()
    }
}

pub fn get_data_path() -> String {
    if let Ok(p) = env::var(ENV_OPENMM_6_PATH) {
        format!("{}/data", p)
    } else {
        String::from("data/mm6/data")
    }
}

