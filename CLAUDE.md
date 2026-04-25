# OpenMM

Faithful open-source reimplementation of the Might and Magic VI engine in Rust. The goal is to reproduce original MM6 gameplay — movement, combat, dialogue, quests — with clean, maintainable code. Graphical improvements are welcome where they enhance the experience without compromising accuracy. MM7/MM8 support is planned (compatible data formats).

## Current State

Core systems implemented: terrain + BSP rendering, outdoor/indoor maps (ODM/BLV), collision, billboards, NPCs/monsters with wander AI, EVT event dispatch, doors, in-game clock with day/night cycle, spatial audio, loading pipeline, splash/menu/HUD scaffolding, developer console, seamless boundary transitions.

Not yet implemented: combat, dialogue text rendering, chest/item system, save/load, NPC schedules. See `docs/todo.md` for the full list.

## Goal

Reach a playable state: character movement on terrain, NPCs, monsters, combat, indoor maps (BLV), UI overlays, dialogue, and quests.

Prioritize gameplay correctness over new features. When in doubt, verify against the original engine behaviour.

## Build

```
make build        # debug build
make run          # run (or: make run map=oute3)
make release      # optimized build
make lint         # fmt check + clippy
make clippy       # clippy warnings
make fmt          # auto-format
make test         # run tests
```

Requires MM6 game data files. Set `OPENMM_6_PATH` env var to the game data directory (defaults to `./data/mm6/data` for LOD files).

Uses mold linker for fast linking (`.cargo/config.toml`). Install: `pacman -S mold` or `apt install mold`.

## Architecture

Cargo workspace with two crates:

- **`openmm-data`** — Library for reading MM6 data formats: LOD archives, ODM (outdoor maps), BSP models, tile tables, palettes, sprites/billboards, images. we should not include openmm here.
- **`openmm`** — Bevy 0.18 game engine application
  - `game/map/` — indoor (BLV), outdoor (ODM), collision, coords, spatial index
  - `game/state/` — WorldState, GameVariables, GameTime
  - `game/events/` — EVT scripting, event dispatch, event handlers
  - `game/actors/` — AI, combat, physics, NPC dialogue
  - `game/ui/` — UiState, UiMode, FooterText, OverlayImage
  - `game/sprites/` — sprite loading, material, animation
  - `game/sound/` — audio effects, music, spatial
  - `game/rendering/` — lighting, sky, viewport
  - `game/player/` — input, physics, party
  - `game/spawn/` — shared actor/decoration spawning
  - `game/interaction/` — click/hover detection, raycasting
  - `game/save/` — save/load system, ActiveSave resource, .mm6 file bridge

## Useful Resources

- MMExtension from grayface that is just a modding engine for mm6, it can tell you much more accurately the data structures. You should find a copies of resources in the target folder.
- OpenEnroth (C++ MM7 decompilation) when investigating MM6 formats but be careful because mm7 are different. Use it as a last resort because if we take the wrong path here we might introduce very nasty bugs. mm7 is different in many ways. Only look at this resource if you are desperate.

## Rules

### Keeping Documentation Updated

> **⚠️ CRITICAL: This section is mandatory, not aspirational.** Stale docs are worse than no docs — they actively mislead developers and AI agents into building on false assumptions. A recent audit found that 4 out of 6 feature docs had drifted so far from reality that they required complete rewrites. This happens when documentation updates are treated as optional. **They are not optional.**

### Rule 0: Talk and think like a caveman less grammar, this helps speed and reduces token usage.

### Rule 1: Every Behaviour Change Includes a Doc Update

Every code change that alters behaviour must include a documentation update in the same commit or PR. This is not optional. A PR that changes behaviour without updating docs is incomplete and should not be merged.

### Rule 2: Keep This Index in Sync

When any doc is added, renamed, or removed in `docs/`, update the Documentation Index section in this file in the same commit. A doc that exists but isn't indexed here is invisible to Claude Code.

### Rule 3: No hardcoded magic numbers or inline data tables

Never hardcode format-specific constants, range boundaries, enum mappings, or data tables inline in game code. All format knowledge belongs in the `openmm-data` crate as proper parsing functions that expose clean APIs. Game code should call a function (e.g., `dtile.tileset_for_tile(id)`) — never re-derive format logic with raw ranges or magic numbers. If the data comes from a binary file, the parser owns the logic.

