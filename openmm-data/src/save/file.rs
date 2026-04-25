//! High-level wrapper around an MM6 save LOD archive.
//!
//! Provides typed accessors for save chunks (header, party, clock)
//! and a `write_patched` helper for modifying saves.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::assets::provider::archive::Archive;
use crate::assets::provider::archive::lod::{LodArchive, LodWriter};
use image::DynamicImage;

use super::clock::SaveClock;
use super::header::SaveHeader;
use super::party::SaveParty;

/// An opened MM6 save file (.mm6 LOD archive).
pub struct SaveFile {
    lod: LodArchive,
    /// Filename stem (slot name), e.g. `"save000"`, `"autosave"`, `"quiksave"`.
    pub slot: String,
    pub path: PathBuf,
}

impl SaveFile {
    /// Open a .mm6 save file from disk.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let path = path.as_ref().to_path_buf();
        let slot = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let lod = LodArchive::open(&path)?;
        Ok(Self { lod, slot, path })
    }

    /// Parse the save header from `header.bin`.
    pub fn header(&self) -> SaveHeader {
        let data = self
            .lod
            .get_file("header.bin")
            .unwrap_or_else(|| panic!("save '{}' missing header.bin", self.path.display()));
        SaveHeader::parse(&data)
    }

    /// Parse the party data from `party.bin`.
    pub fn party(&self) -> SaveParty {
        let data = self
            .lod
            .get_file("party.bin")
            .unwrap_or_else(|| panic!("save '{}' missing party.bin", self.path.display()));
        SaveParty::parse(&data)
    }

    /// Parse the clock data from `clock.bin`.
    pub fn clock(&self) -> SaveClock {
        self.lod
            .get_file("clock.bin")
            .as_deref()
            .map(SaveClock::parse)
            .unwrap_or_else(|| SaveClock::parse(&[]))
    }

    /// Decode the save screenshot (`image.pcx`), or `None` if absent/corrupt.
    pub fn screenshot(&self) -> Option<DynamicImage> {
        let data = self.lod.get_file("image.pcx")?;
        crate::assets::pcx::decode(&data)
    }

    /// Get raw file data by exact name.
    pub fn get_file(&self, name: &str) -> Option<Vec<u8>> {
        self.lod.get_file(name)
    }

    /// Get raw file data by case-insensitive name lookup.
    pub fn get_file_ci(&self, name: &str) -> Option<Vec<u8>> {
        self.lod.get_file_case_insensitive(name)
    }

    /// List all file names in the save archive.
    pub fn list_files(&self) -> Vec<String> {
        self.lod.list_files().iter().map(|e| e.name.clone()).collect()
    }

    /// Detect the current map from the save archive.
    ///
    /// MM6 doesn't store the current map name explicitly. Instead, the LOD
    /// directory entry for `party.bin` shares its `name_tail` bytes (12-15 of
    /// the 16-byte name field) with the DDM/DLV of the current map — both are
    /// written in the same save cycle. We find the map file whose name_tail
    /// matches party.bin's.
    ///
    /// Returns the map filename stem (e.g. "oute3", "d01"), or None if
    /// detection fails.
    pub fn detect_current_map(&self) -> Option<String> {
        use crate::assets::provider::archive::Archive;
        let entries = self.lod.list_files();

        // Find party.bin's name_tail
        let party_tail = entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case("party.bin"))?
            .name_tail;

        // Find DDM/DLV entries whose name_tail matches
        let candidates: Vec<&str> = entries
            .iter()
            .filter(|e| {
                let lower = e.name.to_lowercase();
                (lower.ends_with(".ddm") || lower.ends_with(".dlv")) && e.name_tail == party_tail
            })
            .map(|e| e.name.as_str())
            .collect();

        // If exactly one match, use it. If multiple (e.g. all zeros for
        // unvisited maps), fall back to header.bin.
        let map_file = if candidates.len() == 1 {
            candidates[0]
        } else {
            // Fallback: use header.bin map name
            let header = self.header();
            let stem = header.map_stem().to_string();
            log::info!(
                "detect_current_map: {} DDM/DLV candidates with name_tail {:?}, falling back to header '{}'",
                candidates.len(),
                party_tail,
                stem
            );
            return Some(stem);
        };

        // Strip extension to get stem
        let stem = map_file.rsplit_once('.').map(|(s, _)| s).unwrap_or(map_file);
        log::info!(
            "detect_current_map: matched '{}' via name_tail {:?}",
            map_file,
            party_tail
        );
        Some(stem.to_lowercase())
    }

    /// Write a patched copy of `src` save to `dest`, replacing named entries.
    ///
    /// Entries not in `overrides` are copied verbatim from `src`.
    pub fn write_patched<P: AsRef<Path>>(
        src: &Path,
        dest: P,
        overrides: &[(&str, Vec<u8>)],
    ) -> Result<(), Box<dyn Error>> {
        LodWriter::patch(src, dest, overrides)
    }
}

/// Open all `.mm6` save files from `dir`, sorted by slot name.
///
/// Sort order: autosave first, then quiksave, then alphabetical.
pub fn list_saves<P: AsRef<Path>>(dir: P) -> Vec<SaveFile> {
    let dir = dir.as_ref();
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut saves: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("mm6"))
                .unwrap_or(false)
        })
        .map(|e| {
            let stem = e
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            (stem, e.path())
        })
        .collect();

    saves.sort_by_key(|a| save_slot_order(&a.0));

    saves.into_iter().filter_map(|(_, p)| SaveFile::open(&p).ok()).collect()
}

/// Ordering key: autosave(0), quiksave(1), everything else(2). Alphabetical within tier.
fn save_slot_order(slot: &str) -> (u8, String) {
    match slot {
        s if s.starts_with("autosave") => (0, s.to_string()),
        s if s.starts_with("quiksave") => (1, s.to_string()),
        s => (2, s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEW_LOD: &str = "../data/mm6/data/new.lod";

    #[test]
    fn open_new_lod() {
        let save = SaveFile::open(NEW_LOD).expect("failed to open new.lod");
        let header = save.header();
        assert_eq!(header.map_name, "oute3.odm");
    }

    #[test]
    fn party_access() {
        let save = SaveFile::open(NEW_LOD).expect("failed to open new.lod");
        let party = save.party();
        // new.lod default starting gold
        assert!(party.gold > 0, "gold should be positive");
    }

    #[test]
    fn clock_access() {
        let save = SaveFile::open(NEW_LOD).expect("failed to open new.lod");
        let clock = save.clock();
        let bytes = clock.to_bytes();
        assert_eq!(bytes.len(), super::super::clock::CLOCK_SIZE);
    }

    #[test]
    fn list_files_new_lod() {
        let save = SaveFile::open(NEW_LOD).expect("failed to open new.lod");
        let files = save.list_files();
        assert!(files.iter().any(|f| f == "header.bin"), "should contain header.bin");
        assert!(files.iter().any(|f| f == "clock.bin"), "should contain clock.bin");
    }

    #[test]
    fn get_file_ci_ddm() {
        let save = SaveFile::open(NEW_LOD).expect("failed to open new.lod");
        // case-insensitive lookup should find DDM regardless of case
        let lower = save.get_file_ci("oute3.ddm");
        let upper = save.get_file_ci("OUTE3.DDM");
        assert!(lower.is_some(), "lowercase DDM lookup should succeed");
        assert!(upper.is_some(), "uppercase DDM lookup should succeed");
        assert_eq!(lower.unwrap().len(), upper.unwrap().len());
    }
}
