//! Parser for npcprof.txt — NPC profession definitions from the icons LOD.
//!
//! Tab-separated text file with 4 header lines, then one profession per row.
//! Column layout (0-indexed):
//!   0: #, 1: Name, 2: RandomChance, 3: JoinCostPerWeek,
//!   4: Personality, 5: ActionText, 6: InPartyBenefit, 7: JoinText

use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::Cursor;

use crate::Assets;
use crate::LodSerialise;

/// One NPC profession definition from `npcprof.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcProfession {
    /// 1-based profession ID.
    pub id: u16,
    /// Profession name (e.g. "Smith", "Guide").
    pub name: String,
    /// Chance of randomly appearing (0-100).
    pub random_chance: u8,
    /// Base hire cost in gold per week.
    pub cost_per_week: u32,
    /// Personality archetype string.
    pub personality: String,
    /// Short action text shown in combat/UI.
    pub action_text: String,
    /// Description of the benefit while NPC is in party.
    pub in_party_benefit: String,
    /// Dialogue text shown when the NPC offers to join.
    pub join_text: String,
}

/// All NPC profession definitions.
#[derive(Debug, Serialize, Deserialize)]
pub struct NpcProfTable {
    pub professions: Vec<NpcProfession>,
}

impl NpcProfTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/npcprof.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let body: String = text.lines().skip(4).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut professions = Vec::new();
        for result in rdr.records() {
            let rec = result?;

            let id: u16 = match rec.get(0).unwrap_or("").trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if id == 0 {
                continue;
            }
            let name = rec.get(1).unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }

            professions.push(NpcProfession {
                id,
                name,
                random_chance: rec.get(2).unwrap_or("0").trim().parse().unwrap_or(0),
                cost_per_week: rec.get(3).unwrap_or("0").trim().parse().unwrap_or(0),
                personality: rec.get(4).unwrap_or("").trim().to_string(),
                action_text: rec.get(5).unwrap_or("").trim().to_string(),
                in_party_benefit: rec.get(6).unwrap_or("").trim().to_string(),
                join_text: rec.get(7).unwrap_or("").trim().to_string(),
            });
        }
        Ok(NpcProfTable { professions })
    }

    /// Look up a profession by its ID.
    pub fn get(&self, id: u16) -> Option<&NpcProfession> {
        self.professions.iter().find(|p| p.id == id)
    }
}

impl TryFrom<&[u8]> for NpcProfTable {
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

impl LodSerialise for NpcProfTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        // MM6 npcprof.txt header (4 lines)
        out.push_str("MM6 NPC Professions\r\n");
        out.push_str("#\tProf\tChance\tL-Cost\tPers\tCombatText\tBenefit\tJoinText\r\n");
        out.push_str("\r\n");
        out.push_str("\r\n");

        for p in &self.professions {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\r\n",
                p.id,
                p.name,
                p.random_chance,
                p.cost_per_week,
                p.personality,
                p.action_text,
                p.in_party_benefit,
                p.join_text
            ));
        }
        out.into_bytes()
    }
}
