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
    /// Max HP from monsters.txt (initialized at spawn, variant-specific).
    pub hp: i16,
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
            // Skip item/treasure spawns (spawn_type=2); only process monsters (spawn_type=3).
            if sp.spawn_type != 3 {
                continue;
            }

            // Resolve monster type and variant class for this spawn point.
            let Some((mon_name, mapstats_display, slot, forced_variant)) = cfg.monster_for_index(sp.monster_index)
            else {
                continue;
            };

            // Group size from mapstats Mon1Low/Mon1Hi range.
            // Seed from |x|*|y| gives better spread across spawns than |x|+|y|.
            let pos_seed = sp.position[0]
                .unsigned_abs()
                .wrapping_mul(sp.position[1].unsigned_abs());
            let (count_min, count_max) = cfg.count_range_for_slot(slot);
            let range = (count_max - count_min) as u32 + 1;
            let group_size = count_min as usize + (pos_seed % range) as usize;

            for g in 0..group_size {
                // Each monster independently rolls for A/B/C variant from the difficulty table.
                // No special "champion" rule — all members use the same probability distribution.
                let variant = if forced_variant != 0 {
                    forced_variant
                } else {
                    // Mix position seed with member index using Knuth multiplicative hash.
                    let member_seed = pos_seed.wrapping_add((g as u32).wrapping_mul(2654435761));
                    let roll = (member_seed % 100) as u8;
                    cfg.variant_from_roll(slot, roll)
                };

                let Some(desc) = game_data.monlist.find_by_name(mon_name, variant) else {
                    continue;
                };

                // Per-variant name from monsters.txt takes priority over the mapstats base name.
                // e.g. PeasantM2 A="Apprentice Mage", B="Journeyman Mage", C="Mage".
                let display_name = game_data
                    .monsters_txt
                    .display_name(mon_name, variant)
                    .unwrap_or(mapstats_display);

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
                let hp = game_data.monsters_txt.max_hp(mon_name, variant).unwrap_or(1);

                entries.push(Monster {
                    name: display_name.to_string(),
                    spawn_position: sp.position,
                    spawn_radius: sp.radius,
                    group_index: g,
                    standing_sprite: st_root,
                    walking_sprite: wa_root,
                    attacking_sprite: at_root,
                    palette_id: palette_id as u16,
                    variant,
                    height: desc.height,
                    move_speed: desc.move_speed,
                    hostile: true,
                    radius: sp.radius.max(300),
                    sound_ids: desc.sound_ids,
                    to_hit_radius: desc.to_hit_radius,
                    hp,
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
    /// Variant derived from internal_name suffix: 1=A, 2=B, 3=C.
    pub variant: u8,
    /// Full internal name from dmonlist.bin (e.g. "GoblinA"). Used for monsters.txt HP lookup.
    pub internal_name: String,
    pub is_peasant: bool,
    pub is_female: bool,
    /// Melee attack reach in MM6 world units (from dmonlist.bin).
    pub to_hit_radius: u16,
    pub sound_ids: [u16; 4],
}

/// Derive A/B/C variant (1/2/3) from a dmonlist internal_name suffix.
/// "GoblinA" → 1, "GoblinB" → 2, "GoblinC" → 3, anything else → 1.
pub fn variant_from_internal_name(name: &str) -> u8 {
    match name.chars().last() {
        Some('A') | Some('a') => 1,
        Some('B') | Some('b') => 2,
        Some('C') | Some('c') => 3,
        _ => 1,
    }
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
        variant: variant_from_internal_name(&desc.internal_name),
        internal_name: desc.internal_name.clone(),
        is_peasant: game_data.monlist.is_peasant(monlist_id),
        is_female: game_data.monlist.is_female_peasant(monlist_id),
        to_hit_radius: desc.to_hit_radius,
        sound_ids: desc.sound_ids,
    })
}

#[cfg(test)]
#[path = "monster_tests.rs"]
mod tests;