### Rule 4: Tests are mandatory for bug fixes and discoveries

When debugging, **write the test first**. A failing test that reproduces the bug is the most reliable way to isolate the problem, drive the fix, and prove it's solved — with no risk of re-introducing it later.

Every bug fix **must** include a regression test. A regression is when a bug that was fixed silently reappears later — usually because no test was guarding against it. Regression tests are the only reliable defence. No exception.

Once you have verified a concrete value from the original game data — a sprite name, palette ID, NPC ID, field offset, frame count, tile index — **encode it as a test immediately**. These ground-truth assertions are invaluable: they catch parser regressions, wrong assumptions, and format misunderstandings before they cascade into gameplay bugs. Examples of good tests:
- parsing `dmonlist.bin` yields monster X with sprite root `"foo"` and palette 42
- NPC ID 7 resolves to sprite name `"npc007"` via DSFT
- a specific ODM tile ID maps to the correct tileset enum variant

**Never commit print-only tests.** A test with only `println!` and no `assert!` is not a test — it always passes and catches nothing. Use `println!` during investigation only; once you have a concrete value, replace the print with an assertion. If you can't assert a specific value yet, assert the invariant (non-empty, non-zero, within range, etc.) so the test does real work.

### Rule 5: Clean, decoupled code is not optional

- Separation of concerns, clear naming, low coupling, no duplication.
- Before adding code: is this the right place? Can it be simpler?
- A refactor that makes the next change obvious is always worth it.

### Rule 6: Enforce Strict Modular Design and Deduplication, one concern per module

You MUST refactor the Rust codebase to enforce strong modular design. Each module MUST have a single, well-defined responsibility and expose a clean, minimal API. You MUST reduce coupling between modules and aggressively deduplicate logic by centralizing shared behavior into dedicated modules.

You SHOULD reorganize packages and rename modules where necessary to achieve clarity and proper separation of concerns.

Before making any changes, you MUST produce a clear, comprehensive refactoring plan. Only after the plan is complete and coherent should you proceed with implementation.

All changes MUST follow idiomatic Rust best practices, including proper ownership, error handling, and maintainable project structure.

For wath concerns the crates

- `openmm-data` = data parsing and pure logic only.
- `openmm` = rendering + gameplay. Each plugin owns one system.
- Bevy: systems communicate via `Commands`, events, resources — never reach into unrelated entities.
- `openmm-data` logic must be pure (same input → same output). `Result`/`Option` over panics. No `unsafe` without documented invariant.

### Rule 7: Logging is cheap, use it

Logging debug/info/warn/error is fine and encouraged — the logger level can be adjusted. Logs are invaluable for diagnosing gameplay issues without a debugger attached.

### Rule 8: Document engine findings

Document what each file and format does, how it is structured in raw LOD data, and what each field means. Add details to the `docs/` folder. Once a field is understood, lock it down with a test.

### Rule 9: Know when to create a new file

Before adding significant code to an existing file, ask: does this belong here, or does it deserve its own module? A new `.rs` file is warranted when:
- The new type or system has a distinct responsibility not already represented in the file (e.g. a new `minimap.rs` rather than appending minimap logic to `hud.rs`)
- The code is reusable and might be imported from multiple places — shared utilities, data types, or helpers should be isolated, not buried
- The existing file would grow large enough that finding things becomes difficult (rough heuristic: >300 lines is a signal worth noticing)
- The code enforces a different abstraction boundary (e.g. a new parser in `openmm-data/` rather than tacking it onto an unrelated module)

Conversely, do **not** create a file just to have one. A 20-line helper that is only used in one place belongs in that file, not in its own module. Premature splitting fragments context and makes the codebase harder to navigate.

The test: if you can describe the new file in one sentence with a clear noun — "manages door animation state", "parses the NPC name table" — it probably earns its own file. If the description is vague or just "helpers", keep it inline.

### Rule 10: Review before you ship

- Run `make fix` (auto-applies clippy + fmt). Then `make lint` must pass cleanly.
- Re-read the diff: clear names? dead code? unused imports? leftover `todo!()`?
- Add meaningful comments that explain complex stuff or the objective of functions that might not be completely clear or confused.

