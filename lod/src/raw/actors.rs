//! Per-map actor roster: pre-resolved DDM actors (NPCs and named monsters).
//!
//! `Actors::new(lod, map_name, state)` loads and resolves all DDM actors for the map.
//! Each `Actor` carries pre-resolved sprite roots and NPC identity so openmm needs
//! no direct LOD queries during spawn.

use crate::raw::global::GameData;
use super::monster;
use crate::LodManager;
use crate::raw::ddm::{MonsterSchedule, SpellBuff};
use std::error::Error;

/// Stub for future map-state persistence. When populated, actors whose index
/// is in `dead_actor_ids` are excluded from the returned roster.
pub struct MapStateSnapshot {
    pub dead_actor_ids: Vec<u16>,
}

/// A fully resolved actor from the map's DDM file.
pub struct Actor {
    pub position: [i32; 3],
    pub standing_sprite: String,
    pub walking_sprite: String,
    /// Sprite root for the attack1 animation (from MonsterEntry.attacking_sprite).
    pub attacking_sprite: String,
    /// Sprite root for the dying animation (from MonsterEntry.dying_sprite).
    pub dying_sprite: String,
    pub palette_id: u16,
    /// Variant: 1=A (base), 2=B, 3=C. Derived from dmonlist internal_name suffix (e.g. "GoblinB" → 2).
    /// actors sharing the same standing_sprite root. Never 0 after construction.
    pub variant: u8,
    pub name: String,
    pub portrait_name: Option<String>, // NPC only, e.g. "NPC042"
    pub profession_id: Option<u8>,     // NPC only
    pub radius: u16,
    pub height: u16,
    pub move_speed: u16,
    pub hp: i16,
    pub tether_distance: u16,
    npc_id: i16,
    pub monlist_id: u8,
    pub is_peasant: bool,
    pub is_female: bool,
    /// Melee attack reach in MM6 world units (from dmonlist.bin).
    pub to_hit_radius: u16,
    /// Sound IDs from DDM: [attack, die, got_hit, fidget].
    pub sound_ids: [u16; 4],
    /// Actor attribute bitflags (DDM Bits field). Controls behavior (flee, NPC flags, etc.).
    pub attributes: u32,
    /// Range-attack type indicator from DDM (0 = melee only).
    pub range_attack: i16,
    /// Secondary monster type ID from DDM (Id2).
    pub monster_id_type: i16,
    /// Faction group ID for diplomacy (0 = none).
    pub group: i32,
    /// Ally faction ID (0 = none).
    pub ally: i32,
    /// Active spell buffs (14 slots). Typically zero in freshly saved maps.
    pub spell_buffs: [SpellBuff; 14],
    /// Time-based AI schedules (8 slots: position + action + time of day).
    pub schedules: [MonsterSchedule; 8],
    /// Whether this monster can fly (from monsters.txt can_fly column).
    pub can_fly: bool,
    /// AI behaviour type: "Normal", "Aggress", "Wimp", "Suicidal" (from monsters.txt ai_type).
    pub ai_type: String,
    /// Aggro detection radius in MM6 world units (derived from hostile_type in monsters.txt).
    pub aggro_range: f32,
    /// Attack recovery in seconds (derived from recovery ticks in monsters.txt).
    pub recovery_secs: f32,
}

impl Actor {
    pub fn is_npc(&self) -> bool {
        self.npc_id > 0
    }
    pub fn is_monster(&self) -> bool {
        self.npc_id == 0
    }
    pub fn npc_id(&self) -> i16 {
        self.npc_id
    }
}

/// Per-map roster of pre-resolved DDM actors. Created once per map load.
pub struct Actors {
    actors: Vec<Actor>,
}

