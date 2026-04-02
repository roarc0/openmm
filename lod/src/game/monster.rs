//! Monster sprite resolution — maps DSFT group names to sprite file roots.

use std::error::Error;
use crate::LodManager;

/// A single resolved monster spawn entry for a map.
/// One entry per group member. Spread position is computed by the caller using
/// `spawn_position`, `spawn_radius`, and `group_index`.
pub struct Monster {
    /// Group center in MM6 coordinates (NOT yet spread). Caller applies angle × radius.
    pub spawn_position: [i32; 3],
    /// Radius from center for position spreading.
    pub spawn_radius: u16,
    /// Index within the group (0..group_size). Used by caller to compute spread angle.
    pub group_index: usize,
    /// DSFT-resolved standing sprite root (e.g. "gobl").
    pub standing_sprite: String,
    /// DSFT-resolved walking sprite root (falls back to standing_sprite).
    pub walking_sprite: String,
    /// DSFT palette_id for this variant.
    pub palette_id: u16,
    /// Difficulty variant: 1=A (base), 2=B, 3=C. From mapstats difficulty + RNG seed.
    pub variant: u8,
    /// Height in MM6 units (from dmonlist.bin).
    pub height: u16,
    /// Movement speed in MM6 units/tick (from dmonlist.bin).
    pub move_speed: u16,
    /// True for all ODM spawn monsters (as opposed to DDM placed actors).
    pub hostile: bool,
    /// Collision radius for the actor AI tether.
    pub radius: u16,
}

/// Per-map resolved monster spawn roster. Created once per map load via `Monsters::new()`.
pub struct Monsters {
    entries: Vec<Monster>,
}

impl Monsters {
    /// Resolve all monster spawns for the given outdoor map.
    ///
    /// Loads MapStats and MonsterList from LOD; resolves sprite roots via DSFT.
    /// Returns one `Monster` per group member, with `spawn_position` = group center.
    /// Position spreading (angle × radius) is left to the caller.
    pub fn new(lod: &LodManager, map_name: &str) -> Result<Self, Box<dyn Error>> {
        let odm = crate::odm::Odm::new(lod, map_name)?;
        let mapstats = crate::mapstats::MapStats::new(lod)?;
        let monlist = crate::monlist::MonsterList::new(lod)?;
        let map_name_lower = map_name.to_lowercase();
        let cfg = mapstats.get(&map_name_lower).ok_or("map not found in mapstats")?;

        let mut entries = Vec::new();
        for sp in &odm.spawn_points {
            let group_size = 3 + ((sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs()) % 3) as usize;
            for g in 0..group_size {
                let seed = sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs() + g as u32;
                let Some((mon_name, dif)) = cfg.monster_for_index(sp.monster_index, seed) else { continue };
                let Some(desc) = monlist.find_by_name(mon_name, dif) else { continue };

                let st_group = &desc.sprite_names[0];
                let wa_group = &desc.sprite_names[1];
                let Some((st_root, palette_id)) = resolve_sprite_group(st_group, lod) else {
                    log::warn!("Monster '{}' standing sprite '{}' not found in DSFT — skipping", mon_name, st_group);
                    continue;
                };
                let wa_root = resolve_sprite_group(wa_group, lod)
                    .map(|(n, _)| n)
                    .unwrap_or_else(|| st_root.clone());

                entries.push(Monster {
                    spawn_position: sp.position,
                    spawn_radius: sp.radius.max(200),
                    group_index: g,
                    standing_sprite: st_root,
                    walking_sprite: wa_root,
                    palette_id: palette_id as u16,
                    variant: dif,
                    height: desc.height,
                    move_speed: desc.move_speed,
                    hostile: true,
                    radius: sp.radius.max(300),
                });
            }
        }
        Ok(Monsters { entries })
    }

