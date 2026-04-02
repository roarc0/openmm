# OpenMM

Faithful open-source reimplementation of the Might and Magic VI engine in Rust. The goal is to reproduce original MM6 gameplay — movement, combat, dialogue, quests — with clean, maintainable code. Graphical improvements are welcome where they enhance the experience without compromising accuracy. MM7/MM8 support is planned (compatible data formats).

## Current State

- Terrain rendering with textures (outdoor maps / ODM files)
- BSP model rendering (buildings) with textures
- Billboards (decorations: trees, rocks, fountains) with sprite caching
- NPCs and monsters with directional sprites, wander AI, and animation
- Player entity with terrain-following movement and first-person camera
- Loading screen with step-based map loader and sprite preloading
- Splash screen and menu scaffolding
- Developer console (Tab key) with commands: load, msaa, fullscreen, borderless, windowed, exit
- Seamless map boundary transitions between adjacent outdoor zones
- Indoor map rendering (BLV files) with face-based geometry and collision
- Indoor door interaction: clickable faces dispatch EVT events, door animation state machine

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

### Rule 7: Logging is cheap, use it

Logging debug/info/warn/error is fine and encouraged — the logger level can be adjusted. Logs are invaluable for diagnosing gameplay issues without a debugger attached.

### Rule 8: Document engine findings

Document what each file and format does, how it is structured in raw LOD data, and what each field means. Add details to the `docs/` folder. Once a field is understood, lock it down with a test.

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

- `SoundManager` resource holds DSounds table + SndArchive + cached Bevy audio handles
- `PlayMusicEvent` — any system can request map music (track number + volume)
- `PlaySoundEvent` — plays a sound at a 3D world position (spatial audio via SpatialListener on player camera)
- `PlayUiSoundEvent` — plays a non-positional UI sound
- Sound files are WAV stored in `Sounds/Audio.snd` (zlib-compressed), resolved by name from dsounds.bin
- Sounds are loaded on-demand and cached by sound_id
- Decorations with `sound_id > 0` trigger PlaySoundEvent on spawn
- `SoundManager::load_sound` validates WAV: checks RIFF header, WAVE tag, and PCM format byte (audio_fmt == 1). Unsupported formats are skipped with a warning.
- `chest_open_sound_id` is pre-cached at startup by looking up `"openchest0101"` in DSounds.
- Spatial scale: `1.0 / 1000.0` — approximately 2 terrain tiles (1024 units) maps to normal attenuation distance.
- `PlaySoundEvent` is **looping** spatial audio (e.g. decoration ambient sounds, footsteps). `PlayUiSoundEvent` spawns a **once-and-despawn** non-positional entity.
- Music is loaded **from the filesystem** (`Music/{track}.mp3` relative to the game data directory), NOT from a LOD archive. The `MapMusic` component marks the music entity so it can be despawned on map change.
- Music volume syncs from `cfg.music_volume` whenever `GameConfig` changes (e.g. after a console command).
- Footstep sound IDs by terrain tileset (from OpenEnroth `SoundEnums.h`): Grass=93, Snow=97, Desert=91, Volcanic=88, Dirt=92, Water=101, CrackedSwamp/Swamp=100, Road=96.
- Footsteps use a looping audio entity that is despawned and respawned when the player transitions to a different tileset. No footstep sound plays in fly mode.

### Sprites and actors

- NPC sprites are resolved from the DSFT table at runtime (no hardcoded sprite list). Preloaded during loading screen via `build_npc_sprite_table()`.
- Sprite variant system: monsters have difficulty variants (1=A base, 2=B blue tint, 3=C red tint). The `tint_variant()` function in `lod::image` applies color shifts.
- Cache key format: `"root"`, `"root@v2"`, `"root@v2p223"`, `"root@64x128"`, `"root@64x128@v2"`, or `"root@64x128@v2p223"` — encodes sprite root, optional minimum dimensions, variant, and palette_id (palette only appended when variant > 1 AND palette_id > 0).
- Entity spawning is lazy: `spawn_world` creates terrain/models immediately, then `lazy_spawn` spawns billboards, NPCs, and monsters in batches per frame (time-budgeted) sorted by distance from player.
- `SpriteSheet` component: `states[state_idx][frame_idx]` = `[Handle<StandardMaterial>; 5]` (5 directions). State 0 = standing, state 1 = walking (if available).
- Walking sprites are loaded first (tend to be wider) to determine target dimensions; standing sprites are then padded to match so neither animation gets stretched.
- `load_entity_sprites` falls back to progressively shorter root names if a sprite isn't found (e.g. `"gobla"` → `"gobl"` → `"gob"`).
- `FacingYaw` component for directional decorations (e.g. ships) whose sprite depends on camera angle; distinct from `Actor.facing_yaw` which drives live rotation.
- `distance_culling` system hides entities beyond `cfg.draw_distance`; runs before sprite updates each frame.
- `billboard_face_camera` skips entities with `SpriteSheet` (those are updated by `update_sprite_sheets` instead).

