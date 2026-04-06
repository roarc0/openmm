# Event Dispatch System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an event dispatch system that routes game events (building interactions, map transitions, chests, hints) through a queue processed one-per-frame, replacing the inline handling in interaction.rs.

**Architecture:** Rename `EventAction` to `GameEvent` in the openmm-data crate. Add `EventQueue` resource and `process_events` system in a new `event_dispatch.rs`. Refactor `interaction.rs` to push events instead of handling them. MoveToMap uses the existing `LoadRequest` + `GameState::Loading` pipeline.

**Tech Stack:** Rust, Bevy 0.18 ECS, openmm-data crate

---

## File Structure

```
openmm-data/src/evt.rs                     — Rename EventAction -> GameEvent, derive Clone
openmm/src/game/event_dispatch.rs  — EventQueue, EventDispatchPlugin, process_events, building_background
openmm/src/game/interaction.rs     — Simplified: trigger-only (push to queue), exit input, hover hints
openmm/src/game/mod.rs             — Register EventDispatchPlugin
```

---

### Task 1: Rename EventAction to GameEvent in openmm-data crate

**Files:**
- Modify: `openmm-data/src/evt.rs`

- [ ] **Step 1: Rename the enum and update all internal references**

In `openmm-data/src/evt.rs`, rename `EventAction` to `GameEvent`. The enum already derives `Debug, Clone` from the previous work. Update the `EvtFile` type alias and `primary_action` method:

```rust
// Line 22: rename enum
#[derive(Debug, Clone)]
pub enum GameEvent {
    SpeakInHouse { house_id: u32 },
    MoveToMap { x: i32, y: i32, z: i32, direction: i32, map_name: String },
    OpenChest { id: u8 },
    Hint { str_id: u8, text: String },
}

// Line 42: update HashMap type
pub events: HashMap<u16, Vec<GameEvent>>,

// Lines 96-138: update all match arms from EventAction:: to GameEvent::
// Line 100: Some(GameEvent::SpeakInHouse { ... })
// Line 114: Some(GameEvent::Hint { ... })
// Line 127: Some(GameEvent::MoveToMap { ... })
// Line 134: Some(GameEvent::OpenChest { ... })

// Lines 152-155: update primary_action
pub fn primary_action(&self, event_id: u16) -> Option<&GameEvent> {
    self.events.get(&event_id)?.iter().find(|a| matches!(a,
        GameEvent::SpeakInHouse { .. } | GameEvent::MoveToMap { .. }
    ))
}
```

- [ ] **Step 2: Update all references in openmm crate**

In `openmm/src/game/interaction.rs`, update `resolve_building_name` which references `openmm_data::evt::EventAction`:

```rust
// Lines 245, 248, 256, 261: change EventAction:: to GameEvent::
openmm_data::evt::GameEvent::OpenChest { id } => { ... }
openmm_data::evt::GameEvent::SpeakInHouse { house_id } => { ... }
openmm_data::evt::GameEvent::Hint { text, .. } => { ... }
openmm_data::evt::GameEvent::MoveToMap { map_name, .. } => { ... }
```

Also in `resolve_image` (lines 129-142), change all `EventAction::` to `GameEvent::`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | grep "^error" | head -10`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add openmm-data/src/evt.rs openmm/src/game/interaction.rs
git commit --no-gpg-sign -m "rename EventAction to GameEvent throughout codebase"
```

---

### Task 2: Create event_dispatch.rs with EventQueue and process_events

**Files:**
- Create: `openmm/src/game/event_dispatch.rs`
- Modify: `openmm/src/game/mod.rs`

- [ ] **Step 1: Create event_dispatch.rs**

