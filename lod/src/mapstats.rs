//! Parser for mapstats.txt — maps spawn point indices to monster types.

use std::error::Error;

use crate::LodManager;

/// Per-map monster spawn configuration from mapstats.txt.
pub struct MapMonsterConfig {
    /// Monster picture/dmonlist prefix for each of 3 slots (1-indexed in spawn points).
    pub monster_names: [String; 3],
    /// Difficulty level for each monster (1-5, controls A/B/C variant odds).
    pub difficulty: [u8; 3],
}

/// All map stats.
pub struct MapStats {
    pub maps: Vec<(String, MapMonsterConfig)>, // (filename, config)
}

impl MapStats {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/mapstats.txt")?;
        // Try decompressing first (LOD entries may be compressed)
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let mut maps = Vec::new();
        for line in text.lines().skip(3) {
            // Skip header lines
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 25 {
                continue;
            }
            let filename = cols[2].trim().to_lowercase();
            if filename.is_empty() || !filename.ends_with(".odm") {
                continue;
            }

            // Mon1 Pic at col 13, Mon1 Dif at col 15
            // Mon2 Pic at col 17, Mon2 Dif at col 19
            // Mon3 Pic at col 21, Mon3 Dif at col 23
            let m1_name = cols.get(13).unwrap_or(&"").trim().to_string();
            let m1_dif: u8 = cols.get(15).unwrap_or(&"0").trim().parse().unwrap_or(1);
            let m2_name = cols.get(17).unwrap_or(&"").trim().to_string();
            let m2_dif: u8 = cols.get(19).unwrap_or(&"0").trim().parse().unwrap_or(1);
            let m3_name = cols.get(21).unwrap_or(&"").trim().to_string();
            let m3_dif: u8 = cols.get(23).unwrap_or(&"0").trim().parse().unwrap_or(1);

            maps.push((
                filename,
                MapMonsterConfig {
                    monster_names: [m1_name, m2_name, m3_name],
                    difficulty: [m1_dif, m2_dif, m3_dif],
                },
            ));
        }
        Ok(MapStats { maps })
    }

    /// Get monster config for a specific map file.
    pub fn get(&self, map_filename: &str) -> Option<&MapMonsterConfig> {
        let lower = map_filename.to_lowercase();
        self.maps.iter().find(|(f, _)| *f == lower).map(|(_, c)| c)
    }
}

/// Weighted odds for A/B/C variant selection per difficulty level.
/// From OpenEnroth word_4E8152: [A%, B%, C%] for each difficulty 0-5.
const VARIANT_ODDS: [[u8; 3]; 6] = [
    [100, 0, 0],   // difficulty 0: all A
    [90, 8, 2],    // difficulty 1: mostly A
    [70, 20, 10],  // difficulty 2
    [50, 30, 20],  // difficulty 3
    [30, 40, 30],  // difficulty 4
    [10, 50, 40],  // difficulty 5: mostly B/C
];

impl MapMonsterConfig {
    /// Resolve a spawn point's monster_index to (monster_name_prefix, variant).
    ///
    /// MM6 spawn index mapping (from OpenEnroth SpawnEncounter):
    /// - 1-3: Mon1/Mon2/Mon3 with random A/B/C based on difficulty odds
    /// - 4-6: Mon1/Mon2/Mon3, forced variant A
    /// - 7-9: Mon1/Mon2/Mon3, forced variant B
    /// - 10-12: Mon1/Mon2/Mon3, forced variant C
    ///
    /// Returns (name, difficulty_for_variant_selection).
    /// The `difficulty` here encodes: 1=A, 2=B, 3=C for forced variants,
    /// or the random roll result for indices 1-3.
    pub fn monster_for_index(&self, index: u16, seed: u32) -> Option<(&str, u8)> {
        if index == 0 || index > 12 { return None; }

        let idx0 = (index - 1) as usize;
        let slot = idx0 % 3; // 0=Mon1, 1=Mon2, 2=Mon3
        let group = idx0 / 3; // 0=base, 1=A, 2=B, 3=C

        let name = &self.monster_names[slot];
        if name.is_empty() || name == "0" {
            return None;
        }

        // MM6 spawn index mapping (from OpenEnroth SpawnEncounter):
        // 1-3:   Mon1/Mon2/Mon3 — base group (uses difficulty odds for A/B/C mix)
        // 4-6:   Mon1/Mon2/Mon3 — forced variant A
        // 7-9:   Mon1/Mon2/Mon3 — forced variant B
        // 10-12: Mon1/Mon2/Mon3 — forced variant C
        let variant = match group {
            0 => {
                // Base group: each individual spawn rolls A/B/C using difficulty odds.
                // The seed is per-spawn-point so the mix is deterministic across loads.
                let dif = (self.difficulty[slot] as usize).min(5);
                let odds = &VARIANT_ODDS[dif];
                // Hash the seed for better distribution (simple xorshift)
                let h = seed.wrapping_mul(2654435761); // Knuth multiplicative hash
                let roll = ((h >> 16) % 100) as u8;
                if roll < odds[0] { 1 }
                else if roll < odds[0] + odds[1] { 2 }
                else { 3 }
            }
            1 => 1, // forced A
            2 => 2, // forced B
            3 => 3, // forced C
            _ => return None,
        };

        Some((name, variant))
    }
}
