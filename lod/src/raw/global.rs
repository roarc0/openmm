//! Global game data loaded once at startup — map-independent, shared across all maps.
//!
//! `GameData::load(lod)` parses DSFT, MonsterList, MapStats, and the NPC tables once.
//! Pass `&GameData` to `Monsters::new`, `Actors::new`, and related functions to avoid
//! repeated LOD reads and binary/text parsing on every map load.

use crate::LodManager;
use std::error::Error;

pub struct GameData {
    /// Sprite-frame table (icons/dsft.bin) — 6 455 frames, ~568 KB parsed.
    pub dsft: crate::raw::dsft::DSFT,
    /// Monster species list (icons/dmonlist.bin) — sprite names, heights, speeds.
    pub monlist: crate::raw::monlist::MonsterList,
    /// Per-variant monster display names and stats (icons/monsters.txt).
    pub monsters: crate::raw::monsters::Monsters,
    /// Per-map monster configuration (icons/mapstats.txt).
    pub mapstats: crate::raw::mapstats::MapStats,
    /// Global NPC metadata table (icons/npcdata.txt). `None` if the file is missing.
    pub street_npcs: Option<crate::raw::npc::StreetNpcs>,
    /// Name pool (icons/npcnames.txt) for fallback peasant name generation.
    pub name_pool: Option<crate::raw::npc::NpcNamePool>,
    /// NPC profession definitions (icons/npcprof.txt). `None` if the file is missing.
    pub prof_table: Option<crate::raw::npcprof::NpcProfTable>,
    /// Regional NPC news lines (icons/npcnews.txt). `None` if the file is missing.
    pub news_table: Option<crate::raw::npcnews::NpcNewsTable>,
    /// Award/achievement definitions (icons/awards.txt). `None` if the file is missing.
    pub awards_table: Option<crate::raw::awards::AwardsTable>,
    /// Item definitions (icons/items.txt). `None` if the file is missing.
    pub items_table: Option<crate::raw::items::ItemsTable>,
    /// Spell definitions (icons/spells.txt). `None` if the file is missing.
    pub spells_table: Option<crate::raw::spells::SpellsTable>,
    /// Player class descriptions (icons/class.txt). `None` if the file is missing.
    pub class_table: Option<crate::raw::class::ClassTable>,
}

impl GameData {
    /// Load all global data from the LOD archive. Called once at game startup.
    pub fn load(lod: &LodManager) -> Result<Self, Box<dyn Error>> {
        let dsft = crate::raw::dsft::DSFT::load(lod)?;
        let monlist = crate::raw::monlist::MonsterList::load(lod)?;
        let monsters_txt = crate::raw::monsters::Monsters::load(lod)?;
        let mapstats = crate::raw::mapstats::MapStats::load(lod)?;

        let name_pool = lod
            .get_decompressed("icons/npcnames.txt")
            .ok()
            .and_then(|d| crate::raw::npc::NpcNamePool::parse(&d).ok());
        let street_npcs = lod
            .get_decompressed("icons/npcdata.txt")
            .ok()
            .and_then(|d| crate::raw::npc::StreetNpcs::parse(&d, name_pool.as_ref()).ok());

        let prof_table = crate::raw::npcprof::NpcProfTable::load(lod).ok();
        let news_table = crate::raw::npcnews::NpcNewsTable::load(lod).ok();
        let awards_table = crate::raw::awards::AwardsTable::load(lod).ok();
        let items_table = crate::raw::items::ItemsTable::load(lod).ok();
        let spells_table = crate::raw::spells::SpellsTable::load(lod).ok();
        let class_table = crate::raw::class::ClassTable::load(lod).ok();

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
        })
    }
}
