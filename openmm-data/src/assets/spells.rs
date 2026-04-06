//! Parser for spells.txt — spell definitions from the icons LOD.
//!
//! Tab-separated text file with 3 header lines, then one row per spell.
//! Column layout (0-indexed):
//!   0: #, 1: SpellNumber, 2: Name, 3: Resistance, 4: ShortName,
//!   5: SpCostA, 6: SpCostX, 7: SpCostM, 8: Description,
//!   9: NormalEffect, 10: ExpertEffect, 11: MasterEffect

use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::Cursor;

use crate::Assets;
use crate::LodSerialise;

/// A single spell definition from `spells.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
pub struct SpellsTable {
    pub spells: Vec<SpellInfo>,
}

impl SpellsTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/spells.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let body: String = text.lines().skip(3).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut spells = Vec::new();
        for result in rdr.records() {
            let rec = result?;

            let index: u16 = match rec.get(0).unwrap_or("").trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if index == 0 {
                continue;
            }

            let name = rec.get(2).unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }

            spells.push(SpellInfo {
                index,
                spell_number: rec.get(1).unwrap_or("0").trim().parse().unwrap_or(0),
                name,
                resistance: rec.get(3).unwrap_or("none").trim().to_string(),
                short_name: rec.get(4).unwrap_or("").trim().to_string(),
                sp_cost_normal: rec.get(5).unwrap_or("0").trim().parse().unwrap_or(0),
                sp_cost_expert: rec.get(6).unwrap_or("0").trim().parse().unwrap_or(0),
                sp_cost_master: rec.get(7).unwrap_or("0").trim().parse().unwrap_or(0),
                description: rec.get(8).unwrap_or("").trim().to_string(),
                effect_normal: rec.get(9).unwrap_or("").trim().to_string(),
                effect_expert: rec.get(10).unwrap_or("").trim().to_string(),
                effect_master: rec.get(11).unwrap_or("").trim().to_string(),
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

impl TryFrom<&[u8]> for SpellsTable {
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

impl LodSerialise for SpellsTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        // MM6 spells.txt header (3 lines)
        out.push_str("Spells\t\t\r\n");
        out.push_str("#\tSpellNumber\tName\tResistance\tShortName\tSpCostA\tSpCostX\tSpCostM\tDescription\tNormalEffect\tExpertEffect\tMasterEffect\r\n");
        out.push_str("\r\n");

        for s in &self.spells {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\r\n",
                s.index,
                s.spell_number,
                s.name,
                s.resistance,
                s.short_name,
                s.sp_cost_normal,
                s.sp_cost_expert,
                s.sp_cost_master,
                s.description,
                s.effect_normal,
                s.effect_expert,
                s.effect_master
            ));
        }
        out.into_bytes()
    }
}
