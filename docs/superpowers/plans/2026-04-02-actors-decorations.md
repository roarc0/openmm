# Actors and Decorations lod Game Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move DSFT sprite resolution, NPC identity lookup, and decoration metadata out of `openmm/src/game/odm.rs` into a clean game API layer in the `lod` crate.

**Architecture:** Add four new modules under `lod/src/game/`: `npc.rs` (rename of npctable.rs), `monster.rs` (DSFT sprite resolution), `actors.rs` (per-map DDM actor roster), `decorations.rs` (per-map billboard roster). `openmm` iterates pre-resolved entries from `Actors` and `Decorations` instead of querying LOD archives directly during spawn.

**Tech Stack:** Rust 2024, `lod` crate (no Bevy), `openmm` crate (Bevy 0.18). Run tests with `make test`. Build with `make build`.

---

## File Map

**Create:**
- `lod/src/game/npc.rs` — rename of `npctable.rs`; `StreetNpcTable`→`StreetNpcs`, `StreetNpcEntry`→`NpcEntry`
- `lod/src/game/monster.rs` — `MonsterEntry`, `resolve_sprite_group()`, `resolve_entry()`
- `lod/src/game/actors.rs` — `Actor`, `MapStateSnapshot`, `Actors`
- `lod/src/game/decorations.rs` — `DecorationEntry`, `Decorations`

**Modify:**
- `lod/src/game/mod.rs` — add module declarations, update `GameLod` methods
- `lod/src/lib.rs` — re-export new public types if needed
- `openmm/src/game/odm.rs` — remove `resolve_dsft_sprite`, `build_npc_sprite_table`, `NpcSpriteEntry`; simplify NPC + billboard lazy_spawn
- `openmm/src/states/loading.rs` — simplify PreloadSprites billboard section
- `openmm/src/game/events/map_events.rs` — update `npc_table` field type

**Delete (after task 5+6):**
- Dead code in `odm.rs`: `resolve_dsft_sprite`, `build_npc_sprite_table`, `NpcSpriteEntry`
- Dead code in `sprites.rs`: `has_directional_sprites`
- `lod/src/game/npctable.rs` (replaced by `npc.rs`)

---

## Task 1: Rename `npctable.rs` → `npc.rs` and update type names

**Files:**
- Rename: `lod/src/game/npctable.rs` → `lod/src/game/npc.rs`
- Modify: `lod/src/game/mod.rs`
- Modify: `openmm/src/game/events/map_events.rs` (uses `npctable::` path)
- Modify: `openmm/src/game/odm.rs` (uses `npctable::GeneratedNpc`)

- [ ] **Step 1: Copy npctable.rs to npc.rs**

```bash
cp lod/src/game/npctable.rs lod/src/game/npc.rs
```

- [ ] **Step 2: Rename types inside `npc.rs`**

In `lod/src/game/npc.rs`, make these replacements throughout the file:
- `StreetNpcTable` → `StreetNpcs`
- `StreetNpcEntry` → `NpcEntry`

The `pub struct NpcNamePool` and `pub struct GeneratedNpc` names stay the same.

- [ ] **Step 3: Update `lod/src/game/mod.rs`**

Replace:
```rust
pub mod font;
pub mod npctable;
```
With:
```rust
pub mod font;
pub mod npc;
/// Backwards-compatible re-export — remove once openmm is updated.
pub mod npctable {
    pub use super::npc::*;
}
```

Also update the `GameLod` methods at the bottom of `mod.rs`:

```rust
/// Load and parse the global NPC metadata table from `npcdata.txt`.
pub fn npc_table(&self) -> Option<npc::StreetNpcs> {
    let data = self.lod.get_decompressed("icons/npcdata.txt").ok()?;
    let name_pool = self.npc_name_pool();
    npc::StreetNpcs::parse(&data, name_pool.as_ref()).ok()
}

/// Load the NPC name pool from `npcnames.txt`.
pub fn npc_name_pool(&self) -> Option<npc::NpcNamePool> {
    let data = self.lod.get_decompressed("icons/npcnames.txt").ok()?;
    npc::NpcNamePool::parse(&data).ok()
}
```

- [ ] **Step 4: Fix all `npctable::StreetNpcTable` references in openmm**

In `openmm/src/game/events/map_events.rs`, update the field type:
- `npc_table: Option<lod::game::npctable::StreetNpcTable>` → `npc_table: Option<lod::game::npc::StreetNpcs>`

