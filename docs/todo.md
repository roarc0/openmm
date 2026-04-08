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
