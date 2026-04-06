//! Monster sprite resolution — maps DSFT group names to sprite file roots.

use crate::Assets;
use crate::assets::GameData;
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
    /// Sprite root for the dying animation (sprite_names[5], lowercased).
    pub dying_sprite: String,
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
    /// Physical body radius from dmonlist.bin (bytes 2-3). Used for attack reach.
    /// Note: `to_hit_radius` (bytes 6-7) is always 0 in MM6 (MM7+ field); use this instead.
    pub body_radius: u16,
    /// Sound IDs from dmonlist.bin: [attack, die, got_hit, fidget].
    pub sound_ids: [u16; 4],
    /// Bytes 6-7 of dmonlist.bin record. Always 0 in MM6 (this is the MM7 Radius2 field).
    /// Kept for completeness; use `body_radius * 2` for attack range instead.
    pub to_hit_radius: u16,
    /// Max HP from monsters.txt (initialized at spawn, variant-specific).
    pub hp: i16,
    /// Whether this monster can fly (from monsters.txt can_fly column).
    pub can_fly: bool,
    /// AI behaviour type: "Normal", "Aggress", "Wimp", "Suicidal" (from monsters.txt ai_type).
    pub ai_type: String,
    /// Aggro detection radius in MM6 world units (derived from hostile_type in monsters.txt).
    pub aggro_range: f32,
    /// Attack recovery in seconds (derived from recovery ticks in monsters.txt).
    pub recovery_secs: f32,
}

/// Per-map resolved monster spawn roster. Created once per map load via `Monsters::load()`.
pub struct Monsters {
    entries: Vec<Monster>,
}

/// Minimal spawn-point interface shared by ODM and BLV.
pub struct SpawnPointRef {
    pub position: [i32; 3],
    pub radius: u16,
    pub spawn_type: u16,
    pub monster_index: u16,
}

impl Monsters {
    /// Resolve all monster spawns for the given outdoor map.
    ///
    /// Uses pre-loaded `GameData` (MapStats, MonsterList, DSFT) — no per-call LOD reads.
    /// Returns one `Monster` per group member, with `spawn_position` = group center.
    /// Position spreading (angle × radius) is left to the caller.
    pub fn load(assets: &Assets, map_name: &str, game_data: &GameData) -> Result<Self, Box<dyn Error>> {
        let odm = crate::assets::odm::Odm::load(assets, map_name)?;
        let spawn_points: Vec<SpawnPointRef> = odm
            .spawn_points
            .iter()
            .map(|sp| SpawnPointRef {
                position: sp.position,
                radius: sp.radius,
                spawn_type: sp.spawn_type,
                monster_index: sp.monster_index,
            })
            .collect();
        Self::from_spawn_points(&spawn_points, map_name, game_data, assets)
    }

    /// Resolve monster spawns for an indoor (BLV) map from its spawn points.
    pub fn load_for_blv(
        blv_spawn_points: &[crate::assets::blv::BlvSpawnPoint],
        map_name: &str,
        game_data: &GameData,
        assets: &Assets,
    ) -> Result<Self, Box<dyn Error>> {
        let spawn_points: Vec<SpawnPointRef> = blv_spawn_points
            .iter()
            .map(|sp| SpawnPointRef {
                position: sp.position,
                radius: sp.radius,
                spawn_type: sp.spawn_type,
                monster_index: sp.monster_index,
            })
            .collect();
        Self::from_spawn_points(&spawn_points, map_name, game_data, assets)
    }

