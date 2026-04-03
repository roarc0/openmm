# Pixel-Accurate Raycast & Hover System

**Date:** 2026-04-03  
**Status:** Approved

## Problem

The current outdoor interaction and hover system uses a wide angular cone (~7° half-angle) to find the nearest entity to the camera's forward ray. At 2000 units this cone has a 240-unit radius — objects far to the side of the crosshair still trigger. This applies to every outdoor object type: BSP buildings, decorations, NPCs, and monsters (not wired at all). Indoor faces already use correct ray-plane + polygon containment.

The fix applies to both hover hints (footer text) and click interaction — same detection code drives both.

## Goals

- Hover and interact trigger exactly when the crosshair is over an opaque pixel of the object
- Billboard sprites: ray-billboard plane intersection + per-pixel alpha mask test
- BSP building faces: use already-built `ClickableFaces` face geometry (same as indoor)
- Indoor hover hints: add missing system (indoor faces already support click, not hover)
- Monster hover: add `MonsterInteractable` component and wire into hover system
- No impact on the event dispatch pipeline — only detection changes

## Architecture

### New file: `openmm/src/game/raycast.rs`

Pure geometry math — no Bevy ECS, no systems, no resources. All functions are free functions.

**Moved from `blv.rs`:**
- `ray_plane_intersect(origin, dir, normal, plane_dist) -> Option<f32>`
- `point_in_polygon(point, vertices, normal) -> bool`

**New:**
- `billboard_hit_test(ray_origin, ray_dir, center, rotation, half_w, half_h, mask: Option<&AlphaMask>) -> Option<f32>`  
  1. Compute billboard plane normal from `rotation * Vec3::Z`  
  2. Intersect ray with plane via `ray_plane_intersect`  
  3. Check `|local_x| <= half_w && |local_y| <= half_h`  
  4. Compute `u = local_x / (2*half_w) + 0.5`, `v = 0.5 - local_y / (2*half_h)` (Y flipped for image coords)  
  5. If mask provided: `mask.test(u, v)` — miss on transparent pixel  
  6. Return `Some(t)` on hit

**Moved from `interaction.rs` (and unified):**
- `resolve_event_name(event_id: u16, map_events: &Option<Res<MapEvents>>) -> Option<String>`  
  Consolidates `resolve_building_name` + `resolve_decoration_name`. Checks EVT steps in order: `Hint`, `StatusText`, `SpeakInHouse`, `OpenChest`, `MoveToMap`. Returns the first non-empty match.

### Alpha mask: `openmm/src/game/entities/sprites.rs`

`AlphaMask` lives alongside sprite loading:

```rust
pub struct AlphaMask {
    width: u32,
    height: u32,
    data: Vec<bool>,  // row-major, true = opaque
}

impl AlphaMask {
    pub fn from_image(img: &RgbaImage) -> Self { ... }
    pub fn test(&self, u: f32, v: f32) -> bool { ... }  // clamps u,v to [0,1]
}
```

`SpriteCache` gains a parallel map:
```rust
masks: HashMap<String, Arc<AlphaMask>>
```
Keyed identically to `materials` — same cache key, built at the same time as the material.

`SpriteSheet` gains:
```rust
pub current_mask: Option<Arc<AlphaMask>>,
```
Updated by `update_sprite_sheets` in the same branch that swaps `mat_handle` — whenever `(state, frame, direction)` changes, `current_mask` is swapped to the matching cached mask.

### Modified: `openmm/src/game/interaction.rs`

Removed:
- `BuildingInfo` component
- `make_building_info` helper
- `raycast_nearest` function
- `find_nearest_building`, `find_nearest_decoration`, `find_nearest_npc`
- `resolve_building_name`, `resolve_decoration_name` (replaced by `raycast::resolve_event_name`)

The `interact_system` is removed — BSP outdoor faces are now handled by `indoor_interact_system` via `ClickableFaces` (which already exists for outdoor maps).

