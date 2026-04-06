//! Parser for class.txt — player class descriptions from the icons LOD.
//!
//! Tab-separated text file, 2 columns: Class name, Description.
//! First line is a header row.

use std::error::Error;
use std::io::Cursor;
use csv::ReaderBuilder;
use serde::{Serialize, Deserialize};

use crate::LodSerialise;
use crate::Assets;

/// One player class entry from `class.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassInfo {
    /// Class name (e.g. "Knight", "Cavalier", "Champion").
    pub name: String,
    /// Human-readable description of the class.
    pub description: String,
}

/// All class definitions.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClassTable {
    pub classes: Vec<ClassInfo>,
}

impl ClassTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/class.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Result<Self, Box<dyn Error>> {
        let body: String = text.lines().skip(1).collect::<Vec<_>>().join("\n");
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .flexible(true)
            .from_reader(Cursor::new(body.as_bytes()));

        let mut classes = Vec::new();
        for result in rdr.records() {
            let rec = result?;

            let name = rec.get(0).unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }
            let description = rec.get(1).unwrap_or("").trim().to_string();
            classes.push(ClassInfo { name, description });
        }
        Ok(ClassTable { classes })
    }

    /// Look up a class by exact name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&ClassInfo> {
        self.classes.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

impl TryFrom<&[u8]> for ClassTable {
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

impl LodSerialise for ClassTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        // MM6 class.txt header (1 line)
        out.push_str("Class\tDescription\r\n");

        for c in &self.classes {
            out.push_str(&format!("{}\t{}\r\n", c.name, c.description));
        }
        out.into_bytes()
    }
}
