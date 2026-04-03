# OpenMM

Faithful open-source reimplementation of the Might and Magic VI engine in Rust. The goal is to reproduce original MM6 gameplay — movement, combat, dialogue, quests — with clean, maintainable code. Graphical improvements are welcome where they enhance the experience without compromising accuracy. MM7/MM8 support is planned (compatible data formats).

## Current State

- Terrain rendering with textures (outdoor maps / ODM files)
- BSP model rendering (buildings) with textures
- Billboards (decorations: trees, rocks, fountains) with sprite caching, animation, flicker, and point lights
- NPCs and monsters with directional sprites, wander AI, and animation
- Player entity with terrain-following movement and first-person camera
- Loading screen with step-based map loader and sprite preloading
- Splash screen and menu scaffolding
- Developer console (Tab key) with commands: load, msaa, fullscreen, borderless, windowed, exit, time
- Seamless map boundary transitions between adjacent outdoor zones
- Indoor map rendering (BLV files) with face-based geometry and collision
- Indoor door interaction: clickable faces dispatch EVT events, door animation state machine
- Full in-game calendar clock (1 real second = 1 game minute, starts Jan 1 Year 1000 9am) with day/night cycle, sun arc, and ambient lighting

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

Requires MM6 game data files. Set `OPENMM_6_PATH` env var to the game data directory (defaults to `./target/mm6/data` for LOD files).

Uses mold linker for fast linking (`.cargo/config.toml`). Install: `pacman -S mold` or `apt install mold`.

## Architecture

Cargo workspace with two crates:

- **`lod`** — Library for reading MM6 data formats: LOD archives, ODM (outdoor maps), BSP models, tile tables, palettes, sprites/billboards, images. we should not include openmm here.
- **`openmm`** — Bevy 0.18 game engine application

## Useful Resources

- MMExtension from grayface that is just a modding engine for mm6, it can tell you much more accurately the data structures. You should find a copies of resources in the target folder.
- OpenEnroth (C++ MM7 decompilation) when investigating MM6 formats but be careful because mm7 are different. Use it as a last resort because if we take the wrong path here we might introduce very nasty bugs. mm7 is different in many ways. Only look at this resource if you are desperate.

## Reference Documentation

