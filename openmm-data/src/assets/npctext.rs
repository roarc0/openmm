//! Parser for `npctext.txt` — NPC dialogue text strings from the icons LOD.
//!
//! TSV file with 2 header lines, then one entry per row.
//! Columns: #, Text, Notes

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// One NPC dialogue text entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcText {
    /// 1-based text ID.
    pub id: u16,
    pub text: String,
    pub notes: String,
}

/// All NPC dialogue text strings.
#[derive(Debug, Serialize, Deserialize)]
pub struct NpcTextTable {
    pub entries: Vec<NpcText>,
}

impl NpcTextTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/npctext.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        for line in text.lines().skip(2) {
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
            let notes = fields.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
            entries.push(NpcText {
                id,
                text: text_val,
                notes,
            });
        }
        Self { entries }
    }

    /// Look up text by 1-based ID.
    pub fn get(&self, id: u16) -> Option<&NpcText> {
        self.entries.iter().find(|e| e.id == id)
    }
}

impl TryFrom<&[u8]> for NpcTextTable {
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

impl LodSerialise for NpcTextTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("Text Number From NPC Events.Doc\t\t\r\n#\tText\tNotes\r\n");
        for e in &self.entries {
            out.push_str(&format!("{}\t{}\t{}\r\n", e.id, e.text, e.notes));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<NpcTextTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        NpcTextTable::load(&assets).ok()
    }

    #[test]
    fn entry_1_contains_seal() {
        let Some(table) = load() else { return };
        let e = table.get(1).expect("entry 1 missing");
        assert!(e.text.contains("Seal"), "unexpected: {}", e.text);
    }
}
