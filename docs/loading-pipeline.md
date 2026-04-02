# Loading Pipeline

## Triggering a Load

Set `LoadRequest` resource fields before transitioning to `GameState::Loading`:
- `map_name: MapName` — the map to load
- `spawn_position: Option<[i32; 3]>` — override spawn position (used by `MoveToMap` events)
- `spawn_yaw: Option<i32>` — override spawn yaw angle

## Pipeline Steps

`ParseMap → BuildTerrain → BuildAtlas → BuildModels → BuildBillboards → PreloadSprites → Done`

Each step runs in a single frame. `LoadingProgress` is a private resource that carries intermediate data between steps; it is discarded once the final prepared world resource is produced.

## Cleanup on Load

`loading_setup` explicitly removes all per-map resources before loading starts:
`PreparedWorld`, `PreparedIndoorWorld`, `BlvDoors`, `DoorColliders`, `ClickableFaces`, `TouchTriggerFaces`

**If you add a new per-map resource, add it to this cleanup list** or it will persist across map changes.

## Prepared World Resources

**`PreparedWorld`** (outdoor maps): contains `Odm`, terrain/water meshes and textures, models, decorations, actors, resolved monsters, start_points, sprite_cache, billboard_cache, water_cells, terrain_lookup, music_track.

**`PreparedIndoorWorld`** (indoor maps): contains models, start_points, collision geometry (walls/floors/ceilings), door definitions, door face meshes, clickable_faces, touch_trigger_faces, map_base string for EVT loading, actors from DLV.

## Touch Trigger Faces

`TouchTriggerFaces` resource holds faces flagged `EVENT_BY_TOUCH` — these fire EVT events when the player walks within proximity. Each entry has: face_index, event_id, center, radius (half the bounding box diagonal for floor faces).

## Sprite Preloading

`PreloadQueue` batches sprite preloading across frames to avoid hitching:
- Queues `(root, variant, palette_id)` triples
- Also resolves billboard textures and map music track

## Spawn Position Priority

**Indoor maps:** `start_points[0]` — set from `LoadRequest.spawn_position` for `MoveToMap` events, or sector center as fallback.

**Outdoor maps:** non-zero save position → decoration named `"party start"` / `"party_start"` → world origin.
