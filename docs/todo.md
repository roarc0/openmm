# OpenMM Roadmap & TODOs

## Save System

### Phase 1 — Load from .mm6 ✅
- [x] Parse party.bin (position, calendar, gold/food, quest bits, characters)
- [x] Parse header.bin (map name, save name)
- [x] Parse clock.bin (passthrough for round-trip)
- [x] ActiveSave resource replaces JSON GameSave
- [x] Centralized state sync: save → WorldState + Party + GameTime (via `populate_state_from_save()`)
- [x] DDM/DLV loading from save file (killed monsters persist)
- [x] NewGame creates save from new.lod template

### Phase 2 — Save to .mm6
- [ ] Collect runtime state back into SaveParty/SaveHeader
- [ ] WorldState → party.bin (position, calendar, gold, quest bits)
- [ ] Party → party.bin characters (HP, SP, skills, experience)
- [ ] GameTime → party.bin calendar fields
- [ ] Snapshot current map DDM (actor state: dead, position, HP)
- [ ] LodWriter::patch() to write modified .mm6 file
- [ ] Autosave on map transition (rotate autosave1→autosave6)
- [ ] Quicksave console command
- [ ] Sync dead_actor_ids from DDM actor ai_state on map exit

### Phase 3 — Save/Load UI
- [ ] Save slot selection UI
- [ ] Screenshot capture (viewport only, no HUD) → PCX encoding
- [ ] Load game from UI (slot list with screenshot previews)
- [ ] Named saves (user-chosen slot names)
- [ ] Save slot display: name, map, date, screenshot thumbnail

### Phase 4 — Full State Round-Trip
- [ ] npcdata.bin parse/serialize (NPC roster state)
- [ ] overlay.bin parse/serialize (active spell overlays)
- [ ] Party creation → override party.bin in save before loading
- [ ] Map state reset after N game months (respawn killed monsters)
- [ ] Multiple save profiles
- [ ] Autosave count configurable via openmm.toml

### Known Format Details
- party.bin = 64720 byte memory dump. Base addr 0x908C70.
- Character struct: 0x161C (5660) bytes, 4 chars at offset 0x02C4
- Calendar: 28-day months, 12 months/year, 336 days/year
- MM6 direction: 0-2047, 0=east, 512=north, 1024=west, 1536=south
- Skills: u8[31] (MM6), different from MM7 i16[37]
- Conditions: i64[17] timestamps (MM6 has 17 vs MM7 20)

## Gameplay (Priority)
- [ ] **Monster combat stats** — HP, speed, attack logic
- [ ] **Ground items / pickable objects** — Parse DDM `MapObject`, spawn `GroundItem` entities
- [ ] **Chest / item system** — Items inside chests, inventory logic
- [ ] **Save / load** — Full round-trip persistence for party and map state
- [ ] **NPC time-of-day schedules** — AI schedule-following system
- [ ] **Faction and diplomacy** — Diplomacy table, aggression logic
- [ ] **Random encounters** — Camping interrupt monster spawns
- [ ] **Actor-actor collision** — Prevent actors from occupying the same XZ space. Requires an O(n²) spatial query (or spatial grid) per frame across all `Actor` entities; skip dead/flying actors. Not yet implemented — player-vs-actor collision is done via `WorldObstacle`, but actor-to-actor pushout needs a dedicated system.

## Rendering & Visual
- [ ] **Texture animation (TFT)** — Terrain shader animated tile cycling
- [ ] **Sky texture variation** — Day/night sky texture swapping
- [ ] **Indoor minimap** — Minimap logic for BLV maps (only ODM handled now)
- [ ] **HUD GameTime integration** — Tap frame switching (morning/day/evening/night), date/time text
- [ ] **Post-process AA fix** — FXAA/SMAA/TAA break terrain rendering

