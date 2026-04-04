//! Parser for npcprof.txt — NPC profession definitions from the icons LOD.
//!
//! Tab-separated text file with 4 header lines, then one profession per row.
//! Column layout (0-indexed):
//!   0: #, 1: Name, 2: RandomChance, 3: JoinCostPerWeek,
//!   4: Personality, 5: ActionText, 6: InPartyBenefit, 7: JoinText

use std::error::Error;
use std::io::Cursor;

use csv::ReaderBuilder;

use crate::LodManager;

/// One NPC profession definition from `npcprof.txt`.
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
pub struct NpcProfTable {
    pub professions: Vec<NpcProfession>,
}

impl NpcProfTable {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/npcprof.txt")?;
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
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
