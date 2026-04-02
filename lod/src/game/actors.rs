//! Per-map actor roster: pre-resolved DDM actors (NPCs and named monsters).
//!
//! `Actors::new(lod, map_name, state)` loads and resolves all DDM actors for the map.
//! Each `Actor` carries pre-resolved sprite roots and NPC identity so openmm needs
//! no direct LOD queries during spawn.

use std::error::Error;
use crate::LodManager;
use super::monster;
use super::global::GameData;

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
    pub palette_id: u16,
    /// Palette variant: 1=A (base), 2=B, 3=C. Pre-computed from palette_id offset within
    /// actors sharing the same standing_sprite root. Never 0 after construction.
    pub variant: u8,
    pub name: String,
    pub portrait_name: Option<String>,  // NPC only, e.g. "NPC042"
    pub profession_id: Option<u8>,      // NPC only
    pub radius: u16,
    pub height: u16,
    pub move_speed: u16,
    pub hp: i16,
    pub tether_distance: u16,
    npc_id: i16,
    pub monlist_id: u8,
    pub is_peasant: bool,
    pub is_female: bool,
}

impl Actor {
    pub fn is_npc(&self) -> bool      { self.npc_id > 0 }
    pub fn is_monster(&self) -> bool  { self.npc_id == 0 }
    pub fn npc_id(&self) -> i16       { self.npc_id }
}

