use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

pub mod archive;

pub use self::archive::lod::{LodArchive, Version};
pub use self::archive::smk::SmkArchive;
pub use self::archive::snd::SndArchive;
pub use self::archive::{Archive, ArchiveEntry};
use crate::assets::dsounds::DSounds;
use crate::assets::image::Image;
use crate::assets::lod_data::LodData;
use crate::assets::palette::Palettes;

pub use self::archive::lod::LodWriter;

pub mod actors;
pub mod decorations;
pub mod lod_decoder;
pub mod monster;
pub mod npc;

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
    pub street_npcs: Option<npc::StreetNpcs>,
    /// Name pool (icons/npcnames.txt) for fallback peasant name generation.
    pub name_pool: Option<crate::assets::npcnames::NpcNamePools>,
    /// NPC profession definitions (icons/npcprof.txt).
    pub prof_table: Option<crate::assets::npcprof::NpcProfTable>,
    /// Regional NPC news lines (icons/npcnews.txt).
    pub news_table: Option<crate::assets::npcnews::NpcNewsTable>,
    /// Award/achievement definitions (icons/awards.txt).
    pub awards_table: Option<crate::assets::awards::AwardsTable>,
    /// Auto-journal entry definitions (icons/autonotes.txt).
    pub autonotes_table: Option<crate::assets::autonotes::AutonotesTable>,
    /// Item definitions (icons/items.txt).
    pub items_table: Option<crate::assets::items::ItemsTable>,
    /// Spell definitions (icons/spells.txt).
    pub spells_table: Option<crate::assets::spells::SpellsTable>,
    /// Player class descriptions (icons/class.txt).
    pub class_table: Option<crate::assets::class::ClassTable>,
    /// NPC beg/bribe/threat flags and greeting messages (icons/npcbtb.txt).
    pub npcbtb_table: Option<crate::assets::npcbtb::NpcBtbTable>,
    /// NPC dialogue text strings (icons/npctext.txt).
    pub npctext_table: Option<crate::assets::npctext::NpcTextTable>,
    /// NPC dialogue topic labels (icons/npctopic.txt).
    pub npctopic_table: Option<crate::assets::npctopic::NpcTopicTable>,
    /// NPC profession day-of-week dialogue (icons/PROFTEXT.txt).
    pub proftext_table: Option<crate::assets::proftext::ProfTextTable>,
    /// Scroll item text strings (icons/scroll.txt).
    pub scroll_table: Option<crate::assets::scroll::ScrollTable>,
    /// Area transition descriptions (icons/trans.txt).
    pub trans_table: Option<crate::assets::trans::TransTable>,
    /// Dungeon password questions and answers (icons/passwords.txt).
    pub passwords_table: Option<crate::assets::passwords::PasswordsTable>,
    /// Merchant dialogue strings (icons/merchant.txt).
    pub merchant_table: Option<crate::assets::merchant::MerchantTable>,
    /// Decoration descriptors (icons/ddeclist.bin).
    pub ddeclist: crate::assets::ddeclist::DDecList,
    /// QBit ID → human-readable label (icons/quests.txt).
    pub quests: crate::assets::quests::QuestNames,
}

impl StaticGameData {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let dsft = crate::assets::dsft::DSFT::load(assets)?;
        let monlist = crate::assets::dmonlist::MonsterList::load(assets)?;
        let monsters_txt = crate::assets::monsters::MonsterStatsTable::load(assets)?;
        let mapstats = crate::assets::mapstats::MapStats::load(assets)?;
        let ddeclist = crate::assets::ddeclist::DDecList::load(assets)?;

        let name_pool = crate::assets::npcnames::NpcNamePools::load(assets).ok();
        let street_npcs = npc::StreetNpcs::load(assets).ok();

        let prof_table = crate::assets::npcprof::NpcProfTable::load(assets).ok();
        let news_table = crate::assets::npcnews::NpcNewsTable::load(assets).ok();
        let awards_table = crate::assets::awards::AwardsTable::load(assets).ok();
        let autonotes_table = crate::assets::autonotes::AutonotesTable::load(assets).ok();
        let items_table = crate::assets::items::ItemsTable::load(assets).ok();
        let spells_table = crate::assets::spells::SpellsTable::load(assets).ok();
        let class_table = crate::assets::class::ClassTable::load(assets).ok();
        let npcbtb_table = crate::assets::npcbtb::NpcBtbTable::load(assets).ok();
        let npctext_table = crate::assets::npctext::NpcTextTable::load(assets).ok();
        let npctopic_table = crate::assets::npctopic::NpcTopicTable::load(assets).ok();
        let proftext_table = crate::assets::proftext::ProfTextTable::load(assets).ok();
        let scroll_table = crate::assets::scroll::ScrollTable::load(assets).ok();
        let trans_table = crate::assets::trans::TransTable::load(assets).ok();
        let passwords_table = crate::assets::passwords::PasswordsTable::load(assets).ok();
        let merchant_table = crate::assets::merchant::MerchantTable::load(assets).ok();

