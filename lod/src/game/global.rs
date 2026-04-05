//! Global game data loaded once at startup — map-independent, shared across all maps.
//!
//! `GameData::new(lod)` parses DSFT, MonsterList, MapStats, and the NPC tables once.
//! Pass `&GameData` to `Monsters::new`, `Actors::new`, and related functions to avoid
//! repeated LOD reads and binary/text parsing on every map load.

use crate::LodManager;
use std::error::Error;

pub struct GameData {
    /// Sprite-frame table (icons/dsft.bin) — 6 455 frames, ~568 KB parsed.
    pub dsft: crate::dsft::DSFT,
    /// Monster species list (icons/dmonlist.bin) — sprite names, heights, speeds.
    pub monlist: crate::monlist::MonsterList,
    /// Per-variant monster display names and stats (icons/monsters.txt).
    pub monsters: crate::monsters::Monsters,
    /// Per-map monster configuration (icons/mapstats.txt).
    pub mapstats: crate::mapstats::MapStats,
    /// Global NPC metadata table (icons/npcdata.txt). `None` if the file is missing.
    pub street_npcs: Option<crate::game::npc::StreetNpcs>,
    /// Name pool (icons/npcnames.txt) for fallback peasant name generation.
    pub name_pool: Option<crate::game::npc::NpcNamePool>,
    /// NPC profession definitions (icons/npcprof.txt). `None` if the file is missing.
    pub prof_table: Option<crate::npcprof::NpcProfTable>,
    /// Regional NPC news lines (icons/npcnews.txt). `None` if the file is missing.
    pub news_table: Option<crate::npcnews::NpcNewsTable>,
    /// Award/achievement definitions (icons/awards.txt). `None` if the file is missing.
    pub awards_table: Option<crate::awards::AwardsTable>,
    /// Item definitions (icons/items.txt). `None` if the file is missing.
    pub items_table: Option<crate::items::ItemsTable>,
    /// Spell definitions (icons/spells.txt). `None` if the file is missing.
    pub spells_table: Option<crate::spells::SpellsTable>,
    /// Player class descriptions (icons/class.txt). `None` if the file is missing.
    pub class_table: Option<crate::class::ClassTable>,
}

impl GameData {
    /// Load all global data from the LOD archive. Called once at game startup.
    pub fn new(lod: &LodManager) -> Result<Self, Box<dyn Error>> {
        let dsft = crate::dsft::DSFT::new(lod)?;
        let monlist = crate::monlist::MonsterList::new(lod)?;
        let monsters_txt = crate::monsters::Monsters::new(lod)?;
        let mapstats = crate::mapstats::MapStats::new(lod)?;

        let name_pool = lod
            .get_decompressed("icons/npcnames.txt")
            .ok()
            .and_then(|d| crate::game::npc::NpcNamePool::parse(&d).ok());
        let street_npcs = lod
            .get_decompressed("icons/npcdata.txt")
            .ok()
            .and_then(|d| crate::game::npc::StreetNpcs::parse(&d, name_pool.as_ref()).ok());

        let prof_table = crate::npcprof::NpcProfTable::new(lod).ok();
        let news_table = crate::npcnews::NpcNewsTable::new(lod).ok();
        let awards_table = crate::awards::AwardsTable::new(lod).ok();
        let items_table = crate::items::ItemsTable::new(lod).ok();
        let spells_table = crate::spells::SpellsTable::new(lod).ok();
        let class_table = crate::class::ClassTable::new(lod).ok();

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