In `openmm/src/game/odm.rs`, update:
- `lod::game::npctable::GeneratedNpc` → `lod::game::npc::GeneratedNpc`

Search for any other `npctable::` usages:
```bash
grep -r "npctable::" openmm/src/
```
Fix each one to use `npc::` or the new type name.

- [ ] **Step 5: Delete `npctable.rs`**

```bash
rm lod/src/game/npctable.rs
```

- [ ] **Step 6: Run tests**

```bash
make test
```

Expected: all tests pass. Fix any compilation errors from missed renames.

---

## Task 2: Add `lod/src/game/monster.rs` with DSFT sprite resolution

Move `resolve_dsft_sprite()` from `openmm/src/game/odm.rs` into the lod crate as `resolve_sprite_group()`. Add `resolve_entry()` as a higher-level wrapper.

**Files:**
- Create: `lod/src/game/monster.rs`
- Modify: `lod/src/game/mod.rs`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `lod/src/game/monster.rs` (create the file with just the test module first):

```rust
//! Monster sprite resolution — maps DSFT group names to sprite file roots.

use crate::LodManager;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, LodManager};

    #[test]
    fn goblin_a_resolves_standing_sprite() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        // Goblin A monlist_id is 0 in MM6 dmonlist.bin
        let entry = resolve_entry(0, &lod);
        assert!(entry.is_some(), "GoblinA (monlist_id=0) should resolve");
        let entry = entry.unwrap();
        assert!(!entry.standing_sprite.is_empty(), "standing sprite should not be empty");
        assert!(!entry.walking_sprite.is_empty(), "walking sprite should not be empty");
    }

    #[test]
    fn resolve_sprite_group_goblin_standing() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        // GoblinA standing group from dmonlist.bin
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
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cargo test -p lod game::monster 2>&1 | head -20
```

Expected: compile error — `resolve_entry` and `resolve_sprite_group` not defined.

- [ ] **Step 3: Implement `monster.rs`**

Write `lod/src/game/monster.rs` with the full implementation. The `resolve_sprite_group` logic is copied verbatim from `openmm/src/game/odm.rs:resolve_dsft_sprite()` — only the function name and signature change:

```rust
//! Monster sprite resolution — maps DSFT group names to sprite file roots.

use crate::LodManager;

/// Resolved sprite data for one monlist entry.
pub struct MonsterEntry {
    pub standing_sprite: String,
    pub walking_sprite: String,
    pub palette_id: u16,
    pub is_peasant: bool,
    pub is_female: bool,
}

/// Resolve a DSFT group name to a sprite file root and palette_id.
/// Searches DSFT frames for the group, strips the frame letter and trailing
/// digits to get the root (e.g. "fmpstaa" → "fmpsta"), verifies the file exists.
/// Falls back to progressively shorter prefixes of the group name itself.
/// Returns (sprite_root, palette_id) or None if not found.
pub fn resolve_sprite_group(group_name: &str, lod: &LodManager) -> Option<(String, i16)> {
    if group_name.is_empty() { return None; }
    let Ok(dsft) = crate::dsft::DSFT::new(lod) else { return None };
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
    if st_group.is_empty() { return None; }
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
    // ... (tests from Step 1)
}
```

- [ ] **Step 4: Declare the module in `lod/src/game/mod.rs`**

Add after the existing module declarations:
```rust
pub mod monster;
```

- [ ] **Step 5: Run tests**

```bash
make test
```

Expected: all new monster tests pass, existing tests unaffected.

---

## Task 3: Add `lod/src/game/actors.rs` — per-map DDM actor roster