### Wander AI

- `Actor` component fields: `name`, `hp`/`max_hp`, `move_speed`, `initial_position`, `guarding_position`, `tether_distance`, `wander_timer`, `wander_target`, `facing_yaw`, `hostile`.
- Wander uses position-based seeding (`initial_position.x * 7.3 + z * 13.7`) to desynchronize actors — using shared `time.elapsed_secs()` as seed caused all actors to fire collision checks in the same frame.
- Walk timing: 3–6 s walking, 2–5 s idle. Walk speed is capped at 60 units/step regardless of `move_speed`. Tether effectively defaults to 300 units minimum.
- Actors doing collision use `BuildingColliders::resolve_movement` with radius=20, eye_height=140.

### Interaction system

- `BuildingInfo` component on BSP model entities (model name, position, event_ids list).
- `DecorationInfo` on billboard entities (event_id, position, billboard_index for SetSprite targeting).
- `NpcInteractable` on NPC entities (name, position, npc_id — zero means no dialogue).
- Interaction trigger: `KeyE`, `Enter`, left mouse click, or gamepad East. Range: `INTERACT_RANGE = 250` (close) or `RAYCAST_RANGE = 2000` (ray).
- Ray targeting uses a cone with `RAY_ANGLE_TAN = 0.12` (~7°); minimum perpendicular threshold `RAY_MIN_PERP = 60` for very close objects.
- `interaction_input` also handles exiting Building/NpcDialogue/Chest views (presses E/Enter/click while view is non-World).

### Loading pipeline details

- `LoadRequest` resource: set `map_name` + optionally `spawn_position` and `spawn_yaw` before transitioning to `GameState::Loading`.
- `loading_setup` removes all previous map resources (`PreparedWorld`, `PreparedIndoorWorld`, `BlvDoors`, `DoorColliders`, `ClickableFaces`, `TouchTriggerFaces`) to avoid stale state across map changes.
- `PreparedWorld` resource (outdoor): contains `Odm`, terrain/water meshes and textures, models, decorations, actors, resolved monsters, start_points, sprite_cache, billboard_cache, water_cells, terrain_lookup, music_track.
- `PreparedIndoorWorld` resource (indoor): contains models, start_points, collision geometry (walls/floors/ceilings), door definitions, door face meshes, clickable_faces, **touch_trigger_faces**, map_base string for EVT loading, actors from DLV.
- `TouchTriggerFaces` resource (`EVENT_BY_TOUCH` flag): faces that fire EVT events when the player walks within proximity. Each has face_index, event_id, center, radius (half the bounding box diagonal for floor faces).
- `LoadingProgress` private resource tracks all intermediate data between pipeline steps; discarded once `PreparedWorld`/`PreparedIndoorWorld` is produced.
- `PreloadQueue` batches sprite preloading across frames: sprite_roots `(root, variant, palette_id)` triples + billboard_idx + music_resolved.
- Player spawn priorities: **indoor** → `start_points[0]` (set from LoadRequest.spawn_position for MoveToMap events, or sector center as fallback). **outdoor** → non-zero save position → decoration named "party start" / "party_start" → origin.

### Player details

