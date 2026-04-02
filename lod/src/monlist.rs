use std::error::Error;

use crate::{lod_data::LodData, LodManager};

const RECORD_SIZE: usize = 148;

/// A monster/NPC description from dmonlist.bin.
pub struct MonsterDesc {
    pub height: u16,
    pub radius: u16,
    pub move_speed: u16,
    pub to_hit_radius: u16,
    pub sound_ids: [u16; 4],
    pub internal_name: String,
    /// Sprite names for each animation state:
    /// [0]=standing, [1]=walking, [2]=attack1, [3]=attack2,
    /// [4]=hit, [5]=dying, [6]=dead, [7]=fidget
    pub sprite_names: [String; 8],
}

/// The full monster description table loaded from dmonlist.bin.
pub struct MonsterList {
    pub monsters: Vec<MonsterDesc>,
}

impl MonsterList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dmonlist.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        if data.len() < 4 {
            return Err("dmonlist.bin too short".into());
        }
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + count * RECORD_SIZE {
            return Err("dmonlist.bin truncated".into());
        }

        let mut monsters = Vec::with_capacity(count);
        for i in 0..count {
            let off = 4 + i * RECORD_SIZE;
            let rec = &data[off..off + RECORD_SIZE];
            monsters.push(Self::parse_record(rec));
        }

        Ok(MonsterList { monsters })
    }

    fn parse_record(rec: &[u8]) -> MonsterDesc {
        let height = u16::from_le_bytes([rec[0], rec[1]]);
        let radius = u16::from_le_bytes([rec[2], rec[3]]);
        let move_speed = u16::from_le_bytes([rec[4], rec[5]]);
        let to_hit_radius = u16::from_le_bytes([rec[6], rec[7]]);
        let sound_ids = [
            u16::from_le_bytes([rec[8], rec[9]]),
            u16::from_le_bytes([rec[10], rec[11]]),
            u16::from_le_bytes([rec[12], rec[13]]),
            u16::from_le_bytes([rec[14], rec[15]]),
        ];

        let name_bytes = &rec[16..48];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let internal_name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        let mut sprite_names: [String; 8] = Default::default();
        for (j, sn) in sprite_names.iter_mut().enumerate() {
            let s = 48 + j * 10;
            let bytes = &rec[s..s + 10];
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(10);
            *sn = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
        }

        // Bytes 128-147: 20 bytes of padding/unused data at end of each record.
        // The full monster stats (level, HP, AC, resistances, attacks, spells)
        // come from monstxt.txt (text table), not from this binary file.
        // Kept unread for now; will need to be preserved for round-trip LOD writing.

        MonsterDesc {
            height,
            radius,
            move_speed,
            to_hit_radius,
            sound_ids,
            internal_name,
            sprite_names,
        }
    }

    /// Get a monster description by 0-based index.
    pub fn get(&self, id: usize) -> Option<&MonsterDesc> {
        self.monsters.get(id)
    }

    /// Returns true if the given 0-based monlist_id is a peasant type.
    /// Checks the actual internal_name from dmonlist.bin for a "Peasant" prefix.
    pub fn is_peasant(&self, monlist_id: u8) -> bool {
        self.monsters.get(monlist_id as usize)
            .is_some_and(|m| m.internal_name.to_ascii_lowercase().starts_with("peasant"))
    }

    /// Returns true if the given peasant monlist_id is female (PeasantF types).
    /// Checks the internal_name for the "F" marker after "Peasant".
    pub fn is_female_peasant(&self, monlist_id: u8) -> bool {
        self.monsters.get(monlist_id as usize)
            .is_some_and(|m| {
                let lower = m.internal_name.to_ascii_lowercase();
                lower.starts_with("peasantf")
            })
    }

    /// Find a monster by internal name prefix + difficulty (1=A, 2=B, 3=C).
    /// Tries the requested variant first, then falls back to A, B, C.
    pub fn find_by_name(&self, name: &str, difficulty: u8) -> Option<&MonsterDesc> {
        let preferred = match difficulty {
            1 => "A",
            2 => "B",
            3 | _ => "C",
        };
        // Try preferred variant first, then all variants
        for suffix in &[preferred, "A", "B", "C"] {
            let target = format!("{}{}", name, suffix);
            if let Some(m) = self.monsters.iter().find(|m| m.internal_name.eq_ignore_ascii_case(&target)) {
                return Some(m);
            }
        }
        None
    }

    /// Find a monster whose sprite actually exists in the LOD.
    /// Uses try_get_bytes (cheap) instead of sprite() (expensive decode).
    pub fn find_with_sprite(&self, name: &str, difficulty: u8, lod_manager: &crate::LodManager) -> Option<&MonsterDesc> {
        let preferred = match difficulty {
            1 => "A",
            2 => "B",
            3 | _ => "C",
        };
        for suffix in &[preferred, "A", "B", "C"] {
            let target = format!("{}{}", name, suffix);
            if let Some(m) = self.monsters.iter().find(|m| m.internal_name.eq_ignore_ascii_case(&target)) {
                let sprite = &m.sprite_names[0];
                if !sprite.is_empty() {
                    // Cheap check: just see if the raw bytes exist in the LOD
                    let root = sprite.trim_end_matches(|c: char| c.is_ascii_digit());
                    let check_name = if root.ends_with('a') {
                        format!("{}0", root)
                    } else {
                        format!("{}a0", root)
                    };
                    if lod_manager.try_get_bytes(&format!("sprites/{}", check_name)).is_ok() {
                        return Some(m);
                    }
                    // Try stripped root
                    if root.len() > 1 {
                        let shorter = &root[..root.len()-1];
                        let check2 = format!("{}a0", shorter);
                        if lod_manager.try_get_bytes(&format!("sprites/{}", check2)).is_ok() {
                            return Some(m);
                        }
                    }
                }
            }
        }
        self.find_by_name(name, difficulty)
    }
}

