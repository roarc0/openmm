//! Parser for monsters.txt — per-variant monster display names and stats.
//!
//! Tab-separated text with 3 header lines then one row per monster variant.
//! Column layout (0-indexed):
//!   0: row #, 1: internal name (e.g. "PeasantM2A"), 2: display name (e.g. "Apprentice Mage"),
//!   3: LVL, 4: HP, 5: AC, 6: EXP, ... (stats, combat, resistances — parsed as needed)

use std::collections::HashMap;
use std::error::Error;

use crate::LodManager;

/// Per-entry stats from monsters.txt, keyed by full internal name (e.g. "GoblinA").
struct MonsterEntry {
    display_name: String,
    /// Max HP from column 4.
    hp: i16,
}

/// Display names and basic stats keyed by internal monster name (e.g. "PeasantM2A").
pub struct MonstersTxt {
    entries: HashMap<String, MonsterEntry>,
}

impl MonstersTxt {
    pub fn new(lod: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod.try_get_bytes("icons/monsters.txt")?;
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let mut entries = HashMap::new();
        // Skip 2 header lines (description + column names); data starts at line 3.
        for line in text.lines().skip(2) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 5 {
                continue;
            }
            let internal = cols[1].trim();
            let display = cols[2].trim();
            // Skip empty rows, blank separators, and single-letter category markers (e.g. "A").
            if internal.len() < 2 || display.is_empty() {
                continue;
            }
            let hp: i16 = cols[4].trim().parse().unwrap_or(1);
            entries.insert(
                internal.to_string(),
                MonsterEntry {
                    display_name: display.to_string(),
                    hp,
                },
            );
        }
        Ok(MonstersTxt { entries })
    }

    /// Look up the display name for a specific monster variant.
    ///
    /// `internal_prefix` is the dmonlist prefix (e.g. "PeasantM2"), `variant` is 1=A, 2=B, 3=C.
    /// Falls back to `None` if the entry is not in monsters.txt.
    pub fn display_name(&self, internal_prefix: &str, variant: u8) -> Option<&str> {
        let key = self.key(internal_prefix, variant);
        self.entries.get(&key).map(|e| e.display_name.as_str())
    }

    /// Look up the max HP for a specific monster variant.
    ///
    /// `internal_prefix` is the dmonlist prefix (e.g. "Goblin"), `variant` is 1=A, 2=B, 3=C.
    /// Falls back to `None` if the entry is not in monsters.txt.
    pub fn max_hp(&self, internal_prefix: &str, variant: u8) -> Option<i16> {
        let key = self.key(internal_prefix, variant);
        self.entries.get(&key).map(|e| e.hp)
    }

    fn key(&self, prefix: &str, variant: u8) -> String {
        let suffix = match variant {
            1 => "A",
            2 => "B",
            _ => "C",
        };
        format!("{}{}", prefix, suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_lod;

    #[test]
    fn peasant_m2_variants_have_distinct_names() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        assert_eq!(txt.display_name("PeasantM2", 1), Some("Apprentice Mage"));
        assert_eq!(txt.display_name("PeasantM2", 2), Some("Journeyman Mage"));
        assert_eq!(txt.display_name("PeasantM2", 3), Some("Mage"));
    }

    #[test]
    fn goblin_variants_have_distinct_names() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        assert_eq!(txt.display_name("Goblin", 1), Some("Goblin"));
        // B and C variants must also resolve
        assert!(txt.display_name("Goblin", 2).is_some());
        assert!(txt.display_name("Goblin", 3).is_some());
    }

    #[test]
    fn goblin_a_has_positive_hp() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        let hp = txt.max_hp("Goblin", 1);
        assert!(hp.is_some(), "GoblinA should have HP in monsters.txt");
        assert!(hp.unwrap() > 0, "GoblinA HP must be positive");
    }

    #[test]
    fn peasant_m2_a_has_positive_hp() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        let hp = txt.max_hp("PeasantM2", 1);
        assert!(hp.is_some(), "PeasantM2A should have HP in monsters.txt");
        assert!(hp.unwrap() > 0, "PeasantM2A HP must be positive");
    }

    #[test]
    fn unknown_monster_returns_none() {
        let Some(lod) = test_lod() else { return };
        let txt = MonstersTxt::new(&lod).unwrap();
        assert!(txt.display_name("NonExistentXyz", 1).is_none());
    }
}
