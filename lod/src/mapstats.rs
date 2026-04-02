//! Parser for mapstats.txt — per-map configuration (encounters, traps, music, etc.).
//!
//! Tab-separated text file with 3 header lines, then one row per map.
//! Column layout (0-indexed):
//!   0: Index, 1: Name, 2: Filename, 3: ResetCount, 4: FirstVisitDay,
//!   5: RefillDays, 6: Lock(0-10), 7: Trap d20(0-10), 8: Treasure(0-6),
//!   9: EncounterChance%, 10: Mon1Enc%, 11: Mon2Enc%, 12: Mon3Enc%,
//!   13: Mon1Pic, 14: Mon1Name, 15: Mon1Dif(1-5), 16: Mon1Count(e.g."2-4"),
//!   17-20: Mon2 same, 21-24: Mon3 same, 25: RedbookTrack, 26: Designer

use std::error::Error;

use crate::LodManager;

/// Per-map info from mapstats.txt.
/// From OpenEnroth MapInfo and MMExtension MapStatsItem.
pub struct MapInfo {
    /// Display name (e.g. "New Sorpigal").
    pub name: String,
    /// File name (e.g. "oute3.odm").
    pub filename: String,
    /// Monster picture/dmonlist prefix for each of 3 slots.
    pub monster_names: [String; 3],
    /// Difficulty level for each monster (1-5, controls A/B/C variant odds).
    pub difficulty: [u8; 3],
    /// Number of map resets.
    pub reset_count: u16,
    /// First visit day.
    pub first_visit_day: u16,
    /// Respawn interval in days.
    pub respawn_days: u16,
    /// Lock difficulty (0-10, "x5 Lock" from mapstats.txt).
    pub lock: u8,
    /// Trap damage (d20 count, 0-10).
    pub trap_d20_count: u8,
    /// Map treasure level (0-6).
    pub treasure_level: u8,
    /// Encounter chance when resting [0, 100].
    pub encounter_chance: u8,
    /// Per-encounter slot chances (should add to 100 or all 0).
    pub encounter_chances: [u8; 3],
    /// Min monster count per encounter slot.
    pub encounter_min: [u8; 3],
    /// Max monster count per encounter slot.
    pub encounter_max: [u8; 3],
    /// Music track ID (maps to Music/{track}.mp3). 0 = no music.
    pub music_track: u8,
}

/// All map stats.
pub struct MapStats {
    pub maps: Vec<MapInfo>,
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
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 25 {
                continue;
            }
            let filename = cols[2].trim().to_lowercase();
            if filename.is_empty() {
                continue;
            }
            // Accept both .odm (outdoor) and .blv (indoor) maps
            if !filename.ends_with(".odm") && !filename.ends_with(".blv") {
                continue;
            }

            let name = cols[1].trim().to_string();
            let reset_count: u16 = cols.get(3).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let first_visit_day: u16 = cols.get(4).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let respawn_days: u16 = cols.get(5).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let lock: u8 = cols.get(6).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let trap_d20_count: u8 = cols.get(7).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let treasure_level: u8 = cols.get(8).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let encounter_chance: u8 = cols.get(9).unwrap_or(&"0").trim().parse().unwrap_or(0);

            // Encounter slot chances (cols 10-12)
            let enc1: u8 = cols.get(10).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let enc2: u8 = cols.get(11).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let enc3: u8 = cols.get(12).unwrap_or(&"0").trim().parse().unwrap_or(0);

            // Monster names (cols 13, 17, 21)
            let m1_name = cols.get(13).unwrap_or(&"").trim().to_string();
            let m2_name = cols.get(17).unwrap_or(&"").trim().to_string();
            let m3_name = cols.get(21).unwrap_or(&"").trim().to_string();

            // Monster difficulty (cols 15, 19, 23)
            let m1_dif: u8 = cols.get(15).unwrap_or(&"0").trim().parse().unwrap_or(1);
            let m2_dif: u8 = cols.get(19).unwrap_or(&"0").trim().parse().unwrap_or(1);
            let m3_dif: u8 = cols.get(23).unwrap_or(&"0").trim().parse().unwrap_or(1);

            // Monster count ranges (cols 16, 20, 24) — format "min-max" or just a number
            let (min1, max1) = parse_count_range(cols.get(16).unwrap_or(&"0"));
            let (min2, max2) = parse_count_range(cols.get(20).unwrap_or(&"0"));
            let (min3, max3) = parse_count_range(cols.get(24).unwrap_or(&"0"));

            let music_track: u8 = cols.get(25).unwrap_or(&"0").trim().parse().unwrap_or(0);

            maps.push(MapInfo {
                name,
                filename,
                monster_names: [m1_name, m2_name, m3_name],
                difficulty: [m1_dif, m2_dif, m3_dif],
                reset_count,
                first_visit_day,
                respawn_days,
                lock,
                trap_d20_count,
                treasure_level,
                encounter_chance,
                encounter_chances: [enc1, enc2, enc3],
                encounter_min: [min1, min2, min3],
                encounter_max: [max1, max2, max3],
                music_track,
            });
        }
        Ok(MapStats { maps })
    }

    /// Get map info for a specific map file.
    pub fn get(&self, map_filename: &str) -> Option<&MapInfo> {
        let lower = map_filename.to_lowercase();
        self.maps.iter().find(|m| m.filename == lower)
    }
}

