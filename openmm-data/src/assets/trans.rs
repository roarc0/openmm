//! Parser for `trans.txt` — dungeon/outdoor area transition descriptions from the icons LOD.
//!
//! TSV file with 1 header line, then one entry per row.
//! Columns: 2D#, Transition Description

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// One transition description entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransEntry {
    /// 1-based 2D ID.
    pub id: u16,
    pub description: String,
}

/// All transition descriptions.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransTable {
    pub entries: Vec<TransEntry>,
}

impl TransTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/trans.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        for line in text.lines().skip(1) {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let mut cols = line.splitn(2, '\t');
            let id: u16 = match cols.next().and_then(|s| s.trim().parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let description = cols.next().unwrap_or("").trim().trim_matches('"').to_string();
            entries.push(TransEntry { id, description });
        }
        Self { entries }
    }

    /// Look up a transition by 1-based ID.
    pub fn get(&self, id: u16) -> Option<&TransEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

impl TryFrom<&[u8]> for TransTable {
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

impl LodSerialise for TransTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("2D#\tTransition Description\r\n");
        for e in &self.entries {
            out.push_str(&format!("{}\t{}\r\n", e.id, e.description));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<TransTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        TransTable::load(&assets).ok()
    }

    #[test]
    fn entry_1_mentions_dragon() {
        let Some(table) = load() else { return };
        let e = table.get(1).expect("entry 1 missing");
        assert!(e.description.contains("dragon") || e.description.contains("Dragon"), "unexpected: {}", e.description);
    }
}