- `PlayerSettings` defaults: `speed=2048`, `fly_speed=4096`, `eye_height=160`, `gravity=9800`, `jump_velocity=1300`, `collision_radius=24`, `max_slope_height=200` (step-up for terrain), `max_xz` clamps to the playable ODM boundary.
- FOV: 75° outdoors, 60° indoors (aligned with OpenEnroth values). Camera spawns with -8° pitch tilt.
- `SpatialListener` (ear gap=4.0) is on the `PlayerCamera` child entity, not the `Player` root.
- Walk speed is half of `settings.speed`; `cfg.always_run` skips the halving. Fly mode uses `fly_speed`.
- Fly mode: toggle with F2 or gamepad Select; stored in `WorldState.player.fly_mode`.
- `MouseLookEnabled` resource: initialized from `cfg.mouse_look`, toggled at runtime with CapsLock (if `cfg.capslock_toggle_mouse_look`).
- `MouseSensitivity` resource: adjusted at runtime with Home (increase) / End (decrease) keys in 5-unit steps.
- Gamepad: left stick moves, right stick looks. Unmapped controllers (e.g. GameSir) expose right stick as LeftZ/RightZ axes — the code has a fallback for this.
- `PlayerInputSet` system set label — order other systems after player input using `.after(PlayerInputSet)`.

### Physics and collision

- `gravity_system` applies gravity (`settings.gravity = 9800` units/s²), vertical velocity, and ceiling/floor clamping each frame. Does NOT run when `HudView != World`.
- Effective ground Y = `max(terrain_height, bsp_floor_height)`. Ceiling Y from `BuildingColliders::ceiling_height_at`.
- Slope sliding: triggered on outdoor terrain when slope angle > `MAX_SLOPE_ANGLE = 0.6` rad (~35°). Slide speed: `SLOPE_SLIDE_SPEED = 4000`. Not applied indoors.
- `BuildingColliders::resolve_movement` iterates 3× to handle corners. `MAX_STEP_UP = 50` — walls shorter than this are steppable.
- `CollisionWall`: plane (normal + dist) + XZ polygon for containment (ray-cast + edge distance). `CollisionTriangle`: 3 vertices + precomputed AABB + normal for barycentric floor height sampling.
- `WaterMap` resource (outdoor, `cells: Vec<bool>`) + `WaterWalking` resource (toggled by EVT). Both are per-map resources inserted by `setup_collision_data`.
- `DoorColliders` resource (indoor): rebuilt from `DoorCollisionFace` data + live door positions each frame by the door animation system; used by player movement to be pushed out of moving doors.

### WorldState and game variables

- `WorldState` resource is the single source of truth for runtime player/map state. `GameSave` is the persistent serialization form; call `WorldState::write_to_save` / `read_from_save` to transfer.
- `WorldState.time_of_day`: 0.0=midnight, 0.25=sunrise, 0.375=9am (default), 0.5=noon, 0.75=sunset. Drives sun position, ambient light, and sky color.
- `GameVariables` fields: `map_vars[100]` (reset on map change), `quest_bits: HashSet<i32>`, `autonotes: HashSet<i32>`, `gold` (starting 200), `food` (starting 7), `reputation`.
- Use `GameVariables::set_qbit` / `clear_qbit` / `has_qbit` and `add_autonote` — these log every change at info level.

### Party system

- `Party` resource holds exactly 4 `PartyMember`s (indices 0–3 = Player1–4 in EVT).
- Default party: Zoltan (Knight), Roderick (Paladin), Alexei (Archer), Serena (Cleric), all level 1.
- `Party::active_target` is set by the `ForPartyMember` EVT opcode and used by subsequent variable reads/writes.
- `Party::max_skill(target, var)` returns the highest skill level across all members matching `target`.
- `PartyMember::skills` is `[u8; 31]` indexed by `EvtVariable::skill_index()` (covers skills 0x38–0x56). Use `set_skill` / `get_skill` with `EvtVariable`.

### Map events loading

