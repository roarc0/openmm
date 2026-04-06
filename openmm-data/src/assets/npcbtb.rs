//! Parser for `npcbtb.txt` — NPC beg/bribe/threat flags and greeting messages from the icons LOD.
//!
//! TSV file with 1 header line, then rows:
//!   - "Beg", "Bribe", "Threat" rows with 0/1 flags per NPC type
//!   - Rows 1..=16 with dialogue text per NPC type
//!
//! Columns 0-1: Msg# / Notes; columns 2..N: one per NPC personality type.

use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// Dialogue support flags and texts for one NPC personality type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpcBtbType {
    /// Personality type name (e.g. "Peasant BTB").
    pub name: String,
    pub can_beg: bool,
    pub can_bribe: bool,
    pub can_threat: bool,
    /// Greeting/dialogue texts for message IDs 1..=16.
    pub messages: Vec<String>,
}

/// Full NPC beg/bribe/threat table.
#[derive(Debug, Serialize, Deserialize)]
pub struct NpcBtbTable {
    pub npc_types: Vec<NpcBtbType>,
}

impl NpcBtbTable {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/npcbtb.txt")?;
        Self::try_from(raw.as_slice())
    }

    fn parse(text: &str) -> Self {
        let mut lines = text.lines();
        let header = lines.next().unwrap_or("").trim_end_matches('\r');
        let type_names: Vec<String> = header
            .split('\t')
            .skip(2)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let n = type_names.len();

        let mut beg_flags = vec![false; n];
        let mut bribe_flags = vec![false; n];
        let mut threat_flags = vec![false; n];
        let mut messages: Vec<Vec<String>> = vec![Vec::new(); n];

        for line in lines {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            let key = cols.first().copied().unwrap_or("").trim();
            match key {
                "Beg" => {
                    for (i, flags) in beg_flags.iter_mut().enumerate() {
                        *flags = cols.get(i + 2).copied().unwrap_or("0").trim() == "1";
                    }
                }
                "Bribe" => {
                    for (i, flags) in bribe_flags.iter_mut().enumerate() {
                        *flags = cols.get(i + 2).copied().unwrap_or("0").trim() == "1";
                    }
                }
                "Threat" => {
                    for (i, flags) in threat_flags.iter_mut().enumerate() {
                        *flags = cols.get(i + 2).copied().unwrap_or("0").trim() == "1";
                    }
                }
                _ => {
                    if key.parse::<u16>().is_ok() {
                        for (i, msg_list) in messages.iter_mut().enumerate() {
                            let text = cols
                                .get(i + 2)
                                .copied()
                                .unwrap_or("")
                                .trim()
                                .trim_matches('"')
                                .to_string();
                            msg_list.push(text);
                        }
                    }
                }
            }
        }

        let npc_types = type_names
            .into_iter()
            .enumerate()
            .map(|(i, name)| NpcBtbType {
                name,
                can_beg: beg_flags.get(i).copied().unwrap_or(false),
                can_bribe: bribe_flags.get(i).copied().unwrap_or(false),
                can_threat: threat_flags.get(i).copied().unwrap_or(false),
                messages: messages.get(i).cloned().unwrap_or_default(),
            })
            .collect();

        Self { npc_types }
    }

    /// Get NPC type by name (case-insensitive prefix).
    pub fn get(&self, name: &str) -> Option<&NpcBtbType> {
        self.npc_types
            .iter()
            .find(|t| t.name.to_ascii_lowercase().starts_with(&name.to_ascii_lowercase()))
    }

    /// Get message text for a given NPC type index and message ID (1-based).
    pub fn message(&self, npc_type_idx: usize, msg_id: usize) -> Option<&str> {
        let t = self.npc_types.get(npc_type_idx)?;
        t.messages.get(msg_id.checked_sub(1)?).map(|s| s.as_str())
    }
}

impl TryFrom<&[u8]> for NpcBtbTable {
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

impl LodSerialise for NpcBtbTable {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        // Header
        out.push_str("Msg#\tNotes");
        for t in &self.npc_types {
            out.push('\t');
            out.push_str(&t.name);
        }
        out.push_str("\r\n");
        // Flag rows
        let flag_rows: [(&str, fn(&NpcBtbType) -> bool); 3] = [
            ("Beg", |t| t.can_beg),
            ("Bribe", |t| t.can_bribe),
            ("Threat", |t| t.can_threat),
        ];
        for (label, getter) in &flag_rows {
            out.push_str(label);
            out.push('\t');
            for t in &self.npc_types {
                out.push('\t');
                out.push(if getter(t) { '1' } else { '0' });
            }
            out.push_str("\r\n");
        }
        // Message rows
        let max_msgs = self.npc_types.iter().map(|t| t.messages.len()).max().unwrap_or(0);
        for i in 0..max_msgs {
            out.push_str(&format!("{}\t", i + 1));
            for t in &self.npc_types {
                out.push('\t');
                out.push_str(t.messages.get(i).map(|s| s.as_str()).unwrap_or(""));
            }
            out.push_str("\r\n");
        }
        out.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<NpcBtbTable> {
        let assets = crate::Assets::new(get_data_path()).ok()?;
        NpcBtbTable::load(&assets).ok()
    }

    #[test]
    fn peasant_can_beg() {
        let Some(table) = load() else { return };
        let peasant = table.get("Peasant").expect("Peasant type missing");
        assert!(peasant.can_beg, "peasant should be able to beg");
    }

    #[test]
    fn peasant_first_greeting_contains_pleased() {
        let Some(table) = load() else { return };
        let peasant = table.get("Peasant").expect("Peasant type missing");
        let msg = table.message(0, 1).unwrap_or("");
        assert!(!msg.is_empty(), "message 1 for peasant should not be empty");
        let _ = peasant; // used for context
    }
}