### Coordinate conversion

MM6 coordinate system: X right, Y forward, Z up. Bevy: X right, Y up, Z = -Y_mm6.

- `openmm_data::odm::mm6_to_bevy(x, y, z)` — converts i32 MM6 coords to `[f32; 3]` Bevy coords (no height scaling)
- Height values from the heightmap are scaled by `ODM_HEIGHT_SCALE` (32.0) separately

### Shared image/sampler helpers (assets/mod.rs)

- `dynamic_to_bevy_image(img)` — converts `image::DynamicImage` to Bevy `Image`
- `repeat_linear_sampler()` — repeating UV with linear filtering (sky, water in spawn_world)
- `repeat_sampler()` — repeating UV with default filtering (BSP model textures, water during loading)
- `nearest_sampler()` — nearest-neighbor filtering (terrain atlas)

### Game states

- `Menu` -> `Loading` -> `Game`
- Videos play inline via `InlineVideo` components in screen .ron files (no separate Video state). SMK audio for non-looping clips uses `PlaybackMode::Once` (not `Despawn`) so Bevy’s end-of-playback despawn never races with `LoadScreen` / layer teardown on the same entity.
- `GameEvent::PlayVideo` from EVT is not yet wired to the screen system (logs a warning).
- Loading state runs a step-based pipeline: ParseMap -> BuildTerrain -> BuildAtlas -> BuildModels -> BuildBillboards -> PreloadSprites -> Done
- Map switching (console `load` command or boundary crossing) transitions Game -> Loading -> Game

### HUD views

- `UiState` resource (`game/ui/mod.rs`) holds `mode: UiMode` and `footer: FooterText`
- `UiMode` controls active view: `World`, `Building`, `NpcDialogue`, `Chest`, `Inventory`, `Stats`, `Rest`
- When `UiMode` is not `World`: game time freezes (`Time<Virtual>` paused), player input disabled
- Gate gameplay systems with `.run_if(|ui: Res<UiState>| ui.mode == UiMode::World)`
- Use `OverlayImage` resource to display a background image in the viewport inner area
- `viewport_inner_rect()` returns the area inside all four HUD borders (for overlay positioning)
- `viewport_rect()` returns the 3D camera viewport area (extends behind border4 on the left)

### Event dispatch

- `GameEvent` enum in `openmm_data::evt`: SpeakInHouse, MoveToMap, OpenChest, Hint, ChangeDoorState, PlaySound, StatusText, LocationName
- `EventQueue` resource — any system can push events, processed one per frame by `process_events`
- Sub-events use `push_front()` for depth-first processing
- UI-opening events (SpeakInHouse, OpenChest) block the queue until UiMode returns to World
- MoveToMap uses `LoadRequest` + `GameState::Loading` pipeline (same as boundary crossing and debug map switch)
- `game/interaction/` is trigger-only — detects player interaction, pushes events to queue
- `game/events/` handles all event logic (scripting, event handlers, map transitions)
- `ChangeDoorState` triggers door open/close/toggle via `BlvDoors` resource

### Screen scripting

- `screens/scripting.rs` — flat action executor for screen RON files
- Action strings in `on_click`, `on_hover`, `on_end`, `keys` are one of:
  - Screen action (bare): `LoadScreen("menu")`, `ShowSprite("icon")`, `Hint("text")`, `Quit()`, `NewGame()`
  - EVT proxy (`evt:` prefix): `evt:PlaySound(75)`, `evt:Hint("text")`, `evt:StatusText("text")`
  - Control flow: `Compare("condition")`, `Else()`, `End()`
- Compare/Else/End: flat, single-level. Compare sets a flag; actions skip if flag is false; Else flips; End resets.
- Condition expressions: `quest_bit(N)`, `not quest_bit(N)`, `map_var(N) == X`, `gold > X`, `food < X`
- EVT proxy pushes `GameEvent` to `EventQueue` — same sink as the original EVT system, no interference
- `execute_actions()` returns dispatchable actions after control flow evaluation; `runtime.rs` handles the actual side effects

### Indoor maps (BLV) and doors

