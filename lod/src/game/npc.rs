//! Parser for `npcdata.txt` from icons.lod — the global MM6 NPC metadata table.
//!
//! # Format
//! Tab-separated text file with 2 header rows followed by data rows.
//! Columns (0-indexed):
//!   0  = NPC id (#)
//!   1  = name
//!   2  = portrait index (Pic) — e.g. 81 → "NPC081" image
//!   3  = state
//!   4  = fame
//!   5  = reputation
//!   6  = 2D map location id
//!   7  = profession id (index into npcprof.txt)
//!   8  = join cost
//!   9  = news
//!   10 = event A
//!   11 = event B
//!   12 = event C
//!   13 = notes (may be absent)

use std::collections::HashMap;
use std::error::Error;
use std::io::Cursor;

use csv::ReaderBuilder;

/// A name+portrait+profession for a dynamically generated street NPC (peasant).
#[derive(Debug, Clone)]
pub struct GeneratedNpc {
    pub name: String,
    /// Direct portrait number (e.g. 42 → "NPC042").
    pub portrait: u32,
    /// Profession ID from npcprof.txt (e.g. 56 = Farmer). 0 if unknown.
    pub profession_id: u32,
}

/// Pool of first names from `npcnames.txt` for dynamic NPC generation.
/// The file has a "Male\tFemale" header then one pair per line.
#[derive(Clone)]
pub struct NpcNamePool {
    male: Vec<String>,
    female: Vec<String>,
}

impl NpcNamePool {
    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let text: String = data.iter().map(|&b| b as char).collect();
        // Skip "Male\tFemale" header line
        let body: String = text.lines().skip(1).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut male = Vec::new();
        let mut female = Vec::new();
        for result in rdr.records() {
            let rec = result?;
            let m = rec.get(0).unwrap_or("").trim();
            if !m.is_empty() {
                male.push(m.to_string());
            }
            let f = rec.get(1).unwrap_or("").trim();
            if !f.is_empty() {
                female.push(f.to_string());
            }
        }
        Ok(Self { male, female })
    }

    /// Pick a name deterministically from the pool using `seed` as the index.
    pub fn name_for(&self, is_female: bool, seed: usize) -> &str {
        let pool = if is_female { &self.female } else { &self.male };
        if pool.is_empty() {
            return "Peasant";
        }
        &pool[seed % pool.len()]
    }

    /// Classify a first name as female (true), male (false), or unknown (None)
    /// by checking against the male/female name pools from npcnames.txt.
    pub fn classify_name(&self, first_name: &str) -> Option<bool> {
        let lower = first_name.to_ascii_lowercase();
        let in_female = self.female.iter().any(|n| n.to_ascii_lowercase() == lower);
        let in_male = self.male.iter().any(|n| n.to_ascii_lowercase() == lower);
        match (in_female, in_male) {
            (true, false) => Some(true),  // female
            (false, true) => Some(false), // male
            _ => None,                    // ambiguous or not found
        }
    }
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
    /// Peasant-profession entries split by sex for deterministic identity assignment.
    /// Each entry is a (name, portrait, profession_id) triple from npcdata.txt with profession 52-77.
    /// Sex is determined by cross-referencing the NPC's first name against npcnames.txt pools.
    pub peasant_male: Vec<(String, u32, u32)>,
    pub peasant_female: Vec<(String, u32, u32)>,
    /// Portrait IDs collected from entries with peasant professions (52-77).
    /// Used for deterministic portrait assignment to generated street NPCs.
    pub peasant_portraits: Vec<u32>,
}

impl StreetNpcs {
    /// Parse raw bytes from `npcdata.txt`.
    /// The file is Latin-1 (Windows-1252) encoded, not UTF-8.
    /// If `name_pool` is provided, peasant-profession entries are split by sex
    /// by cross-referencing each NPC's first name against the male/female name pools.
    pub fn parse(data: &[u8], name_pool: Option<&NpcNamePool>) -> Result<Self, Box<dyn Error>> {
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

        // Build sex-split peasant entry lists for identity assignment.
        // Each peasant-profession NPC from npcdata.txt is classified by cross-referencing
        // their first name against npcnames.txt male/female pools.
        let mut peasant_male: Vec<(String, u32, u32)> = Vec::new();
        let mut peasant_female: Vec<(String, u32, u32)> = Vec::new();

        // Collect peasant entries sorted by id for deterministic ordering
        let mut peasant_entries: Vec<&NpcEntry> = npcs
            .values()
            .filter(|e| (52..=77).contains(&e.profession_id) && e.portrait > 0)
            .collect();
        peasant_entries.sort_by_key(|e| e.id);

        for entry in peasant_entries {
            let triple = (entry.name.clone(), entry.portrait, entry.profession_id);
            let classified = if let Some(pool) = name_pool {
                let first = entry.name.split_whitespace().next().unwrap_or("");
                pool.classify_name(first)
            } else {
                None
            };
            match classified {
                Some(true) => peasant_female.push(triple),
                Some(false) => peasant_male.push(triple),
                None => {
                    // Unknown sex: skip rather than pollute gendered pools.
                    // ~40% of peasant-profession names aren't in npcnames.txt.
                    log::debug!(
                        "peasant NPC '{}' (pic={}) sex unknown, skipping",
                        entry.name,
                        entry.portrait
                    );
                }
            }
        }

        Ok(Self {
            npcs,
            peasant_male,
            peasant_female,
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

    /// Pick a complete peasant identity (name + portrait + profession_id) from the sex-appropriate pool.
    /// Uses a npcdata.txt entry with peasant profession, deterministically selected by `seed`.
    /// Returns (name, portrait_number, profession_id).
    pub fn peasant_identity(&self, is_female: bool, seed: usize) -> Option<(&str, u32, u32)> {
        let pool = if is_female {
            &self.peasant_female
        } else {
            &self.peasant_male
        };
        if pool.is_empty() {
            return None;
        }
        let entry = &pool[seed % pool.len()];
        Some((&entry.0, entry.1, entry.2))
    }
}

#[cfg(test)]
#[path = "npc_tests.rs"]
mod tests;