    fn from_spawn_points(
        spawn_points: &[SpawnPointRef],
        map_name: &str,
        game_data: &GameData,
        assets: &Assets,
    ) -> Result<Self, Box<dyn Error>> {
        let map_name_lower = map_name.to_lowercase();
        let cfg = game_data
            .mapstats
            .get(&map_name_lower)
            .ok_or("map not found in mapstats")?;

        let mut entries = Vec::new();
        for sp in spawn_points {
            // Skip item/treasure spawns (spawn_type=2); only process monsters (spawn_type=3).
            if sp.spawn_type != 3 {
                continue;
            }

            // Resolve monster type and variant class for this spawn point.
            let Some((mon_name, mapstats_display, slot, forced_variant)) = cfg.monster_for_index(sp.monster_index)
            else {
                continue;
            };

            // Deterministic substitute for MM6's global LCG Rand(). |x|*|y| gives better
            // spread across spawn points than |x|+|y|.
            let pos_seed = sp.position[0]
                .unsigned_abs()
                .wrapping_mul(sp.position[1].unsigned_abs());

            // MM6 fcn_00455910: forced-variant spawn points (monster_index 4-12) always
            // produce exactly 1 monster — ebx=1 at function start, never updated for
            // forced cases (they jump to label_4, bypassing the Rand() group-size calc).
            // Random spawn points (monster_index 1-3) use Rand() % (max-min+1) + min.
            let group_size = if forced_variant != 0 {
                1
            } else {
                let (count_min, count_max) = cfg.count_range_for_slot(slot);
                let range = (count_max - count_min) as u32 + 1;
                count_min as usize + (pos_seed % range) as usize
            };

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
                    .monsters
                    .display_name(mon_name, variant)
                    .unwrap_or(mapstats_display);

                let st_group = &desc.sprite_names[0];
                let wa_group = &desc.sprite_names[1];
                let at_group = &desc.sprite_names[2];
                let Some((st_root, palette_id)) = resolve_sprite_group(st_group, &game_data.dsft, assets) else {
                    log::warn!(
                        "Monster '{}' standing sprite '{}' not found in DSFT — skipping",
                        mon_name,
                        st_group
                    );
                    continue;
                };
                let wa_root = resolve_sprite_group(wa_group, &game_data.dsft, assets)
                    .map(|(n, _)| n)
                    .unwrap_or_else(|| st_root.clone());
                // sprite_names[2] and [5] are DSFT group name, not sprite file roots.
                // Must resolve through DSFT to get the actual LOD file root.
                let at_root = resolve_sprite_group(at_group, &game_data.dsft, assets)
                    .map(|(n, _)| n)
                    .unwrap_or_else(|| at_group.to_lowercase());
                let dy_group = &desc.sprite_names[5];
                let dying_root = resolve_sprite_group(dy_group, &game_data.dsft, assets)
                    .map(|(n, _)| n)
                    .unwrap_or_else(|| dy_group.to_lowercase());
                let stats = game_data.monsters.get(mon_name, variant);
                let hp = stats.map(|s| s.hp).unwrap_or(1);
                let aggro_range = stats.map(|s| s.aggro_range()).unwrap_or(2560.0);
                let recovery_secs = stats.map(|s| s.recovery_secs()).unwrap_or(2.0);
                let can_fly = stats.map(|s| s.can_fly).unwrap_or(false);
                let ai_type = stats.map(|s| s.ai_type.clone()).unwrap_or_default();

                entries.push(Monster {
                    name: display_name.to_string(),
                    spawn_position: sp.position,
                    spawn_radius: sp.radius,
                    group_index: g,
                    standing_sprite: st_root,
                    walking_sprite: wa_root,
                    attacking_sprite: at_root,
                    dying_sprite: dying_root,
                    palette_id: palette_id as u16,
                    variant,
                    height: desc.height,
                    move_speed: desc.move_speed,
                    hostile: true,
                    radius: sp.radius.max(300),
                    body_radius: desc.radius,
                    sound_ids: desc.sound_ids,
                    to_hit_radius: desc.to_hit_radius,
                    hp,
                    can_fly,
                    ai_type,
                    aggro_range,
                    recovery_secs,
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
    /// Sprite root for the dying animation (sprite_names[5], lowercased).
    pub dying_sprite: String,
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
    /// Whether this monster can fly (from monsters.txt can_fly column).
    pub can_fly: bool,
    /// AI behaviour type: "Normal", "Aggress", "Wimp", "Suicidal" (from monsters.txt ai_type column).
    pub ai_type: String,
    /// Aggro detection radius in MM6 world units (derived from hostile_type in monsters.txt).
    pub aggro_range: f32,
    /// Attack recovery in seconds (derived from recovery ticks in monsters.txt).
    pub recovery_secs: f32,
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
pub fn resolve_sprite_group(
    group_name: &str,
    dsft: &crate::assets::dsft::DSFT,
    assets: &Assets,
) -> Option<(String, i16)> {
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
                if assets.get_bytes(&test).is_ok() {
                    return Some((root.to_lowercase(), frame.palette_id));
                }
                // Single-frame sprite: the DSFT sprite_name IS the file name with no
                // frame/direction suffix (e.g. "arc1diq"). Test it directly.
                let test_root = format!("sprites/{}", root.to_lowercase());
                if assets.get_bytes(&test_root).is_ok() {
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
        if assets.get_bytes(&test).is_ok() {
            return Some((try_root.to_lowercase(), 0));
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    None
}

/// Resolve sprite roots and metadata for a monlist entry by 0-based index.
/// Uses pre-loaded `GameData` — no per-call LOD reads.
pub fn resolve_entry(monlist_id: u8, game_data: &GameData, assets: &Assets) -> Option<MonsterEntry> {
    let desc = game_data.monlist.get(monlist_id as usize)?;
    let st_group = &desc.sprite_names[0];
    let wa_group = &desc.sprite_names[1];
    if st_group.is_empty() {
        return None;
    }
    let (standing_sprite, palette_id) = resolve_sprite_group(st_group, &game_data.dsft, assets)?;
    let walking_sprite = resolve_sprite_group(wa_group, &game_data.dsft, assets)
        .map(|(n, _)| n)
        .unwrap_or_else(|| standing_sprite.clone());
    // sprite_names[2] and [5] are DSFT group names, not sprite file roots.
    let at_group = &desc.sprite_names[2];
    let dy_group = &desc.sprite_names[5];
    let attacking_sprite = resolve_sprite_group(at_group, &game_data.dsft, assets)
        .map(|(n, _)| n)
        .unwrap_or_else(|| at_group.to_lowercase());
    let dying_sprite = resolve_sprite_group(dy_group, &game_data.dsft, assets)
        .map(|(n, _)| n)
        .unwrap_or_else(|| dy_group.to_lowercase());
    // Look up per-variant stats from monsters.txt for behavior parameters.
    let prefix = desc.internal_name.trim_end_matches(|c: char| c.is_ascii_uppercase());
    let v = variant_from_internal_name(&desc.internal_name);
    let stats = game_data.monsters.get(prefix, v);
    let aggro_range = stats
        .map(|s: &crate::assets::monsters::MonsterStats| s.aggro_range())
        .unwrap_or(2560.0);
    let recovery_secs = stats
        .map(|s: &crate::assets::monsters::MonsterStats| s.recovery_secs())
        .unwrap_or(2.0);
    let can_fly = stats.map(|s| s.can_fly).unwrap_or(false);
    let ai_type = stats.map(|s| s.ai_type.clone()).unwrap_or_default();
    Some(MonsterEntry {
        standing_sprite,
        walking_sprite,
        attacking_sprite,
        dying_sprite,
        palette_id: palette_id as u16,
        variant: variant_from_internal_name(&desc.internal_name),
        internal_name: desc.internal_name.clone(),
        is_peasant: game_data.monlist.is_peasant(monlist_id),
        is_female: game_data.monlist.is_female_peasant(monlist_id),
        to_hit_radius: desc.to_hit_radius,
        sound_ids: desc.sound_ids,
        can_fly,
        ai_type,
        aggro_range,
        recovery_secs,
    })
}

#[cfg(test)]
#[path = "monster_tests.rs"] // relative to provider/
mod tests;
