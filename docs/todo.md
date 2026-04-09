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
- [x] **Allocation Audit** — DONE. `resolve_movement` no longer allocates: `SpatialGrid::cells_overlapping` returns `impl Iterator<Item = usize>` instead of `Vec<usize>`, and the explicit `HashSet` wall dedupe was dropped — a wall spanning multiple cells is processed once per cell, but the second visit is a guaranteed no-op (signed distance already ≥ radius after the first push), so correctness is preserved at zero allocation cost. Two regression tests cover the push-through-wall and free-movement paths.
- [ ] **Change Detection** — Increase usage of `Changed<T>` and `Added<T>` filters in queries to skip processing for entities that haven't moved or updated.

## 2026-04-09 Audit — Plugin size & Performance

Findings from a full-project audit of plugin structure, perf hotspots, and CLAUDE.md compliance. Ordered roughly by impact/effort ratio. **Note these, do not implement yet.**

### A. Critical perf (high impact, localized fix)

- [x] **A1. Day/night tint is O(actors × states × frames × 5)** — DONE (0676f4f). Replaced per-material uniform with a shared `ShaderStorageBuffer` (regular + selflit), updated in place once per threshold crossing. See `game/sprites/tint_buffer.rs`. **Known regression: see "Standing vs walking tint mismatch" below.**
- [x] **A2. Monster AI probes 8 directions per actor per frame** — DONE. `steer_toward` now probes a 4-angle fan (`±45 / ±90°` instead of 8 offsets) and caches the winning detour on `Actor::cached_steer_offset`. Next frame retries the cached offset before fanning out, so a monster hugging the same wall uses 2 `resolve_movement` calls per frame instead of 9. Worst case 5 (was 9) when both direct + cache fail. Regression tests cover the direct-path cache clear. See `game/actors/ai.rs:33-44,82-158`.
- [x] **A3. `probe_ground_height` ignores the spatial grid** — DONE. `probe_ground_height` in `collision.rs` now prunes via `BuildingColliders.grid.cell_idx(x, z)` and iterates only that cell's floor list — the same pattern `floor_height_at` already used. Four regression tests cover the grid-hit, outside-AABB, empty-cell, and triangle-corner-gap cases.
- [x] **A4. Interaction ray-tests every entity every frame** — DONE. `hover_hint_system` and `world_interact_system` now query the new `EntitySpatialIndex` for entities within `MAX_INTERACT_RANGE` (typically a 3×3 cell block ≈ a handful of entities) and call `Query::get(entity)` per candidate. The heavy matrix / polygon / mask work no longer runs for every `WorldEntity` on the map. See `game/spatial_index.rs`, `game/interaction/mod.rs`.
- [x] **A5. Sprite distance culling writes `Visibility` for all `WorldEntity` every frame** — DONE. `distance_culling` was folded into `spatial_index::rebuild_and_cull`: one `iter_mut` pass now both buckets entities into the grid and does the `set_if_neq` visibility update, so we pay the cost once instead of twice. The standalone `distance_culling` system is gone.
- [x] **A6. Material churn on texture swap** — DONE. `SwapMaterialCache` resource keyed by texture name now owns the pooled `Handle<StandardMaterial>`. Second swap to the same texture reuses the cached handle (no image decode, no material alloc, no bind-group churn).
- [x] **A7. Indoor sector lookup is linear each frame** — DONE. `LightingState::last_sector_index` caches the sector the player was last inside; the linear scan only runs when the player leaves that bbox.

### B. Plugin structure — god-resources and god-systems

