//! Aggregate loader for `npcdata.txt` — loads and cross-references NPC names against
//! `npcnames.txt` to build sex-split peasant portrait pools.

use std::collections::HashMap;
use std::error::Error;
use std::io::Cursor;

use csv::ReaderBuilder;

use crate::Assets;
use crate::assets::npcnames::NpcNamePools;

/// A name+portrait+profession for a dynamically generated street NPC (peasant).
#[derive(Debug, Clone)]
pub struct GeneratedNpc {
    pub name: String,
    /// Direct portrait number (e.g. 42 → "NPC042").
    pub portrait: u32,
    /// Profession ID from npcprof.txt (e.g. 56 = Farmer). 0 if unknown.
    pub profession_id: u32,
}

/// Metadata for one street NPC from npcdata.txt.
#[derive(Debug, Clone)]
pub struct NpcEntry {
    pub id: u32,
    pub name: String,
    /// Index for the portrait image: portrait=81 → "NPC081".
    pub portrait: u32,
    /// NPC status (col 3): 0=active, 1=dead, etc.
    pub state: u32,
    /// Fame score (col 4).
    pub fame: i32,
    /// Reputation score (col 5).
    pub reputation: i32,
    /// 2D map location ID (col 6).
    pub location_id: u32,
    /// Profession index (into npcprof.txt) (col 7).
    pub profession_id: u32,
    /// Hire/join cost in gold (col 8).
    pub join_cost: i32,
    /// News string ID (col 9).
    pub news: u32,
    /// Event script IDs A/B/C (cols 10-12).
    pub event_a: u32,
    pub event_b: u32,
    pub event_c: u32,
}

/// The complete NPC data table parsed from `npcdata.txt`.
/// Keyed by NPC id (1-based).
#[derive(Debug, Default, Clone)]
pub struct StreetNpcs {
    pub npcs: HashMap<u32, NpcEntry>,
    /// Peasant-profession (52-77) portrait+profession pairs for female actors.
    /// Classified by cross-referencing npcdata.txt first names against npcnames.txt.
    /// Entries with unclassifiable names are excluded from both pools.
    pub peasant_female: Vec<(u32, u32)>,
    /// Peasant-profession (52-77) portrait+profession pairs for male actors.
    pub peasant_male: Vec<(u32, u32)>,
    /// Portrait IDs collected from entries with peasant professions (52-77).
    /// Used for deterministic portrait assignment to generated street NPCs.
    pub peasant_portraits: Vec<u32>,
}

