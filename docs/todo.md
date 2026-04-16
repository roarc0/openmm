# OpenMM Roadmap & TODOs

## Module Organization

### Done
- [x] Consolidated `HudView` + `FooterText` into `UiState` resource (`game/world/ui_state.rs`)
- [x] Moved `ui_assets` from `game/` to `screens/` (where it belongs)
- [x] Renamed `game/physics.rs` → `game/player_physics.rs` (disambiguate from `actors/physics.rs`)
- [x] Moved `sync_player_to_world_state()` from `world/state.rs` → `player/mod.rs`
- [x] Grouped `lighting.rs` + `sky.rs` into `game/rendering/` module

### Remaining
- [ ] **Split `viewport.rs`** — mixed concerns: camera rendering (`RenderScaleState`, `update_viewport`) + HUD layout (`HudDimensions`, `hud_dimensions`, `parse_aspect_ratio`). Entangled because `viewport_base()` uses `HudDimensions` — needs careful separation.
- [ ] **Consolidate geometry utilities** — `collision.rs` has private 2D geometry helpers (`point_in_triangle_2d`, `barycentric_2d`, `point_in_polygon_2d`), `interaction/raycast.rs` has 3D equivalents. Could share a `game/geom.rs`.
- [ ] **Extract shared input helpers** — `apply_deadzone()` and `right_stick_with_fallback()` in `player/input.rs`, `check_exit_input()` in `interaction/mod.rs`. Small but scattered.
- [ ] **Centralize Input** — Create `game/input/` to handle KeyCode/MouseButton checks and high-level Action mapping.
- [ ] **Deconstruct `hud/mod.rs`** — Split into `hud/layout.rs`, `hud/builder.rs`, and `hud/constants.rs`.

## Screen Editor
- [ ] **Inline text element editing** — Text properties (source, font, color, align) are only editable in the Inspector panel. Add a floating edit panel on the canvas when a Text element is selected, so text config is accessible inline like Image position/size drag-editing.
- [ ] **Consolidate inspector + event editor** — `inspector.rs` and `canvas.rs`/`element_editor.rs` both edit on_click, on_hover, states, position, size. Two code paths for the same data = drift risk. Unify into a single tabbed inspector.
- [ ] **Element ID validation** — No collision detection on element IDs. Duplicate IDs silently allowed. ShowSprite/HideSprite can reference nonexistent element IDs. Validate on save, warn about dangling references.
- [ ] **Texture name validation** — No check that texture names exist in LOD archives. Validate at edit time by caching LOD file lists at startup.
- [ ] **Editor undo/redo** — Changes are immediately reflected on `EditorScreen` with no rollback. Implement a command-based undo stack.

## Screen Runtime
- [ ] **Text source registry** — `text_update()` in `runtime.rs` hardcodes source names (`"footer_text"`, `"gold"`, `"food"`, `"loading_step"`). Adding a new source requires editing runtime.rs. Move to a registry or dispatch mechanism.
- [ ] **Condition evaluation for element states** — `ElementState` supports a `condition` string but conditions are never evaluated at runtime (only stored). Hover state is hardcoded as a special case. Generalize to a runtime state machine.
- [ ] **Texture asset pooling** — `load_texture_with_transparency()` creates new Image assets on every spawn. Same texture loaded multiple times wastes memory. Implement dedup or pooling.

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

### 4b. Finish `run_if` refactor for indoor/outdoor gating — DONE.
- [x] **`indoor::spawn_indoor_world`** — DONE. Gated on `resource_exists::<PreparedIndoorWorld>`, dropped `Option` and early return.
- [x] **`debug::hud::player::update_tile_text`** — DONE. Gated on `resource_exists::<PreparedWorld>`, dropped `Option` and early return.

### 5. Performance Optimizations
- [x] **Allocation Audit** — DONE. `resolve_movement` no longer allocates: `SpatialGrid::cells_overlapping` returns `impl Iterator<Item = usize>` instead of `Vec<usize>`, and the explicit `HashSet` wall dedupe was dropped — a wall spanning multiple cells is processed once per cell, but the second visit is a guaranteed no-op (signed distance already ≥ radius after the first push), so correctness is preserved at zero allocation cost. Two regression tests cover the push-through-wall and free-movement paths.
- [ ] **Change Detection** — Increase usage of `Changed<T>` and `Added<T>` filters in queries to skip processing for entities that haven't moved or updated.

### B. Plugin structure — god-resources and god-systems

- [ ] **B1. `WorldState` is a bag-of-state** — `game/world/state.rs`. Combines player runtime, map runtime, quest bits, inventory, NPC overrides, actor flags, chest flags, decoration state. Split into focused resources (`GameFlags`, `PartyInventory`, `NpcOverrides`, `ActorOverrides`, `ChestFlags`, `MapRuntimeState`). Every system mutating one field currently takes a `ResMut<WorldState>` on the whole thing.
- [ ] **B2. `process_events` monolith** — `game/world/scripting.rs:445-1124`, ~600 line match, uses 3 custom `SystemParam` bundles to dodge Bevy's 16-arg limit. Already listed above, but worth re-flagging: convert side-effects (door state, map transition, overlay, sound) to events consumed by specialized handlers. The custom SystemParam bundles are the clearest symptom that this needs splitting.
- [x] **B3. `execute_command` in console.rs is a 567-line match** — DONE. Split `console.rs` into `console/mod.rs` (UI + thin dispatcher) and `console/commands.rs` (one `cmd_*` function per command). The dispatcher match is now ~50 lines; each handler takes only the state it needs. Adding a new command means writing one function and adding one match arm.
- [ ] **B4. `loading_step` is ~880 lines with 7 phases** — `states/loading.rs:440-1328`. Frame-budgeted sprite preloading is tangled with map geometry building. Split phases into individual systems driven by a `LoadingPhase` state enum; move sprite preloading into its own system in `Update` gated by phase.
- [ ] **B5. Split `states/loading.rs` (1410 lines)** — Data types (`PreparedWorld`, `PreparedIndoorWorld`, door geometry, clickable/touch/occluder face data) belong in sibling modules. Keep only the pipeline driver in `loading.rs`.
- [ ] **B6. Split `game/indoor/indoor.rs` (1166 lines)** — Door animation, clickable/trigger raycasts, and indoor world spawning are separate concerns currently chained under one plugin.
- [x] **B7. Overzealous `.chain()` in plugins** — DONE (563a730). HUD plugin split into a parallel tuple with only `update_hud_layout → update_viewport` kept as a sub-chain. Player plugin split: toggles `.before(player_movement).before(player_look)`, everything else parallel. BlvPlugin was already parallel — audit was wrong about it.