#[cfg(test)]
mod tests {
    use crate::{get_lod_path, LodManager};
    use super::MonsterList;

    /// Peasant detection and gender must be derived from monlist internal_name,
    /// not from hardcoded ranges. Verifies against actual dmonlist.bin data.
    #[test]
    fn peasant_detection_from_monlist_data() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let monlist = MonsterList::new(&lod_manager).unwrap();

        // PeasantF1A-C (ids 120-122) must be detected as female peasants
        for id in 120..=122u8 {
            assert!(monlist.is_peasant(id), "monlist_id={} should be peasant", id);
            assert!(monlist.is_female_peasant(id), "monlist_id={} should be female", id);
        }
        // PeasantM1A-C (ids 132-134) must be detected as male peasants
        for id in 132..=134u8 {
            assert!(monlist.is_peasant(id), "monlist_id={} should be peasant", id);
            assert!(!monlist.is_female_peasant(id), "monlist_id={} should be male", id);
        }
        // Non-peasant types (Goblin=0, Ooze=114, Rat=144) must NOT be peasants
        for id in [0u8, 114, 144] {
            assert!(!monlist.is_peasant(id), "monlist_id={} should NOT be peasant", id);
        }
    }

    /// Table-driven test: find_by_name must return the correct A/B/C variant.
    /// Each row is (monster_name, difficulty, expected_suffix).
    /// Add new rows as we discover variant resolution issues.
    #[test]
    fn find_by_name_returns_correct_variant() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let monlist = MonsterList::new(&lod_manager).unwrap();

        let test_cases: &[(&str, u8, &str)] = &[
            // (monster_name, difficulty, expected_internal_name_suffix)
            ("Goblin",   1, "A"),
            ("Goblin",   2, "B"),
            ("Goblin",   3, "C"),
            ("Ghost",    1, "A"),
            ("Ghost",    2, "B"),
            ("Ghost",    3, "C"),
            ("Skeleton", 1, "A"),
            ("Skeleton", 2, "B"),
            ("Skeleton", 3, "C"),
            ("Spider",   1, "A"),
            ("Spider",   2, "B"),
        ];

        for &(name, dif, expected_suffix) in test_cases {
            let desc = monlist.find_by_name(name, dif)
                .unwrap_or_else(|| panic!("{} difficulty {} should exist", name, dif));
            assert!(
                desc.internal_name.ends_with(expected_suffix),
                "{} difficulty {} should return variant {}, got '{}'",
                name, dif, expected_suffix, desc.internal_name
            );
        }
    }

    /// Variants A vs B/C must have different standing sprite groups.
    /// Regression: find_with_sprite fell back to variant A for B/C,
    /// giving all variants the same sprite group and palette.
    #[test]
    fn variants_have_distinct_sprite_groups() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let monlist = MonsterList::new(&lod_manager).unwrap();

        // (monster_name, variants that must have distinct sprite groups)
        let test_cases: &[(&str, &[u8])] = &[
            ("Goblin",   &[1, 2, 3]),
            ("Ghost",    &[1, 2, 3]),
            ("Skeleton", &[1, 2, 3]),
        ];

        for &(name, variants) in test_cases {
            let descs: Vec<_> = variants.iter()
                .filter_map(|&dif| monlist.find_by_name(name, dif))
                .collect();

            // All requested variants should exist
            assert_eq!(
                descs.len(), variants.len(),
                "{} should have {} variants, found {}",
                name, variants.len(), descs.len()
            );

            // Standing sprite groups should all differ
            for i in 0..descs.len() {
                for j in (i + 1)..descs.len() {
                    assert_ne!(
                        descs[i].sprite_names[0], descs[j].sprite_names[0],
                        "{}: variant '{}' and '{}' should have different standing sprite groups",
                        name, descs[i].internal_name, descs[j].internal_name
                    );
                }
            }
        }
    }

    /// Regression: ghost walking and standing groups must resolve to the same
    /// sprite root and DSFT palette_id. Before the cache-key fix, the walking
    /// animation used a stale preload entry (palette_id=0 offset path) while
    /// standing decoded fresh with the correct DSFT palette — causing the ghost
    /// to display the wrong color during walking in outc3.
    #[test]
    fn ghost_walking_and_standing_palette_match() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let gd = crate::game::global::GameData::new(&lod).unwrap();
        for dif in 1..=3u8 {
            let desc = gd.monlist.find_by_name("Ghost", dif)
                .unwrap_or_else(|| panic!("Ghost variant {} should exist", dif));
            let st_group = &desc.sprite_names[0];
            let wa_group = &desc.sprite_names[1];
            let st = crate::game::monster::resolve_sprite_group(st_group, &gd.dsft, &lod)
                .unwrap_or_else(|| panic!("Ghost {} standing group '{}' should resolve", dif, st_group));
            let wa = crate::game::monster::resolve_sprite_group(wa_group, &gd.dsft, &lod)
                .unwrap_or_else(|| panic!("Ghost {} walking group '{}' should resolve", dif, wa_group));
            assert_eq!(
                st.0, wa.0,
                "Ghost {}: standing root '{}' should match walking root '{}'",
                dif, st.0, wa.0
            );
            assert_eq!(
                st.1, wa.1,
                "Ghost {}: standing palette_id {} should match walking palette_id {} — mismatch causes wrong color during walking animation",
                dif, st.1, wa.1
            );
            // B/C variants must have non-zero DSFT palette_id so sprite_with_palette is used
            if dif > 1 {
                assert!(
                    st.1 > 0,
                    "Ghost {} DSFT palette_id should be non-zero (got {}), otherwise sprite-header offset path is used instead",
                    dif, st.1
                );
            }
        }
    }

}