/// Compute palette variant for each actor based on the minimum palette_id of actors
/// sharing the same standing_sprite root.
fn compute_variants(actors: &mut Vec<Actor>) {
    use std::collections::HashMap;

    let mut base_pals: HashMap<&str, u16> = HashMap::new();
    for a in actors.iter() {
        let e = base_pals.entry(&a.standing_sprite).or_insert(a.palette_id);
        if a.palette_id < *e { *e = a.palette_id; }
    }

    // base_pals keys borrow from actors, so we need to collect first
    let base_pals: HashMap<String, u16> = base_pals
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();

    for actor in actors.iter_mut() {
        let base = base_pals[&actor.standing_sprite];
        actor.variant = ((actor.palette_id - base + 1) as u8).min(3);
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
        let ddm = crate::ddm::Ddm::new(lod, map_name)?;
        let street_npcs = game_data.street_npcs.as_ref();

        let dead_ids: &[u16] = state
            .map(|s| s.dead_actor_ids.as_slice())
            .unwrap_or(&[]);

        let mut actors = Vec::with_capacity(ddm.actors.len());

        for (idx, raw) in ddm.actors.iter().enumerate() {
            if dead_ids.contains(&(idx as u16)) {
                continue;
            }
            if raw.hp <= 0 { continue; }
            if raw.position[0].abs() > 20000 || raw.position[1].abs() > 20000 { continue; }

            let Some(entry) = monster::resolve_entry(raw.monlist_id, game_data, lod) else {
                log::warn!("Actor '{}' monlist_id={} has no DSFT sprite — skipping",
                    raw.name, raw.monlist_id);
                continue;
            };

            let (portrait_name, profession_id, name) = if raw.npc_id > 0 {
                let portrait = street_npcs
                    .and_then(|t| t.portrait_name(raw.npc_id as i32));
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

            actors.push(Actor {
                position: [
                    raw.position[0] as i32,
                    raw.position[1] as i32,
                    raw.position[2] as i32,
                ],
                standing_sprite: entry.standing_sprite,
                walking_sprite: entry.walking_sprite,
                palette_id: entry.palette_id,
                variant: 1, // overwritten by compute_variants() before return
                name,
                portrait_name,
                profession_id,
                radius: raw.radius,
                height: raw.height,
                move_speed: raw.move_speed,
                hp: raw.hp,
                tether_distance: raw.tether_distance,
                npc_id: raw.npc_id,
                monlist_id: raw.monlist_id,
                is_peasant: entry.is_peasant,
                is_female: entry.is_female,
            });
        }

        compute_variants(&mut actors);
        Ok(Actors { actors })
    }

    /// Build the actor roster from pre-parsed DDM actor records.
    /// Used when the caller already has the raw actors (e.g., from a DLV file).
    pub fn from_raw_actors(
        lod: &LodManager,
        raw_actors: &[crate::ddm::DdmActor],
        state: Option<&MapStateSnapshot>,
        game_data: &GameData,
    ) -> Result<Self, Box<dyn Error>> {
        let street_npcs = game_data.street_npcs.as_ref();

        let dead_ids: &[u16] = state
            .map(|s| s.dead_actor_ids.as_slice())
            .unwrap_or(&[]);

        let mut actors = Vec::with_capacity(raw_actors.len());

        for (idx, raw) in raw_actors.iter().enumerate() {
            if dead_ids.contains(&(idx as u16)) {
                continue;
            }
            if raw.hp <= 0 { continue; }
            if raw.position[0].abs() > 20000 || raw.position[1].abs() > 20000 { continue; }

            let Some(entry) = monster::resolve_entry(raw.monlist_id, game_data, lod) else {
                log::warn!("Actor '{}' monlist_id={} has no DSFT sprite — skipping",
                    raw.name, raw.monlist_id);
                continue;
            };

            let (portrait_name, profession_id, name) = if raw.npc_id > 0 {
                let portrait = street_npcs
                    .and_then(|t| t.portrait_name(raw.npc_id as i32));
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

            actors.push(Actor {
                position: [
                    raw.position[0] as i32,
                    raw.position[1] as i32,
                    raw.position[2] as i32,
                ],
                standing_sprite: entry.standing_sprite,
                walking_sprite: entry.walking_sprite,
                palette_id: entry.palette_id,
                variant: 1, // overwritten by compute_variants() before return
                name,
                portrait_name,
                profession_id,
                radius: raw.radius,
                height: raw.height,
                move_speed: raw.move_speed,
                hp: raw.hp,
                tether_distance: raw.tether_distance,
                npc_id: raw.npc_id,
                monlist_id: raw.monlist_id,
                is_peasant: entry.is_peasant,
                is_female: entry.is_female,
            });
        }

        compute_variants(&mut actors);
        Ok(Actors { actors })
    }

    pub fn get_actors(&self) -> &[Actor] { &self.actors }

    pub fn get_npcs(&self) -> impl Iterator<Item = &Actor> {
        self.actors.iter().filter(|a| a.is_npc())
    }

    pub fn get_monsters(&self) -> impl Iterator<Item = &Actor> {
        self.actors.iter().filter(|a| a.is_monster())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, LodManager};
    use super::super::global::GameData;

    fn game_data(lod: &LodManager) -> GameData {
        GameData::new(lod).expect("GameData::new failed")
    }

    #[test]
    fn actors_loads_oute3() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let gd = game_data(&lod);
        let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
        assert!(!actors.get_actors().is_empty(), "oute3 should have actors");
    }

    #[test]
    fn get_npcs_all_have_sprites() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let gd = game_data(&lod);
        let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
        for npc in actors.get_npcs() {
            assert!(npc.is_npc());
            assert!(!npc.standing_sprite.is_empty(),
                "NPC '{}' should have a standing sprite", npc.name);
        }
    }

    #[test]
    fn get_npcs_returns_only_npcs() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let gd = game_data(&lod);
        let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
        for npc in actors.get_npcs() {
            assert!(npc.is_npc(), "get_npcs() should not return monsters");
        }
        for monster in actors.get_monsters() {
            assert!(monster.is_monster(), "get_monsters() should not return NPCs");
        }
    }

    #[test]
    fn npc_portrait_name_format() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let gd = game_data(&lod);
        let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
        for npc in actors.get_npcs() {
            if let Some(portrait) = &npc.portrait_name {
                assert!(portrait.starts_with("NPC"), "portrait '{}' should start with NPC", portrait);
                assert_eq!(portrait.len(), 6, "portrait '{}' should be 6 chars", portrait);
            }
        }
    }

    #[test]
    fn state_snapshot_empty_filters_nothing() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let gd = game_data(&lod);
        let actors_all = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
        let all_count = actors_all.get_actors().len();
        let snapshot = MapStateSnapshot { dead_actor_ids: vec![] };
        let actors_with_state = Actors::new(&lod, "oute3.odm", Some(&snapshot), &gd).unwrap();
        assert_eq!(actors_with_state.get_actors().len(), all_count,
            "empty snapshot should not filter anything");
    }

    #[test]
    fn variant_is_precomputed() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let gd = game_data(&lod);
        let actors = Actors::new(&lod, "oute3.odm", None, &gd).unwrap();
        // Every actor should have variant 1, 2, or 3 — never 0 (unless there is truly only one palette).
        // The assertion checks that ALL actors have variant >= 1.
        // Actors with a unique standing_sprite will always be variant 1.
        let all_variants_nonzero = actors.get_actors().iter().all(|a| a.variant >= 1);
        assert!(all_variants_nonzero, "all actors should have variant >= 1 after pre-computation");
    }
}
