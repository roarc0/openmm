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

## Rendering & Visual
- [ ] **Texture animation (TFT)** — Terrain shader animated tile cycling
- [ ] **Sky texture variation** — Day/night sky texture swapping
- [ ] **Indoor minimap** — Minimap logic for BLV maps (only ODM handled now)
- [ ] **HUD GameTime integration** — Tap frame switching (morning/day/evening/night), date/time text
- [ ] **Post-process AA fix** — FXAA/SMAA/TAA break terrain rendering

## Module Organization
- [ ] **Split `viewport.rs`** — Mixed concerns: camera rendering (`RenderScaleState`, `update_viewport`) + HUD layout (`HudDimensions`). Entangled via `viewport_base()`.
- [ ] **Consolidate geometry utilities** — `collision.rs` 2D helpers + `interaction/raycast.rs` 3D equivalents → `game/geom.rs`
- [ ] **Centralize input** — Create `game/input/` for KeyCode/MouseButton checks + high-level Action mapping
- [ ] **Deconstruct `hud/mod.rs`** — Split into `hud/layout.rs`, `hud/builder.rs`, `hud/constants.rs`

## Refactoring (High Impact)
- [x] **Deduplicate actor spawning (3 sites → 1 helper)** — Shared `spawn_actor()` in `game/spawn/monster.rs` covers monsters + NPCs. ~200 lines saved.
- [x] **Deduplicate decoration spawning** ��� Shared `spawn_decoration()` in `game/spawn/decoration.rs`. Indoor gains triggers, flicker, material caching. ~250 lines saved.
- [ ] **Split `loading.rs` (1211 lines)** — Indoor/outdoor share one file. Split into `loading/mod.rs` + `loading/outdoor.rs` + `loading/indoor.rs`. Extract `build_textured_mesh()` and `build_standard_material()` helpers.

## Refactoring (Medium Impact)
- [ ] **Split `sprites/loading.rs` (857 lines)** — Animation runtime (`update_sprite_sheets`) is different concern from load-time decoding. Split into `sprites/decode.rs` + `sprites/animation.rs`.
- [ ] **`handle_move_to_map` split** — `event_handlers.rs` handles same-map teleport AND cross-map transition in one function. Separate them.

## Refactoring (Low Impact)
- [ ] **Overlay image pattern** — `handle_speak_in_house` and `handle_open_chest` share identical OverlayImage + cursor logic. Extract helper.
- [x] **Indoor sprite caching** — Now uses shared `DecSpriteCache` via `spawn_decoration()`.
- [ ] **Texture size collection** — 2 near-identical HashMap-building loops for indoor vs outdoor texture sizes. Extract `collect_texture_sizes()`.

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
