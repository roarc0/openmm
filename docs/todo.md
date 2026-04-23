# OpenMM Roadmap & TODOs

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