impl Actors {
    /// Load and fully resolve all actors for the given map.
    /// Uses pre-loaded `GameData` — no per-call LOD reads for sprites or NPC tables.
    /// Actors at indices in `state.dead_actor_ids` are excluded.
    pub fn new(
        lod: &LodManager,
        map_name: &str,
        state: Option<&MapStateSnapshot>,
        game_data: &GameData,
    ) -> Result<Self, Box<dyn Error>> {
        let ddm = crate::raw::ddm::Ddm::load(lod, map_name)?;
        let street_npcs = game_data.street_npcs.as_ref();

        let dead_ids: &[u16] = state.map(|s| s.dead_actor_ids.as_slice()).unwrap_or(&[]);

        let raw_monsters = ddm.actors.iter().filter(|a| a.npc_id == 0).count();
        let raw_npcs = ddm.actors.iter().filter(|a| a.npc_id > 0).count();
        log::warn!(
            "Actors::new {}: {} raw actors ({} monsters npc_id=0, {} NPCs)",
            map_name,
            ddm.actors.len(),
            raw_monsters,
            raw_npcs
        );

        let mut actors = Vec::with_capacity(ddm.actors.len());

        for (idx, raw) in ddm.actors.iter().enumerate() {
            if dead_ids.contains(&(idx as u16)) {
                continue;
            }
            // Do NOT filter by hp == 0 here. Fresh DDM files (never saved) have hp=0 for all
            // monsters because the original engine initialises HP from monsters.txt at load time.
            // Dead actors are tracked by dead_actor_ids once map-state persistence is implemented.

            let Some(entry) = crate::raw::monster::resolve_entry(raw.monlist_id, game_data, lod) else {
                log::warn!(
                    "Actor '{}' idx={} npc_id={} monlist_id={} has no DSFT sprite — skipping",
                    raw.name,
                    idx,
                    raw.npc_id,
                    raw.monlist_id
                );
                continue;
            };

            let (portrait_name, profession_id, name) = if raw.npc_id > 0 {
                let portrait = street_npcs.and_then(|t| t.portrait_name(raw.npc_id as i32));
                let prof = street_npcs
                    .and_then(|t| t.get(raw.npc_id as i32))
                    .map(|e| e.profession_id as u8);
                let name = street_npcs
                    .and_then(|t| t.npc_name(raw.npc_id as i32))
                    .unwrap_or(&raw.name)
                    .to_string();
                (portrait, prof, name)
            } else {
                (None, None, raw.name.clone())
            };

            // Initialise HP from monsters.txt when the DDM value is 0 (fresh/unvisited map).
            let hp = if raw.hp > 0 {
                raw.hp
            } else {
                let prefix = entry.internal_name.trim_end_matches(|c: char| c.is_ascii_uppercase());
                game_data.monsters.max_hp(prefix, entry.variant).unwrap_or(1)
            };

            actors.push(Actor {
                position: [raw.position[0] as i32, raw.position[1] as i32, raw.position[2] as i32],
                standing_sprite: entry.standing_sprite,
                walking_sprite: entry.walking_sprite,
                attacking_sprite: entry.attacking_sprite,
                dying_sprite: entry.dying_sprite,
                palette_id: entry.palette_id,
                variant: entry.variant,
                name,
                portrait_name,
                profession_id,
                radius: raw.radius,
                height: raw.height,
                move_speed: raw.move_speed,
                hp,
                tether_distance: raw.tether_distance,
                npc_id: raw.npc_id,
                monlist_id: raw.monlist_id,
                is_peasant: entry.is_peasant,
                is_female: entry.is_female,
                to_hit_radius: entry.to_hit_radius,
                sound_ids: raw.sound_ids,
                attributes: raw.attributes,
                range_attack: raw.range_attack,
                monster_id_type: raw.monster_id_type,
                group: raw.group,
                ally: raw.ally,
                spell_buffs: raw.spell_buffs,
                schedules: raw.schedules,
                aggro_range: entry.aggro_range,
                recovery_secs: entry.recovery_secs,
                can_fly: entry.can_fly,
                ai_type: entry.ai_type.clone(),
            });
        }

        let n_monsters = actors.iter().filter(|a| a.npc_id == 0).count();
        let n_npcs = actors.iter().filter(|a| a.npc_id > 0).count();
        log::warn!(
            "Actors::new {}: resolved {} actors ({} monsters, {} NPCs)",
            map_name,
            actors.len(),
            n_monsters,
            n_npcs
        );

        Ok(Actors { actors })
    }

