# lod Game Layer Part 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Push three related clean-ups into the `lod` crate: pre-compute palette `variant` inside `Actors`, expose monster spawn resolution as `lod::game::monster::Monsters`/`Monster`, and eliminate the `PreparedBillboard` struct by replacing it with `lod::game::decorations::Decorations` throughout.

**Architecture:** All three tasks are independent but ordered by risk. Task 1 is pure `lod` crate (no openmm changes). Task 2 adds a new `lod` type + updates `openmm`. Task 3 removes `PreparedBillboard` from `loading.rs` and simplifies the billboard spawn path in `odm.rs`.

**Tech Stack:** Rust 2024 edition, cargo workspace (`lod` + `openmm` crates), `make test` to verify.

---

## File Structure

| File | Change |
|------|--------|
| `lod/src/game/actors.rs` | Add variant second-pass to `new()` and `from_raw_actors()` |
| `lod/src/game/monster.rs` | Add `Monster` struct and `Monsters` struct with `new()` |
| `openmm/src/game/odm.rs` | Replace `resolve_monsters()` + `PreparedMonster` with `Monsters`; remove variant computation from NPC loop; fix billboard spawn to use `Decorations` directly |
| `openmm/src/states/loading.rs` | Replace `PreparedWorld.billboards: Vec<PreparedBillboard>` with `Decorations`; delete `PreparedBillboard`; delete `PreparedMonster`; update `BuildBillboards` and `PreloadSprites` steps |

---

## Task 1: Pre-compute `variant` in `Actors`

**Files:**
- Modify: `lod/src/game/actors.rs`

### Context

`Actor.variant` is currently always `0`. Both `openmm/src/game/odm.rs:677` and `openmm/src/game/blv.rs:344` independently compute:

```rust
let base_pal = actors.iter()
    .filter(|a| a.standing_sprite == actor.standing_sprite)
    .map(|a| a.palette_id).min().unwrap_or(actor.palette_id);
let variant = (actor.palette_id - base_pal + 1).min(3) as u8;
```

Move this into `Actors` as a second pass so callers just read `actor.variant`.

- [ ] **Step 1: Write the failing test**

Add to `lod/src/game/actors.rs` in the `#[cfg(test)]` block:

```rust
#[test]
fn variant_is_precomputed() {
    let lod = LodManager::new(get_lod_path()).unwrap();
    let actors = Actors::new(&lod, "oute3.odm", None).unwrap();
    // Every actor should have variant 1, 2, or 3 — never 0 (unless there is truly only one palette).
    // The test checks that at least one actor has variant > 0 (proving the pass ran).
    // Actors with a unique standing_sprite will always be variant 1.
    let all_variants_nonzero = actors.get_actors().iter().all(|a| a.variant >= 1);
    assert!(all_variants_nonzero, "all actors should have variant >= 1 after pre-computation");
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cd /home/roarc/repos/openmm && cargo test -p lod variant_is_precomputed 2>&1 | tail -5
```

Expected: FAIL — `assertion failed: all_variants_nonzero`

- [ ] **Step 3: Implement the variant second pass**

In `lod/src/game/actors.rs`, add a helper function above `impl Actors`:

```rust
fn compute_variants(actors: &mut Vec<Actor>) {
    let mut base_pals: std::collections::HashMap<&str, u16> = std::collections::HashMap::new();
    for a in actors.iter() {
        let e = base_pals.entry(&a.standing_sprite).or_insert(a.palette_id);
        if a.palette_id < *e { *e = a.palette_id; }
    }
    // base_pals keys borrow from actors, so we need to collect first
    let base_pals: std::collections::HashMap<String, u16> = base_pals
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    for actor in actors.iter_mut() {
        let base = base_pals[&actor.standing_sprite];
        actor.variant = ((actor.palette_id - base + 1) as u8).min(3);
    }
}
```

In `Actors::new()`, add just before `Ok(Actors { actors })`:

```rust
compute_variants(&mut actors);
Ok(Actors { actors })
```

In `Actors::from_raw_actors()`, add just before `Ok(Actors { actors })`:

```rust
compute_variants(&mut actors);
Ok(Actors { actors })
```

Update the doc comment on `Actor.variant` from:
```rust
pub variant: u8,       // 0 = unset, openmm computes from palette_id offset at spawn
```
to:
```rust
/// Palette variant: 1=A (base), 2=B, 3=C. Pre-computed from palette_id offset within
/// actors sharing the same standing_sprite root. Never 0 after construction.
pub variant: u8,
```