**Files:**
- Create: `lod/src/game/actors.rs`
- Modify: `lod/src/game/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `lod/src/game/actors.rs` with only the test module:

```rust
//! Per-map actor roster: pre-resolved DDM actors (NPCs and named monsters).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, LodManager};

    #[test]
    fn actors_loads_oute3() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let actors = Actors::new(&lod, "oute3.odm", None).unwrap();
        assert!(!actors.get_actors().is_empty(), "oute3 should have actors");
    }

    #[test]
    fn get_npcs_all_have_sprites() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let actors = Actors::new(&lod, "oute3.odm", None).unwrap();
        for npc in actors.get_npcs() {
            assert!(npc.is_npc());
            assert!(!npc.standing_sprite.is_empty(),
                "NPC '{}' should have a standing sprite", npc.name);
        }
    }

    #[test]
    fn get_npcs_returns_only_npcs() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let actors = Actors::new(&lod, "oute3.odm", None).unwrap();
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
        let actors = Actors::new(&lod, "oute3.odm", None).unwrap();
        for npc in actors.get_npcs() {
            if let Some(portrait) = &npc.portrait_name {
                assert!(portrait.starts_with("NPC"), "portrait '{}' should start with NPC", portrait);
                assert_eq!(portrait.len(), 6, "portrait '{}' should be 6 chars", portrait);
            }
        }
    }

    #[test]
    fn state_snapshot_filters_dead_actors() {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let actors_all = Actors::new(&lod, "oute3.odm", None).unwrap();
        let all_count = actors_all.get_actors().len();

        // Build a snapshot that kills the first actor if any
        if all_count > 0 {
            // We don't have real actor IDs from DDM yet — just verify None vs Some differs
            let snapshot = MapStateSnapshot { dead_actor_ids: vec![] };
            let actors_with_state = Actors::new(&lod, "oute3.odm", Some(&snapshot)).unwrap();
            assert_eq!(actors_with_state.get_actors().len(), all_count,
                "empty snapshot should not filter anything");
        }
    }
}
```

- [ ] **Step 2: Run to confirm compile failure**

```bash
cargo test -p lod game::actors 2>&1 | head -20
```

Expected: compile error — types not defined.

- [ ] **Step 3: Implement `actors.rs`**

```rust
//! Per-map actor roster: pre-resolved DDM actors (NPCs and named monsters).
//!
//! `Actors::new(lod, map_name, state)` loads and resolves all DDM actors for the map.
//! Each `Actor` carries pre-resolved sprite roots and NPC identity so openmm needs
//! no direct LOD queries during spawn.

use std::error::Error;
use crate::LodManager;
use super::monster;
use super::npc::StreetNpcs;

/// Stub for future map-state persistence. When populated, actors whose id
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
    pub variant: u8,
    pub name: String,
    pub portrait_name: Option<String>,  // NPC only, e.g. "NPC042"
    pub profession_id: Option<u8>,      // NPC only
    pub radius: u16,
    pub height: u16,
    pub move_speed: u16,
    pub hp: i16,
    pub tether_distance: u16,
    npc_id: i16,     // >0 = NPC dialogue id; 0 = monster
    pub monlist_id: u8,
    pub is_peasant: bool,
    pub is_female: bool,
}

impl Actor {
    pub fn is_npc(&self) -> bool     { self.npc_id > 0 }
    pub fn is_monster(&self) -> bool  { self.npc_id == 0 }
    pub fn npc_id(&self) -> i16       { self.npc_id }
}

/// Per-map roster of pre-resolved DDM actors. Created once per map load.
pub struct Actors {
    actors: Vec<Actor>,
}

