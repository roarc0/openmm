//! Parser for `npctopic.txt` — NPC dialogue topic labels from the icons LOD.
//!
//! TSV file with 2 header lines, then one entry per row.
//! Columns: #, Topic, (Notes)

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// One NPC topic entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcTopic {
    /// 1-based topic ID.
    pub id: u16,
    pub topic: String,
}

/// All NPC topic labels.
#[derive(Debug, Serialize, Deserialize)]
pub struct NpcTopicTable {
    pub entries: Vec<NpcTopic>,
}

impl NpcTopicTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/npctopic.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        for line in text.lines().skip(2) {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let mut cols = line.splitn(3, '\t');
            let id: u16 = match cols.next().and_then(|s| s.trim().parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let topic = cols.next().unwrap_or("").trim().trim_matches('"').to_string();
            entries.push(NpcTopic { id, topic });
        }
        Self { entries }
    }

    /// Look up a topic by 1-based ID.
    pub fn get(&self, id: u16) -> Option<&NpcTopic> {
        self.entries.iter().find(|e| e.id == id)
    }
}

impl TryFrom<&[u8]> for NpcTopicTable {
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

impl LodSerialise for NpcTopicTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("Text Number From NPC Events.Doc\t\tNotes\r\n#\tTopic\t\r\n");
        for e in &self.entries {
            out.push_str(&format!("{}\t{}\t\r\n", e.id, e.topic));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<NpcTopicTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        NpcTopicTable::load(&assets).ok()
    }

    #[test]
    fn entry_1_is_the_letter() {
        let Some(table) = load() else { return };
        let e = table.get(1).expect("entry 1 missing");
        assert_eq!(e.topic, "The Letter");
    }
}
