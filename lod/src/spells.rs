//! Parser for spells.txt — spell definitions from the icons LOD.
//!
//! Tab-separated text file with 3 header lines, then one row per spell.
//! Column layout (0-indexed):
//!   0: #, 1: SpellNumber, 2: Name, 3: Resistance, 4: ShortName,
//!   5: SpCostA, 6: SpCostX, 7: SpCostM, 8: Description,
//!   9: NormalEffect, 10: ExpertEffect, 11: MasterEffect

use std::error::Error;

use crate::LodManager;

/// A single spell definition from `spells.txt`.
pub struct SpellInfo {
    /// 1-based row index.
    pub index: u16,
    /// Spell number within its school (1-based within school).
    pub spell_number: u8,
    /// Spell display name.
    pub name: String,
    /// Resistance type (e.g. "Fire", "none").
    pub resistance: String,
    /// Short / abbreviated name for UI.
    pub short_name: String,
    /// Spell point cost at Normal skill.
    pub sp_cost_normal: u8,
    /// Spell point cost at Expert skill.
    pub sp_cost_expert: u8,
    /// Spell point cost at Master skill.
    pub sp_cost_master: u8,
    /// Full flavour description.
    pub description: String,
    /// Normal mastery level effect text.
    pub effect_normal: String,
    /// Expert mastery level effect text.
    pub effect_expert: String,
    /// Master mastery level effect text.
    pub effect_master: String,
}

/// All spell definitions.
pub struct SpellsTable {
    pub spells: Vec<SpellInfo>,
}

impl SpellsTable {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/spells.txt")?;
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let mut spells = Vec::new();
        // Skip 3 header lines (2 blank + column header)
        for line in text.lines().skip(3) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 8 {
                continue;
            }
            let index: u16 = match cols[0].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if index == 0 {
                continue;
            }

            let spell_number: u8 = cols.get(1).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let name = cols.get(2).unwrap_or(&"").trim().to_string();
            if name.is_empty() {
                continue;
            }
            let resistance = cols.get(3).unwrap_or(&"none").trim().to_string();
            let short_name = cols.get(4).unwrap_or(&"").trim().to_string();
            let sp_cost_normal: u8 = cols.get(5).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let sp_cost_expert: u8 = cols.get(6).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let sp_cost_master: u8 = cols.get(7).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let description = cols.get(8).unwrap_or(&"").trim().trim_matches('"').to_string();
            let effect_normal = cols.get(9).unwrap_or(&"").trim().to_string();
            let effect_expert = cols.get(10).unwrap_or(&"").trim().to_string();
            let effect_master = cols.get(11).unwrap_or(&"").trim().to_string();

            spells.push(SpellInfo {
                index,
                spell_number,
                name,
                resistance,
                short_name,
                sp_cost_normal,
                sp_cost_expert,
                sp_cost_master,
                description,
                effect_normal,
                effect_expert,
                effect_master,
            });
        }
        Ok(SpellsTable { spells })
    }

    /// Look up a spell by its 1-based index.
    pub fn get(&self, index: u16) -> Option<&SpellInfo> {
        self.spells.iter().find(|s| s.index == index)
    }

    /// Find all spells matching a name (case-insensitive).
    pub fn find_by_name(&self, name: &str) -> impl Iterator<Item = &SpellInfo> {
        let lower = name.to_lowercase();
        self.spells
            .iter()
            .filter(move |s| s.name.to_lowercase().contains(&lower))
    }
}
