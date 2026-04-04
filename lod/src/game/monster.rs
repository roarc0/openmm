//! Monster sprite resolution — maps DSFT group names to sprite file roots.

use super::global::GameData;
use crate::LodManager;
use std::error::Error;

/// A single resolved monster spawn entry for a map.
/// One entry per group member. Spread position is computed by the caller using
/// `spawn_position`, `spawn_radius`, and `group_index`.
pub struct Monster {
    /// Display name from mapstats (e.g. "Goblin", "Orc").
    pub name: String,
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
    /// Sprite root for the attack1 animation (sprite_names[2], lowercased).
    pub attacking_sprite: String,
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
    /// Sound IDs from dmonlist.bin: [attack, die, got_hit, fidget].
    pub sound_ids: [u16; 4],
    /// Melee attack reach in MM6 world units (from dmonlist.bin).
    pub to_hit_radius: u16,
}

/// Per-map resolved monster spawn roster. Created once per map load via `Monsters::new()`.
pub struct Monsters {
    entries: Vec<Monster>,
}

impl Monsters {
    /// Resolve all monster spawns for the given outdoor map.
    ///
    /// Uses pre-loaded `GameData` (MapStats, MonsterList, DSFT) — no per-call LOD reads.
    /// Returns one `Monster` per group member, with `spawn_position` = group center.
    /// Position spreading (angle × radius) is left to the caller.
    pub fn new(lod: &LodManager, map_name: &str, game_data: &GameData) -> Result<Self, Box<dyn Error>> {
        let odm = crate::odm::Odm::new(lod, map_name)?;
        let map_name_lower = map_name.to_lowercase();
        let cfg = game_data
            .mapstats
            .get(&map_name_lower)
            .ok_or("map not found in mapstats")?;

        let mut entries = Vec::new();
        for sp in &odm.spawn_points {
            let group_size = 3 + ((sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs()) % 3) as usize;
            for g in 0..group_size {
                let seed = sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs() + g as u32;
                let Some((mon_name, dif)) = cfg.monster_for_index(sp.monster_index, seed) else {
                    continue;
                };
                let Some(desc) = game_data.monlist.find_by_name(mon_name, dif) else {
                    continue;
                };

                let st_group = &desc.sprite_names[0];
                let wa_group = &desc.sprite_names[1];
                let at_group = &desc.sprite_names[2];
                let Some((st_root, palette_id)) = resolve_sprite_group(st_group, &game_data.dsft, lod) else {
                    log::warn!(
                        "Monster '{}' standing sprite '{}' not found in DSFT — skipping",
                        mon_name,
                        st_group
                    );
                    continue;
                };
                let wa_root = resolve_sprite_group(wa_group, &game_data.dsft, lod)
                    .map(|(n, _)| n)
                    .unwrap_or_else(|| st_root.clone());
                let at_root = at_group.to_lowercase();

                entries.push(Monster {
                    name: mon_name.to_string(),
                    spawn_position: sp.position,
                    spawn_radius: sp.radius.max(200),
                    group_index: g,
                    standing_sprite: st_root,
                    walking_sprite: wa_root,
                    attacking_sprite: at_root,
                    palette_id: palette_id as u16,
                    variant: dif,
                    height: desc.height,
                    move_speed: desc.move_speed,
                    hostile: true,
                    radius: sp.radius.max(300),
                    sound_ids: desc.sound_ids,
                    to_hit_radius: desc.to_hit_radius,
                });
            }
        }
        Ok(Monsters { entries })
    }

    pub fn default_empty() -> Self {
        Monsters { entries: Vec::new() }
    }
    pub fn entries(&self) -> &[Monster] {
        &self.entries
    }
    pub fn iter(&self) -> impl Iterator<Item = &Monster> {
        self.entries.iter()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Resolved sprite data for one monlist entry.
pub struct MonsterEntry {
    pub standing_sprite: String,
    pub walking_sprite: String,
    /// Sprite root for the attack1 animation (sprite_names[2], lowercased).
    pub attacking_sprite: String,
    pub palette_id: u16,
    pub is_peasant: bool,
    pub is_female: bool,
    /// Melee attack reach in MM6 world units (from dmonlist.bin).
    pub to_hit_radius: u16,
    pub sound_ids: [u16; 4],
}

/// Resolve a DSFT group name to a sprite file root and palette_id.
/// `dsft` must be pre-loaded by the caller — this function never allocates it.
pub fn resolve_sprite_group(group_name: &str, dsft: &crate::dsft::DSFT, lod: &LodManager) -> Option<(String, i16)> {
    if group_name.is_empty() {
        return None;
    }
    for frame in &dsft.frames {
        if let Some(gname) = frame.group_name()
            && gname.eq_ignore_ascii_case(group_name)
        {
            if let Some(sprite_name) = frame.sprite_name() {
                let without_digits = sprite_name.trim_end_matches(|c: char| c.is_ascii_digit());
                let root = if without_digits.len() > 1 {
                    let last = without_digits.as_bytes()[without_digits.len() - 1];
                    if (b'a'..=b'f').contains(&last) {
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
/// Uses pre-loaded `GameData` — no per-call LOD reads.
pub fn resolve_entry(monlist_id: u8, game_data: &GameData, lod: &LodManager) -> Option<MonsterEntry> {
    let desc = game_data.monlist.get(monlist_id as usize)?;
    let st_group = &desc.sprite_names[0];
    let wa_group = &desc.sprite_names[1];
    let at_group = &desc.sprite_names[2];
    if st_group.is_empty() {
        return None;
    }
    let (standing_sprite, palette_id) = resolve_sprite_group(st_group, &game_data.dsft, lod)?;
    let walking_sprite = resolve_sprite_group(wa_group, &game_data.dsft, lod)
        .map(|(n, _)| n)
        .unwrap_or_else(|| standing_sprite.clone());
    let attacking_sprite = at_group.to_lowercase();
    Some(MonsterEntry {
        standing_sprite,
        walking_sprite,
        attacking_sprite,
        palette_id: palette_id as u16,
        is_peasant: game_data.monlist.is_peasant(monlist_id),
        is_female: game_data.monlist.is_female_peasant(monlist_id),
        to_hit_radius: desc.to_hit_radius,
        sound_ids: desc.sound_ids,
    })
}

#[cfg(test)]
#[path = "monster_tests.rs"]
mod tests;
