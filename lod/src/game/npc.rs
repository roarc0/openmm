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

/// A name+portrait for a dynamically generated street NPC (peasant).
#[derive(Debug, Clone)]
pub struct GeneratedNpc {
    pub name: String,
    /// Direct portrait number (e.g. 42 → "NPC042").
    pub portrait: u32,
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
        let mut male = Vec::new();
        let mut female = Vec::new();
        let mut lines = text.lines();
        lines.next(); // skip "Male\tFemale" header
        for line in lines {
            let cols: Vec<&str> = line.split('\t').collect();
            if let Some(m) = cols.first() {
                let m = m.trim();
                if !m.is_empty() { male.push(m.to_string()); }
            }
            if let Some(f) = cols.get(1) {
                let f = f.trim();
                if !f.is_empty() { female.push(f.to_string()); }
            }
        }
        Ok(Self { male, female })
    }

    /// Pick a name deterministically from the pool using `seed` as the index.
    pub fn name_for(&self, is_female: bool, seed: usize) -> &str {
        let pool = if is_female { &self.female } else { &self.male };
        if pool.is_empty() { return "Peasant"; }
        &pool[seed % pool.len()]
    }

    /// Classify a first name as female (true), male (false), or unknown (None)
    /// by checking against the male/female name pools from npcnames.txt.
    pub fn classify_name(&self, first_name: &str) -> Option<bool> {
        let lower = first_name.to_ascii_lowercase();
        let in_female = self.female.iter().any(|n| n.to_ascii_lowercase() == lower);
        let in_male = self.male.iter().any(|n| n.to_ascii_lowercase() == lower);
        match (in_female, in_male) {
            (true, false) => Some(true),   // female
            (false, true) => Some(false),  // male
            _ => None,                     // ambiguous or not found
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
    /// Profession index (into npcprof.txt).
    pub profession_id: u32,
}

/// The complete NPC data table parsed from `npcdata.txt`.
/// Keyed by NPC id (1-based).
#[derive(Debug, Default, Clone)]
pub struct StreetNpcs {
    pub npcs: HashMap<u32, NpcEntry>,
    /// Peasant-profession entries split by sex for deterministic identity assignment.
    /// Each entry is a (name, portrait) pair from npcdata.txt with profession 52-77.
    /// Sex is determined by cross-referencing the NPC's first name against npcnames.txt pools.
    pub peasant_male: Vec<(String, u32)>,
    pub peasant_female: Vec<(String, u32)>,
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

        let mut lines = text.lines();
        // Skip the two header rows
        lines.next();
        lines.next();

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 3 {
                continue;
            }
            let id: u32 = match cols[0].trim().parse() {
                Ok(v) if v > 0 => v,
                _ => continue,
            };
            let name = cols[1].trim().to_string();
            let portrait: u32 = cols[2].trim().parse().unwrap_or(0);
            let profession_id: u32 = cols.get(7).and_then(|s| s.trim().parse().ok()).unwrap_or(0);

            npcs.insert(id, NpcEntry { id, name, portrait, profession_id });
        }

        // Collect portrait IDs from peasant-profession entries (52-77) for
        // deterministic portrait assignment to generated street NPCs.
        let mut peasant_portraits: Vec<u32> = npcs.values()
            .filter(|e| (52..=77).contains(&e.profession_id) && e.portrait > 0)
            .map(|e| e.portrait)
            .collect();
        peasant_portraits.sort();
        peasant_portraits.dedup();

        // Build sex-split peasant entry lists for identity assignment.
        // Each peasant-profession NPC from npcdata.txt is classified by cross-referencing
        // their first name against npcnames.txt male/female pools.
        let mut peasant_male = Vec::new();
        let mut peasant_female = Vec::new();

        // Collect peasant entries sorted by id for deterministic ordering
        let mut peasant_entries: Vec<&NpcEntry> = npcs.values()
            .filter(|e| (52..=77).contains(&e.profession_id) && e.portrait > 0)
            .collect();
        peasant_entries.sort_by_key(|e| e.id);

        for entry in peasant_entries {
            let pair = (entry.name.clone(), entry.portrait);
            let classified = if let Some(pool) = name_pool {
                let first = entry.name.split_whitespace().next().unwrap_or("");
                pool.classify_name(first)
            } else {
                None
            };
            match classified {
                Some(true) => peasant_female.push(pair),
                Some(false) => peasant_male.push(pair),
                None => {
                    // Unknown sex: skip rather than pollute gendered pools.
                    // ~40% of peasant-profession names aren't in npcnames.txt.
                    log::debug!("peasant NPC '{}' (pic={}) sex unknown, skipping", entry.name, entry.portrait);
                }
            }
        }

        Ok(Self { npcs, peasant_male, peasant_female, peasant_portraits })
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

    /// Pick a complete peasant identity (name + portrait) from the sex-appropriate pool.
    /// Uses a npcdata.txt entry with peasant profession, deterministically selected by `seed`.
    /// Returns (name, portrait_number).
    pub fn peasant_identity(&self, is_female: bool, seed: usize) -> Option<(&str, u32)> {
        let pool = if is_female { &self.peasant_female } else { &self.peasant_male };
        if pool.is_empty() {
            return None;
        }
        let entry = &pool[seed % pool.len()];
        Some((&entry.0, entry.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LodManager, get_lod_path};

    #[test]
    fn npc_name_pool_parse_and_lookup() {
        let data = b"Male\tFemale\nJohn\tJane\nBob\tAlice\n";
        let pool = NpcNamePool::parse(data).unwrap();
        assert_eq!(pool.name_for(false, 0), "John");
        assert_eq!(pool.name_for(false, 1), "Bob");
        assert_eq!(pool.name_for(true, 0), "Jane");
        assert_eq!(pool.name_for(true, 1), "Alice");
        // Wraps around
        assert_eq!(pool.name_for(false, 2), "John");
    }

    #[test]
    fn npc_name_pool_classify_name() {
        let data = b"Male\tFemale\nJohn\tJane\nBob\tAlice\n";
        let pool = NpcNamePool::parse(data).unwrap();
        assert_eq!(pool.classify_name("John"), Some(false)); // male
        assert_eq!(pool.classify_name("Jane"), Some(true));  // female
        assert_eq!(pool.classify_name("Unknown"), None);
        // Case insensitive
        assert_eq!(pool.classify_name("JOHN"), Some(false));
    }

    #[test]
    fn npc_name_pool_empty_pool_returns_fallback() {
        let data = b"Male\tFemale\n";
        let pool = NpcNamePool::parse(data).unwrap();
        assert_eq!(pool.name_for(false, 0), "Peasant");
        assert_eq!(pool.name_for(true, 0), "Peasant");
    }

    #[test]
    fn street_npc_table_parse_synthetic() {
        // 2 header rows, then 2 NPC entries (tab-separated, 8+ cols)
        let data = b"# header row 1\nid\tname\tpic\tstate\tfame\trep\tmap\tprof\n\
            1\tJohn Smith\t42\t0\t0\t0\t0\t52\n\
            2\tJane Doe\t81\t0\t0\t0\t0\t53\n";
        let table = StreetNpcs::parse(data, None).unwrap();
        assert_eq!(table.npcs.len(), 2);

        let e1 = table.get(1).unwrap();
        assert_eq!(e1.name, "John Smith");
        assert_eq!(e1.portrait, 42);
        assert_eq!(e1.profession_id, 52);

        assert_eq!(table.portrait_name(1), Some("NPC042".to_string()));
        assert_eq!(table.npc_name(1), Some("John Smith"));
        assert_eq!(table.npc_name(2), Some("Jane Doe"));
    }

    #[test]
    fn street_npc_table_get_invalid_id_returns_none() {
        let data = b"h1\nh2\n1\tFoo\t10\t0\t0\t0\t0\t52\n";
        let table = StreetNpcs::parse(data, None).unwrap();
        assert!(table.get(0).is_none());  // id must be > 0
        assert!(table.get(-1).is_none()); // negative id
        assert!(table.get(99).is_none()); // non-existent
    }

    #[test]
    fn street_npc_table_peasant_portraits_sorted_and_unique() {
        let data = b"h1\nh2\n\
            1\tAlice\t42\t0\t0\t0\t0\t52\n\
            2\tBob\t81\t0\t0\t0\t0\t55\n\
            3\tCarol\t42\t0\t0\t0\t0\t60\n"; // portrait 42 duplicate
        let table = StreetNpcs::parse(data, None).unwrap();
        // dedup'd: [42, 81]
        assert_eq!(table.peasant_portraits, vec![42, 81]);
    }

    #[test]
    fn street_npc_table_peasant_portrait_selection_wraps() {
        let data = b"h1\nh2\n\
            1\tAlice\t10\t0\t0\t0\t0\t52\n\
            2\tBob\t20\t0\t0\t0\t0\t55\n";
        let table = StreetNpcs::parse(data, None).unwrap();
        assert_eq!(table.peasant_portrait(0), Some(10));
        assert_eq!(table.peasant_portrait(1), Some(20));
        assert_eq!(table.peasant_portrait(2), Some(10)); // wraps
    }

    #[test]
    fn street_npc_table_from_lod() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        // npcdata.txt may be zlib-compressed in the LOD archive
        let raw = lod.get_decompressed("icons/npcdata.txt").unwrap();
        let table = StreetNpcs::parse(&raw, None).unwrap();
        assert!(!table.npcs.is_empty(), "npcdata.txt should have entries");
        // NPC id 1 should exist in MM6
        let npc1 = table.get(1).expect("NPC id 1 should exist");
        assert!(!npc1.name.is_empty(), "NPC 1 should have a name");
        assert!(npc1.portrait > 0, "NPC 1 should have a portrait");
    }

    #[test]
    fn street_npc_table_portrait_name_format() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let raw = lod.get_decompressed("icons/npcdata.txt").unwrap();
        let table = StreetNpcs::parse(&raw, None).unwrap();
        // portrait_name should format as "NPC###" with 3-digit zero-padded number
        if let Some(name) = table.portrait_name(1) {
            assert!(name.starts_with("NPC"), "portrait name should start with NPC");
            assert_eq!(name.len(), 6, "NPC+3digits = 6 chars");
        }
    }
}
