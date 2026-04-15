# Code Consolidation — 6 Refactors

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate duplicated logic, move format knowledge to openmm-data, and split oversized functions into focused units.

**Architecture:** Each refactor is independent — commit after each task. No new features, no behavior changes. Pure structural cleanup.

**Tech Stack:** Rust, Bevy 0.18, openmm-data crate, image crate

---

## Task 1: Expose sprite palette_id from openmm-data

`sprites/loading.rs:501-508` reads raw bytes at offset 20 to extract `palette_id`. This is Rule 3 violation — format knowledge belongs in openmm-data. The sprite parser at `openmm-data/src/assets/image.rs:77` already reads this field internally.

**Files:**
- Modify: `openmm-data/src/assets/image.rs` — add `pub fn sprite_palette_id(data: &[u8]) -> Option<u16>`
- Modify: `openmm/src/game/sprites/loading.rs:497-508` — use new function
- Test: `openmm-data/src/assets/image.rs` — inline test

- [ ] **Step 1: Add sprite_palette_id function to openmm-data**

In `openmm-data/src/assets/image.rs`, add near the existing palette_id parsing:

```rust
/// Extract the palette_id from a raw sprite header without full decode.
/// The palette_id is a u16 LE at byte offset 20 in the sprite file format.
pub fn sprite_palette_id(data: &[u8]) -> Option<u16> {
    if data.len() < 22 {
        return None;
    }
    Some(u16::from_le_bytes([data[20], data[21]]))
}
```

- [ ] **Step 2: Add test for sprite_palette_id**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_palette_id_extracts_from_header() {
        let mut data = vec![0u8; 22];
        data[20] = 0x2A;
        data[21] = 0x00;
        assert_eq!(sprite_palette_id(&data), Some(42));
    }

    #[test]
    fn sprite_palette_id_none_for_short_data() {
        assert_eq!(sprite_palette_id(&[0u8; 10]), None);
    }
}
```

Run: `cargo test -p openmm-data sprite_palette_id`

- [ ] **Step 3: Replace inline parsing in loading.rs**

In `openmm/src/game/sprites/loading.rs`, replace lines 501-508:

```rust
// Before:
let sprite_data = assets.get_bytes(format!("sprites/{}", sprite_name.to_lowercase())).ok()?;
if sprite_data.len() < 22 { return None; }
let base_palette_id = u16::from_le_bytes([sprite_data[20], sprite_data[21]]);

// After:
let sprite_data = assets.get_bytes(format!("sprites/{}", sprite_name.to_lowercase())).ok()?;
let base_palette_id = openmm_data::assets::image::sprite_palette_id(&sprite_data)?;
```

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`

- [ ] **Step 5: Commit**

```
git add openmm-data/src/assets/image.rs openmm/src/game/sprites/loading.rs
git commit --no-gpg-sign -m "move sprite palette_id parsing to openmm-data"
```

---

## Task 2: Deduplicate image padding in sprites/loading.rs

Identical 13-line pixel-copy blocks at lines 454-466 and 765-777. Extract to a shared helper.

**Files:**
- Modify: `openmm/src/game/sprites/loading.rs` — add helper, replace both call sites

- [ ] **Step 1: Add pad_sprite_image helper**

Add to `openmm/src/game/sprites/loading.rs` near the top (after imports):

```rust
/// Pad an RGBA image to target dimensions, centered horizontally and bottom-aligned.
fn pad_sprite_image(rgba: image::RgbaImage, target_w: u32, target_h: u32) -> image::RgbaImage {
    if rgba.width() == target_w && rgba.height() == target_h {
        return rgba;
    }
    let mut padded = image::RgbaImage::new(target_w, target_h);
    let x_off = (target_w - rgba.width()) / 2;
    let y_off = target_h - rgba.height();
    for py in 0..rgba.height() {
        for px in 0..rgba.width() {
            padded.put_pixel(px + x_off, py + y_off, *rgba.get_pixel(px, py));
        }
    }
    padded
}
```

- [ ] **Step 2: Replace first call site (line ~454)**