    pub fn default_empty() -> Self { Monsters { entries: Vec::new() } }
    pub fn entries(&self) -> &[Monster] { &self.entries }
    pub fn iter(&self) -> impl Iterator<Item = &Monster> { self.entries.iter() }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

/// Resolved sprite data for one monlist entry.
pub struct MonsterEntry {
    pub standing_sprite: String,
    pub walking_sprite: String,
    pub palette_id: u16,
    pub is_peasant: bool,
    pub is_female: bool,
}

/// Resolve a DSFT group name to a sprite file root and palette_id.
/// This is the canonical implementation — copied verbatim from
/// openmm/src/game/odm.rs:resolve_dsft_sprite(), only name changes.
pub fn resolve_sprite_group(group_name: &str, lod: &LodManager) -> Option<(String, i16)> {
    if group_name.is_empty() {
        return None;
    }
    let Ok(dsft) = crate::dsft::DSFT::new(lod) else {
        return None;
    };
    for frame in &dsft.frames {
        if let Some(gname) = frame.group_name() {
            if gname.eq_ignore_ascii_case(group_name) {
                if let Some(sprite_name) = frame.sprite_name() {
                    let without_digits = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
                    let root = if without_digits.len() > 1 {
                        let last = without_digits.as_bytes()[without_digits.len() - 1];
                        if last >= b'a' && last <= b'f' {
                            &without_digits[..without_digits.len() - 1]
                        } else {
                            without_digits
                        }
                    } else {
                        without_digits
                    };
                    let test = format!("sprites/{}a0", root.to_lowercase());
                    if lod.try_get_bytes(&test).is_ok() {
                        return Some((root.to_lowercase(), frame.palette_id));
                    }
                }
                break;
            }
        }
    }
    // Fallback: try the group name directly with progressively shorter prefixes
    let root = group_name.trim_end_matches(|c: char| c.is_ascii_digit());
    let mut try_root = root;
    while try_root.len() >= 3 {
        let test = format!("sprites/{}a0", try_root.to_lowercase());
        if lod.try_get_bytes(&test).is_ok() {
            return Some((try_root.to_lowercase(), 0));
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    None
}

/// Resolve sprite roots and metadata for a monlist entry by 0-based index.
/// Loads MonsterList and DSFT internally.
pub fn resolve_entry(monlist_id: u8, lod: &LodManager) -> Option<MonsterEntry> {
    let monlist = crate::monlist::MonsterList::new(lod).ok()?;
    let desc = monlist.get(monlist_id as usize)?;
    let st_group = &desc.sprite_names[0];
    let wa_group = &desc.sprite_names[1];
    if st_group.is_empty() {
        return None;
    }
    let (standing_sprite, palette_id) = resolve_sprite_group(st_group, lod)?;
    let walking_sprite = resolve_sprite_group(wa_group, lod)
        .map(|(n, _)| n)
        .unwrap_or_else(|| standing_sprite.clone());
    Some(MonsterEntry {
        standing_sprite,
        walking_sprite,
        palette_id: palette_id as u16,
        is_peasant: monlist.is_peasant(monlist_id),
        is_female: monlist.is_female_peasant(monlist_id),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, LodManager};

    #[test]
    fn monsters_loads_oute3() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let monsters = Monsters::new(&lod, "oute3.odm").unwrap();
        assert!(!monsters.is_empty(), "oute3 should have monster spawns");
    }

    #[test]
    fn monsters_all_have_sprites() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let monsters = Monsters::new(&lod, "oute3.odm").unwrap();
        for m in monsters.iter() {
            assert!(!m.standing_sprite.is_empty(),
                "every monster should have a standing sprite");
        }
    }

    #[test]
    fn monsters_variant_in_range() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let monsters = Monsters::new(&lod, "oute3.odm").unwrap();
        for m in monsters.iter() {
            assert!(m.variant >= 1 && m.variant <= 3,
                "variant should be 1-3, got {}", m.variant);
        }
    }

    #[test]
    fn monsters_group_index_within_group_size() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let monsters = Monsters::new(&lod, "oute3.odm").unwrap();
        // group_size is 3..=5, so group_index must be < 6
        for m in monsters.iter() {
            assert!(m.group_index < 6,
                "group_index {} seems too large", m.group_index);
        }
    }

    #[test]
    fn goblin_a_resolves_standing_sprite() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let entry = resolve_entry(0, &lod);
        assert!(entry.is_some(), "GoblinA (monlist_id=0) should resolve");
        let entry = entry.unwrap();
        assert!(!entry.standing_sprite.is_empty(), "standing sprite should not be empty");
        assert!(!entry.walking_sprite.is_empty(), "walking sprite should not be empty");
    }

    #[test]
    fn resolve_sprite_group_goblin_standing() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let monlist = crate::monlist::MonsterList::new(&lod).unwrap();
        let goblin_a = monlist.find_by_name("Goblin", 1).unwrap();
        let group = &goblin_a.sprite_names[0];
        let result = resolve_sprite_group(group, &lod);
        assert!(result.is_some(), "GoblinA standing group '{}' should resolve", group);
    }

    #[test]
    fn resolve_sprite_group_empty_name_returns_none() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        assert!(resolve_sprite_group("", &lod).is_none());
    }

    #[test]
    fn resolve_entry_peasant_male_is_flagged() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        // PeasantM1A is monlist_id 132 (from monlist tests)
        let entry = resolve_entry(132, &lod);
        assert!(entry.is_some(), "PeasantM1A should resolve");
        let entry = entry.unwrap();
        assert!(entry.is_peasant);
        assert!(!entry.is_female);
    }

    #[test]
    fn resolve_entry_peasant_female_is_flagged() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        // PeasantF1A is monlist_id 120
        let entry = resolve_entry(120, &lod);
        assert!(entry.is_some(), "PeasantF1A should resolve");
        let entry = entry.unwrap();
        assert!(entry.is_peasant);
        assert!(entry.is_female);
    }
}
