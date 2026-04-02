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
    /// Per-map monster configuration (icons/mapstats.txt).
    pub mapstats: crate::mapstats::MapStats,
    /// Global NPC metadata table (icons/npcdata.txt). `None` if the file is missing.
    pub street_npcs: Option<crate::game::npc::StreetNpcs>,
    /// Name pool (icons/npcnames.txt) for fallback peasant name generation.
    pub name_pool: Option<crate::game::npc::NpcNamePool>,
}

impl GameData {
    /// Load all global data from the LOD archive. Called once at game startup.
    pub fn new(lod: &LodManager) -> Result<Self, Box<dyn Error>> {
        let dsft = crate::dsft::DSFT::new(lod)?;
        let monlist = crate::monlist::MonsterList::new(lod)?;
        let mapstats = crate::mapstats::MapStats::new(lod)?;

        let name_pool = lod
            .get_decompressed("icons/npcnames.txt")
            .ok()
            .and_then(|d| crate::game::npc::NpcNamePool::parse(&d).ok());
        let street_npcs = lod
            .get_decompressed("icons/npcdata.txt")
            .ok()
            .and_then(|d| crate::game::npc::StreetNpcs::parse(&d, name_pool.as_ref()).ok());

        Ok(Self {
            dsft,
            monlist,
            mapstats,
            street_npcs,
            name_pool,
        })
    }
}