```rust
// Before (13 lines):
let rgba = if rgba.width() != max_w || rgba.height() != max_h {
    let mut padded = image::RgbaImage::new(max_w, max_h);
    // ... pixel copy loop ...
    padded
} else {
    rgba
};

// After (1 line):
let rgba = pad_sprite_image(rgba, max_w, max_h);
```

- [ ] **Step 3: Replace second call site (line ~765)**

Same replacement as step 2.

- [ ] **Step 4: Build**

Run: `cargo build`

- [ ] **Step 5: Commit**

```
git add openmm/src/game/sprites/loading.rs
git commit --no-gpg-sign -m "deduplicate sprite image padding into shared helper"
```

---

## Task 3: Deduplicate Actor component construction

4 nearly identical Actor constructions in `spawn_actors.rs` (lines 91, 225, 310) and `indoor.rs` (line 813). Only 5 fields differ: `hostile`, `ddm_id`, `group_id`, `tether_distance`, `attack_range`.

**Files:**
- Modify: `openmm/src/game/actors/actor.rs` — add `Actor::new()` constructor
- Modify: `openmm/src/game/outdoor/spawn_actors.rs` — use constructor
- Modify: `openmm/src/game/indoor/indoor.rs` — use constructor

- [ ] **Step 1: Add Actor::new() constructor**

In `openmm/src/game/actors/actor.rs`, add an impl block:

```rust
/// Common actor data needed by the constructor.
pub struct ActorParams {
    pub name: String,
    pub hp: i16,
    pub move_speed: f32,
    pub position: Vec3,
    pub hostile: bool,
    pub variant: u8,
    pub sound_ids: [u16; 4],
    pub tether_distance: f32,
    pub attack_range: f32,
    pub ddm_id: i32,
    pub group_id: i32,
    pub aggro_range: f32,
    pub recovery_secs: f32,
    pub sprite_half_height: f32,
    pub can_fly: bool,
    pub ai_type: String,
}

impl Actor {
    pub fn new(p: ActorParams) -> Self {
        let pos = p.position;
        Self {
            name: p.name,
            hp: p.hp,
            max_hp: p.hp,
            move_speed: p.move_speed,
            initial_position: pos,
            guarding_position: pos,
            tether_distance: p.tether_distance,
            wander_timer: (pos.x * 0.011 + pos.z * 0.017).abs().fract() * 4.0,
            wander_target: pos,
            facing_yaw: 0.0,
            hostile: p.hostile,
            variant: p.variant,
            sound_ids: p.sound_ids,
            fidget_timer: (pos.x * 0.013 + pos.z * 0.019).abs().fract() * 15.0 + 5.0,
            attack_range: p.attack_range,
            attack_timer: (pos.x * 0.007 + pos.z * 0.023).abs().fract() * 3.0 + 1.0,
            attack_anim_remaining: 0.0,
            ddm_id: p.ddm_id,
            group_id: p.group_id,
            aggro_range: p.aggro_range,
            recovery_secs: p.recovery_secs,
            sprite_half_height: p.sprite_half_height,
            can_fly: p.can_fly,
            vertical_velocity: 0.0,
            ai_type: p.ai_type,
            cached_steer_offset: None,
        }
    }
}
```

- [ ] **Step 2: Replace DDM monster construction in spawn_actors.rs (~line 91)**

```rust
actor::Actor::new(actor::ActorParams {
    name: actor.name.clone(),
    hp: actor.hp,
    move_speed: actor.move_speed as f32,
    position: pos,
    hostile: true,
    variant: actor.variant,
    sound_ids: actor.sound_ids,
    tether_distance: actor.tether_distance as f32,
    attack_range: actor.radius as f32 * 2.0,
    ddm_id: i as i32,
    group_id: actor.group,
    aggro_range: actor.aggro_range,
    recovery_secs: actor.recovery_secs,
    sprite_half_height: sh / 2.0,
    can_fly: actor.can_fly,
    ai_type: actor.ai_type.clone(),
})
```

- [ ] **Step 3: Replace DDM NPC construction (~line 225)**

Same as step 2 but with `hostile: false`.

- [ ] **Step 4: Replace ODM monster construction (~line 310)**

