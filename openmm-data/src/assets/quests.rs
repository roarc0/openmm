use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::Assets;
use crate::LodSerialise;

/// Quest bit name table loaded from `icons/quests.txt`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestNames {
    /// `names[i]` = label for QBit `(i+1)`, i.e. 0-indexed over 1-based IDs.
    pub names: Vec<Option<String>>,
}

impl QuestNames {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/quests.txt")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(text: &str) -> Self {
        let mut names = Vec::with_capacity(512);

        for line in text.lines().skip(1) {
            // Strip Windows \r
            let line = line.trim_end_matches('\r');
            let fields: Vec<&str> = line.split('\t').collect();
            names.push(pick_label(&fields).map(str::to_string));
        }

        Self { names }
    }

    /// Return the label for a QBit ID (1-based), if one exists.
    pub fn name(&self, id: u16) -> Option<&str> {
        if id == 0 {
            return None;
        }
        self.names.get((id as usize) - 1)?.as_deref()
    }

    /// Replace every `QBit[N]` substring in `s` with `QBit[N:Label]` when a label is known.
    pub fn annotate(&self, s: &str) -> String {
        const MARKER: &str = "QBit[";
        let mut result = String::with_capacity(s.len() + 64);
        let mut rest = s;

        while let Some(pos) = rest.find(MARKER) {
            result.push_str(&rest[..pos + MARKER.len()]);
            rest = &rest[pos + MARKER.len()..];

            if let Some(end) = rest.find(']') {
                let num_str = &rest[..end];
                if let Ok(id) = num_str.parse::<u16>() {
                    if let Some(name) = self.name(id) {
                        // Show number:name; truncate name at 100 chars
                        result.push_str(num_str);
                        result.push(':');
                        if name.len() <= 100 {
                            result.push_str(name);
                        } else {
                            result.push_str(&name[..100]);
                            result.push('…');
                        }
                    } else {
                        result.push_str(num_str);
                    }
                } else {
                    result.push_str(num_str);
                }
                result.push(']');
                rest = &rest[end + 1..];
            } else {
                // No closing bracket — emit the rest verbatim
                result.push_str(rest);
                rest = "";
                break;
            }
        }

        result.push_str(rest);
        result
    }
}

impl TryFrom<&[u8]> for QuestNames {
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

impl LodSerialise for QuestNames {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        // MM6 quests.txt header
        out.push_str("Q Bit\tActual Quest Note Text\tNotes\tOld Quest Note Text\r\n");

        for (i, name) in self.names.iter().enumerate() {
            let label = name.as_deref().unwrap_or("");
            out.push_str(&format!("{}\t\t\t{}\r\n", i + 1, label));
        }
        out.into_bytes()
    }
}

/// Developer-only annotations that carry no useful quest information.
const DEV_ANNOTATIONS: &[&str] = &["NPC", "Dave", "Tim", "Peter", "marks quest items"];

/// Pick the best short label from a tab-split row.
/// Prefers field[3] ("Old Quest Note Text"), falls back to field[2] ("Notes").
fn pick_label<'a>(fields: &[&'a str]) -> Option<&'a str> {
    let clean = |s: &'a str| -> &'a str { s.trim().trim_matches('"').trim() };

    let old = fields.get(3).map(|s| clean(s)).unwrap_or("");
    if !old.is_empty() {
        return Some(old);
    }

    let notes = fields.get(2).map(|s| clean(s)).unwrap_or("");
    if !notes.is_empty() && !DEV_ANNOTATIONS.iter().any(|&d| notes.starts_with(d)) {
        return Some(notes);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::get_data_path;

    fn load() -> Option<QuestNames> {
        let path = get_data_path();
        let lod = crate::Assets::new(path).ok()?;
        Some(QuestNames::load(&lod).unwrap())
    }

    #[test]
    fn qbit_302_is_sword_in_stone() {
        let Some(names) = load() else { return };
        let label = names.name(302).unwrap_or("");
        assert!(
            label.contains("Sword") || label.contains("Stone"),
            "expected QBit 302 to mention Sword/Stone, got: {:?}",
            label
        );
    }

    #[test]
    fn annotate_replaces_known_qbit() {
        let Some(names) = load() else { return };
        let input = "Compare(QBit[302] set? skip step 8)";
        let out = names.annotate(input);
        assert!(out.contains("QBit[302:"), "expected QBit[302:...], got: {}", out);
        assert!(
            out.contains("Sword") || out.contains("Stone"),
            "expected label in output, got: {}",
            out
        );
    }

    #[test]
    fn annotate_unknown_qbit_unchanged() {
        let names = QuestNames { names: vec![] };
        let input = "Compare(QBit[9999] set? skip step 1)";
        let out = names.annotate(input);
        assert_eq!(out, input);
    }
}