        let quests = crate::assets::quests::QuestNames::load(assets)
            .unwrap_or_else(|_| crate::assets::quests::QuestNames { names: vec![] });

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
            autonotes_table,
            items_table,
            spells_table,
            class_table,
            npcbtb_table,
            npctext_table,
            npctopic_table,
            proftext_table,
            scroll_table,
            trans_table,
            passwords_table,
            merchant_table,
            ddeclist,
            quests,
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
    /// Palettes loaded once at startup — reloading all ~200 palette files per sprite decode was a perf killer.
    palettes_cache: Option<Palettes>,
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
            palettes_cache: None,
        };

        assets.refresh()?;
        assets.palettes_cache = Palettes::try_from(&assets).ok();
        Ok(assets)
    }

    /// Refresh the asset list by scanning the game directory and known sibling
    /// directories (e.g. `Anims/`) for LOD, SND, and VID archives.
    pub fn refresh(&mut self) -> Result<(), Box<dyn Error>> {
        self.scan_dir(&self.game_dir.clone())?;

        // Also scan sibling directories of the game data path for archives.
        // MM6 stores Smacker videos in `Anims/` and sound archives in `Sounds/`.
        let sibling_dirs: Vec<_> = if let Some(parent) = self.game_dir.parent() {
            ["Anims", "Sounds"]
                .iter()
                .filter_map(|name| crate::utils::find_path_case_insensitive(parent, name))
                .filter(|p| p.is_dir())
                .collect()
        } else {
            vec![]
        };
        for dir in sibling_dirs {
            self.scan_dir(&dir)?;
        }

        // Lazy load dsounds.bin if possible
        if let Ok(dsounds) = DSounds::load(self) {
            self.dsounds = Some(dsounds);
        }

        Ok(())
    }

    /// Scan a single directory for LOD, SND, and VID archives.
    fn scan_dir(&mut self, dir: &Path) -> Result<(), Box<dyn Error>> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()), // directory may not exist
        };

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
            if let Some(data) = smk.get_file(name) {
                return Ok(data);
            }
        }
        Err(format!("Smacker video not found: {}", name).into())
    }

    /// Retrieve a music file by track name (e.g. `"13"` → `Music/13.mp3`).
    ///
    /// Searches for `Music/{track}.mp3` under the game directory's parent,
    /// case-insensitively (for Linux compatibility with Windows-era paths).
    pub fn get_music(&self, track: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let parent = self.game_dir.parent().unwrap_or(&self.game_dir);
        for dir in &["Music", "Sounds"] {
            let rel = format!("{}/{}.mp3", dir, track);
            if let Some(path) = crate::utils::find_path_case_insensitive(parent, &rel) {
                return Ok(fs::read(&path)?);
            }
        }
        Err(format!("Music not found: Music/{t}.mp3 or Sounds/{t}.mp3", t = track).into())
    }

    /// Decode all audio from an SMK video into a WAV buffer.
    ///
    /// Looks up the video by name in the loaded VID archives, then extracts
    /// audio via `SmkDecoder::extract_audio_wav`.  Returns `None` if the video
    /// is not found or has no audio.
    pub fn video_audio_wav(&self, name: &str) -> Option<Vec<u8>> {
        let smk_bytes = self.get_smk(name).ok()?;
        crate::assets::SmkDecoder::extract_audio_wav(&smk_bytes)
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

    pub fn palettes(&self) -> Result<&Palettes, Box<dyn Error>> {
        self.palettes_cache.as_ref().ok_or_else(|| "palettes not loaded".into())
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

    /// Retrieve an already-opened SND archive by stem name (e.g. `"audio"` for `Audio.snd`).
    pub fn get_snd(&self, name: &str) -> Option<&SndArchive> {
        self.snds.get(&name.to_lowercase())
    }

    /// Access the loaded dsounds.bin table (sound ID → filename mapping).
    pub fn dsounds(&self) -> Option<&DSounds> {
        self.dsounds.as_ref()
    }

    /// O(1) check: does `name` exist in the given LOD archive?
    /// No allocation, no decoding — just a hash lookup.
    pub fn lod_contains(&self, archive: &str, name: &str) -> bool {
        self.lods
            .get(&archive.to_lowercase())
            .is_some_and(|lod| lod.contains(name))
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
            let logical = format!("{}/{}", archive, file_name);
            let Some(data) = lod.get_file_raw(file_name) else {
                log::warn!("dump_lod {}: missing raw data", logical);
                continue;
            };

            if let Ok(image) = Image::try_from(data.as_slice()) {
                let dest = out_path.join(format!("{}.png", file_name));
                match image.save(&dest) {
                    Ok(()) => log::info!("dump_lod {} -> {} (bitmap)", logical, dest.display()),
                    Err(e) => log::warn!("dump_lod {}: failed to save bitmap: {}", logical, e),
                }
            } else if let Ok(sprite) = Image::try_from((data.as_slice(), palettes)) {
                let dest = out_path.join(format!("{}.png", file_name));
                match sprite.save(&dest) {
                    Ok(()) => log::info!("dump_lod {} -> {} (sprite)", logical, dest.display()),
                    Err(e) => log::warn!("dump_lod {}: failed to save sprite: {}", logical, e),
                }
            } else if let Ok(lod_data) = LodData::try_from(data.as_slice()) {
                let dest = out_path.join(file_name);
                match lod_data.dump(&dest) {
                    Ok(()) => log::info!("dump_lod {} -> {} (lod_data)", logical, dest.display()),
                    Err(e) => log::warn!("dump_lod {}: failed to dump lod_data: {}", logical, e),
                }
            } else {
                log::warn!(
                    "dump_lod {}: no bitmap/sprite/lod_data decoder matched ({} bytes)",
                    logical,
                    data.len()
                );
            }
        }
        Ok(())
    }
}
