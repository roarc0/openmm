# Event Dispatch System Design

## Problem

Event processing is currently embedded in `interaction.rs` — it resolves one background image inline and only handles the visual overlay. There's no event queue, no MoveToMap handling, and no way for events to trigger sub-events. The `EventAction` naming is redundant — rename to `GameEvent`. The `picture_id` field in `HouseEntry` is parsed but unused — it should drive which background image to load instead of the string-matching `building_background()` function.

## Goals

1. Rename `EventAction` to `GameEvent` throughout `openmm-data/src/evt.rs`.
2. An `EventQueue` resource with depth-first sub-event support.
3. A `process_events` system dispatching each event type to its handler.
4. **MoveToMap** actually transitions maps via `LoadRequest` + `GameState::Loading`.
5. **SpeakInHouse** loads background using `picture_id` from `HouseEntry` (not string matching).
6. `interaction.rs` becomes trigger-only — pushes events, doesn't handle them.

## Design

### Rename: EventAction -> GameEvent

In `openmm-data/src/evt.rs`:

```rust
#[derive(Debug, Clone)]
pub enum GameEvent {
    SpeakInHouse { house_id: u32 },
    MoveToMap { x: i32, y: i32, z: i32, direction: i32, map_name: String },
    OpenChest { id: u8 },
    Hint { str_id: u8, text: String },
}
```

Derive `Clone` (needed for queue). Update `EvtFile` to use `HashMap<u16, Vec<GameEvent>>`.

### EventQueue Resource

```rust
#[derive(Resource, Default)]
pub struct EventQueue {
    queue: VecDeque<GameEvent>,
}

impl EventQueue {
    pub fn push(&mut self, event: GameEvent);
    pub fn push_front(&mut self, event: GameEvent);
    pub fn pop(&mut self) -> Option<GameEvent>;
    pub fn push_all(&mut self, event_id: u16, evt: &EvtFile);
    pub fn is_empty(&self) -> bool;
}
```

`push_all` looks up the event_id in the EvtFile and pushes all actions for that event.

### process_events System

Runs every frame in `GameState::Game`. When `HudView` is not `World`, skips processing (current UI is blocking). Otherwise pops one event per frame:

**Hint { text }** — Sets `FooterText`. Non-blocking.

**SpeakInHouse { house_id }** — Looks up `HouseEntry` from `MapEvents.houses`. Uses `picture_id` to construct the background image name (e.g. `picture_id` 27 -> tries loading icon "evt27" or similar pattern — needs investigation of what picture_id actually maps to in the LOD archives). Falls back to string-matched `building_background()` if picture_id lookup fails. Inserts `OverlayImage`, sets `HudView::Building`, releases cursor. Blocking.

**OpenChest { id }** — Loads "chest01" image, inserts `OverlayImage`, sets `HudView::Chest`, releases cursor. Blocking.

**MoveToMap { x, y, z, direction, map_name }** — Parses `map_name` into `MapName` (outdoor "outXY.odm" or indoor "name.blv"). Inserts `LoadRequest { map_name }`. Updates `GameSave` position/direction (converting MM6 coords to Bevy via `mm6_to_bevy`). Converts MM6 direction (0-65535 = 0-360 degrees) to Bevy yaw radians. Sets `GameState::Loading`.

### MoveToMap Details

The existing map transition machinery:
- `LoadRequest` resource tells `loading_setup` which map to load (takes priority over save data)
- `GameSave.map` stores `MapState { map_x, map_y }` for outdoor maps
- `GameSave.player` stores `PlayerState { position, yaw }`
- `MapName::try_from(str)` parses "outXY.odm" into `Outdoor(OdmName)` or "name.blv" into `Indoor(String)`

The dispatch handler:
1. Parse `map_name` into `MapName` via `MapName::try_from()`
2. Insert `LoadRequest { map_name }`
3. Convert MM6 position (x, y, z) to Bevy coords via `openmm_data::odm::mm6_to_bevy(x, y, z)`
4. Convert MM6 direction (0-65535) to Bevy yaw: `direction as f32 / 65536.0 * TAU`
5. Update `GameSave.player.position` and `GameSave.player.yaw`
6. For outdoor maps, update `GameSave.map.map_x` and `map_y` from the OdmName
7. Set `NextState(GameState::Loading)`

### interaction.rs Refactor

Becomes trigger-only:

```rust
// interact_system: on E key / click near building
for &eid in &info.event_ids {
    event_queue.push_all(eid, evt);
}
```

Functions removed: `resolve_image()`, `building_background()`, all HudView/OverlayImage/cursor management.

Functions kept: `interact_system` (push events), `interaction_input` (exit Building/Chest views), `hover_hint_system`, `find_nearest_building`, `resolve_building_name`.

### Module Structure

```
game/
  event_dispatch.rs  — EventQueue, EventDispatchPlugin, process_events
  interaction.rs     — InteractionPlugin (trigger only)
```

### picture_id Investigation

The `HouseEntry.picture_id` likely maps to an icon in the LOD archive. Common patterns in MM6:
- Direct numeric: icon name is `"evt{picture_id:02}"` (e.g. picture_id=2 -> "evt02")
- Or it's an index into a separate lookup table

The implementation should try loading `format!("evt{:02}", picture_id)` as an icon, falling back to the current string-matching `building_background()` if not found. Log the picture_id so we can verify the mapping.

## Non-Goals

- Full building/shop UI (item grids, NPC portraits, buy/sell menus)
- Chest item contents
- Inventory/Stats/Rest UIs
- Conditional event scripting (Compare/GoTo/Set opcodes)
- Sound playback (PlaySound opcode)