- [ ] **Step 4: Run the test to verify it passes**

```
cd /home/roarc/repos/openmm && cargo test -p lod variant_is_precomputed 2>&1 | tail -5
```

Expected: PASS

- [ ] **Step 5: Run full lod test suite**

```
cd /home/roarc/repos/openmm && cargo test -p lod 2>&1 | tail -10
```

Expected: all tests pass

- [ ] **Step 6: Update callers in openmm — remove local variant computation**

In `openmm/src/game/odm.rs`, replace lines 669–677 (the local variant computation in the NPC lazy_spawn loop):

```rust
// BEFORE (remove this block):
// Palette variant: base palette is minimum among actors sharing the same sprite root.
let base_pal = p.actors.as_ref()
    .map(|actors| actors.get_actors().iter()
        .filter(|a| a.standing_sprite == actor.standing_sprite)
        .map(|a| a.palette_id)
        .min()
        .unwrap_or(actor.palette_id))
    .unwrap_or(actor.palette_id);
let variant = (actor.palette_id - base_pal + 1).min(3) as u8;

// AFTER (use pre-computed value):
let variant = actor.variant;
```

In `openmm/src/game/blv.rs`, replace lines 338–344 (the local variant computation in the BLV NPC spawn loop):

```rust
// BEFORE (remove this block):
// Compute palette variant: base palette is minimum among actors sharing the same sprite root.
let base_pal = actors.get_actors().iter()
    .filter(|a| a.standing_sprite == actor.standing_sprite)
    .map(|a| a.palette_id)
    .min()
    .unwrap_or(actor.palette_id);
let variant = (actor.palette_id - base_pal + 1).min(3) as u8;

// AFTER:
let variant = actor.variant;
```

- [ ] **Step 7: Build openmm to confirm no compilation errors**

```
cd /home/roarc/repos/openmm && cargo build -p openmm 2>&1 | grep -E "^error" | head -20
```

Expected: no errors

- [ ] **Step 8: Run full test suite**

```
cd /home/roarc/repos/openmm && make test 2>&1 | tail -15
```

Expected: all tests pass

- [ ] **Step 9: Commit**

```bash
cd /home/roarc/repos/openmm
git add lod/src/game/actors.rs openmm/src/game/odm.rs openmm/src/game/blv.rs
git commit --no-gpg-sign -m "refactor: pre-compute palette variant in Actors, remove duplicate callers"
```

---

## Task 2: `lod::game::monster::Monsters` and `Monster`

**Files:**
- Modify: `lod/src/game/monster.rs`
- Modify: `openmm/src/game/odm.rs`
- Modify: `openmm/src/states/loading.rs`

### Context

`resolve_monsters()` in `openmm/src/game/odm.rs:428` mixes sprite/palette resolution (belongs in `lod`) with position spreading (geometry, stays in openmm). The function:

1. Loads `MapStats` + `MonsterList` from LOD
2. Iterates `prepared.map.spawn_points` (each ODM `SpawnPoint` has `position: [i32; 3]`, `radius: u16`, `monster_index: u16`)
3. Computes `group_size = 3 + ((sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs()) % 3) as i32`
4. For each group member, calls `cfg.monster_for_index(sp.monster_index, seed)` → `(&str name, u8 dif)`
5. Resolves sprite via `resolve_sprite_group`, gets `(st_root, palette_id)`
6. **Computes spread position**: `angle = g as f32 * 2.094`, `spread = sp.radius.max(200) as f32 * 0.5`, position offset
7. Pushes a `PreparedMonster` with the already-spread position

The new design: `Monsters::new()` in `lod` does steps 1–5 and produces one `Monster` per group member with the **center position** and **group_index**. The spread computation (step 6) stays in `openmm/src/game/odm.rs`.

### New types to add at the top of `lod/src/game/monster.rs`

