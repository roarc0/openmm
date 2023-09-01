use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::{self};
use std::path::{Path, PathBuf};

use ::image::DynamicImage;
use lod::Lod;
use palette::Palettes;

pub mod bsp_model;
pub mod dtile;
pub mod odm;

pub mod ddeclist;
pub mod dsft;
pub mod image;
pub mod sprite;

mod lod;
pub mod lod_data;
pub mod palette;
mod utils;
mod zlib;

pub const ENV_OPENMM_6_PATH: &str = "OPENMM_6_PATH";

pub struct LodManager {
    lods: HashMap<String, Lod>,
}

impl LodManager {
    pub fn new<P>(path: P) -> Result<Self, Box<dyn Error>>
    where
        P: AsRef<Path>,
    {
        let lod_files = Self::list_lod_files(path)?;
        let lod_map = Self::create_lod_file_map(lod_files)?;
        Ok(Self { lods: lod_map })
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
            if let Some(name) = file_name.to_str() {
                if name.to_lowercase().ends_with(".lod") {
                    lod_files.push(Path::join(path.as_ref(), name));
                }
            }
        }

        Ok(lod_files)
    }

    fn create_lod_file_map(
        lod_files: Vec<PathBuf>,
    ) -> Result<HashMap<String, Lod>, Box<dyn Error>> {
        let mut lod_file_map: HashMap<String, Lod> = HashMap::new();

        for path in lod_files.iter() {
            let lod = Lod::open(path)?;
            let key = path
                .file_stem()
                .ok_or("file should have a .lod extension")?
                .to_string_lossy()
                .to_lowercase();
            lod_file_map.insert(key, lod);
        }

        Ok(lod_file_map)
    }

    pub fn try_get_bytes<P: AsRef<Path>>(&self, path: P) -> Result<&[u8], Box<dyn Error>> {
        let lod_archive: String = path
            .as_ref()
            .parent()
            .ok_or("invalid path")?
            .to_string_lossy()
            .to_string();
        let lod = self
            .lods
            .get(&lod_archive)
            .ok_or(format!("lod file not found in {lod_archive} "))?;
        let lod_entry: String = path
            .as_ref()
            .file_name()
            .ok_or("invalid lod entry")?
            .to_string_lossy()
            .to_string();
        let lod_data = lod.try_get_bytes(&lod_entry).ok_or(format!(
            "unable to open lod entry {:?}",
            path.as_ref().to_str()
        ))?;
        Ok(lod_data)
    }

    fn palettes(&self) -> Result<Palettes, Box<dyn Error>> {
        // TODO cache palettes
        let bitmaps_lod = self
            .lods
            .get("bitmaps")
            .ok_or("expected to have bitmaps.lod")?;
        let palettes = palette::Palettes::try_from(bitmaps_lod)?;
        Ok(palettes)
    }

    pub fn sprite(&self, name: &str) -> Option<DynamicImage> {
        let sprite = self.try_get_bytes(format!("sprites/{}", name)).ok()?;
        let palettes = self.palettes().ok()?;
        let sprite = crate::image::Image::try_from((sprite, &palettes)).ok()?;
        sprite.to_image_buffer().ok()
    }
}

pub fn get_data_path() -> String {
    env::var(ENV_OPENMM_6_PATH).unwrap_or("./target/mm6".into())
}

pub fn get_lod_path() -> String {
    env::var(ENV_OPENMM_6_PATH).unwrap_or("./target/mm6/data".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_manager_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let grastyl = lod_manager.try_get_bytes("bitmaps/grastyl");
        assert_eq!(17676, grastyl.unwrap().len());
    }

    #[test]
    fn sprite_works() {
        let lod_path = get_lod_path();
        let lod_manager = LodManager::new(lod_path).unwrap();
        let rock = lod_manager.sprite("rok1");
        assert!(rock.is_some());
    }
}