```rust
use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use openmm_data::evt::{EvtFile, GameEvent};
use openmm_data::odm::mm6_to_bevy;

use crate::GameState;
use crate::assets::GameAssets;
use crate::game::events::MapEvents;
use crate::game::hud::{HudView, OverlayImage};
use crate::game::map_name::MapName;
use crate::save::GameSave;
use crate::states::loading::LoadRequest;

/// Queue of game events to process sequentially, one per frame.
#[derive(Resource, Default)]
pub struct EventQueue {
    queue: VecDeque<GameEvent>,
}

impl EventQueue {
    /// Push an event to the back of the queue.
    pub fn push(&mut self, event: GameEvent) {
        self.queue.push_back(event);
    }

    /// Push an event to the front (for sub-events during processing).
    pub fn push_front(&mut self, event: GameEvent) {
        self.queue.push_front(event);
    }

    /// Pop the next event to process.
    pub fn pop(&mut self) -> Option<GameEvent> {
        self.queue.pop_front()
    }

    /// Push all actions for an event ID from the evt file.
    pub fn push_all(&mut self, event_id: u16, evt: &EvtFile) {
        if let Some(actions) = evt.events.get(&event_id) {
            for action in actions {
                self.queue.push_back(action.clone());
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

pub struct EventDispatchPlugin;

impl Plugin for EventDispatchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EventQueue>()
            .add_systems(
                Update,
                process_events
                    .run_if(in_state(GameState::Game)),
            );
    }
}

/// Map a building type string to its background image name.
fn building_background(building_type: &str) -> &'static str {
    let lower = building_type.to_lowercase();
    if lower.contains("weapon") { return "wepntabl"; }
    if lower.contains("armor") { return "armory"; }
    if lower.contains("magic") || lower.contains("guild") || lower.contains("alchemy") { return "magshelf"; }
    if lower.contains("general") || lower.contains("store") { return "genshelf"; }
    "evt02"
}

/// Load an icon image from the LOD archive as a Bevy Image handle.
fn load_icon(name: &str, game_assets: &GameAssets, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let img = game_assets.lod_manager().icon(name)?;
    let mut bevy_img = crate::assets::dynamic_to_bevy_image(img);
    bevy_img.sampler = bevy::image::ImageSampler::nearest();
    Some(images.add(bevy_img))
}

/// Try loading background image by picture_id first, fall back to building_type string match.
fn resolve_building_image(
    house_id: u32,
    map_events: &MapEvents,
    game_assets: &GameAssets,
    images: &mut Assets<Image>,
) -> Option<Handle<Image>> {
    let houses = map_events.houses.as_ref()?;
    let entry = houses.houses.get(&house_id);

    // Try picture_id first: "evt{id:02}" pattern
    if let Some(entry) = entry {
        if entry.picture_id > 0 {
            let pic_name = format!("evt{:02}", entry.picture_id);
            if let Some(handle) = load_icon(&pic_name, game_assets, images) {
                info!("Loaded building image '{}' from picture_id={}", pic_name, entry.picture_id);
                return Some(handle);
            }
        }
        // Fall back to building_type string match
        let bg_name = building_background(&entry.building_type);
        if let Some(handle) = load_icon(bg_name, game_assets, images) {
            return Some(handle);
        }
    }

    // Last resort fallback
    load_icon("evt02", game_assets, images)
}

fn grab_cursor(cursor_query: &mut Query<&mut CursorOptions, With<PrimaryWindow>>, grab: bool) {
    if let Ok(mut cursor) = cursor_query.single_mut() {
        if grab {
            cursor.grab_mode = CursorGrabMode::Confined;
            cursor.visible = false;
        } else {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    }
}

/// Process one event per frame from the queue.
fn process_events(
    mut queue: ResMut<EventQueue>,
    mut view: ResMut<HudView>,
    mut footer: ResMut<crate::game::hud::FooterText>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    game_assets: Res<GameAssets>,
    map_events: Option<Res<MapEvents>>,
    mut save_data: ResMut<GameSave>,
    mut game_state: ResMut<NextState<GameState>>,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    // Don't process while a UI view is blocking
    if *view != HudView::World {
        return;
    }

    let Some(event) = queue.pop() else { return };

    match event {
        GameEvent::Hint { text, .. } => {
            footer.set(&text);
        }

        GameEvent::SpeakInHouse { house_id } => {
            if let Some(ref me) = map_events {
                if let Some(handle) = resolve_building_image(house_id, me, &game_assets, &mut images) {
                    info!("SpeakInHouse: house_id={}", house_id);
                    commands.insert_resource(OverlayImage { image: handle });
                    *view = HudView::Building;
                    grab_cursor(&mut cursor_query, false);
                }
            }
        }

        GameEvent::OpenChest { id } => {
            if let Some(handle) = load_icon("chest01", &game_assets, &mut images) {
                info!("OpenChest: id={}", id);
                commands.insert_resource(OverlayImage { image: handle });
                *view = HudView::Chest;
                grab_cursor(&mut cursor_query, false);
            }
        }

        GameEvent::MoveToMap { x, y, z, direction, map_name } => {
            info!("MoveToMap: map='{}' pos=({},{},{}) dir={}", map_name, x, y, z, direction);

            // Parse map name into MapName
            let Ok(target_map) = MapName::try_from(map_name.as_str()) else {
                warn!("Failed to parse map name: '{}'", map_name);
                return;
            };

            // Convert MM6 coords to Bevy
            let bevy_pos = mm6_to_bevy(x, y, z);
            save_data.player.position = bevy_pos;

            // Convert MM6 direction (0-65535 = full circle) to Bevy yaw radians
            // MM6 direction: 0 = east, increases counterclockwise
            // Bevy yaw: 0 = north (negative Z), increases clockwise
            let mm6_angle_rad = (direction as f32 / 65536.0) * std::f32::consts::TAU;
            // MM6 0=east -> Bevy: east is yaw=-PI/2, and MM6 goes CCW while Bevy goes CW
            let bevy_yaw = -(mm6_angle_rad - std::f32::consts::FRAC_PI_2);
            save_data.player.yaw = bevy_yaw;

            // Update map state for outdoor maps
            if let MapName::Outdoor(ref odm) = target_map {
                save_data.map.map_x = odm.x;
                save_data.map.map_y = odm.y;
            }

            // Insert LoadRequest and transition to Loading
            commands.insert_resource(LoadRequest { map_name: target_map });
            game_state.set(GameState::Loading);
        }
    }
}
```