```rust
actor::Actor::new(actor::ActorParams {
    name: mon.name.clone(),
    hp: mon.hp,
    move_speed: mon.move_speed as f32,
    position: pos,
    hostile: true,
    variant: mon.variant,
    sound_ids: mon.sound_ids,
    tether_distance: mon.radius as f32 * 2.0,
    attack_range: mon.body_radius as f32 * 2.0,
    ddm_id: -1,
    group_id: 0,
    aggro_range: mon.aggro_range,
    recovery_secs: mon.recovery_secs,
    sprite_half_height: sh / 2.0,
    can_fly: mon.can_fly,
    ai_type: mon.ai_type.clone(),
})
```

- [ ] **Step 5: Replace indoor monster construction in indoor.rs (~line 813)**

Same pattern as step 4 (indoor monsters use `mon.*` fields, `ddm_id: -1`, `group_id: 0`).

- [ ] **Step 6: Build and test**

Run: `cargo build && cargo test`

- [ ] **Step 7: Commit**

```
git add openmm/src/game/actors/actor.rs openmm/src/game/outdoor/spawn_actors.rs openmm/src/game/indoor/indoor.rs
git commit --no-gpg-sign -m "deduplicate Actor construction with Actor::new(ActorParams)"
```

---

## Task 4: Split process_events() in scripting.rs

680-line function with 50 match arms. Strategy: extract the complex arms (MoveToMap 51 lines, SetSprite 47 lines, SpeakNPC 22 lines, OpenChest 26 lines) into handler functions. Keep simple 1-5 line arms inline — extracting those would hurt readability.

**Files:**
- Modify: `openmm/src/game/world/scripting.rs` — extract handler functions

- [ ] **Step 1: Extract handle_move_to_map()**

Move the MoveToMap arm body (lines ~561-636) into:

```rust
fn handle_move_to_map(
    ev: &GameEvent,  // the MoveToMap variant fields
    event_queue: &mut EventQueue,
    world_state: &mut super::state::WorldState,
    party: &mut Party,
    transition: &mut TransitionParams,
    audio: &mut AudioParams,
    commands: &mut Commands,
) {
    // ... extracted body ...
}
```

The match arm becomes a single function call.

- [ ] **Step 2: Extract handle_set_sprite()**

Move the SetSprite arm body (lines ~769-815) into:

```rust
fn handle_set_sprite(
    decoration_name: &str,
    sprite_name: &str,
    entities: &mut MapEntityParams,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
    meshes: &mut Assets<Mesh>,
    sprite_materials: &mut Option<ResMut<Assets<SpriteMaterial>>>,
) {
    // ... extracted body ...
}
```

- [ ] **Step 3: Extract handle_speak_in_house() and handle_open_chest()**

Same pattern — move arm bodies into functions. SpeakInHouse (~20 lines) and OpenChest (~26 lines).

- [ ] **Step 4: Build and test**

Run: `cargo build && cargo test`

- [ ] **Step 5: Commit**

```
git add openmm/src/game/world/scripting.rs
git commit --no-gpg-sign -m "extract large event handlers from process_events match"
```

---

## Task 5: Split spawn_indoor_world() in indoor.rs

514-line function with 11 distinct sections. Extract each section into a focused function. The main function becomes a sequence of calls.

**Files:**
- Modify: `openmm/src/game/indoor/indoor.rs` — extract section functions

- [ ] **Step 1: Extract resource-building functions**

Extract these small, self-contained sections into functions:

```rust
fn build_blv_doors(prepared: &PreparedIndoorWorld) -> BlvDoors { ... }
fn build_door_colliders(prepared: &PreparedIndoorWorld) -> DoorColliders { ... }
fn build_clickable_faces(prepared: &PreparedIndoorWorld) -> ClickableFaces { ... }
fn build_occluder_faces(prepared: &PreparedIndoorWorld) -> OccluderFaces { ... }
fn build_touch_triggers(prepared: &PreparedIndoorWorld) -> TouchTriggerFaces { ... }
```

Each takes `&PreparedIndoorWorld` and returns the resource. The main function inserts them with `commands.insert_resource()`.

