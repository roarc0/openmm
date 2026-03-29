# BLV Door System Design

## Summary

Implement interactive sliding doors for indoor (BLV) maps. Doors open/close when clicked or activated via Enter key, using the EVT event system for dispatch. Door faces are spawned as individual entities so their vertices can be animated per-frame.

## Data Parsing (lod crate)

### Face Extras — event_id and cog_number

The 36-byte face extra records contain fields we currently skip:

| Offset | Size | Field |
|--------|------|-------|
| 0x14 | i16 | textureDeltaU (already parsed) |
| 0x16 | i16 | textureDeltaV (already parsed) |
| 0x18 | i16 | cogNumber |
| 0x1A | u16 | eventId |

Add `event_id: u16` to `BlvFace` (resolved via `face_extra_id` like the texture deltas). The `cogNumber` field is not needed — `eventId` is what links to EVT scripts.

### Door Data Blob

After `door_count` in the BLV parse, read the door definitions. Each door has a 50-byte header:

```
attributes: u32
door_id: u32
time_since_triggered: u32  (runtime, ignore from file)
direction: [f32; 3]        (sliding direction vector, MM6 coords)
move_length: i32           (total slide distance in map units)
open_speed: i32            (map units per real-time second)
close_speed: i32           (map units per real-time second)
num_vertices: u16
num_faces: u16
num_sectors: u16           (skip)
num_offsets: u16
state: u16                 (runtime, default Closed)
padding: u16
```

Followed by flat arrays per door (sizes from the counts above):
- `vertex_ids: [u16; num_vertices]` — indices into `blv.vertices`
- `face_ids: [u16; num_faces]` — indices into `blv.faces`
- `sector_ids: [u16; num_sectors]` — skip
- `delta_us: [i16; num_faces]` — initial texture U deltas per face
- `delta_vs: [i16; num_faces]` — initial texture V deltas per face
- `x_offsets: [i16; num_offsets]` — base X position per vertex
- `y_offsets: [i16; num_offsets]` — base Y position per vertex
- `z_offsets: [i16; num_offsets]` — base Z position per vertex

Store as `Vec<BlvDoor>` on the `Blv` struct.

### EVT Opcode 0x0F — SetDoorState

Add to `GameEvent` enum:
```rust
SetDoorState { door_id: u8, action: u8 }
```
Where action: 0=Open, 1=Close, 2=Toggle.

Parse from EVT: `params[0]` = door_id, `params[4]` = action (based on OpenEnroth's EVT processor).

## Mesh Spawning

### Face Classification

During the loading pipeline, split BLV faces into:

1. **Static faces** — `!moves_by_door() && !is_clickable()` — batched by texture (existing `textured_meshes`)
2. **Door faces** — `moves_by_door()` — one entity per face with `DoorFace` component
3. **Clickable static faces** — `is_clickable() && !moves_by_door()` — batched for rendering, but geometry stored separately for ray intersection

Add `is_clickable()` helper: `(attributes & 0x02000000) != 0`.

### Door Face Entities

Each door face becomes its own Bevy entity with:
- `Mesh3d` + `MeshMaterial3d` (textured like static faces)
- `DoorFace { door_index: usize, face_index: usize }` component
- Mesh handle stored so the animation system can update vertex positions

### Clickable Face Registry

A `ClickableFaces` resource stores geometry for non-door clickable faces:
```rust
struct ClickableFaceInfo {
    face_index: usize,
    event_id: u16,
    normal: Vec3,       // Bevy coords
    distance: f32,      // plane distance
    vertices: Vec<Vec3>, // Bevy coords, for point-in-polygon test
}
```

## Interaction & Raycasting

### Indoor Face Picking

When the player clicks or presses Enter while indoors (`GameState::Game`, `HudView::World`):

1. Cast a ray from camera position along camera forward
2. Test against all door face entities (Bevy mesh intersection or AABB + plane test)
3. Test against stored `ClickableFaces` geometry (ray-plane + point-in-polygon)
4. Take nearest hit within `RAYCAST_RANGE` (2000 units)
5. Look up the hit face's `event_id`
6. Push all EVT actions for that event_id into `EventQueue`

This reuses the existing `check_interact_input` helper (Enter/click/gamepad) and `EventQueue` dispatch.

### SetDoorState Event Handler

In `event_dispatch.rs`, handle `GameEvent::SetDoorState`:
- Look up door by `door_id` in a `BlvDoors` resource
- Apply state transition:
  - Toggle: Open→Closing, Closed→Opening. Mid-animation: reverse with proportional time.
  - Open: if Closed→Opening, if Closing→Opening (proportional)
  - Close: if Open→Closing, if Opening→Closing (proportional)
- Reset `time_since_triggered` to 0 (or proportional remainder)

## Door Animation

### State Machine

```
enum DoorState { Open, Opening, Closed, Closing }
```

Four states with linear interpolation:
- **Opening**: `distance = move_length - (elapsed_ms * open_speed / 1000)`, completes at distance=0
- **Closing**: `distance = elapsed_ms * close_speed / 1000`, completes at distance=move_length
- **Open**: distance=0 (static)
- **Closed**: distance=move_length (static)

### Animation System

Runs every frame in `Update` for `GameState::Game`:

1. For each door in Opening/Closing state:
   - Advance `time_since_triggered` by `time.delta_secs() * 1000.0`
   - Calculate distance, clamp to [0, move_length]
   - If at limit, transition to Open/Closed
2. For each `DoorFace` belonging to an animating door:
   - Recompute vertex positions: `offset[i] + direction * distance` (in MM6 coords, then convert to Bevy)
   - Update the entity's `Mesh` vertex positions
   - If face has `moves_by_door` flag, recalculate texture UV offsets

### Vertex Position Formula

```
vertex[i].x = door.x_offsets[i] + door.direction.x * distance
vertex[i].y = door.y_offsets[i] + door.direction.y * distance
vertex[i].z = door.z_offsets[i] + door.direction.z * distance
```

Then convert to Bevy coordinates with `mm6_to_bevy()`.

## Files to Modify

### lod crate
- `blv.rs` — Parse face extras event_id, parse door data blob, add `BlvDoor` struct, add `is_clickable()`, exclude door faces from `textured_meshes`, add door face mesh generation
- `evt.rs` — Add `SetDoorState` variant to `GameEvent`, parse opcode 0x0F

### openmm crate
- `game/blv.rs` — Spawn door face entities, create `BlvDoors` resource, create `ClickableFaces` resource, add door animation system, add indoor face picking system
- `game/event_dispatch.rs` — Handle `SetDoorState` event
- `game/interaction.rs` — Add indoor picking path (or integrate into blv.rs)
- `states/loading.rs` — Pass door data and clickable face data through `PreparedIndoorWorld`