### D. Lower-impact cleanups

- [ ] **D1. `update_hud_layout` uses an 8-way `ParamSet` with cascading `Without` filters** — `game/hud/mod.rs:531-768`. Replace with a single tag enum/component; gate on `Changed<Window>` so it doesn't run every frame.
- [ ] **D2. HUD overlays are spawned/despawned instead of toggled** — `game/hud/overlay.rs:211-261`. Spawn once, flip `Visibility`.
- [x] **D3. `Flicker` writes its timer every frame** — DONE. `DecorFlicker` dropped its `timer`/`lit` fields; lit state is now a pure function of `Time::elapsed_secs()` + a per-entity phase. `flicker_system` takes `&DecorFlicker` (no writes to the component) and uses `set_if_neq` on `Visibility` to skip redundant writes. 6 new unit tests cover the computation.
- [ ] **D4. Sprite sheet `(w,h)` dimension lookup repeated on every interaction query** — cache as a component updated on state change.
- [ ] **D5. Introduce `CurrentEnvironment { Outdoor, Indoor }` resource** — replaces scattered `resource_exists::<PreparedWorld>` / `resource_exists::<PreparedIndoorWorld>` checks in lighting, sky, HUD, debug.
- [ ] **D6. NPC dialogue strings clone eagerly** — `hud/overlay.rs:141-166`. Use `&str`/`Cow` from the parsed tables.

### Remaining refactoring (not yet implemented)

#### High impact
- [ ] **Deduplicate monster spawning (3 sites → 1 helper)** — `indoor/spawn.rs:582`, `outdoor/spawn_actors.rs:39` (DDM monsters), `outdoor/spawn_actors.rs:239` (ODM monsters) all do: `load_entity_sprites` → dsft_scale → mesh → Actor::new(ActorParams{17 fields}) → MonsterInteractable + MonsterAiMode + SpriteSheet + Billboard + shadow. Only difference: position calc (terrain probe vs mm6_to_bevy vs golden-angle). Extract `spawn_monster_entity(commands, pos, &MonsterData, &SpawnCtx) -> Entity`. ~200 lines saved.
- [ ] **Deduplicate decoration spawning (indoor/outdoor → shared module)** — `indoor/spawn.rs:255-555` and `outdoor/spawn_decorations.rs:17-298` implement the same 3 decoration types (directional, animated, static) with 80% identical logic. Indoor has `InGame` marker; outdoor has `DecorationTrigger` + caching + sounds. Extract shared `spawn_directional()`, `spawn_animated()`, `spawn_static()` into a decoration spawner module. ~250 lines saved.
- [ ] **Split `loading.rs` (1211 lines)** — Indoor and outdoor loading share one file. Mesh creation pattern repeated 3x, StandardMaterial configs repeated 3x. Split into `loading/mod.rs` (state machine), `loading/outdoor.rs` (PreparedWorld, ODM steps), `loading/indoor.rs` (PreparedIndoorWorld, BLV steps). Extract `build_textured_mesh()` and `build_standard_material(preset)` helpers. Single `finalize_loading()` path for both indoor and outdoor.

#### Medium impact
- [ ] **Split `sprites/loading.rs` (857 lines)** — Contains sprite frame decoding, sprite cache, alpha mask, animation update system, direction calculation. Animation runtime (`update_sprite_sheets`, 145 lines) is a different concern from load-time decoding. Split into `sprites/decode.rs` + `sprites/animation.rs`.
- [x] **EVT conditional jump helper** — `scripting.rs:337-765` has 10 identical `steps.iter().position(|s| s.step >= jump_step)` + `log_skipped` + `log_tail_unreachable` blocks. Extract `execute_conditional_jump(steps, pc, jump_step, reason) -> bool`.
- [x] **EVT stub macro** — 21 STUB event arms in `scripting.rs` with near-identical `warn!("STUB ...")` blocks. Replace with `stub_event!()` macro.
- [ ] **`handle_move_to_map` split** — `event_handlers.rs:116-197` handles same-map teleport AND cross-map transition in one function. These are distinct operations that should be separate.

#### Low impact
- [x] **`ai_type: String` → enum** — `Actor.ai_type` is always one of "Normal"/"Aggress"/"Wimp"/"Suicidal". Cloned at 4 spawn sites. Should be an enum.
- [ ] **Overlay image pattern** — `handle_speak_in_house` and `handle_open_chest` in `event_handlers.rs` share identical OverlayImage + cursor logic. Extract helper.
- [ ] **Indoor sprite caching** — Outdoor caches static decoration materials in `dec_sprite_cache`; indoor doesn't. Add caching to indoor for consistency.
- [ ] **Texture size collection** — `loading.rs` has 2 near-identical HashMap-building loops for indoor vs outdoor texture sizes. Extract `collect_texture_sizes()` helper.
