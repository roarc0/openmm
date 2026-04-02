# Design: `lod/src/game` Actor and Decoration Tables

**Date:** 2026-04-02  
**Status:** Approved

## Goal

Move data-layer complexity (DSFT sprite resolution, NPC identity lookup, decoration metadata extraction) out of `openmm/src/game/odm.rs` into the `lod` crate. `openmm` becomes a thin consumer that iterates pre-resolved entries and spawns Bevy entities — no direct LOD queries, no DSFT frame iteration, no sprite name stripping.

---

## Module Structure

```
lod/src/game/
├── mod.rs          — GameLod; re-exports Actors, Decorations; adds constructor helpers
├── actors.rs       — Actors, Actor struct + classification methods
├── npc.rs          — StreetNpcs, NpcNamePool (rename of npctable.rs)
├── monster.rs      — MonsterEntry, DSFT sprite resolution (moved from openmm)
└── decorations.rs  — Decorations, DecorationEntry, directional detection (moved from openmm)
```

`npctable.rs` is renamed to `npc.rs`; `StreetNpcTable` is renamed to `StreetNpcs`.

---

## `actors.rs` — Per-Map Actor Roster

### `Actor` struct

```rust
pub struct Actor {
    pub position: [i32; 3],
    pub standing_sprite: String,   // DSFT-resolved sprite file root
    pub walking_sprite: String,    // DSFT-resolved sprite file root
    pub palette_id: u16,
    pub variant: u8,               // 1=A, 2=B, 3=C
    pub name: String,              // NPC: resolved name; monster: monlist internal name
    pub portrait_name: Option<String>, // NPC only, e.g. "NPC042"
    pub profession_id: Option<u8>, // NPC only
    pub radius: u16,
    pub height: u16,
    pub move_speed: u16,
    npc_id: i16,                   // >0 = NPC dialogue index; 0 = monster
    monlist_id: u8,
}

impl Actor {
    pub fn is_npc(&self) -> bool
    pub fn is_monster(&self) -> bool
    pub fn is_peasant(&self) -> bool    // from monlist internal_name prefix
    pub fn is_aggressive(&self) -> bool // from actor_attributes flags
}
```

### `Actors` struct

```rust
pub struct MapStateSnapshot {
    pub dead_actor_ids: Vec<u16>,  // stub — filters out dead actors on reload
}

pub struct Actors {
    actors: Vec<Actor>,
}

impl Actors {
    /// Load and fully resolve all actors for a map.
    /// Internally loads: MonsterList, DSFT, StreetNpcs, NpcNamePool, Ddm.
    pub fn new(
        lod: &LodManager,
        map_name: &str,
        state: Option<&MapStateSnapshot>,
    ) -> Result<Self, Box<dyn Error>>

    pub fn get_actors(&self)   -> &[Actor]
    pub fn get_npcs(&self)     -> impl Iterator<Item = &Actor>
    pub fn get_monsters(&self) -> impl Iterator<Item = &Actor>
}
```

`get_npcs()` and `get_monsters()` are filtered views over the same `Vec<Actor>` — no duplication.

### Construction logic (inside `Actors::new`)

1. Load `MonsterList`, `Ddm`, `StreetNpcs`, `NpcNamePool`
2. For each `DdmActor`:
   - If `dead_actor_ids` contains its id → skip
   - Resolve sprites via `monster::resolve_entry(monlist_id, lod)` → `MonsterEntry`
   - If `npc_id > 0`: resolve name + portrait from `StreetNpcs`; assign peasant identity via `NpcNamePool` if needed
   - Build `Actor`

---

## `monster.rs` — Monster Sprite Resolution

Houses the DSFT resolution logic currently in `openmm/src/game/odm.rs:resolve_dsft_sprite()`.

```rust
pub struct MonsterEntry {
    pub standing_sprite: String,
    pub walking_sprite: String,
    pub palette_id: u16,
    pub is_peasant: bool,
    pub is_female: bool,
}

/// Resolve DSFT sprite file root + palette for a monlist entry.
/// Moves resolve_dsft_sprite() from openmm verbatim.
pub(super) fn resolve_entry(monlist_id: u8, lod: &LodManager) -> Option<MonsterEntry>
```

`resolve_dsft_sprite()` disappears from `openmm` entirely.

---

## `npc.rs` — NPC Identity (rename of `npctable.rs`)

Renames only — no logic changes:

| Old name | New name |
|---|---|
| `npctable.rs` | `npc.rs` |
| `StreetNpcTable` | `StreetNpcs` |
| `StreetNpcEntry` | `NpcEntry` |

