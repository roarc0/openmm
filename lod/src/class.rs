//! Parser for class.txt — player class descriptions from the icons LOD.
//!
//! Tab-separated text file, 2 columns: Class name, Description.
//! First line is a header row.

use std::error::Error;
use std::io::Cursor;

use csv::ReaderBuilder;

use crate::LodManager;

/// One player class entry from `class.txt`.
pub struct ClassInfo {
    /// Class name (e.g. "Knight", "Cavalier", "Champion").
    pub name: String,
    /// Human-readable description of the class.
    pub description: String,
}

/// All class definitions.
pub struct ClassTable {
    pub classes: Vec<ClassInfo>,
}

impl ClassTable {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/class.txt")?;
        let data = match crate::lod_data::LodData::try_from(raw) {
            Ok(d) => d.data,
            Err(_) => raw.to_vec(),
        };
        let text = String::from_utf8_lossy(&data);
        Self::parse(&text)
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