- [ ] **B1. `WorldState` is a bag-of-state** — `game/world/state.rs`. Combines player runtime, map runtime, quest bits, inventory, NPC overrides, actor flags, chest flags, decoration state. Split into focused resources (`GameFlags`, `PartyInventory`, `NpcOverrides`, `ActorOverrides`, `ChestFlags`, `MapRuntimeState`). Every system mutating one field currently takes a `ResMut<WorldState>` on the whole thing.
- [ ] **B2. `process_events` monolith** — `game/world/scripting.rs:445-1124`, ~600 line match, uses 3 custom `SystemParam` bundles to dodge Bevy's 16-arg limit. Already listed above, but worth re-flagging: convert side-effects (door state, map transition, overlay, sound) to events consumed by specialized handlers. The custom SystemParam bundles are the clearest symptom that this needs splitting.
- [x] **B3. `execute_command` in console.rs is a 567-line match** — DONE. Split `console.rs` into `console/mod.rs` (UI + thin dispatcher) and `console/commands.rs` (one `cmd_*` function per command). The dispatcher match is now ~50 lines; each handler takes only the state it needs. Adding a new command means writing one function and adding one match arm.
- [ ] **B4. `loading_step` is ~880 lines with 7 phases** — `states/loading.rs:440-1328`. Frame-budgeted sprite preloading is tangled with map geometry building. Split phases into individual systems driven by a `LoadingPhase` state enum; move sprite preloading into its own system in `Update` gated by phase.
- [ ] **B5. Split `states/loading.rs` (1410 lines)** — Data types (`PreparedWorld`, `PreparedIndoorWorld`, door geometry, clickable/touch/occluder face data) belong in sibling modules. Keep only the pipeline driver in `loading.rs`.
- [ ] **B6. Split `game/indoor/indoor.rs` (1166 lines)** — Door animation, clickable/trigger raycasts, and indoor world spawning are separate concerns currently chained under one plugin.
- [x] **B7. Overzealous `.chain()` in plugins** — DONE (563a730). HUD plugin split into a parallel tuple with only `update_hud_layout → update_viewport` kept as a sub-chain. Player plugin split: toggles `.before(player_movement).before(player_look)`, everything else parallel. BlvPlugin was already parallel — audit was wrong about it.

### C. CLAUDE.md compliance (real violations)

- [x] **C1. Lighting runs during paused game** — DONE (6753596). `animate_day_cycle` gated on `resource_equals(HudView::World)`.
- [x] **C2. Party torch not HudView-gated** — DONE (6753596). Same gate added.
- [x] **C3. Footsteps not ordered after `PlayerInputSet`** — DONE (6753596). `footstep_system.after(PlayerInputSet)`.
- [ ] **C4. Rule 3 violation — character-var range hardcoded in game code** — `game/world/scripting.rs:252` has `matches!(var.0, 0x01..=0x68)`. NOTE: `!var.is_map_var()` is NOT equivalent — `0x01..=0x68` is the character-scoped range (HP, stats, skills); values outside 0x68 that aren't map vars (0xCD+) are party/global. Needs a new `EvtVariable::is_character_scoped()` method in openmm-data.
- [x] **C5. Rule 3 violation — actor attribute bitflags hardcoded** — DONE (6753596). Uses `ActorAttributes::VISIBLE` / `HOSTILE` from openmm-data.

### D. Lower-impact cleanups

- [ ] **D1. `update_hud_layout` uses an 8-way `ParamSet` with cascading `Without` filters** — `game/hud/mod.rs:531-768`. Replace with a single tag enum/component; gate on `Changed<Window>` so it doesn't run every frame.
- [ ] **D2. HUD overlays are spawned/despawned instead of toggled** — `game/hud/overlay.rs:211-261`. Spawn once, flip `Visibility`.
- [x] **D3. `Flicker` writes its timer every frame** — DONE. `DecorFlicker` dropped its `timer`/`lit` fields; lit state is now a pure function of `Time::elapsed_secs()` + a per-entity phase. `flicker_system` takes `&DecorFlicker` (no writes to the component) and uses `set_if_neq` on `Visibility` to skip redundant writes. 6 new unit tests cover the computation.
- [ ] **D4. Sprite sheet `(w,h)` dimension lookup repeated on every interaction query** — cache as a component updated on state change.
- [ ] **D5. Introduce `CurrentEnvironment { Outdoor, Indoor }` resource** — replaces scattered `resource_exists::<PreparedWorld>` / `resource_exists::<PreparedIndoorWorld>` checks in lighting, sky, HUD, debug.
- [ ] **D6. NPC dialogue strings clone eagerly** — `hud/overlay.rs:141-166`. Use `&str`/`Cow` from the parsed tables.

## 2026-04-09 Session Handoff

