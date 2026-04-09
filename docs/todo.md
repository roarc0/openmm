# OpenMM Roadmap & TODOs

## Recently Completed Reorganization
- [x] **Relocate Actor component** — Moved from `game/entities/actor.rs` to `game/actors/actor.rs`.
- [x] **Rename entities to sprites** — Renamed `game/entities` to `game/sprites`.
- [x] **Relocate loading logic** — Moved `game/entities/sprites.rs` to `game/sprites/loading.rs`.
- [x] **Consolidate sprite materials** — Moved `game/sprite_material.rs` to `game/sprites/material.rs`.
- [x] **Rename engine_config** — Renamed `game/engine_config.rs` to `game/engine.rs`.
- [x] **Extract lazy_spawn() dispatcher** — Split `odm.rs` into localized submodules (spawn_decorations, spawn_actors, etc.).

## Logical Module Grouping (In Progress)
Grouping the flat `src/game/` directory into logical subdirectories.
- [ ] **World Submodule (`game/world/`)**
    - [ ] Move and rename `blv.rs` -> `world/indoor.rs`
    - [ ] Move and rename `odm/` folder -> `world/outdoor/`
    - [ ] Move `sky.rs`, `lighting.rs`, `collision.rs`, `map_name.rs` to `world/`
- [ ] **Systems Submodule (`game/systems/`)**
    - [ ] Move `events.rs`, `game_time.rs`, `world_state.rs`, `optional.rs` to `systems/`
    - [ ] Move and rename `event_dispatch.rs` -> `systems/scripting.rs`
- [ ] **Player Consolidation**
    - [ ] Move `party/` into `player/`
    - [ ] Move `mm6_coords.rs` to `utils/coords.rs`
- [ ] **Global Integration**
    - [ ] Update all project imports to reflect regrouping.

## Architectural Splitting
- [ ] **Centralize Input** — Create `game/systems/input/` to handle KeyCode/MouseButton checks and high-level Action mapping.
- [ ] **Deconstruct `player.rs`** — Split into `player/settings.rs`, `player/systems/motion.rs`, and `player/systems/init.rs`.
- [ ] **Deconstruct `hud/mod.rs`** — Split into `hud/layout.rs`, `hud/builder.rs`, and `hud/constants.rs`.
- [ ] **Refactoring Helpers** — Extract generic actor (`build_actor_component`) and decoration (`apply_decoration_components`) construction.

## Feature Parity & Modernization
- [ ] **Indoor Minimap** — Implement minimap logic for indoor BLV maps; currently only handled for outdoor ODM maps.
- [ ] **HUD GameTime integration** — Tap frame switching (morning/day/evening/night) and date/time text display.
- [ ] **World Variable Ops** — Move read/write helpers out of dispatcher to `game/systems/variable_ops.rs`.
- [ ] **BSP Collision** — Move outdoor BSP collision setup to `game/world/collision.rs`.
- [ ] **Post-process AA fix** — Resolve the bug where FXAA/SMAA/TAA break terrain rendering.
- [ ] **Cleanup** — Resolve remaining dead code and unused field warnings (24+ items).

## New Feature Implementation
- [ ] **Monster aggro range verification** — Cross-check hostile_type 4 range (6656.0).
- [ ] **Texture animation (TFT)** — Terrain shader animated tile cycling.
- [ ] **NPC dialogue text rendering** — Wired up font from LOD and scrolling lines.
- [ ] **Monster combat stats** — Implement HP, speed, and attack logic.
- [ ] **Ground items / pickable objects** — Parse DDM `MapObject` section and spawn `GroundItem` entities.
- [ ] **Chest / item system** — Spawning items inside chests and inventory logic.
- [ ] **Save / load** — Full round-trip persistence for party and map state.
- [ ] **Street NPC randomization** — Seed identity from per-load RNG.
- [ ] **Sky texture variation** — Day/night sky texture swapping.
- [ ] **NPC time-of-day schedules** — Implement AI schedule-following system.
- [ ] **Faction and diplomacy** — Diplomacy table and aggression logic.
- [ ] **Random encounters** — Camping interrupt monster spawns.

## Bevy Best Practices & Technical Debt (Audit)
These items are architectural improvements identified to align the project with Bevy best practices and improve performance/maintainability. **(Note: Note these, do not implement yet)**

### 1. System Granularity & Deconstruction
- [ ] **Deconstruct `process_events` (scripting.rs)** — currently a ~600 line monolithic system.
    - [ ] Move specific event handlers (e.g., `SetSprite`, `MoveToMap`) into dedicated helper systems or functions.
    - [ ] Use Bevy `Events` to trigger complex side-effects (like map transitions) rather than direct mutation inside the loop.
- [ ] **Deconstruct `player.rs` systems** — `player_movement` and `player_look` are oversized.
    - [ ] Split into "Input Capture" (mapping keys to Intent/Actions) and "Kinematics/Physics" (applying Actions to Transform).

### 2. Scheduling & Fixed Time Steps
- [ ] **Migrate Physics to `FixedUpdate`** — `gravity_system` (physics.rs) and `player_movement` (player.rs) currently run in `Update`.
    - [ ] Use `Time<Fixed>` for delta calculations in these systems to ensure deterministic behavior across varying frame rates.
    - [ ] Move `resolve_movement` (collision.rs) calls into the fixed step.

### 3. Input Handling Modernization
- [ ] **Action-Based Input** — Replace direct `ButtonInput<KeyCode>` polling with an action-based abstraction.
    - [ ] Define a `PlayerAction` enum (MoveForward, Jump, ToggleFly, etc.).
    - [ ] Implement a system that maps physical inputs (Keyboard/Gamepad) to these actions once per frame, and other systems read the actions.

### 4. Component & Resource Organization
- [ ] **Group Player State** — Consolidate individual player-related resources (`PlayerSettings`, `PlayerKeyBindings`, `MouseLookEnabled`, `MouseSensitivity`) into a single `PlayerConfig` resource or attach them as components to the `Player` entity.
- [ ] **Decouple Spawning** — `spawn_player` is currently a "God Function" for everything player-related (physics, camera, fog, lighting, tonemapping). 
    - [ ] Split into modular setup systems (e.g. `setup_player_camera`, `setup_player_torch`) and use standard Bevy `OnEnter(GameState::Game)` ordering.

### 4b. Finish `run_if` refactor for indoor/outdoor gating
Several systems still use inline `Option<Res<PreparedWorld>>` / `Option<Res<PreparedIndoorWorld>>` checks with early returns instead of declarative `run_if(resource_exists::<...>)` gates. Most were converted in the indoor-shadow-hang fix; these remain:
- [ ] **`indoor::spawn_indoor_world`** — gate the `OnEnter(Game)` system on `resource_exists::<PreparedIndoorWorld>`; drop the `Option<Res<PreparedIndoorWorld>>` parameter and its early return.
- [ ] **`debug::hud::player::update_tile_text`** — gate on `resource_exists::<PreparedWorld>` (tile debug info is outdoor-only; indoor shows nothing meaningful).

### 5. Performance Optimizations
- [ ] **Allocation Audit** — `resolve_movement` in `collision.rs` uses a `HashSet` for deduplicating walls in the hot path. 
    - [ ] Consider using a pre-allocated `BitSet` or a reused `Vec` to avoid per-frame allocations during movement.
- [ ] **Change Detection** — Increase usage of `Changed<T>` and `Added<T>` filters in queries to skip processing for entities that haven't moved or updated.
