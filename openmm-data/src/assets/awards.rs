//! Parser for awards.txt — achievement / award titles from the icons LOD.
//!
//! Tab-separated text file with 1 header line, then one award per row.
//! Column layout (0-indexed):
//!   0: ID (1-based), 1: Award text, 2: Notes (optional)

use std::error::Error;
use std::io::Cursor;
use csv::ReaderBuilder;
use serde::{Serialize, Deserialize};

use crate::LodSerialise;
use crate::Assets;

/// One award entry from `awards.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Award {
    /// 1-based award ID (matches the bit flag used in party data).
    pub id: u16,
    /// Display text for the award.
    pub text: String,
    /// Optional designer/debug notes.
    pub notes: String,
}

/// All award definitions.
#[derive(Debug, Serialize, Deserialize)]
pub struct AwardsTable {
    pub awards: Vec<Award>,
}

impl AwardsTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/awards.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let body: String = text.lines().skip(1).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut awards = Vec::new();
        for result in rdr.records() {
            let rec = result?;

            let id: u16 = match rec.get(0).unwrap_or("").trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if id == 0 {
                continue;
            }
            let text_val = rec.get(1).unwrap_or("").trim().to_string();
            if text_val.is_empty() {
                continue;
            }

            awards.push(Award {
                id,
                text: text_val,
                notes: rec.get(2).unwrap_or("").trim().to_string(),
            });
        }
        Ok(AwardsTable { awards })
    }

    /// Look up an award by its bit-flag ID.
    pub fn get(&self, id: u16) -> Option<&Award> {
        self.awards.iter().find(|a| a.id == id)
    }
}

impl TryFrom<&[u8]> for AwardsTable {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = match crate::assets::lod_data::LodData::try_from(data) {
            Ok(d) => d.data,
            Err(_) => data.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
    }
}

impl LodSerialise for AwardsTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        // MM6 awards.txt header (1 line)
        out.push_str("ID\tAward\tNotes\r\n");

        for a in &self.awards {
            out.push_str(&format!("{}\t{}\t{}\r\n", a.id, a.text, a.notes));
        }
        out.into_bytes()
    }
}