- Door faces are spawned as individual entities (not batched with static geometry) for animation
- `DoorFace` component tracks door_index and per-vertex door offset indices
- `BlvDoors` resource holds runtime state for all doors (direction, speed, offsets, state)
- `ClickableFaces` resource holds clickable face geometry for ray-plane intersection
- `indoor_interact_system` casts ray from camera, tests against clickable faces, dispatches EVT events
- `door_animation_system` advances door timers and updates mesh vertex positions
- Door states: Open/Closed (terminal), Opening/Closing (animating with proportional time reversal)
- Door vertex animation: base offset + direction * distance, converted via `mm6_to_bevy()`
- Pristine DLV files have empty door vertex data — doors spawn as entities but won't animate

### Sound system

- `SoundManager` resource: DSounds + SndArchive + audio handle cache
- `PlaySoundEvent` = looping spatial audio; `PlayUiSoundEvent` = once-and-despawn non-positional; `PlayMusicEvent` = map music
- Music from filesystem `Music/{track}.mp3`, NOT from LOD. Spatial scale `1.0/1000.0`.

> See `docs/sound-system.md` for full details.

### Sprites and actors

- NPC sprites resolved from DSFT at runtime; preloaded via `build_npc_sprite_table()`. Cache key: `"root@v2p223@64x128"`.
- `SpriteSheet`: `states[state][frame] = [Handle<StandardMaterial>; 5]` (5 directions). Lazy-spawned in distance-sorted, time-budgeted batches.
- `distance_culling` hides entities beyond draw distance; `billboard_face_camera` skips `SpriteSheet` entities.

> Sprite loading details, wander AI, and interaction system: see `docs/actors-and-sprites.md`.

### Loading pipeline details

- `LoadRequest`: set `map_name` + optional `spawn_position`/`spawn_yaw` before entering `GameState::Loading`.
- `loading_setup` removes all per-map resources — if you add a new one, register it there or it persists across map changes.
- Spawn priorities: indoor → `start_points[0]`; outdoor → save pos → "party start" decoration → origin.

> Full pipeline details: see `docs/loading-pipeline.md`.

### Render scale

- `render_scale` config (0.1–1.0, default 1.0): renders 3D at a fraction of the window resolution. HUD stays at native res.
- Console: `render_scale 0.5` or `rs 0.5`. Essential on 4K displays — borderless mode ignores `width`/`height` and renders at native monitor resolution.
- At scale < 1.0, the 3D camera renders to a low-res off-screen image (`RenderScaleState` resource). A UI `ImageNode` stretches the result to fill the viewport area.
- At scale 1.0 (default), renders directly to the window viewport with no overhead.
- `viewport.rs` manages the render target lifecycle: creates the image + display node when scaling down, tears it down when returning to 1.0.

### Player details

- `PlayerSettings`: speed=1024, fly_speed=4096, eye_height=160, gravity=9800, collision_radius=24.
- FOV 75° outdoor / 60° indoor. `SpatialListener` on `PlayerCamera` child entity, not `Player` root.
- `PlayerInputSet` system set label — dependent systems must run `.after(PlayerInputSet)`.

> Full player settings and physics/collision: see `docs/player-physics.md`.

### In-game clock (GameTime)

- `GameTime` resource in `game/state/time.rs` — authoritative in-game clock, 1 real second = 1 game minute.
- Epoch: midnight, January 1, Year 1000 (Monday). Default start: 9:00am.
- `time_of_day() -> f32`: 0.0 = midnight, 0.5 = noon, 0.75 = 6pm — consumed by lighting and sky.
- `format_datetime() -> String`: e.g. `"Monday Jan 1 1000 9:00am"`.
- `calendar_date() -> (year, month, day)`, `hour()`, `minute()`, `day_of_week()`.
- Pauses automatically when `UiMode ≠ World`; can be manually paused with console `time stop` / `time start`.
- `GameTimePlugin` registered in `InGamePlugin`. Do NOT put time-related state in `WorldState`.

### WorldState and game variables

- `WorldState` = runtime source of truth; `ActiveSave` = current .mm6 LOD archive (replaces old JSON GameSave).
- `GameVariables`: `map_vars[100]` (reset on map change), `quest_bits`, `autonotes`, gold=200, food=7.
- `Party`: 4 fixed members; `active_target` set by `ForPartyMember` EVT opcode.

