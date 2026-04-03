//! Parser for npcprof.txt — NPC profession definitions from the icons LOD.
//!
//! Tab-separated text file with 4 header lines, then one profession per row.
//! Column layout (0-indexed):
//!   0: #, 1: Name, 2: RandomChance, 3: JoinCostPerWeek,
//!   4: Personality, 5: ActionText, 6: InPartyBenefit, 7: JoinText

use std::error::Error;

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
        let mut professions = Vec::new();
        // Skip 4 header lines
        for line in text.lines().skip(4) {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 5 {
                continue;
            }
            let id: u16 = match cols[0].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if id == 0 {
                continue;
            }
            let name = cols.get(1).unwrap_or(&"").trim().to_string();
            if name.is_empty() {
                continue;
            }
            let random_chance: u8 = cols.get(2).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let cost_per_week: u32 = cols.get(3).unwrap_or(&"0").trim().parse().unwrap_or(0);
            let personality = cols.get(4).unwrap_or(&"").trim().to_string();
            let action_text = cols.get(5).unwrap_or(&"").trim().to_string();
            let in_party_benefit = cols.get(6).unwrap_or(&"").trim().trim_matches('"').to_string();
            let join_text = cols.get(7).unwrap_or(&"").trim().trim_matches('"').to_string();

            professions.push(NpcProfession {
                id,
                name,
                random_chance,
                cost_per_week,
                personality,
                action_text,
                in_party_benefit,
                join_text,
            });
        }
        Ok(NpcProfTable { professions })
    }

    /// Look up a profession by its ID.
    pub fn get(&self, id: u16) -> Option<&NpcProfession> {
        self.professions.iter().find(|p| p.id == id)
    }
}
