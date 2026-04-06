//! Parser for mapstats.txt — per-map configuration (encounters, traps, music, etc.).
//!
//! Tab-separated text file with 3 header lines, then one row per map.
//! Column layout (0-indexed):
//!   0: Index, 1: Name, 2: Filename, 3: ResetCount, 4: FirstVisitDay,
//!   5: RefillDays, 6: Lock(0-10), 7: Trap d20(0-10), 8: Treasure(0-6),
//!   9: EncounterChance%, 10: Mon1Enc%, 11: Mon2Enc%, 12: Mon3Enc%,
//!   13: Mon1Pic, 14: Mon1Name, 15: Mon1Dif(1-5), 16: Mon1Count(e.g."2-4"),
//!   17-20: Mon2 same, 21-24: Mon3 same, 25: RedbookTrack, 26: Designer
//!
//! ## ODM spawn groups
//!
//! The `Mon1-3Dif` and `Mon1-3Count` fields are used for **both** ODM spawn points and camping
//! interrupts. When the map loads, each ODM spawn point with `spawn_type=3` (monster) expands
//! to a group of monsters:
//!
//! - **Group size**: `MonNLow + Rand() % (MonNHi - MonNLow + 1)` — random within the range.
//!   In MM6 this uses the global MSVC LCG, so exact sizes differ each map load. Our deterministic
//!   implementation seeds from spawn position so the same spawn always produces the same group.
//!
//! - **Variant per monster**: each member independently rolls `Rand() % 100` against a
//!   difficulty-based table (A=90%/B=8%/C=2% at diff 1, … A=10%/B=50%/C=40% at diff 5).
//!   All members roll independently — there is no "champion gets the strongest variant" rule.
//!
//! - **EncounterChance%** and **Mon1-3Enc%** are used exclusively for camping interrupts
//!   (not yet implemented).

use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::Cursor;

use crate::Assets;
use crate::LodSerialise;

/// Per-map info from mapstats.txt.
/// From OpenEnroth MapInfo and MMExtension MapStatsItem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapInfo {
    /// Display name (e.g. "New Sorpigal").
    pub name: String,
    /// File name (e.g. "oute3.odm").
    pub filename: String,
    /// Monster internal name prefix for each of 3 slots (cols 13/17/21).
    /// Used to look up dmonlist.bin entries (e.g. "PeasantM2", "Goblin").
    pub monster_names: [String; 3],
    /// Human-readable monster display name for each slot (cols 14/18/22).
    /// Shown in the game UI (e.g. "Apprentice Mage", "Goblin").
    pub monster_display_names: [String; 3],
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
#[derive(Debug, Serialize, Deserialize)]
pub struct MapStats {
    pub maps: Vec<MapInfo>,
}

