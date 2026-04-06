use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub mod archive;
pub mod smk;

pub use self::archive::lod::{LodArchive, Version};
pub use self::archive::smk::SmkArchive;
pub use self::archive::snd::SndArchive;
pub use self::archive::{Archive, ArchiveEntry};
use self::smk::SmkExt;
use crate::assets::dsounds::DSounds;
use crate::assets::image::Image;
use crate::assets::lod_data::LodData;
use crate::assets::palette::Palettes;

pub use self::archive::lod::LodWriter;

pub mod actors;
pub mod decorations;
pub mod lod_decoder;
pub mod monster;

pub use lod_decoder::LodDecoder;

/// Global game data loaded once at startup — map-independent, shared across all maps.
pub struct StaticGameData {
    /// Sprite-frame table (icons/dsft.bin)
    pub dsft: crate::assets::dsft::DSFT,
    /// Monster species list (icons/dmonlist.bin)
    pub monlist: crate::assets::dmonlist::MonsterList,
    /// Per-variant monster display names and stats (icons/monsters.txt).
    pub monsters: crate::assets::monsters::MonsterStatsTable,
    /// Per-map monster configuration (icons/mapstats.txt).
    pub mapstats: crate::assets::mapstats::MapStats,
    /// Global NPC metadata table (icons/npcdata.txt).
    pub street_npcs: Option<crate::assets::npc::StreetNpcs>,
    /// Name pool (icons/npcnames.txt) for fallback peasant name generation.
    pub name_pool: Option<crate::assets::npc::NpcNamePool>,
    /// NPC profession definitions (icons/npcprof.txt).
    pub prof_table: Option<crate::assets::npcprof::NpcProfTable>,
    /// Regional NPC news lines (icons/npcnews.txt).
    pub news_table: Option<crate::assets::npcnews::NpcNewsTable>,
    /// Award/achievement definitions (icons/awards.txt).
    pub awards_table: Option<crate::assets::awards::AwardsTable>,
    /// Item definitions (icons/items.txt).
    pub items_table: Option<crate::assets::items::ItemsTable>,
    /// Spell definitions (icons/spells.txt).
    pub spells_table: Option<crate::assets::spells::SpellsTable>,
    /// Player class descriptions (icons/class.txt).
    pub class_table: Option<crate::assets::class::ClassTable>,
    /// Decoration descriptors (icons/ddeclist.bin).
    pub ddeclist: crate::assets::ddeclist::DDecList,
}

impl StaticGameData {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let dsft = crate::assets::dsft::DSFT::load(assets)?;
        let monlist = crate::assets::dmonlist::MonsterList::load(assets)?;
        let monsters_txt = crate::assets::monsters::MonsterStatsTable::load(assets)?;
        let mapstats = crate::assets::mapstats::MapStats::load(assets)?;
        let ddeclist = crate::assets::ddeclist::DDecList::load(assets)?;

        let name_pool = assets
            .get_decompressed("icons/npcnames.txt")
            .ok()
            .and_then(|d| crate::assets::npc::NpcNamePool::parse(d.as_slice()).ok());
        let street_npcs = assets
            .get_decompressed("icons/npcdata.txt")
            .ok()
            .and_then(|d| crate::assets::npc::StreetNpcs::parse(d.as_slice(), name_pool.as_ref()).ok());

        let prof_table = crate::assets::npcprof::NpcProfTable::load(assets).ok();
        let news_table = crate::assets::npcnews::NpcNewsTable::load(assets).ok();
        let awards_table = crate::assets::awards::AwardsTable::load(assets).ok();
        let items_table = crate::assets::items::ItemsTable::load(assets).ok();
        let spells_table = crate::assets::spells::SpellsTable::load(assets).ok();
        let class_table = crate::assets::class::ClassTable::load(assets).ok();

        Ok(Self {
            dsft,
            monlist,
            monsters: monsters_txt,
            mapstats,
            street_npcs,
            name_pool,
            prof_table,
            news_table,
            awards_table,
            items_table,
            spells_table,
            class_table,
            ddeclist,
        })
    }
}