- [ ] **Step 2: Extract spawning functions**

```rust
fn spawn_static_meshes(prepared: &PreparedIndoorWorld, commands: &mut Commands, ...) { ... }
fn spawn_door_faces(prepared: &PreparedIndoorWorld, commands: &mut Commands, ...) { ... }
fn spawn_blv_point_lights(prepared: &PreparedIndoorWorld, commands: &mut Commands) { ... }
fn spawn_indoor_monsters(prepared: &PreparedIndoorWorld, commands: &mut Commands, ...) { ... }
```

- [ ] **Step 3: Split decoration spawning into 3 functions**

The decorations section (289 lines) has 3 branches. Extract each:

```rust
fn spawn_directional_decorations(dec: &PreparedDecoration, commands: &mut Commands, ...) { ... }
fn spawn_animated_decorations(dec: &PreparedDecoration, commands: &mut Commands, ...) { ... }
fn spawn_static_decorations(dec: &PreparedDecoration, commands: &mut Commands, ...) { ... }
```

The decoration loop stays in main function, dispatching to the right sub-function based on decoration type.

- [ ] **Step 4: Verify spawn_indoor_world is now a clear sequence**

The main function should read like:

```rust
pub(crate) fn spawn_indoor_world(...) {
    spawn_static_meshes(&prepared, &mut commands, ...);
    spawn_door_faces(&prepared, &mut commands, ...);
    commands.insert_resource(build_blv_doors(&prepared));
    commands.insert_resource(build_door_colliders(&prepared));
    commands.insert_resource(build_clickable_faces(&prepared));
    commands.insert_resource(build_occluder_faces(&prepared));
    commands.insert_resource(build_touch_triggers(&prepared));
    spawn_ambient_light(&mut commands);
    spawn_decorations(&prepared, &mut commands, ...);
    spawn_blv_point_lights(&prepared, &mut commands);
    spawn_indoor_monsters(&prepared, &mut commands, ...);
}
```

- [ ] **Step 5: Build and test**

Run: `cargo build && cargo test`

- [ ] **Step 6: Commit**

```
git add openmm/src/game/indoor/indoor.rs
git commit --no-gpg-sign -m "split spawn_indoor_world into focused functions"
```

---

## Task 6: Move raw path construction from editor/browser.rs to GameAssets

`editor/browser.rs:73-92` calls `openmm_data::get_data_path()` + `find_path_case_insensitive()` to manually discover VID archives. Since `Assets::refresh()` now scans sibling dirs including `Anims/`, the VID archives are already loaded. Use `GameAssets::archives()` and `GameAssets::files_in()` instead.

**Files:**
- Modify: `openmm/src/editor/browser.rs` — replace raw path construction with GameAssets API

- [ ] **Step 1: Read current browser.rs code**

Read the full `build_browser_folders()` function to understand how LOD and VID folders are populated.

- [ ] **Step 2: Replace VID archive discovery**

Replace the raw path construction block with:

```rust
// VID archives are already loaded by Assets::refresh() scanning Anims/ sibling dir
for archive_name in game_assets.assets().archives() {
    if !archive_name.ends_with(".vid") && !archive_name.contains("anim") {
        continue;
    }
    if let Some(files) = game_assets.assets().files_in(&archive_name) {
        let mut sorted = files;
        sorted.sort();
        let prefixed: Vec<String> = sorted.into_iter().map(|f| format!("{}/{}", archive_name, f)).collect();
        browser.folders.push(LodFolder {
            name: archive_name,
            files: prefixed,
            is_video: true,
        });
    }
}
```

Note: verify the exact archive naming pattern first — `archives()` returns keys from `self.smks`, which use the stem (e.g. `"anims1"` not `"Anims1.vid"`).

- [ ] **Step 3: Remove unused imports**

Remove `openmm_data::utils::find_path_case_insensitive`, `SmkArchive`, etc. if no longer needed.

- [ ] **Step 4: Build and test**

Run: `cargo build`

- [ ] **Step 5: Commit**

```
git add openmm/src/editor/browser.rs
git commit --no-gpg-sign -m "use GameAssets API for VID archive discovery in editor browser"
```