    /// Build the actor roster from pre-parsed DDM actor records.
    /// Used when the caller already has the raw actors (e.g., from a DLV file).
    pub fn from_raw_actors(
        lod: &LodManager,
        raw_actors: &[crate::raw::ddm::DdmActor],
        state: Option<&MapStateSnapshot>,
        game_data: &GameData,
    ) -> Result<Self, Box<dyn Error>> {
        let street_npcs = game_data.street_npcs.as_ref();

        let dead_ids: &[u16] = state.map(|s| s.dead_actor_ids.as_slice()).unwrap_or(&[]);

        let mut actors = Vec::with_capacity(raw_actors.len());

        for (idx, raw) in raw_actors.iter().enumerate() {
            if dead_ids.contains(&(idx as u16)) {
                continue;
            }
            // See Actors::new — hp=0 means uninitialized in fresh DDM files, not dead.

            let Some(entry) = crate::raw::monster::resolve_entry(raw.monlist_id, game_data, lod) else {
                log::warn!(
                    "Actor '{}' monlist_id={} has no DSFT sprite — skipping",
                    raw.name,
                    raw.monlist_id
                );
                continue;
            };

            let (portrait_name, profession_id, name) = if raw.npc_id > 0 {
                let portrait = street_npcs.and_then(|t| t.portrait_name(raw.npc_id as i32));
                let prof = street_npcs
                    .and_then(|t| t.get(raw.npc_id as i32))
                    .map(|e| e.profession_id as u8);
                let name = street_npcs
                    .and_then(|t| t.npc_name(raw.npc_id as i32))
                    .unwrap_or(&raw.name)
                    .to_string();
                (portrait, prof, name)
            } else {
                (None, None, raw.name.clone())
            };

            let hp = if raw.hp > 0 {
                raw.hp
            } else {
                let prefix = entry.internal_name.trim_end_matches(|c: char| c.is_ascii_uppercase());
                game_data.monsters.max_hp(prefix, entry.variant).unwrap_or(1)
            };

            actors.push(Actor {
                position: [raw.position[0] as i32, raw.position[1] as i32, raw.position[2] as i32],
                standing_sprite: entry.standing_sprite,
                walking_sprite: entry.walking_sprite,
                attacking_sprite: entry.attacking_sprite,
                dying_sprite: entry.dying_sprite,
                palette_id: entry.palette_id,
                variant: entry.variant,
                name,
                portrait_name,
                profession_id,
                radius: raw.radius,
                height: raw.height,
                move_speed: raw.move_speed,
                hp,
                tether_distance: raw.tether_distance,
                npc_id: raw.npc_id,
                monlist_id: raw.monlist_id,
                is_peasant: entry.is_peasant,
                is_female: entry.is_female,
                to_hit_radius: entry.to_hit_radius,
                sound_ids: raw.sound_ids,
                attributes: raw.attributes,
                range_attack: raw.range_attack,
                monster_id_type: raw.monster_id_type,
                group: raw.group,
                ally: raw.ally,
                spell_buffs: raw.spell_buffs,
                schedules: raw.schedules,
                aggro_range: entry.aggro_range,
                recovery_secs: entry.recovery_secs,
                can_fly: entry.can_fly,
                ai_type: entry.ai_type.clone(),
            });
        }

        Ok(Actors { actors })
    }

    pub fn get_actors(&self) -> &[Actor] {
        &self.actors
    }

    pub fn get_npcs(&self) -> impl Iterator<Item = &Actor> {
        self.actors.iter().filter(|a| a.is_npc())
    }

    pub fn get_monsters(&self) -> impl Iterator<Item = &Actor> {
        self.actors.iter().filter(|a| a.is_monster())
    }
}

#[cfg(test)]
#[path = "actors_tests.rs"]
mod tests;