- [ ] **Step 2: Register EventDispatchPlugin in game/mod.rs**

Add the module declaration and plugin registration:

```rust
// Add near the top with other module declarations:
pub(crate) mod event_dispatch;

// In InGamePlugin::build, add the plugin:
app.add_plugins((
    world::WorldPlugin,
    player::PlayerPlugin,
    physics::PhysicsPlugin,
    odm::OdmPlugin,
    blv::BlvPlugin,
    entities::EntitiesPlugin,
    debug::DebugPlugin,
    hud::HudPlugin,
    interaction::InteractionPlugin,
    event_dispatch::EventDispatchPlugin,  // <-- add this
    MaterialPlugin::<terrain_material::TerrainMaterial>::default(),
))
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | grep "^error" | head -10`
Expected: no errors (warnings about unused EventQueue are fine — interaction.rs doesn't push to it yet)

- [ ] **Step 4: Commit**

```bash
git add openmm/src/game/event_dispatch.rs openmm/src/game/mod.rs
git commit --no-gpg-sign -m "add EventQueue and process_events dispatch system"
```

---

### Task 3: Refactor interaction.rs to push events to EventQueue

**Files:**
- Modify: `openmm/src/game/interaction.rs`

- [ ] **Step 1: Update imports and remove old functions**

Replace the imports at the top:

```rust
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::GameState;
use crate::game::events::MapEvents;
use crate::game::event_dispatch::EventQueue;
use crate::game::hud::{FooterText, HudView};
use crate::game::player::{Player, PlayerCamera};
```

Remove these functions entirely:
- `building_background()` (moved to event_dispatch.rs)
- `resolve_image()` (replaced by event_dispatch.rs handlers)
- `grab_cursor()` (moved to event_dispatch.rs)

Remove the `use crate::assets::GameAssets;` import (no longer needed).
Remove the `use crate::game::hud::OverlayImage;` import (no longer needed).

- [ ] **Step 2: Update InteractionPlugin to handle Building AND Chest exit**

The `interaction_input` system needs to run for both Building and Chest views:

```rust
impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (hover_hint_system, interact_system)
                .chain()
                .run_if(in_state(GameState::Game))
                .run_if(resource_equals(HudView::World)),
        )
        .add_systems(
            Update,
            interaction_input
                .run_if(in_state(GameState::Game))
                .run_if(|view: Res<HudView>| matches!(*view, HudView::Building | HudView::Chest)),
        );
    }
}
```

- [ ] **Step 3: Simplify interact_system to push events**

Replace the entire `interact_system` function:

```rust
fn interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    player_query: Query<&Transform, With<Player>>,
    camera_query: Query<(&GlobalTransform, &Camera), With<PlayerCamera>>,
    buildings: Query<(&BuildingInfo, &GlobalTransform)>,
    map_events: Option<Res<MapEvents>>,
    mut event_queue: ResMut<EventQueue>,
    cursor_query: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(player_tf) = player_query.single() else { return };
    let Ok((cam_global, _)) = camera_query.single() else { return };

    let (key, click, gamepad) = check_interact_input(&keys, &mouse, &gamepads);
    if !key && !click && !gamepad { return; }

    let cursor_grabbed = cursor_query.single()
        .map(|c| !matches!(c.grab_mode, CursorGrabMode::None)).unwrap_or(true);
    if click && !cursor_grabbed { return; }

    let use_raycast = click || gamepad;
    let Some(info) = find_nearest_building(player_tf.translation, cam_global, &buildings, use_raycast) else {
        return;
    };

    // Push all event actions for this building onto the queue
    let Some(me) = map_events else { return };
    let Some(evt) = me.evt.as_ref() else { return };

    for &eid in &info.event_ids {
        event_queue.push_all(eid, evt);
    }

    if !event_queue.is_empty() {
        info!("Queued events for '{}' event_ids={:?}", info.model_name, info.event_ids);
    }
}
```

- [ ] **Step 4: Update interaction_input to use event_dispatch's pattern**

The exit input now needs to handle cursor grab itself since `grab_cursor` was moved. Add a local copy or just inline it:

```rust
fn interaction_input(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut view: ResMut<HudView>,
    mut commands: Commands,
    mut cursor_query: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if check_exit_input(&keys, &gamepads) {
        commands.remove_resource::<crate::game::hud::OverlayImage>();
        *view = HudView::World;
        if let Ok(mut cursor) = cursor_query.single_mut() {
            cursor.grab_mode = CursorGrabMode::Confined;
            cursor.visible = false;
        }
    }
}
```

- [ ] **Step 5: Update resolve_building_name to use GameEvent**

The `resolve_building_name` function references `openmm_data::evt::GameEvent` (already renamed in Task 1). Make sure the match arms use `GameEvent::`:

```rust
fn resolve_building_name(info: &BuildingInfo, map_events: &Option<Res<MapEvents>>) -> Option<String> {
    let me = map_events.as_ref()?;
    let evt = me.evt.as_ref()?;

    for &eid in &info.event_ids {
        if let Some(actions) = evt.events.get(&eid) {
            for action in actions {
                match action {
                    openmm_data::evt::GameEvent::OpenChest { id } => {
                        return Some(format!("Chest #{}", id));
                    }
                    openmm_data::evt::GameEvent::SpeakInHouse { house_id } => {
                        if let Some(houses) = me.houses.as_ref() {
                            if let Some(entry) = houses.houses.get(house_id) {
                                return Some(entry.name.clone());
                            }
                        }
                        return Some(format!("Building #{}", house_id));
                    }
                    openmm_data::evt::GameEvent::Hint { text, .. } => {
                        if !text.is_empty() {
                            return Some(text.clone());
                        }
                    }
                    openmm_data::evt::GameEvent::MoveToMap { map_name, .. } => {
                        return Some(format!("Enter {}", map_name));
                    }
                }
            }
        }
    }
    None
}
```

- [ ] **Step 6: Verify it compiles and runs**

Run: `cargo build 2>&1 | grep "^error" | head -10`
Expected: no errors

Then test manually:
- Walk near a building, press E — should show overlay image (routed through EventQueue now)
- Press ESC — should return to world
- Walk near a map transition trigger — should transition to new map (if any MoveToMap events exist on outdoor buildings)

- [ ] **Step 7: Commit**

```bash
git add openmm/src/game/interaction.rs
git commit --no-gpg-sign -m "refactor interaction.rs to push events to EventQueue instead of handling inline"
```

---

### Task 4: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add event_dispatch.rs to the architecture tree**

In the `game/` section of the openmm crate structure, add after `interaction.rs`:

```
    event_dispatch.rs  — EventDispatchPlugin, EventQueue, process_events system
```

- [ ] **Step 2: Add event dispatch documentation to conventions**

Add a new section after "HUD views":

```
### Event dispatch

- `GameEvent` enum in `openmm_data::evt` — renamed from EventAction: SpeakInHouse, MoveToMap, OpenChest, Hint
- `EventQueue` resource — any system can push events, processed one per frame by `process_events`
- Sub-events use `push_front()` for depth-first processing
- UI-opening events (SpeakInHouse, OpenChest) block the queue until HudView returns to World
- MoveToMap uses `LoadRequest` + `GameState::Loading` pipeline (same as boundary crossing and debug map switch)
- `interaction.rs` is trigger-only — detects player interaction, pushes events to queue
- `event_dispatch.rs` handles all event logic (image loading, view switching, map transitions)
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit --no-gpg-sign -m "docs: add event dispatch system to CLAUDE.md"
```
