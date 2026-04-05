/// Quest bit name table loaded from `icons/quests.txt`.
///
/// Maps QBit IDs (1-based) to human-readable labels for debug logging.
/// The file has 4 tab-separated columns:
///   Q Bit | Actual Quest Note Text | Notes | Old Quest Note Text
/// We prefer "Old Quest Note Text" (col 4) as the label, falling back to "Notes" (col 3)
/// when it contains meaningful content (not just developer annotations like "NPC", "Dave").
use crate::LodManager;

pub struct QuestBitNames {
    /// `names[i]` = label for QBit `(i+1)`, i.e. 0-indexed over 1-based IDs.
    names: Vec<Option<String>>,
}

impl QuestBitNames {
    pub fn load(lod: &LodManager) -> Self {
        let bytes = match lod.get_decompressed("icons/quests.txt") {
            Ok(b) => b,
            Err(e) => {
                log::warn!("QuestBitNames: could not load quests.txt: {}", e);
                return Self { names: vec![] };
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let mut names = Vec::with_capacity(512);

        for line in text.lines().skip(1) {
            // Strip Windows \r
            let line = line.trim_end_matches('\r');
            let fields: Vec<&str> = line.splitn(5, '\t').collect();
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
    use crate::get_lod_path;

    fn load() -> Option<QuestBitNames> {
        let path = get_lod_path();
        let lod = crate::LodManager::new(path).ok()?;
        Some(QuestBitNames::load(&lod))
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
        let names = QuestBitNames { names: vec![] };
        let input = "Compare(QBit[9999] set? skip step 1)";
        let out = names.annotate(input);
        assert_eq!(out, input);
    }
}
