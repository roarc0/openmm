# lod Game Layer Part 2 Implementation Design

## Goal

Three targeted clean-ups to the `lod::game` layer introduced in the actors/decorations refactor: pre-compute palette variant inside `Actors`, consolidate monster spawn resolution into `lod::game::monster` as `Monsters`/`Monster`, and eliminate the now-redundant `PreparedBillboard` struct.

## Architecture

All three changes push data-resolution logic further into the `lod` crate and simplify `openmm` to pure spawning. No new crate boundaries, no Bevy dependencies in `lod`.

## Tech Stack

Rust, Bevy 0.18 (openmm side only), existing `lod` crate infrastructure.

---

## Task 1: Pre-compute `variant` in `Actors`

### What changes

`ActorEntry.variant` is currently set to `0` in `lod/src/game/actors.rs` and recomputed by both `odm.rs` and `blv.rs` after the fact. Move the computation into `Actors` as a second pass.

### Algorithm

After building the `actors` Vec, group by `standing_sprite` and find the minimum `palette_id` per group:

```rust
let mut base_pals: HashMap<String, u16> = HashMap::new();
for a in &actors {
    let entry = base_pals.entry(a.standing_sprite.clone()).or_insert(a.palette_id);
    if a.palette_id < *entry { *entry = a.palette_id; }
}
for actor in &mut actors {
    let base = base_pals[&actor.standing_sprite];
    actor.variant = ((actor.palette_id - base + 1) as u8).min(3);
}
```

This second pass runs in both `Actors::new()` and `Actors::from_raw_actors()`.

### Callers to update

- `openmm/src/game/odm.rs` — remove local variant computation in NPC spawn loop
- `openmm/src/game/blv.rs` — remove local variant computation in NPC spawn loop

---

## Task 2: `lod::game::monster` — `Monsters` and `Monster`

### What changes

`resolve_monsters()` in `openmm/src/game/odm.rs` resolves sprite/palette data from lod. That logic belongs in the `lod` crate. Position spreading (angle × radius) stays in `openmm` because it requires geometry math that doesn't belong in a pure data crate.

### New types in `lod/src/game/monster.rs`

```rust
pub struct Monster {
    pub spawn_position: [i32; 3],   // group center, raw MM6 units
    pub spawn_radius: u16,           // for spread computation in openmm
    pub group_index: usize,          // 0..group_size, for spread angle in openmm
    pub standing_sprite: String,
    pub walking_sprite: String,
    pub palette_id: u16,
    pub variant: u8,                 // pre-computed same formula as Actors
    pub height: u16,
    pub move_speed: u16,
    pub hostile: bool,
}

pub struct Monsters {
    entries: Vec<Monster>,
}

impl Monsters {
    pub fn new(lod: &LodManager, map_name: &str) -> Result<Self, Box<dyn Error>>
    pub fn entries(&self) -> &[Monster]
    pub fn iter(&self) -> impl Iterator<Item = &Monster>
    pub fn len(&self) -> usize
    pub fn is_empty(&self) -> bool
}
```

One `Monster` entry per group member. Group size = `3 + (position_sum % 3)` (current formula in `resolve_monsters`). Sprite resolution uses the existing `resolve_sprite_group()` already in `lod/src/game/monster.rs`. Variant pre-computation uses the same min-palette-per-sprite-root formula as Task 1.

### Callers to update

- `openmm/src/game/odm.rs`:
  - Delete `resolve_monsters()` function
  - `PendingSpawns.resolved_monsters: Vec<PreparedMonster>` → `monsters: Option<Monsters>`
  - `spawn_world`: call `Monsters::new(lod, map_name)` instead of `resolve_monsters()`
  - Monster spawn loop reads `m.spawn_position`, `m.spawn_radius`, `m.group_index` and computes spread inline
- `openmm/src/states/loading.rs`:
  - `PreparedWorld.monsters: Vec<PreparedMonster>` → `monsters: Monsters` (populated lazily in `spawn_world`, passed through `PreparedWorld` as before)
  - Delete `PreparedMonster` struct

---

## Task 3: Eliminate `PreparedBillboard`

### What changes

`PreparedBillboard` in `openmm/src/states/loading.rs` carries the same data already in `lod::game::decorations::Decorations`. Replace the vec with the struct directly.

### Changes in `loading.rs`

- `PreparedWorld.billboards: Vec<PreparedBillboard>` → `PreparedWorld.decorations: Decorations`
- `BuildBillboards` step: call `Decorations::new(lod, &map.billboards)` instead of building a `Vec<PreparedBillboard>`; start-points still extracted from ODM separately
- `PreloadSprites` step: iterate `decorations.iter()` for non-directional entries instead of iterating `billboards`
- Delete `PreparedBillboard` struct

### Changes in `odm.rs`

- `PendingSpawns` loses `decorations: Option<Decorations>` and `dec_by_billboard: HashMap<usize, usize>`
- `billboard_order` now contains indices into `decorations.entries()` (decorations are already filtered/resolved, so `billboard_order` is `0..decorations.len()` sorted by distance)
- Billboard spawn loop: `let dec = &decorations.entries()[order_idx]` — direct index, no HashMap lookup
- `mm6_to_bevy(dec.position[0], dec.position[1], dec.position[2])` called inline at spawn time

### What stays the same

`lod::game::decorations::Decorations` keeps `position: [i32; 3]` (no Bevy dep in lod). The coordinate conversion is the caller's responsibility.
