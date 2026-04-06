//! Parser for `autonotes.txt` — in-game autonote (auto-journal) entries from the icons LOD.
//!
//! TSV file with 1 header line, then one entry per row.
//! Columns: Note bit, Autonote Text, Category

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// One autonote entry from `autonotes.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Autonote {
    /// 1-based bit index.
    pub id: u16,
    pub text: String,
    /// Category tag (e.g. "Stat", "Award", "misc").
    pub category: String,
}

/// All autonote definitions.
#[derive(Debug, Serialize, Deserialize)]
pub struct AutonotesTable {
    pub entries: Vec<Autonote>,
}

impl AutonotesTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/autonotes.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        for line in text.lines().skip(1) {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.splitn(3, '\t').collect();
            let id: u16 = match fields.first().and_then(|s| s.trim().parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let text_val = fields
                .get(1)
                .map(|s| s.trim().trim_matches('"').to_string())
                .unwrap_or_default();
            if text_val.is_empty() {
                continue;
            }
            let category = fields.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
            entries.push(Autonote {
                id,
                text: text_val,
                category,
            });
        }
        Self { entries }
    }

    /// Look up an entry by its 1-based bit ID.
    pub fn get(&self, id: u16) -> Option<&Autonote> {
        self.entries.iter().find(|e| e.id == id)
    }
}

impl TryFrom<&[u8]> for AutonotesTable {
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

impl LodSerialise for AutonotesTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("Note bit\tAutonote Text\tCategory\r\n");
        for e in &self.entries {
            out.push_str(&format!("{}\t{}\t{}\r\n", e.id, e.text, e.category));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<AutonotesTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        AutonotesTable::load(&assets).ok()
    }

    #[test]
    fn first_entry_is_new_sorpigal_fountain() {
        let Some(table) = load() else { return };
        let e = table.get(1).expect("entry 1 missing");
        assert!(e.text.contains("New Sorpigal"), "unexpected: {}", e.text);
    }
}