/// Parse a count range like "2-4" or " 2-4" into (min, max).
/// Falls back to (0, 0) for invalid/empty values.
fn parse_count_range(s: &str) -> (u8, u8) {
    let s = s.trim();
    if let Some((a, b)) = s.split_once('-') {
        let min: u8 = a.trim().parse().unwrap_or(0);
        let max: u8 = b.trim().parse().unwrap_or(0);
        (min, max)
    } else {
        let v: u8 = s.parse().unwrap_or(0);
        (v, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LodManager, get_lod_path};

    fn make_map_info(monster_names: [&str; 3], difficulty: [u8; 3]) -> MapInfo {
        MapInfo {
            name: "Test Map".to_string(),
            filename: "test.odm".to_string(),
            monster_names: monster_names.map(|s| s.to_string()),
            difficulty,
            reset_count: 0,
            first_visit_day: 0,
            respawn_days: 7,
            lock: 0,
            trap_d20_count: 0,
            treasure_level: 0,
            encounter_chance: 10,
            encounter_chances: [50, 30, 20],
            encounter_min: [1, 1, 1],
            encounter_max: [3, 3, 3],
            music_track: 0,
        }
    }

    #[test]
    fn parse_count_range_dash_separated() {
        assert_eq!(parse_count_range("2-4"), (2, 4));
        assert_eq!(parse_count_range(" 1-3 "), (1, 3));
        assert_eq!(parse_count_range("0-10"), (0, 10));
    }

    #[test]
    fn parse_count_range_single_value() {
        assert_eq!(parse_count_range("3"), (3, 3));
        assert_eq!(parse_count_range("0"), (0, 0));
    }

    #[test]
    fn parse_count_range_empty_is_zero() {
        assert_eq!(parse_count_range(""), (0, 0));
        assert_eq!(parse_count_range("  "), (0, 0));
    }

    #[test]
    fn monster_for_index_out_of_range_returns_none() {
        let info = make_map_info(["Goblin", "Orc", "Troll"], [2, 2, 2]);
        assert!(info.monster_for_index(0, 0).is_none());
        assert!(info.monster_for_index(13, 0).is_none());
        assert!(info.monster_for_index(100, 0).is_none());
    }

    #[test]
    fn monster_for_index_forced_a_variant() {
        // Indices 4-6 always produce variant 1 (A), regardless of seed
        let info = make_map_info(["Goblin", "Orc", "Troll"], [5, 5, 5]);
        assert_eq!(info.monster_for_index(4, 0).map(|(_, v)| v), Some(1));
        assert_eq!(info.monster_for_index(5, 99999).map(|(_, v)| v), Some(1));
        assert_eq!(info.monster_for_index(6, 42).map(|(_, v)| v), Some(1));
    }

    #[test]
    fn monster_for_index_forced_b_variant() {
        let info = make_map_info(["Goblin", "Orc", "Troll"], [1, 1, 1]);
        assert_eq!(info.monster_for_index(7, 0).map(|(_, v)| v), Some(2));
        assert_eq!(info.monster_for_index(8, 12345).map(|(_, v)| v), Some(2));
        assert_eq!(info.monster_for_index(9, 99999).map(|(_, v)| v), Some(2));
    }

    #[test]
    fn monster_for_index_forced_c_variant() {
        let info = make_map_info(["Goblin", "Orc", "Troll"], [1, 1, 1]);
        assert_eq!(info.monster_for_index(10, 0).map(|(_, v)| v), Some(3));
        assert_eq!(info.monster_for_index(11, 12345).map(|(_, v)| v), Some(3));
        assert_eq!(info.monster_for_index(12, 99999).map(|(_, v)| v), Some(3));
    }

    #[test]
    fn monster_for_index_name_slot_mapping() {
        let info = make_map_info(["Goblin", "Orc", "Troll"], [2, 2, 2]);
        assert_eq!(info.monster_for_index(1, 0).map(|(n, _)| n), Some("Goblin"));
        assert_eq!(info.monster_for_index(2, 0).map(|(n, _)| n), Some("Orc"));
        assert_eq!(info.monster_for_index(3, 0).map(|(n, _)| n), Some("Troll"));
        // Forced variants use the same slot mapping
        assert_eq!(info.monster_for_index(4, 0).map(|(n, _)| n), Some("Goblin")); // slot 0
        assert_eq!(info.monster_for_index(8, 0).map(|(n, _)| n), Some("Orc"));   // slot 1
        assert_eq!(info.monster_for_index(12, 0).map(|(n, _)| n), Some("Troll")); // slot 2
    }

    #[test]
    fn monster_for_index_empty_name_returns_none() {
        let info = make_map_info(["Goblin", "", "Troll"], [2, 2, 2]);
        // Slot 1 (Mon2) is empty — indices 2, 5, 8, 11 all map to slot 1
        assert!(info.monster_for_index(2, 0).is_none());
        assert!(info.monster_for_index(5, 0).is_none());
    }

    #[test]
    fn monster_for_index_difficulty_0_always_a() {
        // Difficulty 0 => 100% A variant
        let info = make_map_info(["Goblin", "Orc", "Troll"], [0, 0, 0]);
        for seed in [0u32, 1, 100, 99999, u32::MAX] {
            assert_eq!(info.monster_for_index(1, seed).map(|(_, v)| v), Some(1));
        }
    }

    #[test]
    fn mapstats_loads_from_lod() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let stats = MapStats::new(&lod).unwrap();
        assert!(!stats.maps.is_empty(), "mapstats should have map entries");
        // New Sorpigal (oute3.odm) should be present
        let ns = stats.get("oute3.odm");
        assert!(ns.is_some(), "oute3.odm should be in mapstats");
        let ns = ns.unwrap();
        assert!(!ns.name.is_empty(), "map name should not be empty");
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

impl MapInfo {
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

        let variant = match group {
            0 => {
                let dif = (self.difficulty[slot] as usize).min(5);
                let odds = &VARIANT_ODDS[dif];
                let h = seed.wrapping_mul(2654435761);
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