```rust
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
                let seed = (sp.position[0].unsigned_abs() + sp.position[1].unsigned_abs() + g as u32) as u32;
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

    pub fn entries(&self) -> &[Monster] { &self.entries }
    pub fn iter(&self) -> impl Iterator<Item = &Monster> { self.entries.iter() }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}
```

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)]` block in `lod/src/game/monster.rs`:

```rust
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
    // group_size is 3..=5, so group_index must be < 5
    for m in monsters.iter() {
        assert!(m.group_index < 6,
            "group_index {} seems too large", m.group_index);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cd /home/roarc/repos/openmm && cargo test -p lod monsters_loads_oute3 2>&1 | tail -5
```

Expected: FAIL — `Monsters` not defined

- [ ] **Step 3: Add `Monster`, `Monsters`, and `Monsters::new()` to `lod/src/game/monster.rs`**

Add the structs and impl block shown in the Context section above, before the existing `#[cfg(test)]` block. Also add `use std::error::Error;` at the top if not already present.

Note: `crate::odm::Odm` is used here but only in this module — the lod crate already uses it elsewhere.

- [ ] **Step 4: Run the lod tests to verify they pass**

```
cd /home/roarc/repos/openmm && cargo test -p lod monsters_ 2>&1 | tail -10
```

Expected: all 4 `monsters_*` tests PASS

- [ ] **Step 5: Run full lod test suite**

```
cd /home/roarc/repos/openmm && cargo test -p lod 2>&1 | tail -10
```

Expected: all tests pass

- [ ] **Step 6: Update `openmm/src/game/odm.rs` — replace `resolve_monsters` with `Monsters`**

In the `PendingSpawns` struct (lines ~19–36), change:

```rust
// BEFORE:
resolved_monsters: Vec<crate::states::loading::PreparedMonster>,

// AFTER:
monsters: lod::game::monster::Monsters,
```

In `spawn_world` (around line 372), replace:

```rust
// BEFORE:
let resolved_monsters = resolve_monsters(&prepared, &game_assets, &world_state.map.name);

// AFTER:
let monsters = lod::game::monster::Monsters::new(
    game_assets.lod_manager(),
    &world_state.map.name.to_string(),
).unwrap_or_else(|e| {
    warn!("Failed to resolve monsters for {}: {}", world_state.map.name, e);
    lod::game::monster::Monsters::default_empty()
});
```

Add a `default_empty()` helper to `Monsters` in `lod/src/game/monster.rs`:

```rust
pub fn default_empty() -> Self { Monsters { entries: Vec::new() } }
```

Update `sort_by_distance_mm6` call for monsters (around line 398):

```rust
// BEFORE:
let monster_order = sort_by_distance_mm6(&resolved_monsters, player_spawn,
    |m| m.position[0] as f32, |m| m.position[1] as f32);

// AFTER:
let monster_order = sort_by_distance_mm6(monsters.entries(), player_spawn,
    |m| m.spawn_position[0] as f32, |m| m.spawn_position[1] as f32);
```

Update `PendingSpawns` construction (around line 409):

```rust
// BEFORE:
resolved_monsters,

// AFTER:
monsters,
```

In `lazy_spawn`, update `monster_len` (around line 541):

```rust
// BEFORE:
let monster_len = p.resolved_monsters.len();

// AFTER:
let monster_len = p.monsters.len();
```

In the monster spawn loop (around line 757–793), replace:

```rust
// BEFORE:
let m = &p.resolved_monsters[p.monster_order[monster_idx]];
// ...
let wx = m.position[0] as f32;
let wz = -m.position[1] as f32;

// AFTER:
let m = &p.monsters.entries()[p.monster_order[monster_idx]];
// Compute spread position (was done inside resolve_monsters, now done here)
let angle = m.group_index as f32 * 2.094;
let spread = m.spawn_radius as f32 * 0.5;
let wx = m.spawn_position[0] as f32 + angle.cos() * spread * m.group_index as f32;
let wz = -(m.spawn_position[1] as f32 + angle.sin() * spread * m.group_index as f32);
```

Delete the `resolve_monsters()` function entirely (lines ~425–486).

- [ ] **Step 7: Update `openmm/src/states/loading.rs` — delete `PreparedMonster`, update `PreparedWorld`**

Delete the `PreparedMonster` struct (lines ~168–183).

In `PreparedWorld` (around line 258), change:

```rust
// BEFORE:
pub monsters: Vec<PreparedMonster>,

// AFTER:
// (remove this field entirely — monsters resolved lazily in spawn_world)
```

Remove `monsters: progress.monsters.take().unwrap_or_default()` from the `commands.insert_resource(PreparedWorld { ... })` block (around line 1045).

Remove `monsters: None,` from the `LoadingProgress` initialization (around line 330).

Remove `progress.monsters = Some(Vec::new());` from the `BuildTerrain` step (around line 432).

Remove `monsters: Option<Vec<PreparedMonster>>,` from the `LoadingProgress` struct (around line 62).

- [ ] **Step 8: Build openmm**

```
cd /home/roarc/repos/openmm && cargo build -p openmm 2>&1 | grep -E "^error" | head -20
```

Expected: no errors. Fix any remaining references to `PreparedMonster` or `resolved_monsters`.

- [ ] **Step 9: Run full test suite**

```
cd /home/roarc/repos/openmm && make test 2>&1 | tail -15
```

Expected: all tests pass

- [ ] **Step 10: Commit**

```bash
cd /home/roarc/repos/openmm
git add lod/src/game/monster.rs openmm/src/game/odm.rs openmm/src/states/loading.rs
git commit --no-gpg-sign -m "refactor: move monster spawn resolution to lod::game::monster::Monsters"
```

---

## Task 3: Eliminate `PreparedBillboard`

**Files:**
- Modify: `openmm/src/states/loading.rs`
- Modify: `openmm/src/game/odm.rs`

### Context

`PreparedBillboard` (defined at `loading.rs:185`) carries billboard data used in two places:
1. `PreloadSprites` step in `loading.rs` — iterates billboards to preload sprites into `billboard_cache`
2. `lazy_spawn` in `odm.rs` — reads `bb.position`, `bb.declist_name`, `bb.declist_id`, `bb.sound_id`, `bb.facing_yaw`, `bb.event_id`, `bb.billboard_index`

`lod::game::decorations::Decorations` already provides all the same information except:
- `position` is `[i32; 3]` (MM6) instead of `Vec3` (Bevy) — caller converts inline with `mm6_to_bevy`
- No `declist_id` field — but `declist_id` in `PreloadSprites` was only used for `BillboardManager::get(lod, name, declist_id)`, and `Decorations` already pre-resolved the sprite name so we can pass `0` or restructure

Actually `PreparedWorld.billboard_cache` uses `declist_name` as key and passes `declist_id` to `BillboardManager::get()`. With `Decorations`, we have `dec.sprite_name` and no `declist_id`. Check: `BillboardManager::get()` signature:

In `lod/src/billboard.rs`, `get()` takes `name: &str, declist_id: u16` but uses `declist_id` only to look up DSFT scale. We can pass `0` and it will just return `1.0` scale (already handled separately via `dsft_scale_for_group`).

**The plan:**

- `BuildBillboards` step: call `Decorations::new(lod, &odm.billboards)` AND extract start_points separately (start_point detection uses `is_marker()` + name contains "start", which `Decorations` filters out — so start_points must still be extracted from the raw `odm.billboards` before `Decorations::new()` filters them)
- Replace `PreparedWorld.billboards: Vec<PreparedBillboard>` with `PreparedWorld.decorations: lod::game::decorations::Decorations`
- `PreloadSprites`: iterate `decorations.iter()` for non-directional entries; use `dec.sprite_name` and pass `declist_id=0` to `BillboardManager::get()`
- `PendingSpawns`: remove `decorations: Option<Decorations>` and `dec_by_billboard: HashMap<usize, usize>`; add `decorations: lod::game::decorations::Decorations`
- `billboard_order` now indexes `decorations.entries()` directly (0..decorations.len() sorted by distance)
- Billboard spawn loop: `let dec = &p.decorations.entries()[p.billboard_order[bb_idx]];` — direct index

- [ ] **Step 1: Update `PreparedWorld` in `loading.rs`**

In the `PreparedWorld` struct (around line 249), change:

```rust
// BEFORE:
pub billboards: Vec<PreparedBillboard>,

// AFTER:
pub decorations: lod::game::decorations::Decorations,
```

Add `use lod::game::decorations::Decorations;` near the top of the file if not already imported.

- [ ] **Step 2: Update `BuildBillboards` step in `loading.rs`**

Replace the entire `LoadingStep::BuildBillboards` arm (lines ~835–890) with:

```rust
LoadingStep::BuildBillboards => {
    if let Some(odm) = &progress.odm {
        let bb_mgr = lod::billboard::BillboardManager::new(game_assets.lod_manager()).ok();
        let mut start_points = Vec::new();

        // Extract start/teleport markers from raw billboard list (Decorations filters these out)
        for bb in &odm.billboards {
            if bb.data.is_invisible() { continue; }
            let is_marker = bb_mgr.as_ref()
                .and_then(|mgr| mgr.get_declist_item(bb.data.declist_id))
                .map(|item| item.is_marker() || item.is_no_draw())
                .unwrap_or(false);
            let name_lower = bb.declist_name.to_lowercase();
            if name_lower.contains("start") || is_marker {
                let pos = Vec3::from(lod::odm::mm6_to_bevy(
                    bb.data.position[0], bb.data.position[1], bb.data.position[2],
                ));
                let yaw = bb.data.direction_degrees as f32 * std::f32::consts::PI / 1024.0;
                start_points.push(StartPoint {
                    name: bb.declist_name.clone(),
                    position: pos,
                    yaw,
                });
            }
        }

        progress.start_points = Some(start_points);
        // Decorations::new filters out invisible/marker/no-draw entries automatically
        progress.decorations = lod::game::decorations::Decorations::new(
            game_assets.lod_manager(),
            &odm.billboards,
        ).ok();
        progress.step = progress.step.next();
    }
}
```

Add `decorations: Option<lod::game::decorations::Decorations>,` to `LoadingProgress` struct (around line 60), and `decorations: None,` in its initializer (around line 328).

- [ ] **Step 3: Update `PreloadSprites` step in `loading.rs`**

In the billboard preload section of `PreloadSprites` (lines ~975–1011), replace:

```rust
// BEFORE:
let billboards = progress.billboards.take();
let bb_mgr = lod::billboard::BillboardManager::new(game_assets.lod_manager()).ok();
if let Some(ref bbs) = billboards {
    let queue = progress.preload_queue.as_mut().unwrap();
    if let Some(ref mgr) = bb_mgr {
        while queue.billboard_idx < bbs.len() {
            if frame_start.elapsed().as_secs_f32() * 1000.0 > PRELOAD_BUDGET_MS { break; }
            let bb = &bbs[queue.billboard_idx];
            queue.billboard_idx += 1;
            if bb_cache.contains_key(&bb.declist_name) { continue; }
            if let Some(sprite) = mgr.get(game_assets.lod_manager(), &bb.declist_name, bb.declist_id) {
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
}
progress.billboards = billboards;

// AFTER:
let bb_mgr = lod::billboard::BillboardManager::new(game_assets.lod_manager()).ok();
if let Some(ref decs) = progress.decorations {
    let queue = progress.preload_queue.as_mut().unwrap();
    if let Some(ref mgr) = bb_mgr {
        while queue.billboard_idx < decs.len() {
            if frame_start.elapsed().as_secs_f32() * 1000.0 > PRELOAD_BUDGET_MS { break; }
            let dec = &decs.entries()[queue.billboard_idx];
            queue.billboard_idx += 1;
            if dec.is_directional { continue; } // directional sprites loaded at spawn time
            if bb_cache.contains_key(&dec.sprite_name) { continue; }
            if let Some(sprite) = mgr.get(game_assets.lod_manager(), &dec.sprite_name, 0) {
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
                bb_cache.insert(dec.sprite_name.clone(), (m, q, h));
            }
        }
    }
}
```

Update the billboard preload completion check (around line 1014–1016):

```rust
// BEFORE:
let bb_len = progress.billboards.as_ref().map_or(0, |b| b.len());
if queue.sprite_idx >= queue.sprite_roots.len() && queue.billboard_idx >= bb_len {

// AFTER:
let dec_len = progress.decorations.as_ref().map_or(0, |d| d.len());
if queue.sprite_idx >= queue.sprite_roots.len() && queue.billboard_idx >= dec_len {
```

- [ ] **Step 4: Update `Done` step in `loading.rs`**

In the `commands.insert_resource(PreparedWorld { ... })` block (around line 1035), change:

```rust
// BEFORE:
billboards,       // was: progress.billboards.take().unwrap_or_default()

// AFTER:
decorations: progress.decorations.take().unwrap_or_else(|| {
    lod::game::decorations::Decorations::empty()
}),
```

Add `Decorations::empty()` to `lod/src/game/decorations.rs`:

```rust
impl Decorations {
    pub fn empty() -> Self { Decorations { entries: Vec::new() } }
}
```

Remove `billboards: Option<Vec<PreparedBillboard>>,` from `LoadingProgress` and `progress.billboards = Some(billboards);` from `BuildBillboards`. Delete `PreparedBillboard` struct.

- [ ] **Step 5: Update `openmm/src/game/odm.rs` — simplify PendingSpawns**

In `PendingSpawns`, replace:

```rust
// BEFORE:
billboard_cache: std::collections::HashMap<String, (Handle<StandardMaterial>, Handle<Mesh>, f32)>,
decorations: Option<lod::game::decorations::Decorations>,
dec_by_billboard: std::collections::HashMap<usize, usize>,

// AFTER:
billboard_cache: std::collections::HashMap<String, (Handle<StandardMaterial>, Handle<Mesh>, f32)>,
decorations: lod::game::decorations::Decorations,
```

In `spawn_world`, replace the `Decorations::new` + `dec_by_billboard` block (lines ~352–368) with:

```rust
// billboard_order now indexes decorations.entries() directly
let decorations = prepared.decorations.clone();
```

Note: Both `Decorations` and `DecorationEntry` need `#[derive(Clone)]` — add to both structs in `lod/src/game/decorations.rs`.

Replace the billboard distance sort (line ~388):

```rust
// BEFORE:
let bb_order = sort_by_distance_vec3(&prepared.billboards, player_spawn, |bb| bb.position);

// AFTER:
let bb_order = sort_by_distance_mm6(decorations.entries(), player_spawn,
    |d| d.position[0] as f32, |d| d.position[1] as f32);
```

Update `PendingSpawns` construction:

```rust
// BEFORE:
billboard_order: bb_order,
billboard_cache: prepared.billboard_cache.clone(),
decorations,
dec_by_billboard,

// AFTER:
billboard_order: bb_order,
billboard_cache: prepared.billboard_cache.clone(),
decorations,
```

- [ ] **Step 6: Update billboard spawn loop in `lazy_spawn` in `odm.rs`**

Replace the billboard loop setup and `dec` lookup (lines ~539–563):

```rust
// BEFORE:
let bb_len = p.billboard_order.len();
// ...
let idx = p.billboard_order[bb_idx];
bb_idx += 1;
p.idx += 1;
let bb = &prepared.billboards[idx];
let key = &bb.declist_name;
let dec = p.dec_by_billboard.get(&bb.billboard_index)
    .and_then(|&entry_idx| p.decorations.as_ref()?.entries().get(entry_idx));
let Some(dec) = dec else { continue; };

// AFTER:
let bb_len = p.billboard_order.len();
// ...
let dec_idx = p.billboard_order[bb_idx];
bb_idx += 1;
p.idx += 1;
let dec = &p.decorations.entries()[dec_idx];
let key = &dec.sprite_name;
```

Update all references from `bb.position` → compute inline:

```rust
// Wherever bb.position was used (two places: directional and non-directional):
// BEFORE:
let pos = bb.position + Vec3::new(0.0, sh / 2.0, 0.0);

// AFTER:
let dec_pos = Vec3::from(lod::odm::mm6_to_bevy(
    dec.position[0], dec.position[1], dec.position[2],
));
let pos = dec_pos + Vec3::new(0.0, sh / 2.0, 0.0);
```

Update all references from `bb.facing_yaw` → `dec.facing_yaw`, `bb.event_id` → `dec.event_id`, `bb.sound_id` → `dec.sound_id`, `bb.billboard_index` → `dec.billboard_index`, `bb.declist_id` → remove (not needed; directional DSFT scale is already fetched by `dsft_scale_for_group(&dec.sprite_name)`).

Remove the `prepared` parameter dependency for billboard data — `lazy_spawn` still needs `prepared` for `height_map` (monster spawn) but no longer reads `prepared.billboards`.

- [ ] **Step 7: Build openmm**

```
cd /home/roarc/repos/openmm && cargo build -p openmm 2>&1 | grep -E "^error" | head -20
```

Expected: no errors. Fix any remaining `PreparedBillboard` references or field name mismatches.

- [ ] **Step 8: Run full test suite**

```
cd /home/roarc/repos/openmm && make test 2>&1 | tail -15
```

Expected: all tests pass

- [ ] **Step 9: Commit**

```bash
cd /home/roarc/repos/openmm
git add lod/src/game/decorations.rs lod/src/game/monster.rs openmm/src/game/odm.rs openmm/src/states/loading.rs
git commit --no-gpg-sign -m "refactor: eliminate PreparedBillboard, use Decorations throughout"
```
