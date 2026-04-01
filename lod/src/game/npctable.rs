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

/// Metadata for one street NPC from npcdata.txt.
#[derive(Debug, Clone)]
pub struct StreetNpcEntry {
    pub id: u32,
    pub name: String,
    /// Index for the portrait image: portrait=81 → "NPC081".
    pub portrait: u32,
    /// Profession index (into npcprof.txt).
    pub profession_id: u32,
}

/// The complete NPC data table parsed from `npcdata.txt`.
/// Keyed by NPC id (1-based).
#[derive(Debug, Default)]
pub struct StreetNpcTable {
    pub npcs: HashMap<u32, StreetNpcEntry>,
}

impl StreetNpcTable {
    /// Parse raw bytes from `npcdata.txt`.
    /// The file is Latin-1 (Windows-1252) encoded, not UTF-8.
    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
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

            npcs.insert(id, StreetNpcEntry { id, name, portrait, profession_id });
        }

        Ok(Self { npcs })
    }

    /// Look up an entry by NPC id.
    pub fn get(&self, npc_id: i32) -> Option<&StreetNpcEntry> {
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
}
