//! Parser for npcnews.txt — regional NPC news lines from the icons LOD.
//!
//! Tab-separated text file with 2 header lines, then one news entry per row.
//! Column layout (0-indexed):
//!   0: #, 1: Map (MapStats ID), 2: Topic, 3: News Text

use std::error::Error;
use std::io::Cursor;

use csv::ReaderBuilder;

use crate::LodManager;

/// One NPC news entry from `npcnews.txt`.
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
pub struct NpcNewsTable {
    pub items: Vec<NpcNewsItem>,
}

impl NpcNewsTable {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/npcnews.txt")?;
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
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