`decoration_interact_system` and `npc_interact_system` use `billboard_hit_test` from `raycast.rs`:
```rust
// query: (&DecorationInfo, &GlobalTransform, &Transform, &SpriteSheet)
// Position from GlobalTransform (world space), rotation from Transform (local Y-only rotation)
for (info, g_tf, tf, sheet) in decorations.iter() {
    let (sw, sh) = sheet.state_dimensions[sheet.current_state];
    let (hw, hh) = (sw / 2.0, sh / 2.0);
    if let Some(t) = billboard_hit_test(origin, dir, g_tf.translation(), tf.rotation, hw, hh, sheet.current_mask.as_deref()) {
        // track nearest t
    }
}
```

New `MonsterInteractable` component:
```rust
#[derive(Component)]
pub struct MonsterInteractable {
    pub name: String,
}
```
Hover only — no click action (combat not implemented). Added to monster entities in `odm.rs` and `blv.rs`.

`hover_hint_system` queries all four interactable types, collects `(t, name)` pairs, and picks the smallest `t`:
1. `ClickableFaces` faces → `ray_plane_intersect` + `point_in_polygon` → `resolve_event_name`
2. Decorations → `billboard_hit_test`
3. NPCs → `billboard_hit_test`
4. Monsters → `billboard_hit_test` (name only, no event)

### Modified: `openmm/src/game/blv.rs`

- `ray_plane_intersect` and `point_in_polygon` deleted, replaced by imports from `raycast`
- `indoor_interact_system` delegates geometry calls to `raycast` functions — logic unchanged
- Add `indoor_hover_hint_system` to `BlvPlugin`: tests `ClickableFaces` every frame when `HudView::World`, resolves name via `raycast::resolve_event_name`, calls `footer.set(name)`

### Modified: `openmm/src/game/odm.rs`

- Remove `BuildingInfo` inserts on BSP model entities
- Add `MonsterInteractable { name: monster.name.clone() }` to monster entity spawns

## Interaction range

Unchanged global engine constants:
- Outdoor: `RAYCAST_RANGE = 2000.0` (sprites, decorations, NPCs, monsters)
- Indoor/BSP faces: `INDOOR_INTERACT_RANGE = 5120.0`

No per-object range data exists in the MM6 game files for click interaction. Decoration `trigger_radius` in BLV is for proximity/touch triggers only and is not used here.

## Data flow

```
Camera forward ray
    │
    ├─ ClickableFaces (BSP faces, outdoor + indoor)
    │   ray_plane_intersect → point_in_polygon → resolve_event_name → (t, name)
    │
    ├─ DecorationInfo + SpriteSheet
    │   billboard_hit_test (bounds + AlphaMask) → resolve_event_name → (t, name)
    │
    ├─ NpcInteractable + SpriteSheet
    │   billboard_hit_test (bounds + AlphaMask) → npc.name → (t, name)
    │
    └─ MonsterInteractable + SpriteSheet
        billboard_hit_test (bounds + AlphaMask) → monster.name → (t, name)
                │
                └─ pick smallest t
                        │
                        ├─ hover: footer.set(name)
                        └─ interact: event_queue.push_all(event_id, evt)
```

## What does NOT change

- `EventQueue`, `process_events`, `GameEvent` variants — untouched
- `FooterText` resource and rendering — untouched
- `HudView` gating on all systems — unchanged
- `indoor_interact_system` logic — only geometry imports change
- Sprite loading pipeline — `AlphaMask` is built alongside existing material creation

## Testing

- Unit tests for `AlphaMask::test` with known pixel data
- Unit test for `billboard_hit_test`: ray through center hits, ray through transparent corner misses
- Unit test for `resolve_event_name`: correct priority order (Hint > StatusText > SpeakInHouse)
- Regression: existing `ClickableFaces` indoor door tests must still pass
