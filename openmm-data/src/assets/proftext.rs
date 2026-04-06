//! Parser for `PROFTEXT.txt` — NPC profession day-of-week dialogue from the icons LOD.
//!
//! TSV file with 2 header lines, then one profession per row.
//! Columns: #, Profession name, then (topic, text) pairs for Sunday..Saturday (7 days × 2 = 14 cols).

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

pub const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// Dialogue topic + text for one day.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayDialogue {
    pub topic: String,
    pub text: String,
}

/// One profession's day-of-week dialogue from `PROFTEXT.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfText {
    /// 1-based profession ID (matches `NpcProfession.id`).
    pub id: u16,
    pub profession: String,
    /// `days[0]` = Sunday, `days[6]` = Saturday.
    pub days: [DayDialogue; 7],
}

impl ProfText {
    /// Get dialogue for a day index (0 = Sunday).
    pub fn day(&self, day_idx: usize) -> Option<&DayDialogue> {
        self.days.get(day_idx)
    }
}

/// All profession text definitions.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProfTextTable {
    pub professions: Vec<ProfText>,
}

impl ProfTextTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/proftext.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut professions = Vec::new();
        for line in text.lines().skip(2) {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            let id: u16 = match cols.first().and_then(|s| s.trim().parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let profession = cols.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
            if profession.is_empty() {
                continue;
            }
            let days = std::array::from_fn(|i| {
                let base = 2 + i * 2;
                DayDialogue {
                    topic: cols.get(base).map(|s| s.trim().trim_matches('"').to_string()).unwrap_or_default(),
                    text: cols.get(base + 1).map(|s| s.trim().trim_matches('"').to_string()).unwrap_or_default(),
                }
            });
            professions.push(ProfText { id, profession, days });
        }
        Self { professions }
    }

    /// Look up a profession by 1-based ID.
    pub fn get(&self, id: u16) -> Option<&ProfText> {
        self.professions.iter().find(|p| p.id == id)
    }
}

impl TryFrom<&[u8]> for ProfTextTable {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = match crate::assets::lod_data::LodData::try_from(data) {
            Ok(d) => d.data,
            Err(_) => data.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Ok(Self::parse(&text))
    }
}

impl LodSerialise for ProfTextTable {
    fn to_bytes(&self) -> Vec<u8> {
        let day_headers: String = DAYS.iter().flat_map(|d| [format!("{} topic", d), format!("{} text", d)]).collect::<Vec<_>>().join("\t");
        let mut out = format!("Day of Week Profession Text\r\n#\t\t{}\r\n", day_headers);
        for p in &self.professions {
            out.push_str(&format!("{}\t{}", p.id, p.profession));
            for d in &p.days {
                out.push_str(&format!("\t{}\t{}", d.topic, d.text));
            }
            out.push_str("\r\n");
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<ProfTextTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        ProfTextTable::load(&assets).ok()
    }

    #[test]
    fn prof_1_is_smith() {
        let Some(table) = load() else { return };
        let p = table.get(1).expect("profession 1 missing");
        assert_eq!(p.profession, "Smith");
    }

    #[test]
    fn smith_sunday_topic_is_rest() {
        let Some(table) = load() else { return };
        let p = table.get(1).expect("profession 1 missing");
        assert_eq!(p.days[0].topic, "Rest", "Sunday topic for Smith: {:?}", p.days[0].topic);
    }
}
