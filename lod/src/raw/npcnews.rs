//! Parser for npcnews.txt — regional NPC news lines from the icons LOD.
//!
//! Tab-separated text file with 2 header lines, then one news entry per row.
//! Column layout (0-indexed):
//!   0: #, 1: Map (MapStats ID), 2: Topic, 3: News Text

use std::error::Error;
use std::io::Cursor;
use csv::ReaderBuilder;
use serde::{Serialize, Deserialize};

use crate::LodSerialise;
use crate::LodManager;

/// One NPC news entry from `npcnews.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcNewsItem {
    /// 1-based entry index.
    pub index: u16,
    /// MapStats ID of the map where this news appears.
    pub map_id: u16,
    /// Topic/subject label.
    pub topic: String,
    /// Full news text shown in the dialogue.
    pub text: String,
}

/// All NPC news entries.
#[derive(Debug, Serialize, Deserialize)]
pub struct NpcNewsTable {
    pub items: Vec<NpcNewsItem>,
}

impl NpcNewsTable {
    pub fn load(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/npcnews.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let body: String = text.lines().skip(2).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut items = Vec::new();
        for result in rdr.records() {
            let rec = result?;

            let index: u16 = match rec.get(0).unwrap_or("").trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if index == 0 {
                continue;
            }

            items.push(NpcNewsItem {
                index,
                map_id: rec.get(1).unwrap_or("0").trim().parse().unwrap_or(0),
                topic: rec.get(2).unwrap_or("").trim().to_string(),
                text: rec.get(3).unwrap_or("").trim().to_string(),
            });
        }
        Ok(NpcNewsTable { items })
    }

    /// Get all news items for a given map ID.
    pub fn for_map(&self, map_id: u16) -> impl Iterator<Item = &NpcNewsItem> {
        self.items.iter().filter(move |n| n.map_id == map_id)
    }
}

impl TryFrom<&[u8]> for NpcNewsTable {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = match crate::raw::lod_data::LodData::try_from(data) {
            Ok(d) => d.data,
            Err(_) => data.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
    }
}

impl LodSerialise for NpcNewsTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        // MM6 npcnews.txt header (2 lines)
        out.push_str("MM6 NPC News\r\n");
        out.push_str("#\tMap\tTopic\tText\r\n");

        for n in &self.items {
            out.push_str(&format!("{}\t{}\t{}\t{}\r\n", n.index, n.map_id, n.topic, n.text));
        }
        out.into_bytes()
    }
}