`GameLod::npc_table()` is renamed to `GameLod::street_npcs()` for consistency. All existing tests updated to match.

---

## `decorations.rs` — Per-Map Decoration Roster

### `DecorationEntry` struct

```rust
pub struct DecorationEntry {
    pub position: [i32; 3],
    pub sprite_name: String,       // resolved sprite file root
    pub is_directional: bool,      // true if sprite has 0..7 direction variants
    pub width: f32,                // world units, DSFT scale already applied
    pub height: f32,               // world units, DSFT scale already applied
    pub palette_id: i16,
    pub sound_id: u16,
    pub event_id: i16,
    pub facing_yaw: f32,
}
```

### `Decorations` struct

```rust
pub struct Decorations {
    entries: Vec<DecorationEntry>,
}

impl Decorations {
    /// Load and resolve all decorations for a map.
    /// Takes pre-parsed billboards because they are embedded in the ODM file,
    /// which openmm already parses for terrain/models — avoids re-parsing.
    /// Internally loads: BillboardManager, DSFT.
    pub fn new(lod: &LodManager, odm_billboards: &[Billboard]) -> Result<Self, Box<dyn Error>>

    pub fn iter(&self) -> impl Iterator<Item = &DecorationEntry>
    pub fn len(&self) -> usize
}
```

### Construction logic (inside `Decorations::new`)

1. Load `BillboardManager` (wraps DDecList + DSFT)
2. For each `Billboard` in `odm_billboards`:
   - Skip if `is_no_draw()` or `is_marker()`
   - Call `has_directional_sprites(name, lod)` → sets `is_directional`
   - Extract pixel dimensions from `BillboardManager.get()`
   - Apply DSFT scale → final `width`, `height` in world units
   - Build `DecorationEntry`

`has_directional_sprites()` moves from `openmm/src/game/entities/sprites.rs` into `decorations.rs` as a private function.

---

## What Leaves `openmm`

| Function / Struct | Current location | Destination |
|---|---|---|
| `resolve_dsft_sprite()` | `odm.rs` | `lod/game/monster.rs` (private) |
| `build_npc_sprite_table()` | `odm.rs` | absorbed into `Actors::new()` |
| `NpcSpriteEntry` | `odm.rs` | deleted (replaced by `Actor`) |
| `has_directional_sprites()` | `sprites.rs` | `lod/game/decorations.rs` (private) |
| DSFT scale extraction (3 sites) | `loading.rs` + `odm.rs` | `Decorations::new()` |
| BillboardManager queries in spawn | `odm.rs` | `Decorations::new()` |

Estimated reduction in `odm.rs`: ~230 lines of data-layer logic.

## What Stays in `openmm`

- `PreparedMonster` — carries ODM spawn point positions; built from `Actors` + ODM data
- Spawn point grouping (3-6 monster clusters per spawn point)
- Bevy material/mesh/texture creation
- Entity spawning and component attachment
- All caching of Bevy asset handles

---

## State Persistence Hook

`MapStateSnapshot { dead_actor_ids: Vec<u16> }` is a stub in this iteration. `Actors::new()` accepts `Option<&MapStateSnapshot>` and skips matching actors when `Some`. No persistence mechanism is implemented yet — the hook exists so the signature doesn't need to change when persistence is added.

---

## Testing

- `actors.rs`: integration tests loading oute3.odm — assert NPCs have names/portraits, monsters have sprite roots, `get_npcs()`/`get_monsters()` return correct subsets
- `monster.rs`: unit test `resolve_entry` for known monlist IDs (peasant, goblin, etc.)
- `decorations.rs`: integration test loading oute3.odm decorations — assert non-zero entries, dimensions > 0, directional flag correct for known sprites (e.g. trees)
- `npc.rs`: existing tests pass after rename

---

## Migration Order

1. Rename `npctable.rs` → `npc.rs`, update type names and `mod.rs` re-exports
2. Add `monster.rs` with `resolve_entry()` (move `resolve_dsft_sprite` verbatim, add tests)
3. Add `actors.rs` with `Actors::new()`, wire `monster.rs` + `npc.rs`
4. Add `decorations.rs` with `Decorations::new()`, move `has_directional_sprites`
5. Update `openmm`: replace `build_npc_sprite_table` + NPC spawn with `Actors`, replace billboard resolution with `Decorations`
6. Delete dead code from `odm.rs`, `sprites.rs`