impl MapStats {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/mapstats.txt")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let body: String = text.lines().skip(3).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut maps = Vec::new();
        for result in rdr.records() {
            let rec = result?;

            let filename = rec.get(2).unwrap_or("").trim().to_lowercase();
            if filename.is_empty() {
                continue;
            }
            // Accept both .odm (outdoor) and .blv (indoor) maps
            if !filename.ends_with(".odm") && !filename.ends_with(".blv") {
                continue;
            }

            let name = rec.get(1).unwrap_or("").trim().to_string();
            let reset_count: u16 = cs_u16(&rec, 3);
            let first_visit_day: u16 = cs_u16(&rec, 4);
            let respawn_days: u16 = cs_u16(&rec, 5);
            let lock: u8 = cs_u8(&rec, 6);
            let trap_d20_count: u8 = cs_u8(&rec, 7);
            let treasure_level: u8 = cs_u8(&rec, 8);
            let encounter_chance: u8 = cs_u8(&rec, 9);

            // Encounter slot chances (cols 10-12)
            let enc1: u8 = cs_u8(&rec, 10);
            let enc2: u8 = cs_u8(&rec, 11);
            let enc3: u8 = cs_u8(&rec, 12);

            // Monster internal name prefixes (cols 13, 17, 21) — used to look up dmonlist.bin
            let m1_name = cs_str(&rec, 13);
            let m2_name = cs_str(&rec, 17);
            let m3_name = cs_str(&rec, 21);
            // Monster display names (cols 14, 18, 22) — shown in the game UI
            let m1_display = cs_str(&rec, 14);
            let m2_display = cs_str(&rec, 18);
            let m3_display = cs_str(&rec, 22);

            // Monster difficulty (cols 15, 19, 23)
            let m1_dif: u8 = cs_u8(&rec, 15).max(1);
            let m2_dif: u8 = cs_u8(&rec, 19).max(1);
            let m3_dif: u8 = cs_u8(&rec, 23).max(1);

            // Monster count ranges (cols 16, 20, 24) — format "min-max" or just a number
            let (min1, max1) = parse_count_range(rec.get(16).unwrap_or("0"));
            let (min2, max2) = parse_count_range(rec.get(20).unwrap_or("0"));
            let (min3, max3) = parse_count_range(rec.get(24).unwrap_or("0"));

            let music_track: u8 = cs_u8(&rec, 25);

            maps.push(MapInfo {
                name,
                filename,
                monster_names: [m1_name, m2_name, m3_name],
                monster_display_names: [m1_display, m2_display, m3_display],
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

impl TryFrom<&[u8]> for MapStats {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = match crate::assets::lod_data::LodData::try_from(data) {
            Ok(d) => d.data,
            Err(_) => data.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
    }
}

impl LodSerialise for MapStats {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        // MM6 mapstats.txt header (3 lines)
        out.push_str("Map Stats\t\t[40: EncounterChance%, Mon1Enc%, Mon2Enc%, Mon3Enc%]\r\n");
        out.push_str("0: Index\t1: Name\t2: Filename\t3: ResetCount\t4: FirstVisitDay\t5: RefillDays\t6: Lock(0-10)\t7: Trap d20(0-10)\t8: Treasure(0-6)\t9: EncounterChance%\t10: Mon1Enc%\t11: Mon2Enc%\t12: Mon3Enc%\t13: Mon1Pic\t14: Mon1Name\t15: Mon1Dif(1-5)\t16: Mon1Count(e.g.\"2-4\")\t17: Mon2Pic\t18: Mon2Name\t19: Mon2Dif(1-5)\t20: Mon2Count\t21: Mon3Pic\t22: Mon3Name\t23: Mon3Dif(1-5)\t24: Mon3Count\t25: RedbookTrack\t26: Designer\r\n");
        out.push_str(
            "0\t1\t2\t3\t4\t5\t6\t7\t8\t9\t10\t11\t12\t13\t14\t15\t16\t17\t18\t19\t20\t21\t22\t23\t24\t25\t26\r\n",
        );

        for (i, m) in self.maps.iter().enumerate() {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\r\n",
                i, m.name, m.filename, m.reset_count, m.first_visit_day, m.respawn_days,
                m.lock, m.trap_d20_count, m.treasure_level, m.encounter_chance,
                m.encounter_chances[0], m.encounter_chances[1], m.encounter_chances[2],
                m.monster_names[0], m.monster_display_names[0], m.difficulty[0],
                format!("{}-{}", m.encounter_min[0], m.encounter_max[0]),
                m.monster_names[1], m.monster_display_names[1], m.difficulty[1],
                format!("{}-{}", m.encounter_min[1], m.encounter_max[1]),
                m.monster_names[2], m.monster_display_names[2], m.difficulty[2],
                format!("{}-{}", m.encounter_min[2], m.encounter_max[2]),
                m.music_track, "Developer"
            ));
        }
        out.into_bytes()
    }
}

fn cs_str(rec: &csv::StringRecord, i: usize) -> String {
    rec.get(i).unwrap_or("").trim().to_string()
}

fn cs_u8(rec: &csv::StringRecord, i: usize) -> u8 {
    rec.get(i).unwrap_or("").trim().parse().unwrap_or(0)
}

fn cs_u16(rec: &csv::StringRecord, i: usize) -> u16 {
    rec.get(i).unwrap_or("").trim().parse().unwrap_or(0)
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
#[path = "mapstats_tests.rs"]
mod tests;

impl MapInfo {
    /// Resolve a spawn point's monster_index to (internal_name_prefix, display_name, slot, forced_variant).
    ///
    /// MM6 spawn index mapping (from MMExtension SpawnPoint.Index):
    /// - 1-3: Mon1/Mon2/Mon3 — random A/B/C per difficulty odds; caller rolls per group member
    /// - 4-6: Mon1a/Mon2a/Mon3a — forced variant A for every member
    /// - 7-9: Mon1b/Mon2b/Mon3b — forced variant B for every member
    /// - 10-12: Mon1c/Mon2c/Mon3c — forced variant C for every member
    ///
    /// Returns `(internal_prefix, display_name, slot, forced_variant)`:
    /// - `internal_prefix`: dmonlist.bin lookup key (e.g. "Goblin")
    /// - `display_name`: human-readable UI name (e.g. "Goblin")
    /// - `slot`: 0/1/2 = Mon1/Mon2/Mon3 — index into `difficulty[]` and `encounter_min/max[]`
    /// - `forced_variant`: 1=A, 2=B, 3=C for forced indices; **0** for unforced (index 1-3)
    ///   When 0, call `variant_from_roll(slot, roll)` with a 0-99 roll for probabilistic selection.
    pub fn monster_for_index(&self, index: u16) -> Option<(&str, &str, usize, u8)> {
        if index == 0 || index > 12 {
            return None;
        }

        let idx0 = (index - 1) as usize;
        let slot = idx0 % 3; // 0=Mon1, 1=Mon2, 2=Mon3
        let group = idx0 / 3; // 0=random, 1=A, 2=B, 3=C

        let internal = &self.monster_names[slot];
        if internal.is_empty() || internal == "0" {
            return None;
        }

        // Display name falls back to internal if the display name column is empty or "0".
        let display = {
            let d = &self.monster_display_names[slot];
            if d.is_empty() || d == "0" {
                internal.as_str()
            } else {
                d.as_str()
            }
        };

        let forced_variant = match group {
            0 => 0, // random — caller rolls per member with roll_variant()
            1 => 1, // forced A
            2 => 2, // forced B
            3 => 3, // forced C
            _ => return None,
        };

        Some((internal, display, slot, forced_variant))
    }

    /// Select A/B/C variant (1/2/3) from a 0-99 roll based on difficulty for this slot.
    ///
    /// MM6 probability table (verified from MM6.exe data at 0x4C0388):
    ///   diff 1: A=90%, B=8%,  C=2%
    ///   diff 2: A=70%, B=20%, C=10%
    ///   diff 3: A=50%, B=30%, C=20%
    ///   diff 4: A=30%, B=40%, C=30%
    ///   diff 5: A=10%, B=50%, C=40%
    ///
    /// Each monster in a group rolls independently — there is no "champion variant" rule.
    pub fn variant_from_roll(&self, slot: usize, roll_0_99: u8) -> u8 {
        // (A threshold, B threshold); C covers the rest.
        const TABLE: [(u8, u8); 5] = [
            (90, 8),  // diff 1
            (70, 20), // diff 2
            (50, 30), // diff 3
            (30, 40), // diff 4
            (10, 50), // diff 5
        ];
        let diff = self.difficulty[slot].clamp(1, 5) as usize;
        let (a_pct, b_pct) = TABLE[diff - 1];
        if roll_0_99 < a_pct {
            1 // A
        } else if roll_0_99 < a_pct + b_pct {
            2 // B
        } else {
            3 // C
        }
    }

    /// Group size range for a spawn slot from mapstats `#` column.
    ///
    /// Returns `(min, max)` for the number of monsters per spawn point.
    /// Falls back to `(1, 1)` when the range is zero or unset.
    pub fn count_range_for_slot(&self, slot: usize) -> (u8, u8) {
        let min = self.encounter_min[slot];
        let max = self.encounter_max[slot];
        if min == 0 && max == 0 {
            (1, 1)
        } else {
            (min.max(1), max.max(min.max(1)))
        }
    }
}