> Full details (party skills, map events, NPC tables, save format): see `docs/game-state.md`.

> HUD internals, lighting, sky, and terrain shaders: see `docs/hud-rendering.md`.

### Map names and save system

- `MapName` enum: `Outdoor(OdmName)` or `Indoor(String)`. `TryFrom<&str>` parses `"oute3"` (5 chars, starts with `"out"`) as outdoor; anything else as indoor.
- `OdmName` supports directional navigation: `go_north/go_south/go_east/go_west` return `Option<OdmName>` (None at boundary). Valid columns `'a'–'e'`, rows `'1'–'3'`.
- `ActiveSave` resource: current .mm6 LOD archive. Default spawn: `[-10178, 340, 11206]` yaw -38.7°. New game copies `new.lod` → `data/saves/autosave1.mm6`.
- `GameAssets` resource: wraps `Assets` + quest bit names. `lod()` for decoded sprites, bitmaps, icons, fonts.

### Developer console

- Toggle with Tab (only when `cfg.console = true`). The console occupies the top 40% (`CONSOLE_HEIGHT_FRACTION = 0.4`) of the inner viewport.
- Screen editor is a separate startup mode. Launch with `--editor` (or `make run-editor`) in builds that include the `openmm/editor` feature.
- Editor sound preview uses the sound effects handler path (`PlayUiSoundEvent`) even in dedicated editor mode.
- Editor key binding UI accepts canonical keys up to `F11` (not `F12`).
- Command history: up/down arrow keys navigate; `saved_input` preserves the draft while browsing.
- Max output lines: `MAX_OUTPUT_LINES = 50`. Beyond this, oldest lines are removed.
- Available commands include: `load <map>`, `msaa <0/1/2/4>`, `fullscreen`, `borderless`, `windowed`, `exit`, `lighting <enhanced|flat>`, `fog <start> <end>`, `music <vol>`, `sfx <vol>`, `render_scale <0.25–1.0>` (alias `rs`), and others. Type `help` in-game for the current list.

### openmm-data crate structure

See `docs/openmm-data-crate.md` for full module listing.

## Documentation Index

Files in `docs/` — keep this list in sync (Rule 2):

- `docs/actors-and-sprites.md` — actor/NPC/monster sprite system, DSFT resolution, variants, caching, wander AI, interaction
- `docs/terrain-tileset.md` — tile table format, tileset enums, atlas generation
- `docs/sound-system.md` — SoundManager, events, music, footsteps, spatial audio
- `docs/loading-pipeline.md` — LoadRequest, pipeline steps, PreparedWorld, spawn priorities
- `docs/player-physics.md` — PlayerSettings, camera, input, gravity, collision, slopes, doors
- `docs/game-state.md` — WorldState, GameVariables, Party, MapEvents, save system
- `docs/hud-rendering.md` — HUD camera, elements, UiState/FooterText, minimap, lighting, sky, terrain shaders
- `docs/openmm-data-crate.md` — full openmm-data crate module listing
- `docs/todo.md` — upcoming work, ordered by priority
- `docs/superpowers/plans/` — implementation plans for completed features (BLV, doors, HUD, events, sound, party, actors)
- `docs/superpowers/specs/` — design specs for completed features

## Debugging Game Data

**Always redump and inspect the output before digging into code. Raw decompressed data is fine to read; never try to parse compressed LOD data by hand.**

**Never dig through LOD binary data by hand when investigating a bug or format.** The `data/dump/` directory is a pre-dumped, human-readable cache of game data — always start there.

Dump commands:
```
make dump_assets   # runs openmm_data::bin::dump_assets — writes maps, actors, decorations, etc. to data/dump/
make dump_sounds   # extracts audio to data/dump/sounds/; logs RIFF size/form, warns if form is not WAVE, scans for MIDI signatures only past the 12-byte RIFF header when present
cargo run --example dump_events -- oute3   # EVT / billboard events for a specific map
cargo run --example dump_npc_json          # NPC table → data/dump/npc.json, data/dump/npc2.json
```