- [The Rust Book](https://doc.rust-lang.org/book/) — authoritative guide to Rust language features and idioms
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) — naming, error handling, and API design conventions to follow in the `lod` crate
- [Rust Reference](https://doc.rust-lang.org/reference/) — language specification for edge cases
- [Bevy Book](https://bevyengine.org/learn/book/introduction/) — ECS concepts, plugins, systems, schedules
- [Bevy Migration Guides](https://bevyengine.org/learn/migration-guides/) — what changed between versions; consult before upgrading
- [Bevy Examples](https://github.com/bevyengine/bevy/tree/main/examples) — canonical usage patterns for rendering, audio, UI, input

## Rules

### Keeping Documentation Updated

> **⚠️ CRITICAL: This section is mandatory, not aspirational.** Stale docs are worse than no docs — they actively mislead developers and AI agents into building on false assumptions. A recent audit found that 4 out of 6 feature docs had drifted so far from reality that they required complete rewrites. This happens when documentation updates are treated as optional. **They are not optional.**

### Rule 1: Every Behaviour Change Includes a Doc Update

Every code change that alters behaviour must include a documentation update in the same commit or PR. This is not optional. A PR that changes behaviour without updating docs is incomplete and should not be merged.

### Rule 2: Keep This Index in Sync

When any doc is added, renamed, or removed in `docs/`, update the Documentation Index section in this file in the same commit. A doc that exists but isn't indexed here is invisible to Claude Code.

### Rule 3: No hardcoded magic numbers or inline data tables

Never hardcode format-specific constants, range boundaries, enum mappings, or data tables inline in game code. All format knowledge belongs in the `lod` crate as proper parsing functions that expose clean APIs. Game code should call a function (e.g., `dtile.tileset_for_tile(id)`) — never re-derive format logic with raw ranges or magic numbers. If the data comes from a binary file, the parser owns the logic.

### Rule 4: Tests are mandatory for bug fixes and discoveries

When debugging, **write the test first**. A failing test that reproduces the bug is the most reliable way to isolate the problem, drive the fix, and prove it's solved — with no risk of re-introducing it later.

Every bug fix **must** include a regression test. A regression is when a bug that was fixed silently reappears later — usually because no test was guarding against it. Regression tests are the only reliable defence. No exception.

Once you have verified a concrete value from the original game data — a sprite name, palette ID, NPC ID, field offset, frame count, tile index — **encode it as a test immediately**. These ground-truth assertions are invaluable: they catch parser regressions, wrong assumptions, and format misunderstandings before they cascade into gameplay bugs. Examples of good tests:
- parsing `dmonlist.bin` yields monster X with sprite root `"foo"` and palette 42
- NPC ID 7 resolves to sprite name `"npc007"` via DSFT
- a specific ODM tile ID maps to the correct tileset enum variant

### Rule 5: Clean, decoupled code is not optional

This project will be maintained and extended over years. Taking shortcuts now means paying a steep price later. Separation of concerns, clear naming, low coupling, and minimal duplication are not style preferences — they are the foundation that makes new features and bug fixes possible without breaking everything else. Before adding code, ask: is this in the right place? Can I simplify this? A 20-line refactor that makes the next 100 lines obvious is always worth it.

### Rule 6: One concern per module

Always separate concerns: `lod` owns data parsing, `openmm` owns rendering and gameplay. Within `openmm`, each plugin owns one system (rendering, AI, input, audio). Avoid monolithic systems that do too much. If a system needs many unrelated resources, it is doing too much.

Bevy-specific: systems communicate via `Commands`, events, and resources — not by reaching directly into unrelated entities or calling other systems. Parsing logic in `lod` must be pure (same input → same output, no side effects). Prefer `Result`/`Option` over panicking in library code; `unwrap()` is only acceptable when the case is truly unreachable and a comment explains why. Avoid `unsafe` unless there is no alternative; document the invariant it relies on.

### Rule 7: Logging is cheap, use it

Logging debug/info/warn/error is fine and encouraged — the logger level can be adjusted. Logs are invaluable for diagnosing gameplay issues without a debugger attached.

### Rule 8: Document engine findings

Document what each file and format does, how it is structured in raw LOD data, and what each field means. Add details to the `docs/` folder. Once a field is understood, lock it down with a test.

### Rule 9: Know when to create a new file

Before adding significant code to an existing file, ask: does this belong here, or does it deserve its own module? A new `.rs` file is warranted when:
- The new type or system has a distinct responsibility not already represented in the file (e.g. a new `minimap.rs` rather than appending minimap logic to `hud.rs`)
- The code is reusable and might be imported from multiple places — shared utilities, data types, or helpers should be isolated, not buried
- The existing file would grow large enough that finding things becomes difficult (rough heuristic: >300 lines is a signal worth noticing)
- The code enforces a different abstraction boundary (e.g. a new parser in `lod/` rather than tacking it onto an unrelated module)

Conversely, do **not** create a file just to have one. A 20-line helper that is only used in one place belongs in that file, not in its own module. Premature splitting fragments context and makes the codebase harder to navigate.

The test: if you can describe the new file in one sentence with a clear noun — "manages door animation state", "parses the NPC name table" — it probably earns its own file. If the description is vague or just "helpers", keep it inline.

### Rule 10: Review before you ship

Before committing, run `make fix` — this auto-applies clippy suggestions and formats the code. Then verify `make lint` passes cleanly. These steps are mandatory, not optional.

Re-read the diff with fresh eyes:
- Does every public function and type have a name that explains what it does without a comment?
- Is there dead code, unused imports, or `todo!()` left behind?
- Did you run `make fix` and does `make lint` pass cleanly?

A clean diff is a sign of respect for the next person who reads it — which is often you, six months later.

### Coordinate conversion

MM6 coordinate system: X right, Y forward, Z up. Bevy: X right, Y up, Z = -Y_mm6.

- `lod::odm::mm6_to_bevy(x, y, z)` — converts i32 MM6 coords to `[f32; 3]` Bevy coords (no height scaling)
- Height values from the heightmap are scaled by `ODM_HEIGHT_SCALE` (32.0) separately

### Shared image/sampler helpers (assets/mod.rs)

- `dynamic_to_bevy_image(img)` — converts `image::DynamicImage` to Bevy `Image`
- `repeat_linear_sampler()` — repeating UV with linear filtering (sky, water in spawn_world)
- `repeat_sampler()` — repeating UV with default filtering (BSP model textures, water during loading)
- `nearest_sampler()` — nearest-neighbor filtering (terrain atlas)

### Game states

- `Splash` -> `Menu` -> `Loading` -> `Game`
- Loading state runs a step-based pipeline: ParseMap -> BuildTerrain -> BuildAtlas -> BuildModels -> BuildBillboards -> PreloadSprites -> Done
- Map switching (console `load` command or boundary crossing) transitions Game -> Loading -> Game

### HUD views

- `HudView` resource controls the active view: `World`, `Building`, `NpcDialogue`, `Chest`, `Inventory`, `Stats`, `Rest`
- When `HudView` is not `World`: game time freezes (`Time<Virtual>` paused), player input disabled
- Gate gameplay systems with `.run_if(resource_equals(HudView::World))`
- Use `OverlayImage` resource to display a background image in the viewport inner area
- `viewport_inner_rect()` returns the area inside all four HUD borders (for overlay positioning)
- `viewport_rect()` returns the 3D camera viewport area (extends behind border4 on the left)

### Event dispatch

- `GameEvent` enum in `lod::evt`: SpeakInHouse, MoveToMap, OpenChest, Hint, ChangeDoorState, PlaySound, StatusText, LocationName
- `EventQueue` resource — any system can push events, processed one per frame by `process_events`
- Sub-events use `push_front()` for depth-first processing
- UI-opening events (SpeakInHouse, OpenChest) block the queue until HudView returns to World
- MoveToMap uses `LoadRequest` + `GameState::Loading` pipeline (same as boundary crossing and debug map switch)
- `interaction.rs` is trigger-only — detects player interaction, pushes events to queue
- `event_dispatch.rs` handles all event logic (image loading, view switching, map transitions)
- `ChangeDoorState` triggers door open/close/toggle via `BlvDoors` resource

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

### Player details

- `PlayerSettings`: speed=1024, fly_speed=4096, eye_height=160, gravity=9800, collision_radius=24.
- FOV 75° outdoor / 60° indoor. `SpatialListener` on `PlayerCamera` child entity, not `Player` root.
- `PlayerInputSet` system set label — dependent systems must run `.after(PlayerInputSet)`.

> Full player settings and physics/collision: see `docs/player-physics.md`.

### In-game clock (GameTime)

- `GameTime` resource in `game/game_time.rs` — authoritative in-game clock, 1 real second = 1 game minute.
- Epoch: midnight, January 1, Year 1000 (Monday). Default start: 9:00am.
- `time_of_day() -> f32`: 0.0 = midnight, 0.5 = noon, 0.75 = 6pm — consumed by lighting and sky.
- `format_datetime() -> String`: e.g. `"Monday Jan 1 1000 9:00am"`.
- `calendar_date() -> (year, month, day)`, `hour()`, `minute()`, `day_of_week()`.
- Pauses automatically when `HudView ≠ World`; can be manually paused with console `time stop` / `time start`.
- `GameTimePlugin` registered in `InGamePlugin`. Do NOT put time-related state in `WorldState`.

### WorldState and game variables

- `WorldState` = runtime source of truth; `GameSave` = JSON at `target/saves/{slot}.json`.
- `GameVariables`: `map_vars[100]` (reset on map change), `quest_bits`, `autonotes`, gold=200, food=7.
- `Party`: 4 fixed members; `active_target` set by `ForPartyMember` EVT opcode.

> Full details (party skills, map events, NPC tables, save format): see `docs/game-state.md`.

> HUD internals, lighting, sky, and terrain shaders: see `docs/hud-rendering.md`.

### Map names and save system

- `MapName` enum: `Outdoor(OdmName)` or `Indoor(String)`. `TryFrom<&str>` parses `"oute3"` (5 chars, starts with `"out"`) as outdoor; anything else as indoor.
- `OdmName` supports directional navigation: `go_north/go_south/go_east/go_west` return `Option<OdmName>` (None at boundary). Valid columns `'a'–'e'`, rows `'1'–'3'`.
- `GameSave` → JSON at `target/saves/{slot}.json`. Default spawn: `[-10178, 340, 11206]` yaw -38.7°.
- `GameAssets` resource: wraps `LodManager` + `GameData` + `BillboardManager`. `game_lod()` for sprites, bitmaps, icons, fonts.

### Developer console

- Toggle with Tab (only when `cfg.console = true`). The console occupies the top 40% (`CONSOLE_HEIGHT_FRACTION = 0.4`) of the inner viewport.
- Command history: up/down arrow keys navigate; `saved_input` preserves the draft while browsing.
- Max output lines: `MAX_OUTPUT_LINES = 50`. Beyond this, oldest lines are removed.
- Available commands include: `load <map>`, `msaa <0/1/2/4>`, `fullscreen`, `borderless`, `windowed`, `exit`, `lighting <enhanced|flat>`, `fog <start> <end>`, `music <vol>`, `sfx <vol>`, and others. Type `help` in-game for the current list.

### lod crate structure

- `lod.rs` — LOD archive reader (MM6's container format)
- `lod_data.rs` — raw LOD entry data helpers
- `odm.rs` — Outdoor map parser (heightmap, tiles, models, billboards, spawn points), `mm6_to_bevy()` coordinate helper
- `blv.rs` — Indoor map parser (BLV): vertices, faces, sectors, BSP nodes, lights, decorations, doors
- `bsp_model.rs` — BSP model geometry (buildings, structures)
- `dtile.rs` — Tile table and texture atlas generation
- `terrain.rs` — `TerrainLookup`: tileset queries by world position
- `palette.rs` — Color palette handling (8-bit indexed color)
- `image.rs` — Sprite/texture image decoding, `tint_variant()` for monster color variants
- `billboard.rs` — Billboard/decoration sprite manager
- `ddeclist.rs`, `dsft.rs` — Decoration and sprite frame tables
- `dlv.rs` — DLV file parser (indoor delta: actors and doors per BLV map)
- `ddm.rs` — DDM file parser (actors/NPCs per map)
- `dchest.rs` — Chest descriptor table
- `dobjlist.rs` — Object list descriptor table
- `doverlay.rs` — Overlay descriptor table
- `monlist.rs` — Monster list (dmonlist.bin) with sprite name resolution
- `mapstats.rs` — Map statistics (monster groups per map zone)
- `evt.rs` — EVT event script parser → `GameEvent` enum
- `twodevents.rs` — 2DEvents.txt parser (house/building event table)
- `enums.rs` — Shared MM6 enums (face flags, object types, etc.)
- `tft.rs` — TFT (tile frame table) parser
- `dsounds.rs` — Sound descriptor table (dsounds.bin): sound ID -> filename mapping
- `snd.rs` — Audio.snd container reader: extracts/decompresses WAV files
- `game/actors.rs` — `Actor`/`Actors`: per-map DDM actor roster with pre-resolved sprites and palette variants
- `game/decorations.rs` — `DecorationEntry`/`Decorations`: per-map ODM billboard roster with pre-resolved sprite names, dimensions, and DSFT metadata
- `game/monster.rs` — `Monster`/`Monsters`: per-map spawn resolution (MapStats + monlist + DSFT → one `Monster` per group member); also `resolve_entry()` and `resolve_sprite_group()` for DDM actor sprite resolution
- `game/npc.rs` — `NpcEntry`/`StreetNpcs`: street NPC roster with generated names
- `game/font.rs` — Font loading from LOD bitmaps
- `game/global.rs` — `GameData`: top-level container for all global game tables

## Documentation Index

Files in `docs/` — keep this list in sync (Rule 2):

- `docs/actors-and-sprites.md` — actor/NPC/monster sprite system, DSFT resolution, variants, caching, wander AI, interaction
- `docs/terrain-tileset.md` — tile table format, tileset enums, atlas generation
- `docs/sound-system.md` — SoundManager, events, music, footsteps, spatial audio
- `docs/loading-pipeline.md` — LoadRequest, pipeline steps, PreparedWorld, spawn priorities
- `docs/player-physics.md` — PlayerSettings, camera, input, gravity, collision, slopes, doors
- `docs/game-state.md` — WorldState, GameVariables, Party, MapEvents, save system
- `docs/hud-rendering.md` — HUD camera, elements, FooterText, minimap, lighting, sky, terrain shaders
- `docs/superpowers/plans/` — implementation plans for completed features (BLV, doors, HUD, events, sound, party, actors)
- `docs/superpowers/specs/` — design specs for completed features

## Common Mistakes

Known pitfalls that have caused bugs or wasted time — read before starting any task:

- **`dynamic_linking` in release builds**: `bevy = { features = ["dynamic_linking", ...] }` is set unconditionally in `openmm/Cargo.toml`. This is intentional for fast dev iteration but **must not be used in release binaries** — the Bevy shared library won't be present on other machines. CI release builds must disable it (`--no-default-features` or a release feature flag). Do not add it to `lod`.
- **MM6 vs MM7 formats**: OpenEnroth documents MM7. Field offsets, struct layouts, and enum values are often different. Always verify against MMExtension (in `target/`) or original MM6 data before trusting OpenEnroth.
- **Stale map resources on reload**: `loading_setup` explicitly removes `PreparedWorld`, `PreparedIndoorWorld`, `BlvDoors`, `DoorColliders`, `ClickableFaces`, `TouchTriggerFaces`. If you add a new per-map resource, add it to that cleanup list or it will persist across map changes.
- **HudView-gated systems**: any system that touches player state or game time must be gated with `.run_if(resource_equals(HudView::World))`. Forgetting this causes gameplay to run during dialogues and inventory.
- **`PlayerInputSet` ordering**: systems that read player position or react to player input must run `.after(PlayerInputSet)`. Incorrect ordering causes one-frame lag or missed input.
- **Coordinate conversion direction**: MM6 Y is forward (into screen), Bevy Z is backward. The conversion is `bevy_z = -mm6_y`. Getting this sign wrong causes mirrored maps.
- **Camera child vs Player root**: `SpatialListener` is on `PlayerCamera`, not `Player`. Spatial audio attached to the wrong entity will compute wrong distances.
- **`push_front` vs `push_back` in EventQueue**: sub-events from within a running event must use `push_front()` (depth-first). Using `push_back()` delays them until all pending events finish, causing wrong ordering.

## Conventions

- Rust 2024 edition
- Bevy 0.18 ECS patterns: plugins, systems, components, resources
- MM6 coordinate system: X right, Y forward, Z up -> Bevy: X right, Z = -Y, Y = Z. Use `lod::odm::mm6_to_bevy()` for conversions.
- Terrain is a 128x128 heightmap grid with 512-unit tile scale (u8 height values, multiplied by ODM_HEIGHT_SCALE=32)
- Per-state entity markers (InGame, InLoading, etc.) for automatic cleanup on state exit
- Use `OdmName::to_string()` for map filenames instead of inline `format!("out{}{}.odm", ...)`
- Use `assets::dynamic_to_bevy_image()` and sampler helpers instead of inline `Image::from_dynamic()` / `ImageSamplerDescriptor` blocks
- Keep this CLAUDE.md up to date when dependency versions, architecture, or conventions change
- Document notable engine findings in `docs/` — see `docs/actors-and-sprites.md` for the actor/sprite system, but create more sections based on the topic.
- Use gpg no sign when you commit

## Upcoming Work / TODOs

Ordered roughly by impact and readiness:

- **Texture animation (TFT)** — `tft.rs` parses the tile frame table but animated tile cycling is not yet implemented in the terrain shader; some water and lava tiles should cycle frames.
- **NPC dialogue text rendering** — `SpeakInHouse` events open a `HudView::NpcDialogue` placeholder; actual text rendering (font from LOD, scrolling lines) is not yet wired up.
- **Monster combat stats** — `MonList` entries have attack, HP, speed, resistances; combat is not yet implemented; needed for a playable state.
- **Chest / item system** — `DChest` and `DObjList` parsers exist; spawning items inside chests and allowing the player to pick them up is the next inventory step.
- **Save / load** — `GameSave` JSON skeleton exists; full round-trip persistence (party stats, map state, quest bits) is not yet implemented.
- **Street NPC identity randomization** — `peasant_identity()` currently uses spawn index as a deterministic seed; in MM6 each map load assigns fresh random names/professions. Should seed from a per-load RNG. Once save/load exists, identities should be persisted so NPCs don't re-roll on every visit.
- **Sky texture day/night variation** — ODM has a single `sky_texture` field; no format-level time-of-day variants. Need to investigate: does the original MM6 engine swap sky textures based on time (e.g. `plansky1` at day vs a darker variant at night), or does it rely purely on color tinting? Check what sky bitmap names exist in the LOD (`bitmaps` archive), look for naming patterns like `plansky1`/`plansky2` or morning/night variants, and check MMExtension docs. Currently the sky texture is static — only `ClearColor` changes with time of day.
