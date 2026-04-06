//! Parser for `stats.txt` — character stat description strings from the icons LOD.
//!
//! TSV file with 1 header line, then one stat per row.
//! Columns: stat name, description text

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// One stat description entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatDescription {
    pub name: String,
    pub description: String,
}

/// All stat descriptions.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatsTable {
    pub entries: Vec<StatDescription>,
}

impl StatsTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/stats.txt")?;
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
            let name = cols.next().unwrap_or("").trim().to_string();
            if name.is_empty() {
                continue;
            }
            let description = cols.next().unwrap_or("").trim().trim_matches('"').to_string();
            entries.push(StatDescription { name, description });
        }
        Self { entries }
    }

    /// Look up a stat description by name (case-insensitive).
    pub fn get(&self, name: &str) -> Option<&StatDescription> {
        self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))
    }
}

impl TryFrom<&[u8]> for StatsTable {
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

impl LodSerialise for StatsTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("Stats Descriptions\tDescription\r\n");
        for e in &self.entries {
            out.push_str(&format!("{}\t{}\r\n", e.name, e.description));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<StatsTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        StatsTable::load(&assets).ok()
    }

    #[test]
    fn might_description_mentions_strength() {
        let Some(table) = load() else { return };
        let e = table.get("Might").expect("Might stat missing");
        assert!(e.description.to_ascii_lowercase().contains("strength"), "unexpected: {}", e.description);
    }
}