impl StreetNpcs {
    /// Load from assets — reads npcdata.txt and cross-references npcnames.txt for sex classification.
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let name_pool = NpcNamePools::load(assets).ok();
        let data = assets.get_decompressed("icons/npcdata.txt")?;
        Self::parse(&data, name_pool.as_ref())
    }

    /// Parse raw bytes from `npcdata.txt`.
    /// The file is Latin-1 (Windows-1252) encoded, not UTF-8.
    /// `name_pool` (from npcnames.txt) is used to classify peasant entries by sex for
    /// sex-appropriate portrait assignment. Entries whose sex can't be determined are
    /// added to both pools as fallback portraits.
    pub fn parse(data: &[u8], name_pool: Option<&NpcNamePools>) -> Result<Self, Box<dyn Error>> {
        // Decode as Latin-1: every byte is a valid Unicode scalar value
        let text: String = data.iter().map(|&b| b as char).collect();
        let mut npcs = HashMap::new();

        // Skip the two header rows
        let body: String = text.lines().skip(2).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        for result in rdr.records() {
            let rec = result?;

            let id: u32 = match rec.get(0).unwrap_or("").trim().parse() {
                Ok(v) if v > 0 => v,
                _ => continue,
            };
            let name = rec.get(1).unwrap_or("").trim().to_string();

            npcs.insert(
                id,
                NpcEntry {
                    id,
                    name,
                    portrait: rec.get(2).unwrap_or("0").trim().parse().unwrap_or(0),
                    state: rec.get(3).unwrap_or("0").trim().parse().unwrap_or(0),
                    fame: rec.get(4).unwrap_or("0").trim().parse().unwrap_or(0),
                    reputation: rec.get(5).unwrap_or("0").trim().parse().unwrap_or(0),
                    location_id: rec.get(6).unwrap_or("0").trim().parse().unwrap_or(0),
                    profession_id: rec.get(7).unwrap_or("0").trim().parse().unwrap_or(0),
                    join_cost: rec.get(8).unwrap_or("0").trim().parse().unwrap_or(0),
                    news: rec.get(9).unwrap_or("0").trim().parse().unwrap_or(0),
                    event_a: rec.get(10).unwrap_or("0").trim().parse().unwrap_or(0),
                    event_b: rec.get(11).unwrap_or("0").trim().parse().unwrap_or(0),
                    event_c: rec.get(12).unwrap_or("0").trim().parse().unwrap_or(0),
                },
            );
        }

        // Collect portrait IDs from peasant-profession entries (52-77) for
        // deterministic portrait assignment to generated street NPCs.
        let mut peasant_portraits: Vec<u32> = npcs
            .values()
            .filter(|e| (52..=77).contains(&e.profession_id) && e.portrait > 0)
            .map(|e| e.portrait)
            .collect();
        peasant_portraits.sort();
        peasant_portraits.dedup();

        // Build sex-split portrait+profession pools from npcdata.txt peasant entries.
        // npcdata.txt has no sex column, so sex is inferred by cross-referencing each
        // entry's first name against npcnames.txt.
        // Entries whose sex is ambiguous or unknown are excluded from both pools — putting
        // them in both caused nearly 2/3 of male portraits to bleed into the female pool.
        let mut peasant_entries: Vec<&NpcEntry> = npcs
            .values()
            .filter(|e| (52..=77).contains(&e.profession_id) && e.portrait > 0)
            .collect();
        peasant_entries.sort_by_key(|e| e.id);

        let mut peasant_female: Vec<(u32, u32)> = Vec::new();
        let mut peasant_male: Vec<(u32, u32)> = Vec::new();
        for entry in peasant_entries {
            let pair = (entry.portrait, entry.profession_id);
            let sex = name_pool.and_then(|pool| {
                let first = entry.name.split_whitespace().next().unwrap_or("");
                pool.classify_name(first)
            });
            match sex {
                Some(true) => peasant_female.push(pair),
                Some(false) => peasant_male.push(pair),
                None => log::warn!(
                    "peasant NPC id={} name={:?} first={:?} — sex unclassifiable, excluded from portrait pools",
                    entry.id, entry.name, entry.name.split_whitespace().next().unwrap_or("")
                ),
            }
        }

        // Remove portraits that appear in both pools — their portrait image is shared across
        // sexes in npcdata.txt (two different NPC entries, one female-named, one male-named,
        // same portrait number). Using such portraits in both pools causes sex-mismatched
        // portraits, e.g. a female actor showing a male portrait.
        let female_portraits: std::collections::HashSet<u32> =
            peasant_female.iter().map(|(p, _)| *p).collect();
        let male_portraits: std::collections::HashSet<u32> =
            peasant_male.iter().map(|(p, _)| *p).collect();
        let shared: std::collections::HashSet<u32> =
            female_portraits.intersection(&male_portraits).copied().collect();
        if !shared.is_empty() {
            log::warn!(
                "peasant portrait pool: {} portrait(s) appear in both female and male pools — removing from both: {:?}",
                shared.len(),
                { let mut v: Vec<_> = shared.iter().copied().collect(); v.sort(); v }
            );
            peasant_female.retain(|(p, _)| !shared.contains(p));
            peasant_male.retain(|(p, _)| !shared.contains(p));
        }

        Ok(Self {
            npcs,
            peasant_female,
            peasant_male,
            peasant_portraits,
        })
    }

    /// Look up an entry by NPC id.
    pub fn get(&self, npc_id: i32) -> Option<&NpcEntry> {
        if npc_id <= 0 {
            return None;
        }
        self.npcs.get(&(npc_id as u32))
    }

    /// Returns the portrait image name for a given NPC id (e.g. "NPC081").
    pub fn portrait_name(&self, npc_id: i32) -> Option<String> {
        let entry = self.get(npc_id)?;
        if entry.portrait == 0 {
            return None;
        }
        Some(format!("NPC{:03}", entry.portrait))
    }

    /// Returns the display name for a given NPC id.
    pub fn npc_name(&self, npc_id: i32) -> Option<&str> {
        self.get(npc_id).map(|e| e.name.as_str())
    }

    /// Pick a portrait from npcdata.txt entries with peasant professions (52-77).
    /// Uses `seed` (e.g. actor array index) for deterministic selection.
    pub fn peasant_portrait(&self, seed: usize) -> Option<u32> {
        if self.peasant_portraits.is_empty() {
            return None;
        }
        Some(self.peasant_portraits[seed % self.peasant_portraits.len()])
    }

    /// Pick a peasant portrait+profession from the sex-appropriate pool.
    /// Returns (portrait_number, profession_id).
    /// For the NPC name, use `NpcNamePools::name_for(is_female, seed)` — it has sex-split lists.
    pub fn peasant_identity(&self, is_female: bool, seed: usize) -> Option<(u32, u32)> {
        let pool = if is_female {
            &self.peasant_female
        } else {
            &self.peasant_male
        };
        if pool.is_empty() {
            return None;
        }
        Some(pool[seed % pool.len()])
    }
}

#[cfg(test)]
#[path = "npc_tests.rs"]
mod tests;