- `MapEvents` resource: `evt` (map-specific EVT), `houses` (TwoDEvents), `npc_table` (StreetNpcs), `name_pool` (NpcNamePool), `generated_npcs` (HashMap for peasant actors with npc_id ≥ 5000).
- `global.evt` is always loaded and *merged* into the map EVT (entries from global extend, not override, per event_id).
- `TwoDEvents` (2devents.txt) is **NOT loaded for indoor maps** (`indoor=true` → `houses = None`). Building SpeakInHouse events still work but have no metadata.
- `generated_npcs` is populated lazily at actor spawn time for generic peasant/actor entries.
- `EventQueue` internally uses `VecDeque<EventSequence>` where each `EventSequence` is the full step list for one event_id. Use `push_all(event_id, evt)` (script) or `push_single(GameEvent)` (synthesized). `clear()` to abort the queue.

### HUD internals

- HUD uses a 2D camera (order=1, no clear color, `IsDefaultUiCamera`) that renders on top of the 3D camera (order=0).
- Reference dimensions: `REF_W=640`, `REF_H=480`. All asset positions scale by `scale_x = window_w / 640` and `scale_y = window_h / 480` independently (non-uniform scaling for widescreen).
- Six border pieces (border1–6) plus tap frames (tap1=morning, tap2=day, tap3=evening, tap4=night), compass strip, 8 directional arrows (mapdir1=N…mapdir8=NW), and footer strip.
- `FooterText` resource: `set(text)` for hover hints; `set_status(text, duration, now)` for timed status messages that **lock out hover hints** until the timer expires. `tick(now)` must be called each frame to expire the lock.
- `StatsBar` displays gold and food from `WorldState.game_vars`, rendered as bitmapped text using `GameFonts`. Updates only when values change.
- Minimap: 512×512 LOD icon image (e.g. `"oute3"`) scrolled to keep the player dot at center. Zoom=3.0×. Direction arrow selected from 8 frames based on player yaw.

### Lighting and sky

- Day/night cycle duration: `DAY_CYCLE_SECS = 1800` (30 minutes real time). `WorldState.time_of_day` advances each frame while in `HudView::World`.
- Sun color: warm (reddish-orange) at horizon → white at noon. Illuminance: 300–1200 lux during day, 0 at night.
- `cfg.lighting`: `"enhanced"` = full PBR with directional light; any other value = unlit mode. Unlit sets `base_color = srgb(0.69, 0.69, 0.69)`, enhanced uses `srgb(1.4, 1.4, 1.4)` for models. The mode toggle updates both `StandardMaterial` (models) and `TerrainMaterial` (terrain) in the same frame.
- Sky is a large flat quad above the camera rendered with a custom `SkyMaterial` (WGSL shader in `shaders/sky.wgsl`) with time-scrolling UVs. The quad follows the camera each frame. Indoor maps skip the sky dome and set `ClearColor = BLACK`.
- Sky texture name comes from `Odm.sky_texture`; falls back to `"plansky1"` if empty or missing.

### Terrain material

- `TerrainMaterial = ExtendedMaterial<StandardMaterial, WaterExtension>` (defined in `terrain_material.rs`).
- `WaterExtension` adds two extra textures: `water_texture` (animated water), `water_mask` (R8 image, white = water pixel). The WGSL shader (`shaders/terrain_water.wgsl`) replaces cyan marker pixels in the terrain atlas with animated water.
- Water mask uses **nearest** filtering to keep cell boundaries sharp even when the terrain atlas uses linear filtering.

### Map names and save system

- `MapName` enum: `Outdoor(OdmName)` or `Indoor(String)`. `TryFrom<&str>` parses `"oute3"` (5 chars, starts with "out") as outdoor; anything else (with or without `.odm`/`.blv` extension) as indoor.
- `OdmName` supports directional navigation: `go_north/go_south/go_east/go_west` return `Option<OdmName>` (None at map boundary). Valid column range: `'a'–'e'`, row range: `'1'–'3'`.
- `GameSave` is serialized as JSON to `target/saves/{slot}.json`. Default spawn position is `[-10178, 340, 11206]` at yaw -38.7° (MM6 starting area of oute3).
- `GameAssets` resource (from `assets/mod.rs`) wraps `LodManager` + `GameData` + `BillboardManager`. `game_lod()` returns a `GameLod<'_>` view for decoded sprites, bitmaps, icons, and fonts.

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

- `docs/actors-and-sprites.md` — actor/NPC/monster sprite system, DSFT resolution, variants, caching
- `docs/terrain-tileset.md` — tile table format, tileset enums, atlas generation
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
