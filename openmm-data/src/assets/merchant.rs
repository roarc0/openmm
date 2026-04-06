//! Parser for `merchant.txt` — NPC merchant dialogue strings from the icons LOD.
//!
//! TSV file with 1 header line, then one scenario row per line.
//! Columns: (row label), Buy text, Sell text, Repair text, Identify text
//! Row labels: "Not enough gold", "no merchant skill", "regular merchant skill",
//!             "good merchant skill", "wrong type of merchant", "Unnecessary"

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// One dialogue scenario row from `merchant.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantRow {
    /// Scenario label (e.g. "no merchant skill").
    pub label: String,
    /// Dialogue text for buying. "n/a" means not used for this scenario.
    pub buy: String,
    pub sell: String,
    pub repair: String,
    pub identify: String,
}

/// All merchant dialogue definitions.
#[derive(Debug, Serialize, Deserialize)]
pub struct MerchantTable {
    pub rows: Vec<MerchantRow>,
}

impl MerchantTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/merchant.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut rows = Vec::new();
        for line in text.lines().skip(1) {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = split_tsv(line);
            let label = fields.first().map(|s| s.trim().to_string()).unwrap_or_default();
            rows.push(MerchantRow {
                label,
                buy: col(&fields, 1),
                sell: col(&fields, 2),
                repair: col(&fields, 3),
                identify: col(&fields, 4),
            });
        }
        Self { rows }
    }

    /// Look up a row by its label (case-insensitive prefix match).
    pub fn get(&self, label: &str) -> Option<&MerchantRow> {
        self.rows.iter().find(|r| r.label.eq_ignore_ascii_case(label))
    }
}

fn col(fields: &[&str], i: usize) -> String {
    fields.get(i).map(|s| s.trim().trim_matches('"').to_string()).unwrap_or_default()
}

/// Split a TSV line respecting quoted fields that may contain tabs.
fn split_tsv(line: &str) -> Vec<&str> {
    line.split('\t').collect()
}

impl TryFrom<&[u8]> for MerchantTable {
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

impl LodSerialise for MerchantTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("\tBuy\tSell\tRepair\tIdentify\r\n");
        for r in &self.rows {
            out.push_str(&format!("{}\t{}\t{}\t{}\t{}\r\n", r.label, r.buy, r.sell, r.repair, r.identify));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<MerchantTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        MerchantTable::load(&assets).ok()
    }

    #[test]
    fn not_enough_gold_row_exists() {
        let Some(table) = load() else { return };
        let row = table.get("Not enough gold").expect("row missing");
        assert!(row.buy.contains("gold"), "unexpected buy text: {}", row.buy);
    }
}