## Package Restructuring
- [ ] **Scripting handlers**: Extract handler groups from `events/scripting/dispatch.rs` (665 lines) into focused modules as they grow beyond stubs
- [ ] **Consolidate Geometries**: Move `collision.rs` 2D helpers + `interaction/raycast.rs` 3D equivalents → `game/map/geom.rs`
- [ ] **Modular HUD**: Split `hud/mod.rs` into `layout.rs`, `builder.rs`, `constants.rs`

## Refactoring
- [ ] **Split `sprites/loading.rs` (857 lines)** — Animation runtime (`update_sprite_sheets`) is different concern from load-time decoding. Split into `sprites/decode.rs` + `sprites/animation.rs`.
- [ ] **`handle_move_to_map` split** — `event_handlers.rs` handles same-map teleport AND cross-map transition in one function. Separate them.

## Architecture (Bevy Best Practices)
*Note: identified improvements, not immediate priority.*

### Systems & Scheduling
- [ ] **Deconstruct `process_events`** — dispatch.rs match arms are mostly 1-5 line stubs now but will grow as features land. Convert heavy side-effects to Bevy events consumed by specialized handler systems.
- [ ] **Deconstruct `player.rs` systems** — Split into "Input Capture" (keys → Intent/Actions) and "Kinematics/Physics" (Actions → Transform).
- [ ] **Migrate physics to `FixedUpdate`** — `gravity_system` and `player_movement` run in `Update`. Use `Time<Fixed>` for determinism.
- [ ] **Action-based input** — Replace direct `ButtonInput<KeyCode>` polling with `PlayerAction` enum mapping.

### Resources & Components
- [ ] **Split `WorldState` bag-of-state** — Combines player runtime, map runtime, quest bits, inventory, NPC overrides, actor flags, chest flags. Split into focused resources.
- [ ] **Group player config** — Consolidate `PlayerSettings`, `PlayerKeyBindings`, `MouseLookEnabled`, `MouseSensitivity` into single resource or entity components.
- [ ] **Decouple `spawn_player`** — God function for player + camera + fog + lighting. Split into modular setup systems.

### Cleanups
- [ ] **`update_hud_layout` 8-way `ParamSet`** — Replace with tag enum/component; gate on `Changed<Window>`.
- [ ] **HUD overlays spawned/despawned instead of toggled** — Spawn once, flip `Visibility`.
- [ ] **Sprite sheet dimension lookup** — `(w,h)` repeated on every interaction query. Cache as component.
- [ ] **Introduce `CurrentEnvironment` resource** — Replaces scattered `resource_exists::<PreparedWorld>` checks.
- [ ] **NPC dialogue string cloning** — Use `&str`/`Cow` from parsed tables instead of eager clone.
- [ ] **Change detection** — Use `Changed<T>` and `Added<T>` filters more broadly.

## Screen Editor
- [ ] **Inline text element editing** — Floating edit panel on canvas for Text elements
- [ ] **Consolidate inspector + event editor** — Two code paths for same data = drift risk
- [ ] **Element ID validation** — No collision detection on element IDs; dangling ShowSprite/HideSprite references
- [ ] **Texture name validation** — Validate texture names exist in LOD archives at edit time
- [ ] **Editor undo/redo** — Command-based undo stack

## Screen Runtime
- [ ] **Text source registry** — `text_update()` hardcodes source names. Move to registry/dispatch.
- [ ] **Condition evaluation for element states** — `ElementState.condition` stored but never evaluated
- [ ] **Texture asset pooling** — Same texture loaded multiple times. Implement dedup.
- [ ] **Action string variables** — Support `$variable` syntax in RON action strings (e.g. `IncrementStat($active_stat)`) resolved before execution. Would let RON files reference runtime state directly, reducing hardcoded action variants.

## Other
- [ ] **Monster aggro range verification** — Cross-check hostile_type 4 range (6656.0)
- [ ] **Street NPC randomization** — Seed identity from per-load RNG
- [ ] **World variable ops** — Move read/write helpers out of dispatcher to dedicated module
