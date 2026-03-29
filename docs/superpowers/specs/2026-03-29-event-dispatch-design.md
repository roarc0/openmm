# Event Dispatch System Design

## Problem

Event processing is currently embedded in `interaction.rs` — it resolves images inline and only handles the visual overlay. There's no way to process event chains (one event triggering another), handle MoveToMap transitions, or dispatch different event types to different handlers. Each event type needs its own handling logic, and future UI modules need a clean interface to plug into.

## Goals

1. An `EventQueue` resource that any system can push events onto.
2. A `process_events` system that drains the queue one action per frame, dispatching to the correct handler.
3. Event processing is sequential and blocking — when a UI-opening event is active, no new events process until the view returns to World.
4. Sub-events pushed during processing go to the front of the queue (depth-first).
5. `interaction.rs` becomes a trigger only — pushes event actions onto the queue instead of handling them directly.
6. MoveToMap actually transitions maps using the existing Loading pipeline.

## Design

### EventQueue Resource

```rust
#[derive(Resource, Default)]
pub struct EventQueue {
    queue: VecDeque<EventAction>,
}

impl EventQueue {
    /// Push an event to the back of the queue.
    pub fn push(&mut self, action: EventAction);

    /// Push an event to the front (for sub-events triggered during processing).
    pub fn push_front(&mut self, action: EventAction);

    /// Pop the next event to process.
    pub fn pop(&mut self) -> Option<EventAction>;

    /// Push all actions from an event ID (looks up the evt file).
    pub fn push_event(&mut self, event_id: u16, evt: &EvtFile);

    pub fn is_empty(&self) -> bool;
}
```

### process_events System

Runs every frame in `GameState::Game`. Pops one action per frame from the queue and dispatches:

- **Hint { text }** — Sets `FooterText` with the hint text. Non-blocking, continues to next event next frame.
- **SpeakInHouse { house_id }** — Looks up house in `MapEvents.houses`, loads background image via `building_background()`, inserts `OverlayImage`, sets `HudView::Building`, releases cursor. Blocking — queue pauses until `HudView` returns to `World`.
- **OpenChest { id }** — Loads "chest01" image, inserts `OverlayImage`, sets `HudView::Chest`, releases cursor. Blocking — queue pauses until `HudView` returns to `World`.
- **MoveToMap { x, y, z, direction, map_name }** — Updates `GameSave` with new position, direction, and map name. Transitions `GameState` to `Loading`. The existing loading pipeline reads `GameSave` to determine which map to load and where to place the player.

When `HudView` is not `World`, the system skips processing (the current UI-opening event is still active). Processing resumes when the view returns to `World`.

### interaction.rs Refactor

The interaction system no longer resolves images or manages HudView/OverlayImage directly. Instead:

1. Player triggers interaction (E key / click near building).
2. `interact_system` looks up the building's event IDs in the evt file.
3. For each event ID, pushes all actions onto `EventQueue` via `push_event()`.
4. `process_events` handles the rest.

Functions removed from interaction.rs:
- `resolve_image()` — moves to event_dispatch.rs
- `building_background()` — moves to event_dispatch.rs
- All `HudView`/`OverlayImage`/cursor management — handled by event_dispatch.rs

Functions kept in interaction.rs:
- `interact_system` — detects interaction trigger, pushes to EventQueue
- `interaction_input` — handles exit from Building/Chest views (ESC/E)
- `hover_hint_system` — footer text for nearby buildings
- `find_nearest_building` — proximity/raycast detection
- `resolve_building_name` — name lookup for footer hints

### MoveToMap Flow

1. `process_events` receives `MoveToMap { x, y, z, direction, map_name }`.
2. Updates `GameSave.map_name` to the new map.
3. Updates `GameSave.player_position` with the MM6 coordinates converted to Bevy.
4. Updates `GameSave.player_direction` with the yaw angle.
5. Sets `GameState::Loading` via `NextState`.
6. The existing `LoadingPlugin` reads `GameSave`, loads the new map, spawns the player at the saved position.

Note: `GameSave` already stores position and map name. The loading pipeline already reads it. We just need to write the new values before transitioning.

### Module Structure

```
game/
  event_dispatch.rs  — EventQueue, EventDispatchPlugin, process_events, building_background
  interaction.rs     — InteractionPlugin (trigger only: push events, exit input, hover hints)
```

### EventAction Clone

`EventAction` in `lod/src/evt.rs` needs to derive `Clone` so actions can be pushed onto the queue from shared evt data.

## Non-Goals

- Full building/shop UI (separate spec per building type)
- Chest item grid (separate spec)
- Inventory/Stats/Rest UIs (separate specs)
- Event scripting language or conditions (future work)
- Concurrent/parallel event processing