impl Actors {
    /// Load and fully resolve all actors for the given map.
    /// Internally loads: MonsterList, DSFT, StreetNpcs, NpcNamePool, Ddm.
    /// Actors with ids in `state.dead_actor_ids` are excluded.
    pub fn new(
        lod: &LodManager,
        map_name: &str,
        state: Option<&MapStateSnapshot>,
    ) -> Result<Self, Box<dyn Error>> {
        let ddm = crate::ddm::Ddm::new(lod, map_name)?;
        let street_npcs = lod.game().npc_table();
        let name_pool = lod.game().npc_name_pool();

        let dead_ids: &[u16] = state
            .map(|s| s.dead_actor_ids.as_slice())
            .unwrap_or(&[]);

        let mut actors = Vec::with_capacity(ddm.actors.len());

        for (idx, raw) in ddm.actors.iter().enumerate() {
            // Filter out dead actors (stub — no real actor IDs in DDM yet, using index)
            if dead_ids.contains(&(idx as u16)) {
                continue;
            }

            // Skip actors with no health or out-of-bounds positions
            if raw.hp <= 0 { continue; }
            if raw.position[0].abs() > 20000 || raw.position[1].abs() > 20000 { continue; }

            let Some(entry) = monster::resolve_entry(raw.monlist_id, lod) else {
                log::warn!("Actor '{}' monlist_id={} has no DSFT sprite — skipping",
                    raw.name, raw.monlist_id);
                continue;
            };

            // Compute variant from palette_id offset (same logic as odm.rs)
            // Base palette = minimum palette_id among entries sharing the same sprite root
            // Kept as 0 here — openmm computes the variant from palette offset at spawn time
            let variant = 0u8; // openmm fills this in during spawn

            let (portrait_name, profession_id, name) = if raw.npc_id > 0 {
                // Named NPC: look up from npcdata.txt
                let portrait = street_npcs.as_ref()
                    .and_then(|t| t.portrait_name(raw.npc_id as i32));
                let prof = street_npcs.as_ref()
                    .and_then(|t| t.get(raw.npc_id as i32))
                    .map(|e| e.profession_id as u8);
                let name = street_npcs.as_ref()
                    .and_then(|t| t.npc_name(raw.npc_id as i32))
                    .unwrap_or(&raw.name)
                    .to_string();
                (portrait, prof, name)
            } else if entry.is_peasant {
                // Peasant: identity assigned by openmm (needs spawn index for determinism)
                (None, None, raw.name.clone())
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
                variant,
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
    // ... tests from Step 1 above
}
```

**Confirmed `DdmActor` field names** (from `lod/src/ddm.rs`): `name`, `monlist_id: u8`, `npc_id: i16`, `hp: i16`, `position: [i16; 3]`, `radius: u16`, `height: u16`, `move_speed: u16`, `tether_distance: u16`. All accesses in the code above use these exact names.

- [ ] **Step 4: Check `DdmActor` field names**

```bash
grep -n "pub " lod/src/ddm.rs | head -40
```

Adjust the field accesses in `actors.rs` to match exactly.

- [ ] **Step 5: Declare module in `lod/src/game/mod.rs`**

Add:
```rust
pub mod actors;
```

- [ ] **Step 6: Run tests**

```bash
make test
```

Expected: all new actors tests pass.

---

## Task 4: Add `lod/src/game/decorations.rs` — per-map decoration roster

Moves `has_directional_sprites()` from `openmm/src/game/entities/sprites.rs` into the lod crate. Pre-resolves DSFT scale for each decoration.

**Files:**
- Create: `lod/src/game/decorations.rs`
- Modify: `lod/src/game/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `lod/src/game/decorations.rs` with tests only:

```rust
//! Per-map decoration roster from ODM billboards.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, LodManager, odm::Odm};

    fn load_oute3_decorations() -> Decorations {
        let lod = LodManager::new(get_lod_path()).unwrap();
        let raw = lod.try_get_bytes("games/oute3.odm").unwrap();
        let odm = Odm::parse(&raw).unwrap();
        Decorations::new(&lod, &odm.billboards).unwrap()
    }

    #[test]
    fn decorations_loads_oute3() {
        let dec = load_oute3_decorations();
        assert!(!dec.entries.is_empty(), "oute3 should have decorations");
    }

    #[test]
    fn all_entries_have_sprite_name() {
        let dec = load_oute3_decorations();
        for entry in dec.iter() {
            assert!(!entry.sprite_name.is_empty(),
                "every decoration should have a sprite name");
        }
    }

    #[test]
    fn scale_is_positive() {
        let dec = load_oute3_decorations();
        for entry in dec.iter() {
            assert!(entry.scale > 0.0, "scale should be positive, got {}", entry.scale);
        }
    }

    #[test]
    fn non_directional_entries_have_dimensions() {
        let dec = load_oute3_decorations();
        let non_dir: Vec<_> = dec.iter().filter(|e| !e.is_directional).collect();
        assert!(!non_dir.is_empty(), "oute3 should have non-directional decorations");
        for entry in non_dir {
            assert!(entry.width > 0.0, "non-directional '{}' width should be >0", entry.sprite_name);
            assert!(entry.height > 0.0, "non-directional '{}' height should be >0", entry.sprite_name);
        }
    }
}
```

- [ ] **Step 2: Run to confirm compile failure**

```bash
cargo test -p lod game::decorations 2>&1 | head -20
```

Expected: compile error.

- [ ] **Step 3: Implement `decorations.rs`**

```rust
//! Per-map decoration roster from ODM billboards.
//!
//! `Decorations::new(lod, odm_billboards)` resolves all renderable decorations:
//! filters out markers/invisible entries, detects directional sprites, and
//! pre-extracts DSFT scale factors so openmm needs no BillboardManager queries.

use std::error::Error;
use crate::{LodManager, billboard::{Billboard, BillboardManager}};

/// A fully resolved decoration ready for spawning.
pub struct DecorationEntry {
    /// MM6 coordinates.
    pub position: [i32; 3],
    /// Resolved sprite file root (e.g. "tree1", "shp" for directional).
    pub sprite_name: String,
    /// True if the sprite has 0..4 direction variants ({root}0..{root}4).
    pub is_directional: bool,
    /// DSFT scale factor. For non-directional sprites, width/height already
    /// have this applied. For directional sprites, apply to pixel dims after load.
    pub scale: f32,
    /// World-unit width (scale already applied). 0.0 for directional decorations.
    pub width: f32,
    /// World-unit height (scale already applied). 0.0 for directional decorations.
    pub height: f32,
    pub sound_id: u16,
    pub event_id: i16,
    /// Original billboard index in the ODM billboard list (for interaction).
    pub billboard_index: usize,
    /// Pre-converted facing angle in radians.
    pub facing_yaw: f32,
}

/// Per-map roster of pre-resolved decorations.
pub struct Decorations {
    pub entries: Vec<DecorationEntry>,
}

impl Decorations {
    /// Resolve all renderable decorations from the ODM billboard list.
    /// Takes pre-parsed billboards (they are embedded in the ODM file which
    /// openmm already parses for terrain/models — avoids re-parsing).
    /// Internally loads BillboardManager (DDecList + DSFT).
    pub fn new(lod: &LodManager, odm_billboards: &[Billboard]) -> Result<Self, Box<dyn Error>> {
        let mgr = BillboardManager::new(lod)?;
        let mut entries = Vec::new();

        for (bb_i, bb) in odm_billboards.iter().enumerate() {
            if bb.data.is_invisible() { continue; }

            // Skip teleport markers and no-draw decorations
            let is_skip = mgr.get_declist_item(bb.data.declist_id)
                .map(|item| item.is_marker() || item.is_no_draw())
                .unwrap_or(false);
            let name_lower = bb.declist_name.to_lowercase();
            if is_skip || name_lower.contains("start") { continue; }

            let sound_id = mgr.get_declist_item(bb.data.declist_id)
                .map(|item| item.sound_id)
                .unwrap_or(0);

            let facing_yaw = bb.data.direction_degrees as f32 * std::f32::consts::PI / 1024.0;

            // Detect directional sprites: {root}0 and {root}1 must exist in LOD
            let directional_root = find_directional_root(&bb.declist_name, lod);
            let is_directional = directional_root.is_some();
            let sprite_name = directional_root.unwrap_or_else(|| bb.declist_name.to_lowercase());

            // Get DSFT scale
            let scale = mgr.get_declist_item(bb.data.declist_id)
                .and_then(|item| mgr.get_dsft_scale(item))
                .unwrap_or(1.0);

            // Pre-compute world dimensions for non-directional sprites
            let (width, height) = if !is_directional {
                mgr.get(lod, &bb.declist_name, bb.data.declist_id)
                    .map(|s| s.dimensions())
                    .unwrap_or((0.0, 0.0))
            } else {
                (0.0, 0.0)  // computed by openmm after loading sprite frames
            };

            entries.push(DecorationEntry {
                position: [
                    bb.data.position[0] as i32,
                    bb.data.position[1] as i32,
                    bb.data.position[2] as i32,
                ],
                sprite_name,
                is_directional,
                scale,
                width,
                height,
                sound_id,
                event_id: bb.data.event,
                billboard_index: bb_i,
                facing_yaw,
            });
        }

        Ok(Decorations { entries })
    }

    pub fn iter(&self) -> impl Iterator<Item = &DecorationEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

/// Check if a decoration has directional sprites ({root}0 and {root}1 exist).
/// Equivalent to `has_directional_sprites()` in openmm — moved here as it is
/// pure data-layer LOD file existence logic.
fn find_directional_root(name: &str, lod: &LodManager) -> Option<String> {
    let root = name.trim_end_matches(|c: char| c.is_ascii_digit());
    let mut try_root = root;
    while try_root.len() >= 3 {
        let lower = try_root.to_lowercase();
        let test0 = format!("sprites/{}0", lower);
        let test1 = format!("sprites/{}1", lower);
        if lod.try_get_bytes(&test0).is_ok() && lod.try_get_bytes(&test1).is_ok() {
            return Some(lower);
        }
        try_root = &try_root[..try_root.len() - 1];
    }
    None
}

#[cfg(test)]
mod tests {
    // ... tests from Step 1 above
}
```

**Note:** Check whether `BillboardManager::get_dsft_scale` takes a `DDecListItem` reference or the declist_id. Read `lod/src/billboard.rs` to confirm the exact signature and adjust the call above.

- [ ] **Step 4: Check `Billboard` and `BillboardManager` signatures**

```bash
grep -n "pub fn\|pub struct\|pub " lod/src/billboard.rs | head -40
```

Adjust field names (`bb.data.position`, `bb.data.declist_id`, `bb.data.direction_degrees`, `bb.data.event`, `bb.data.is_invisible()`) to match the actual struct. The current code in `loading.rs` uses `bb.data.*` so check whether that matches.

- [ ] **Step 5: Declare module in `lod/src/game/mod.rs`**

Add:
```rust
pub mod decorations;
```

- [ ] **Step 6: Run tests**

```bash
make test
```

Expected: all new decoration tests pass.

---

## Task 5: Update `openmm` — NPC spawn to use `Actors`

Replace `build_npc_sprite_table()` + the NPC spawn section in `lazy_spawn()` with `Actors`.

**Files:**
- Modify: `openmm/src/states/loading.rs` — NPC preload section
- Modify: `openmm/src/game/odm.rs` — NPC spawn section of `lazy_spawn()`

- [ ] **Step 1: Update `PendingSpawns` in `odm.rs` to hold `Actors`**

Find the `PendingSpawns` struct in `openmm/src/game/odm.rs`. It currently has `npc_sprite_table: HashMap<u8, NpcSpriteEntry>`. Add an `actors` field:

```rust
// In PendingSpawns struct, add:
pub actors: Option<lod::game::actors::Actors>,
```

And in the place where `PendingSpawns` is constructed (the call to `build_npc_sprite_table`), replace with:

```rust
actors: lod::game::actors::Actors::new(
    game_assets.lod_manager(),
    &map_name.to_string(),
    None,
).ok(),
```

- [ ] **Step 2: Update the preload queue in `loading.rs`**

In `LoadingStep::PreloadSprites`, the NPC sprite preloading section (around line 902-915) currently does:

```rust
let npc_table = crate::game::odm::build_npc_sprite_table(&game_assets);
if let Some(actors) = &progress.actors {
    for a in actors {
        if a.hp <= 0 { continue; }
        if let Some(entry) = npc_table.get(&a.monlist_id) {
            for root in [entry.standing_root.clone(), entry.walking_root.clone()] {
                if seen.insert(root.clone()) {
                    sprite_roots.push((root, 0));
                }
            }
        }
    }
}
```

Replace with (using `Actors` from the map name):

```rust
if let Ok(actors) = lod::game::actors::Actors::new(
    game_assets.lod_manager(),
    &load_request.map_name.to_string(),
    None,
) {
    for actor in actors.get_actors() {
        for root in [actor.standing_sprite.clone(), actor.walking_sprite.clone()] {
            if seen.insert(root.clone()) {
                sprite_roots.push((root, 0));
            }
        }
    }
}
```

- [ ] **Step 3: Update the NPC lazy_spawn section in `odm.rs`**

Find the NPC spawn section in `lazy_spawn()` (roughly lines 722-817). It currently does:

```rust
let Some(entry) = p.npc_sprite_table.get(&a.monlist_id) else { ... };
let base_pal = p.npc_sprite_table.values()...min()...;
let variant = (entry.palette_id - base_pal + 1).min(3) as u8;
```

Update to use `Actors`. The `PendingSpawns` now holds `actors: Option<Actors>`. The NPC spawn loop should iterate over `p.actors.as_ref().map(|a| a.get_npcs())` instead of `prepared.actors`.

The `p.actor_order` sorting should now sort over `actors.get_actors()` indices. Find where `actor_order` is built and update it to use `p.actors` if available.

The key spawn simplification — replace the `entry.is_peasant` block. The actor already has `is_peasant` and `is_female` flags. Portrait and name are already resolved in `actor.portrait_name` and `actor.name` for non-peasants. For peasants, identity assignment (using `map_events`) stays as-is but reads from `actor.is_peasant` / `actor.is_female` instead of `entry.is_peasant` / `entry.is_female`.

The `dsft_scale` lookup for NPC sprites (line 749-751):
```rust
let dsft_scale = bb_mgr.as_ref()
    .map(|mgr| mgr.dsft_scale_for_group(&entry.standing_root))
    .unwrap_or(1.0);
```
This uses the standing sprite root as a DSFT group name. Since `Actors` pre-resolves the sprite root (file name, not group name), this lookup may return 1.0 for most actors — leave it as-is for now; it's a separate cleanup.

- [ ] **Step 4: Build and check for compile errors**

```bash
make build 2>&1 | head -60
```

Fix any type mismatches. The main change is `entry.standing_root` → `actor.standing_sprite`, `entry.walking_root` → `actor.walking_sprite`, `entry.is_peasant` → `actor.is_peasant`, `entry.is_female` → `actor.is_female`, `a.npc_id` → `actor.npc_id()`.

- [ ] **Step 5: Run the game to verify NPCs spawn correctly**

```bash
make run map=oute3 2>&1 | grep -E "NPC|npc|actor|error|warn" | head -30
```

Walk around New Sorpigal and verify NPCs are visible with correct portraits.

---

## Task 6: Update `openmm` — billboard spawn to use `Decorations`

Replace billboard resolution in `loading.rs` and `odm.rs` with `Decorations`.

**Files:**
- Modify: `openmm/src/states/loading.rs` — PreloadSprites billboard section
- Modify: `openmm/src/game/odm.rs` — billboard lazy_spawn section

- [ ] **Step 1: Add `Decorations` to `PreparedWorld`**

In `openmm/src/game/odm.rs`, find where `PreparedWorld` is defined and add a `decorations` field:

```rust
pub decorations: Option<lod::game::decorations::Decorations>,
```

In the loading step where `PreparedWorld` is constructed (or in `spawn_world`), populate it:

```rust
decorations: odm.as_ref().map(|o| {
    lod::game::decorations::Decorations::new(game_assets.lod_manager(), &o.billboards).ok()
}).flatten(),
```

- [ ] **Step 2: Simplify the billboard preload in `loading.rs`**

The billboard preload section in `PreloadSprites` (lines 975-1010) currently loads `BillboardManager`, iterates `PreparedBillboard`s, and creates materials/meshes. Replace with:

```rust
// Preload billboard/decoration sprites in batches
{
    let sprites_done = progress.preload_queue.as_ref().unwrap().sprite_idx
        >= progress.preload_queue.as_ref().unwrap().sprite_roots.len();
    if sprites_done {
        let mut bb_cache = progress.billboard_cache.take().unwrap_or_default();
        let billboards = progress.billboards.take();
        if let Some(ref bbs) = billboards {
            let queue = progress.preload_queue.as_mut().unwrap();
            while queue.billboard_idx < bbs.len() {
                if frame_start.elapsed().as_secs_f32() * 1000.0 > PRELOAD_BUDGET_MS { break; }
                let bb = &bbs[queue.billboard_idx];
                queue.billboard_idx += 1;
                if bb_cache.contains_key(&bb.declist_name) { continue; }
                // Only preload non-directional sprites (directional loaded during spawn)
                if let Some(sprite) = lod::billboard::BillboardManager::new(game_assets.lod_manager())
                    .ok()
                    .and_then(|mgr| mgr.get(game_assets.lod_manager(), &bb.declist_name, bb.declist_id))
                {
                    let (w, h) = sprite.dimensions();
                    let bevy_img = crate::assets::dynamic_to_bevy_image(sprite.image);
                    let tex = images.add(bevy_img);
                    let m = materials.add(StandardMaterial {
                        base_color_texture: Some(tex),
                        alpha_mode: AlphaMode::Mask(0.5),
                        unlit: true,
                        cull_mode: None, double_sided: true,
                        perceptual_roughness: 1.0, reflectance: 0.0, ..default()
                    });
                    let q = meshes.add(Rectangle::new(w, h));
                    bb_cache.insert(bb.declist_name.clone(), (m, q, h));
                }
            }
        }
        progress.billboards = billboards;
        progress.billboard_cache = Some(bb_cache);
    }
}
```

**Note:** The billboard preload is a larger refactor — for this task, focus on making the billboard spawn in `odm.rs` use `DecorationEntry` data. The preload can remain mostly as-is; the key improvement is removing `has_directional_sprites` from odm.rs.

- [ ] **Step 3: Update billboard `lazy_spawn` in `odm.rs`**

In `lazy_spawn()`, the billboard section (lines 611-720) uses `sprites::has_directional_sprites()` to check directional sprites and queries `bb_mgr` for scale. Replace with:

```rust
// Iterate prepared.decorations instead of prepared.billboards
// DecorationEntry.is_directional replaces the has_directional_sprites() call
// DecorationEntry.scale replaces the bb_mgr.get_dsft_scale() call

let bb_mgr = None::<lod::billboard::BillboardManager>; // no longer needed

while bb_idx < bb_len && spawned < batch_max && ... {
    let idx = p.billboard_order[bb_idx];
    bb_idx += 1;
    p.idx += 1;

    let bb = &prepared.billboards[idx]; // keep using PreparedBillboard for position/event_id

    // Get the resolved decoration entry by billboard_index.
    // Build a HashMap once before the loop (O(n) not O(n²)):
    //   let dec_by_idx: HashMap<usize, &DecorationEntry> = prepared.decorations
    //       .as_ref().map(|d| d.iter().map(|e| (e.billboard_index, e)).collect())
    //       .unwrap_or_default();
    let Some(dec) = dec_by_idx.get(&bb.billboard_index) else {
        continue;
    };

    if dec.is_directional {
        let (dirs, px_w, px_h) = sprites::load_decoration_directions(
            &dec.sprite_name, game_assets.lod_manager(),
            &mut images, &mut materials, &mut Some(&mut p.sprite_cache));
        if px_w > 0.0 {
            let sw = px_w * dec.scale;
            let sh = px_h * dec.scale;
            // ... rest of directional spawn (unchanged)
        }
    } else {
        // Use pre-computed width/height from DecorationEntry
        let (mat, quad, h) = if let Some((m, q, h)) = p.billboard_cache.get(&bb.declist_name) {
            (m.clone(), q.clone(), *h)
        } else {
            // fallback: load on demand
            // ... existing fallback code
        };
        // ... rest of non-directional spawn (unchanged)
    }
}
```

- [ ] **Step 4: Build and check**

```bash
make build 2>&1 | head -60
```

Fix compile errors.

- [ ] **Step 5: Run the game and verify decorations**

```bash
make run map=oute3 2>&1 | grep -E "decoration|billboard|error|warn" | head -30
```

Verify trees, rocks, fountains render correctly. Walk to a ship (directional decoration) and verify it shows the correct direction.

---

## Task 7: Delete dead code

Remove code from `openmm` that has been replaced.

**Files:**
- Modify: `openmm/src/game/odm.rs`
- Modify: `openmm/src/game/entities/sprites.rs`
- Remove: compat shim from `lod/src/game/mod.rs`

- [ ] **Step 1: Delete `resolve_dsft_sprite` from `odm.rs`**

Remove the function `resolve_dsft_sprite()` (lines 509-551). Verify no call sites remain:

```bash
grep -rn "resolve_dsft_sprite" openmm/src/
```

Expected: no matches.

- [ ] **Step 2: Delete `build_npc_sprite_table` and `NpcSpriteEntry` from `odm.rs`**

Remove:
- `pub struct NpcSpriteEntry { ... }` (lines ~459-467)
- `pub fn build_npc_sprite_table(...)` (lines ~473-507)

Verify no remaining call sites:
```bash
grep -rn "build_npc_sprite_table\|NpcSpriteEntry" openmm/src/
```

Expected: no matches.

- [ ] **Step 3: Delete `has_directional_sprites` from `sprites.rs`**

Remove the `pub fn has_directional_sprites(...)` function (lines ~462-477 in sprites.rs).

Verify no remaining call sites:
```bash
grep -rn "has_directional_sprites" openmm/src/
```

Expected: no matches.

- [ ] **Step 4: Remove backwards-compat `npctable` shim from `mod.rs`**

In `lod/src/game/mod.rs`, remove the compat re-export added in Task 1:
```rust
/// Backwards-compatible re-export — remove once openmm is updated.
pub mod npctable {
    pub use super::npc::*;
}
```

- [ ] **Step 5: Final build and tests**

```bash
make build
make test
```

Expected: clean build, all tests pass.

- [ ] **Step 6: Run and smoke test**

```bash
make run map=oute3
```

Walk around New Sorpigal. Verify:
- Trees, rocks, fountains visible
- Ships show correct direction
- NPCs visible with names and correct portraits
- No error/warn spam in terminal
