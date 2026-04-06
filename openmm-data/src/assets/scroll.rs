//! Parser for `scroll.txt` — in-game scroll item text strings from the icons LOD.
//!
//! TSV file with 1 header line, then one entry per row.
//! Columns: Item#, message text, dungeon #, Notes

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// One scroll text entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollEntry {
    /// Item ID (matches the scroll item table).
    pub id: u16,
    pub text: String,
    /// Dungeon tag (e.g. "D1", "T6"). Empty if unused.
    pub dungeon: String,
    /// Designer notes.
    pub notes: String,
}

/// All scroll text definitions.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScrollTable {
    pub entries: Vec<ScrollEntry>,
}

impl ScrollTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/scroll.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        for line in text.lines().skip(1) {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.splitn(4, '\t').collect();
            let id: u16 = match fields.first().and_then(|s| s.trim().parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let text_val = fields
                .get(1)
                .map(|s| s.trim().trim_matches('"').to_string())
                .unwrap_or_default();
            let dungeon = fields.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
            let notes = fields
                .get(3)
                .map(|s| s.trim().trim_matches('"').to_string())
                .unwrap_or_default();
            entries.push(ScrollEntry {
                id,
                text: text_val,
                dungeon,
                notes,
            });
        }
        Self { entries }
    }

    /// Look up a scroll by item ID.
    pub fn get(&self, id: u16) -> Option<&ScrollEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

impl TryFrom<&[u8]> for ScrollTable {
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

impl LodSerialise for ScrollTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("Item#\tmessage text\t dungeon #\tNotes\r\n");
        for e in &self.entries {
            out.push_str(&format!("{}\t{}\t{}\t{}\r\n", e.id, e.text, e.dungeon, e.notes));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<ScrollTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        ScrollTable::load(&assets).ok()
    }

    #[test]
    fn entry_500_exists() {
        let Some(table) = load() else { return };
        let e = table.get(500).expect("scroll 500 missing");
        assert!(!e.text.is_empty(), "scroll 500 text should not be empty");
    }

    #[test]
    fn entry_501_dungeon_is_t6() {
        let Some(table) = load() else { return };
        let e = table.get(501).expect("scroll 501 missing");
        assert_eq!(e.dungeon, "T6");
    }
}