/// Unified entry point for retrieving any kind of game asset.
/// Routes requests between LOD archives, SND archives, SMK archives, and loose files.
pub struct Assets {
    lods: HashMap<String, LodArchive>,
    snds: HashMap<String, SndArchive>,
    smks: HashMap<String, SmkArchive>,
    game_dir: PathBuf,
    dsounds: Option<DSounds>,
    static_data: std::sync::OnceLock<StaticGameData>,
}

impl Assets {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let game_dir = path.as_ref().to_path_buf();

        let mut assets = Self {
            lods: HashMap::new(),
            snds: HashMap::new(),
            smks: HashMap::new(),
            game_dir,
            dsounds: None,
            static_data: std::sync::OnceLock::new(),
        };

        assets.refresh()?;
        Ok(assets)
    }

    /// Refresh the asset list by scanning the game directory.
    pub fn refresh(&mut self) -> Result<(), Box<dyn Error>> {
        let entries = fs::read_dir(&self.game_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();

                match ext.to_lowercase().as_str() {
                    "lod" => {
                        if let Ok(lod) = LodArchive::open(&path) {
                            self.lods.insert(stem, lod);
                        }
                    }
                    "snd" => {
                        if let Ok(snd) = SndArchive::open(&path) {
                            self.snds.insert(stem, snd);
                        }
                    }
                    "vid" => {
                        if let Ok(smk) = SmkArchive::open(&path) {
                            self.smks.insert(stem, smk);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Lazy load dsounds.bin if possible
        if let Ok(dsounds) = DSounds::load_from_assets(self) {
            self.dsounds = Some(dsounds);
        }

        Ok(())
    }

    pub fn game_dir(&self) -> &Path {
        &self.game_dir
    }

    /// Access high-level game tables (DSFT, Monsters, MapStats, etc.)
    pub fn data(&self) -> &StaticGameData {
        self.static_data
            .get_or_init(|| StaticGameData::load(self).expect("failed to load static game data"))
    }

    /// Access the high-level LOD decoder (decoded sprites, bitmaps, icons, fonts).
    pub fn lod(&self) -> lod_decoder::LodDecoder<'_> {
        lod_decoder::LodDecoder::new(self)
    }

    /// Find raw bytes for an asset, searching through applicable archives.
    pub fn get_bytes<P: AsRef<Path>>(&self, path_or_name: P) -> Result<Vec<u8>, Box<dyn Error>> {
        let path = path_or_name.as_ref();
        let ext = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase());
        let name = path.file_name().and_then(|s| s.to_str()).ok_or("invalid path")?;

        // 1. If it's a specific path like "icons/dsounds.bin"
        if let Some(parent) = path.parent()
            && let Some(archive_name) = parent.to_str().filter(|s| !s.is_empty())
            && let Some(lod) = self.lods.get(&archive_name.to_lowercase())
            && let Some(data) = lod.get_file(name)
        {
            return Ok(data);
        }

        // 2. Route by extension
        match ext.as_deref() {
            Some("wav") | Option::None => {
                // Try sound routing
                if let Ok(data) = self.get_sound(name) {
                    return Ok(data);
                }
            }
            Some("smk") | Some("bik") => {
                if let Ok(data) = self.get_smk(name) {
                    return Ok(data);
                }
            }
            _ => {}
        }

        // 3. Fallback: Search all LODs (expensive, but might be needed for loose lookups)
        for lod in self.lods.values() {
            if let Some(data) = lod.get_file(name) {
                return Ok(data);
            }
        }

        Err(format!("Asset not found: {:?}", path).into())
    }

    /// Specialized sound retrieval using dsounds.bin routing.
    pub fn get_sound(&self, name_or_id: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let sound_name = if let Ok(id) = name_or_id.parse::<u32>() {
            self.dsounds
                .as_ref()
                .ok_or("dsounds.bin not loaded")?
                .get_by_id(id)
                .ok_or(format!("sound id {} not found", id))?
                .name()
                .ok_or("sound name not found")?
        } else {
            name_or_id.strip_suffix(".wav").unwrap_or(name_or_id).to_string()
        };

        // Search .snd files
        for snd in self.snds.values() {
            if let Some(data) = snd.get_file(&sound_name) {
                return Ok(data);
            }
        }

        // Search .lod files (e.g. sounds.lod)
        for lod in self.lods.values() {
            if let Some(data) = lod.get_file(&sound_name) {
                return Ok(data);
            }
        }

        Err(format!("Sound not found: {}", sound_name).into())
    }

    /// Specialized Smacker video retrieval.
    pub fn get_smk(&self, name: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let name = name
            .strip_suffix(".smk")
            .or_else(|| name.strip_suffix(".bik"))
            .unwrap_or(name);
        for smk in self.smks.values() {
            if let Some(data) = smk.smk_by_name(name) {
                return Ok(data);
            }
        }
        Err(format!("Smacker video not found: {}", name).into())
    }

    pub fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        self.get_bytes(path).is_ok()
    }

    pub fn get_decompressed<P: AsRef<Path>>(&self, path: P) -> Result<Vec<u8>, Box<dyn Error>> {
        let raw = self.get_bytes(path)?;
        Ok(match LodData::try_from(raw.as_slice()) {
            Ok(d) => d.data,
            Err(_) => raw,
        })
    }

    pub fn palettes(&self) -> Result<Palettes, Box<dyn Error>> {
        Palettes::try_from(self)
    }

    pub fn archives(&self) -> Vec<String> {
        let mut names: Vec<_> = self.lods.keys().cloned().collect();
        names.extend(self.snds.keys().cloned());
        names.extend(self.smks.keys().cloned());
        names
    }

    pub fn files_in(&self, archive: &str) -> Option<Vec<String>> {
        let archive = archive.to_lowercase();
        if let Some(lod) = self.lods.get(&archive) {
            return Some(lod.list_files().iter().map(|e| e.name.clone()).collect());
        }
        if let Some(snd) = self.snds.get(&archive) {
            return Some(snd.list_files().iter().map(|e| e.name.clone()).collect());
        }
        if let Some(smk) = self.smks.get(&archive) {
            return Some(smk.list_files().iter().map(|e| e.name.clone()).collect());
        }
        None
    }

    pub fn get_lod(&self, name: &str) -> Option<&LodArchive> {
        self.lods.get(&name.to_lowercase())
    }

    pub fn lod_names(&self) -> Vec<String> {
        self.lods.keys().cloned().collect()
    }

    /// Dump all assets in an archive to disk (useful for debugging).
    pub fn dump_lod(&self, archive: &str, out_path: &Path) -> Result<(), Box<dyn Error>> {
        let lod = self.get_lod(archive).ok_or("archive not found")?;
        let palettes = self.palettes()?;
        fs::create_dir_all(out_path)?;

        for entry in lod.list_files() {
            let file_name = &entry.name;
            if let Some(data) = lod.get_file_raw(file_name) {
                if let Ok(image) = Image::try_from(data.as_slice()) {
                    if let Err(e) = image.save(out_path.join(format!("{}.png", file_name))) {
                        eprintln!("Error saving image {} : {}", file_name, e);
                    }
                } else if let Ok(sprite) = Image::try_from((data.as_slice(), &palettes)) {
                    if let Err(e) = sprite.save(out_path.join(format!("{}.png", file_name))) {
                        eprintln!("Error saving sprite {} : {}", file_name, e)
                    }
                } else if let Ok(lod_data) = LodData::try_from(data.as_slice())
                    && let Err(e) = lod_data.dump(out_path.join(file_name))
                {
                    eprintln!("Error saving lod data {} : {}", file_name, e)
                }
            }
        }
        Ok(())
    }
}