### Done this session
- `0676f4f` — **A1** shared sprite tint storage buffer (big perf fix)
- `6753596` — **C1/C2/C3/C5** HudView gating + footstep ordering + ActorAttributes constants
- `563a730` — **B7** parallelism in HUD and player plugins
- (next commit) — **B3** split `execute_command` into `console/mod.rs` + `console/commands.rs`

### 2026-04-10 Perf session — stopping point & next steps

Full audit-item sweep landed: **A2 A3 A4 A5** all done plus a bonus
allocation fix in `resolve_movement`. Per-frame O(n) loops in the AI,
interaction, distance-culling, ground-probing, and wall-collision paths
are all either spatially pruned or allocation-free. Remaining guesses
from source reading (`actor_gravity_system` early-exit,
`update_sprite_sheets` parallelism, atan2 caching) are sub-millisecond
shaves with diminishing returns.

**Next perf step is a profile, not more reading.** `cargo flamegraph`
or enabling Bevy's Tracy feature for 5 minutes will say exactly where
the CPU is going — whether it's ECS overhead, render-world extraction,
shadow cascades, BSP rendering, or something not in the audit at all.
After the A1–A7 sweep the guesses are getting thinner and the risk of
shaving microseconds off the wrong loop is real.

Commits this perf session:
- `2e77916` — **A2** monster AI cached steering detour + narrower fan
- `42e0894` — **A4 + A5** entity spatial index + fused distance cull
- `244096b` — **A3** `probe_ground_height` via BSP grid
- `71f3ee8` — `resolve_movement` allocation-free hot path

### 2026-04-10 — Root cause of the A1 sprite tint regression

Confirmed via reading the Bevy 0.18 source (`bevy_render_macros/src/as_bind_group.rs` line 469 + `bevy_render/src/storage.rs::prepare_asset`): **Bevy does not re-prepare material bind groups when a `ShaderStorageBuffer` asset they reference is updated.**

The flow on a `set_data` call is:
1. `Assets::get_mut` marks the storage asset changed.
2. Next frame, `prepare_asset` on `GpuShaderStorageBuffer` creates a **new wgpu `Buffer`** (the buffer description doesn't include `COPY_DST`, so in-place writes aren't possible), and stores it in `RenderAssets` under the same asset id.
3. **Materials that were already prepared still hold bind groups pointing to the OLD wgpu `Buffer`.** Nothing invalidates them — `prepare_asset` for the material only runs on material asset changes, not on dependency changes.

So the tint values we write via `SpriteTintBuffers::write` go into a new GPU buffer that nothing is bound to. The visible symptom is that sprite tints freeze at whatever value was in the storage buffer when each material was first prepared — and because different cache paths (preload vs runtime spawn vs SetSprite) prepare their materials at different times, different sprites see different stale tints.

This is why `time add 10` darkens terrain and buildings but leaves sprites visually unchanged, and why state-swap on an actor (walking ↔ standing) looks like a brightness flash.

**Fix:** revert A1. Use `#[uniform(100)] tint: Vec4` on `SpriteExtension` and propagate tint updates by iterating sprite materials on threshold crossings — the pre-A1 design. The hitch returns (~20k writes a handful of times per in-game day) but correctness is restored. Future perf: amortize the material iteration across multiple frames.

### Original diagnostic notes — **tint mismatch between standing and walking animations** (regression from A1)

When an NPC is standing still, its tint appears **darker** than the same NPC's walking animation. The walking sprites look correctly lit; the standing sprites look dimmer.

Both states go through `load_entity_sprites` in `game/sprites/loading.rs` with the same `&tint_buffer` argument (the regular buffer from `SpriteTintBuffers`), so they should reference the same shared storage buffer and render identically. The fact that they don't suggests one of:

