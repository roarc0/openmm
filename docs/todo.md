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

## 2026-04-09 Audit — Plugin size & Performance

Findings from a full-project audit of plugin structure, perf hotspots, and CLAUDE.md compliance. Ordered roughly by impact/effort ratio. **Note these, do not implement yet.**

### A. Critical perf (high impact, localized fix)

- [ ] **A1. Day/night tint is O(actors × states × frames × 5)** — `animate_day_cycle` at `game/lighting.rs:410-449` calls `get_mut()` on *every* sprite-sheet material every frame a tint changes, forcing GPU re-upload. Move tint to a global shader uniform (or single `GlobalLighting` resource read by the sprite shader extension). Expected: 5–10ms frame time saved on dense maps.
- [ ] **A2. Monster AI probes 8 directions per actor per frame** — `ai.rs:85-102,118-127`. `resolve_movement` called 8× per aggro'd monster; scales with `n_actors × n_walls`. Probe only on block, cache last clear heading, reduce to 2–3 rays.
- [ ] **A3. `probe_ground_height` ignores the spatial grid** — `collision.rs:532-554` iterates every floor triangle. The grid is already built at load time; use it. Called on every actor spawn + physics query.
- [ ] **A4. Interaction ray-tests every entity every frame** — `interaction/mod.rs:175-371` (`world_interact_system`, `hover_hint_system`) raycasts all decorations/NPCs/monsters with no spatial cull and no input-gating. Gate on input-changed frames; use the spatial grid to pre-cull.
- [ ] **A5. Sprite distance culling writes `Visibility` for all `WorldEntity` every frame** — `sprites/mod.rs:110-132`. Same iteration cost even when nothing changes. Use the spatial grid for near-entity checks and skip the rest.
- [ ] **A6. Material churn on texture swap** — `outdoor/texture_swap.rs:27-78` builds a fresh `StandardMaterial` on every swap call. Cache a material pool keyed by texture handle.
- [ ] **A7. Indoor sector lookup is linear each frame** — `lighting.rs:331-346` linearly scans `sector_ambients`. Cache `last_sector_index`, recheck only when player bbox exits.

### B. Plugin structure — god-resources and god-systems

- [ ] **B1. `WorldState` is a bag-of-state** — `game/world/state.rs`. Combines player runtime, map runtime, quest bits, inventory, NPC overrides, actor flags, chest flags, decoration state. Split into focused resources (`GameFlags`, `PartyInventory`, `NpcOverrides`, `ActorOverrides`, `ChestFlags`, `MapRuntimeState`). Every system mutating one field currently takes a `ResMut<WorldState>` on the whole thing.
- [ ] **B2. `process_events` monolith** — `game/world/scripting.rs:445-1124`, ~600 line match, uses 3 custom `SystemParam` bundles to dodge Bevy's 16-arg limit. Already listed above, but worth re-flagging: convert side-effects (door state, map transition, overlay, sound) to events consumed by specialized handlers. The custom SystemParam bundles are the clearest symptom that this needs splitting.
- [ ] **B3. `execute_command` in console.rs is a 567-line match** — `game/debug/console.rs:300-867`. 40+ command handlers in one function. Extract each to `debug/console/commands/<name>.rs` and register via a small command table. Biggest win: adding a new command stops touching a 1091-line file.
- [ ] **B4. `loading_step` is ~880 lines with 7 phases** — `states/loading.rs:440-1328`. Frame-budgeted sprite preloading is tangled with map geometry building. Split phases into individual systems driven by a `LoadingPhase` state enum; move sprite preloading into its own system in `Update` gated by phase.
- [ ] **B5. Split `states/loading.rs` (1410 lines)** — Data types (`PreparedWorld`, `PreparedIndoorWorld`, door geometry, clickable/touch/occluder face data) belong in sibling modules. Keep only the pipeline driver in `loading.rs`.
- [ ] **B6. Split `game/indoor/indoor.rs` (1166 lines)** — Door animation, clickable/trigger raycasts, and indoor world spawning are separate concerns currently chained under one plugin.
- [ ] **B7. Overzealous `.chain()` in plugins** — kills parallelism for systems with no real dependency:
    - `hud/mod.rs:84-101` chains 11 unrelated HUD systems (crosshair, minimap, stats, overlays, map overlay).
    - `player.rs:168-181` chains 7 input toggles + movement + look that mostly don't depend on each other.
    - `indoor/mod.rs:14-24` chains interact + touch-trigger + door animation.
  Convert to `SystemSet` ordering only where ordering is actually required; let the rest run in parallel.

### C. CLAUDE.md compliance (real violations)

- [ ] **C1. Lighting runs during paused game** — `game/lighting.rs:82` gates only on `GameState::Game`, not `HudView::World`. `animate_day_cycle` should follow the HudView-gating rule from CLAUDE.md "Common Mistakes".
- [ ] **C2. Party torch not HudView-gated** — `game/player.rs:182` (`party_torch_system`). Same issue.
- [ ] **C3. Footsteps not ordered after `PlayerInputSet`** — `game/sound/footsteps.rs:37-39,44-49`. Reads player `Transform` with no ordering. One-frame lag possible.
- [ ] **C4. Rule 3 violation — character-var range hardcoded in game code** — `game/world/scripting.rs:252` has `matches!(var.0, 0x01..=0x68)`. Use `!var.is_map_var()` from `openmm_data::enums::EvtVariable`.
- [ ] **C5. Rule 3 violation — actor attribute bitflags hardcoded** — `game/world/scripting.rs:400-401` redefines `VISIBLE = 0x8`, `HOSTILE = 0x01000000`. Use `openmm_data::enums::ActorAttributes`.

### D. Lower-impact cleanups

- [ ] **D1. `update_hud_layout` uses an 8-way `ParamSet` with cascading `Without` filters** — `game/hud/mod.rs:531-768`. Replace with a single tag enum/component; gate on `Changed<Window>` so it doesn't run every frame.
- [ ] **D2. HUD overlays are spawned/despawned instead of toggled** — `game/hud/overlay.rs:211-261`. Spawn once, flip `Visibility`.
- [ ] **D3. `Flicker` writes its timer every frame** — `sprites/mod.rs:137-151`. Compute flicker as a deterministic function of `elapsed` + per-entity phase; zero writes.
- [ ] **D4. Sprite sheet `(w,h)` dimension lookup repeated on every interaction query** — cache as a component updated on state change.
- [ ] **D5. Introduce `CurrentEnvironment { Outdoor, Indoor }` resource** — replaces scattered `resource_exists::<PreparedWorld>` / `resource_exists::<PreparedIndoorWorld>` checks in lighting, sky, HUD, debug.
- [ ] **D6. NPC dialogue strings clone eagerly** — `hud/overlay.rs:141-166`. Use `&str`/`Cow` from the parsed tables.
