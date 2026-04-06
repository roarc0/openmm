use std::{
    error::Error,
    fs::{self},
    path::Path,
};

use crate::raw::{lod_data::LodData, palette};
use openmm_archive::Archive;

pub use openmm_archive::lod::*;

pub type Lod = LodArchive;

pub trait LodExt {
    fn try_get_bytes<'a>(&'a self, name: &str) -> Option<Vec<u8>>;
    fn save_all(&self, path: &Path, palettes: &palette::Palettes) -> Result<(), Box<dyn Error>>;
}

impl LodExt for LodArchive {
    fn try_get_bytes<'a>(&'a self, name: &str) -> Option<Vec<u8>> {
        self.get_file_case_insensitive(name)
    }

    fn save_all(&self, path: &Path, palettes: &palette::Palettes) -> Result<(), Box<dyn Error>> {
        fs::create_dir_all(path)?;
        for entry in self.list_files() {
            let file_name = &entry.name;
            if let Some(data) = self.get_file_raw(file_name) {
                if let Ok(image) = crate::raw::image::Image::try_from(data.as_slice()) {
                    if let Err(e) = image.save(path.join(format!("{}.png", file_name))) {
                        println!("Error saving image {} : {}", file_name, e);
                    }
                } else if let Ok(sprite) = crate::raw::image::Image::try_from((data.as_slice(), palettes)) {
                    if let Err(e) = sprite.save(path.join(format!("{}.png", file_name))) {
                        println!("Error saving sprite {} : {}", file_name, e)
                    }
                } else if let Ok(lod_data) = LodData::try_from(data.as_slice())
                    && let Err(e) = lod_data.dump(path.join(file_name))
                {
                    println!("Error saving lod data {} : {}", file_name, e)
                }
            }
        }
        Ok(())
    }
}
