//! Parser for `passwords.txt` — dungeon password questions and answers from the icons LOD.
//!
//! TSV file with 1 header line, then one entry per row.
//! Columns: Number, Questions, Answers

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// One password entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Password {
    /// 1-based index.
    pub id: u16,
    pub question: String,
    pub answer: String,
}

/// All dungeon passwords.
#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordsTable {
    pub entries: Vec<Password>,
}

impl PasswordsTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/passwords.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut entries = Vec::new();
        for line in text.lines().skip(1) {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.splitn(3, '\t').collect();
            let id: u16 = match fields.first().and_then(|s| s.trim().parse().ok()) {
                Some(v) => v,
                None => continue,
            };
            let question = fields.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
            let answer = fields.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
            entries.push(Password { id, question, answer });
        }
        Self { entries }
    }

    /// Look up a password by 1-based ID.
    pub fn get(&self, id: u16) -> Option<&Password> {
        self.entries.iter().find(|e| e.id == id)
    }
}

impl TryFrom<&[u8]> for PasswordsTable {
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

impl LodSerialise for PasswordsTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::from("Number\tQuestions\tAnswers\r\n");
        for e in &self.entries {
            out.push_str(&format!("{}\t{}\t{}\r\n", e.id, e.question, e.answer));
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<PasswordsTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        PasswordsTable::load(&assets).ok()
    }

    #[test]
    fn entry_1_answer_is_no() {
        let Some(table) = load() else { return };
        let e = table.get(1).expect("entry 1 missing");
        assert_eq!(e.answer, "No");
    }
}
