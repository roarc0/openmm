# OpenMM Roadmap & TODOs

## Gameplay (Priority)
- [ ] **NPC dialogue text rendering** — Wire up font from LOD, scrolling lines
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

## Package Restructuring (Master Plan)
### Phase 1: High-Level Organization
- [x] **Rendering Move** — Move root `engine.rs` → `src/game/rendering/`
- [x] **State Transition Rename** — Rename `src/states/` → `src/prepare/` (Transition/Loading states)

### Phase 2: Game Module Decoupling
- [x] **Simulation State** — Rename `src/game/world/` → `src/game/state/` (Handles Time, Variables, Scripting)
- [x] **Map Umbrella**: Created `src/game/map/` — moved `indoor/`, `outdoor/`, `collision.rs`, `coords.rs`, `spatial_index.rs` into it.
- [x] **Player/Party Umbrella**: Nest `src/game/party/` into `src/game/player/party/`.
- [x] **State Decomposition**: Split `state/` into focused modules:
    - [x] `game/events/` — EVT scripting, event handlers, MapEvents (from `state/scripting.rs`, `state/event_handlers.rs`, `state/events.rs`)
    - [x] `game/ui/` — UiState, UiMode, FooterText, OverlayImage (from `state/ui_state.rs`)
    - [x] `game/actors/npc_dialogue.rs` — NPC dialogue data prep (from `state/npc_dialogue.rs`)
    - `state/` now holds only: WorldState, GameVariables, GameTime, variables
- [ ] **Spawning & Actor Logic**: Create `src/game/spawn/` with subfolders for specialized logic:
    - [ ] `actor/`: Nest all monster/npc logic here (`actors/`)
    - [ ] `decoration/`: Nest prop spawning logic
    - [ ] `sprite/`: Nest world-sprite registration logic (move from `src/game/sprites/`)
- [x] **Input & UI** — Created `src/game/controls/` and moved `input.rs` there.

### Phase 3: Structural Refinements
- [ ] **Scripting Split**: Split `scripting.rs` monolith → `src/game/state/scripting/` (Trigger vs Runtime)
- [ ] **Consolidate Geometries**: Move `collision.rs` 2D helpers + `interaction/raycast.rs` 3D equivalents → `game/map/geom.rs`
- [ ] **Modular HUD**: Split `hud/mod.rs` into `layout.rs`, `builder.rs`, `constants.rs`

## Refactoring (High Impact)
- [x] **Deduplicate actor spawning (3 sites → 1 helper)** — Shared `spawn_actor()` in `game/spawn/monster.rs` covers monsters + NPCs. ~200 lines saved.
- [x] **Deduplicate decoration spawning** ��� Shared `spawn_decoration()` in `game/spawn/decoration.rs`. Indoor gains triggers, flicker, material caching. ~250 lines saved.
- [x] **Split `loading.rs` (1211 lines)** — Split into `loading/mod.rs` + `loading/outdoor.rs` + `loading/indoor.rs` + `loading/helpers.rs`. Extracted `build_textured_mesh()`, `indoor_material()`, `outdoor_material()` helpers.

## Refactoring (Medium Impact)
- [ ] **Split `sprites/loading.rs` (857 lines)** — Animation runtime (`update_sprite_sheets`) is different concern from load-time decoding. Split into `sprites/decode.rs` + `sprites/animation.rs`.
- [ ] **`handle_move_to_map` split** — `event_handlers.rs` handles same-map teleport AND cross-map transition in one function. Separate them.

## Refactoring (Low Impact)
- [x] **Overlay image pattern** — `handle_speak_in_house` and `handle_open_chest` share identical OverlayImage + cursor logic. Extract helper.
- [x] **Indoor sprite caching** — Now uses shared `DecSpriteCache` via `spawn_decoration()`.
- [x] **Texture size collection** — 2 near-identical HashMap-building loops for indoor vs outdoor texture sizes. Extract `collect_texture_sizes()`.

## Architecture (Bevy Best Practices)
*Note: identified improvements, not immediate priority.*

### Systems & Scheduling
- [ ] **Deconstruct `process_events` (scripting.rs)** — ~600 line monolith. Convert side-effects (door state, map transition, overlay, sound) to events consumed by specialized handlers.
- [ ] **Deconstruct `player.rs` systems** — Split into "Input Capture" (keys → Intent/Actions) and "Kinematics/Physics" (Actions → Transform).
- [ ] **Migrate physics to `FixedUpdate`** — `gravity_system` and `player_movement` run in `Update`. Use `Time<Fixed>` for determinism.
- [ ] **Action-based input** — Replace direct `ButtonInput<KeyCode>` polling with `PlayerAction` enum mapping.

### Resources & Components
- [ ] **Split `WorldState` bag-of-state** — Combines player runtime, map runtime, quest bits, inventory, NPC overrides, actor flags, chest flags. Split into focused resources.
- [ ] **Group player config** — Consolidate `PlayerSettings`, `PlayerKeyBindings`, `MouseLookEnabled`, `MouseSensitivity` into single resource or entity components.
- [ ] **Decouple `spawn_player`** — God function for player + camera + fog + lighting. Split into modular setup systems.
- [ ] **Split `loading_step` (~880 lines, 7 phases)** — Split phases into individual systems driven by `LoadingPhase` state enum.
- [ ] **Split `indoor/indoor.rs` (1166 lines)** — Door animation, raycasts, and indoor world spawning are separate concerns.

### Cleanups
- [ ] **`update_hud_layout` 8-way `ParamSet`** — Replace with tag enum/component; gate on `Changed<Window>`.
- [ ] **HUD overlays spawned/despawned instead of toggled** — Spawn once, flip `Visibility`.
- [ ] **Sprite sheet dimension lookup** — `(w,h)` repeated on every interaction query. Cache as component.
- [ ] **Introduce `CurrentEnvironment` resource** — Replaces scattered `resource_exists::<PreparedWorld>` checks.
- [ ] **NPC dialogue string cloning** — Use `&str`/`Cow` from parsed tables instead of eager clone.
- [ ] **Change detection** — Use `Changed<T>` and `Added<T>` filters more broadly.
- [ ] **Dead code / unused field warnings** — Resolve remaining ~24 items.

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

## Other
- [ ] **Monster aggro range verification** — Cross-check hostile_type 4 range (6656.0)
- [ ] **Street NPC randomization** — Seed identity from per-load RNG
- [ ] **World variable ops** — Move read/write helpers out of dispatcher to dedicated module
- [ ] **BSP collision** — Move outdoor BSP collision setup to `game/world/collision.rs`