1. **Cache aliasing** — `load_sprite_frames` caches by `cache_key(root, variant, min_w, min_h, palette_id)`. Standing is loaded *after* walking with `min_w = ww as u32, min_h = wh as u32` (walking's dims), creating a different cache key than a fresh load would. If an earlier unrelated load populated a cache entry for the standing root with a *different* tint buffer handle (e.g. first load happened before `SpriteTintBuffers` was initialized, or from a prior map that somehow kept stale material handles), the standing frames could reference a different or empty storage buffer.
2. **Empty `Handle::default()` leaking through** — if any code path constructs `SpriteExtension` without explicitly passing a tint buffer, the field gets a default empty handle, and the bind group may read zeros (→ dark sprite).
3. **First-frame reload of walking** — inside `load_entity_sprites`, if `sw > ww || sh > wh`, walking is *reloaded* padded to standing dimensions. This creates new walking materials but the *standing* materials were already built with the correct tint buffer. Both should still tint identically, but if there's a path where walking goes through the helper twice and standing only once, they could diverge in cache handling.

Good starting points:
- Add a temporary `assert_eq!` in `decode_sprite_frames` that the passed `tint_buffer` equals the buffers resource's `regular` handle, and log the material asset IDs at create time.
- Alternatively, instrument the shader: set the WGSL fragment to output `tint.rgb` directly (bypass texture sampling) and compare the standing vs walking sprite frames visually — if they show different colours, it confirms different buffers/handles.
- Check `SpriteCache` state between the walking and standing `load_sprite_frames` calls inside `load_entity_sprites` — does the walking pass populate any cache entry under a key that the standing pass then hits with the wrong handle?

The bug is purely a regression of commit `0676f4f`; the pre-A1 code wrote the tint into every material's `extension.tint` in `animate_day_cycle`, which masked any cache-aliasing issue.

### Next task (already agreed): **B3** — split `execute_command` in `game/debug/console.rs`

- The function spans `console.rs:300-867`, 567 lines, ~40 command handlers in one match.
- Goal: extract each command into its own function (or `debug/console/commands/<name>.rs` submodule) and dispatch via a small command table `&[(name, handler)]` or similar.
- This is the "plugins are too big" pain point the user explicitly called out.
- Contained to one file; low risk; good template for future commands.

### Remaining candidates after B3, ordered by rec

1. ~~**A4 + A5** — spatial-grid gate for interaction raycasts and distance culling~~ — DONE. New `game/spatial_index.rs` owns a per-frame XZ grid of `WorldEntity` entities; `rebuild_and_cull` builds it and culls in one pass, and both interaction systems query it via `query_radius`.
2. **A6** — material pool for outdoor texture swap (`outdoor/texture_swap.rs:27-78`). Small and localized.
3. **D3** — compute flicker from global time instead of storing a per-entity timer (`sprites/mod.rs:137-151`). ~10 lines, eliminates per-frame writes on every flickering torch.
4. **A7** — cache last sector index in indoor ambient lookup (`lighting.rs:355-375`). Tiny.
5. ~~**A2** — reduce monster AI probes~~ — DONE (cached detour offset + 4-angle fan).
6. ~~**A3** — use spatial grid in `probe_ground_height`~~ — DONE.
7. **B1** — split `WorldState` into focused resources. Highest-impact cleanup but invasive.
8. **B3**/B4/B5/B6 — other large-file splits (loading.rs 1410 lines, indoor.rs 1166 lines, scripting.rs process_events 600-line match).
9. **D1/D2/D4/D5/D6** — lower-impact cleanups, pick opportunistically.
10. **C4** — add `EvtVariable::is_character_scoped()` in openmm-data, then replace the hardcoded range in `scripting.rs:250-252`.

### Context for the next session
- Project: OpenMM (MM6 reimplementation, Rust + Bevy 0.18). Workspace: `openmm-data/` (pure data), `openmm/` (game).
- Always read `CLAUDE.md` first. Key rules: behaviour change ⇒ doc update, no hardcoded format constants in game code, bug fix ⇒ regression test, keep modules focused.
- `make lint`, `cargo test -p openmm`, and `cargo check -p openmm` are the main verification commands. `openmm-data` tests need MM6 data files and fail on systems without them — ignore those.
- The sprite tint system now lives in `game/sprites/tint_buffer.rs`. `SpriteTintBuffers` is a startup-created resource; every spawn site must pass one of its handles into `unlit_billboard_material`.
- Recent commits to read before touching sprites/lighting: `0676f4f`, `6753596`, `563a730`.