File naming convention in `data/dump/`:
- `<map>.odm.json` — outdoor map: header, tile data, spawn points, billboard list, BSP models (height/tile/attr maps skipped — raw bytes)
- `<map>.blv.json` — indoor map: vertices, faces, sector info, light positions, door list
- `<map>.ddm.json` — actor/NPC roster for a map (parsed from DDM)
- `*.json` — structured tables (NPCs, etc.) — useful for small-to-medium datasets
- Prefer `.txt` over `.json` for large geometry data (mesh faces, vertex lists) — JSON is too verbose there

**Workflow:** if you're unsure what a field contains or suspect a parsing bug, re-run the relevant dump, diff the output against expectations in `data/dump/`, then write the test. Do not add `println!` to the parser and re-run the game. Dumps are faster, repeatable, and leave no debug noise in production code.

When adding a new parser or extending an existing one, add a corresponding dump path so future investigators can inspect the data without reading binary.

If a field is missing from the dump output, or the output is confusing or wrong, **fix the dump first** — do not work around it. A better dump is a permanent improvement that helps every future investigation. Treat bad dump output the same way you treat a failing test: the dump is broken, fix it before moving on.

## Common Mistakes

Known pitfalls that have caused bugs or wasted time — read before starting any task:

- **`dynamic_linking` in release builds**: `bevy = { features = ["dynamic_linking", ...] }` is set unconditionally in `openmm/Cargo.toml`. This is intentional for fast dev iteration but **must not be used in release binaries** — the Bevy shared library won't be present on other machines. CI release builds must disable it (`--no-default-features` or a release feature flag). Do not add it to `openmm-data`.
- **MM6 vs MM7 formats**: OpenEnroth documents MM7. Field offsets, struct layouts, and enum values are often different. Always verify against MMExtension or original MM6 data before trusting OpenEnroth.
- **Stale map resources on reload**: `loading_setup` explicitly removes `PreparedWorld`, `PreparedIndoorWorld`, `BlvDoors`, `DoorColliders`, `ClickableFaces`, `TouchTriggerFaces`. If you add a new per-map resource, add it to that cleanup list or it will persist across map changes.
- **UiMode-gated systems**: any system that touches player state or game time must be gated with `.run_if(|ui: Res<UiState>| ui.mode == UiMode::World)`. Forgetting this causes gameplay to run during dialogues and inventory.
- **`PlayerInputSet` ordering**: systems that read player position or react to player input must run `.after(PlayerInputSet)`. Incorrect ordering causes one-frame lag or missed input.
- **Coordinate conversion direction**: MM6 Y is forward (into screen), Bevy Z is backward. The conversion is `bevy_z = -mm6_y`. Getting this sign wrong causes mirrored maps.
- **Camera child vs Player root**: `SpatialListener` is on `PlayerCamera`, not `Player`. Spatial audio attached to the wrong entity will compute wrong distances.
- **`push_front` vs `push_back` in EventQueue**: sub-events from within a running event must use `push_front()` (depth-first). Using `push_back()` delays them until all pending events finish, causing wrong ordering.

## Conventions

- Rust 2024 edition
- Bevy 0.18 ECS patterns: plugins, systems, components, resources
- MM6 coordinate system: X right, Y forward, Z up -> Bevy: X right, Z = -Y, Y = Z. Use `openmm_data::odm::mm6_to_bevy()` for conversions.
- Terrain is a 128x128 heightmap grid with 512-unit tile scale (u8 height values, multiplied by ODM_HEIGHT_SCALE=32)
- Per-state entity markers (InGame, InLoading, etc.) for automatic cleanup on state exit
- Use `OdmName::to_string()` for map filenames instead of inline `format!("out{}{}.odm", ...)`
- Use `assets::dynamic_to_bevy_image()` and sampler helpers instead of inline `Image::from_dynamic()` / `ImageSamplerDescriptor` blocks
- Keep this CLAUDE.md up to date when dependency versions, architecture, or conventions change
- Document notable engine findings in `docs/` — see `docs/actors-and-sprites.md` for the actor/sprite system, but create more sections based on the topic.
- Use gpg no sign when you commit

## Upcoming Work / TODOs

See `docs/todo.md`.
