//! Parser for mapstats.txt — maps spawn point indices to monster types.

use std::error::Error;

use crate::{lod_data::LodData, LodManager};

/// Per-map monster spawn configuration from mapstats.txt.
pub struct MapMonsterConfig {
    /// Monster picture/dmonlist prefix for each of 3 slots (1-indexed in spawn points).
    pub monster_names: [String; 3],
    /// Difficulty level for each monster (1-5, maps to A/B/C variant).
    pub difficulty: [u8; 3],
}

/// All map stats.
pub struct MapStats {
    pub maps: Vec<(String, MapMonsterConfig)>, // (filename, config)
}

impl MapStats {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/mapstats.txt")?;
        let text = String::from_utf8_lossy(raw);
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

impl MapMonsterConfig {
    /// Get the dmonlist name prefix for a spawn point monster_index (1-based).
    /// Returns (name_prefix, difficulty) or None.
    pub fn monster_for_index(&self, index: u16) -> Option<(&str, u8)> {
        match index {
            1 => {
                let n = &self.monster_names[0];
                if n.is_empty() || n == "0" { None } else { Some((n, self.difficulty[0])) }
            }
            2 => {
                let n = &self.monster_names[1];
                if n.is_empty() || n == "0" { None } else { Some((n, self.difficulty[1])) }
            }
            3 => {
                let n = &self.monster_names[2];
                if n.is_empty() || n == "0" { None } else { Some((n, self.difficulty[2])) }
            }
            _ => None,
        }
    }
}
