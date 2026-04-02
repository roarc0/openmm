# OpenMM

Open-source reimplementation of the Might and Magic VI engine in Rust. Targeting MM6 first, with MM7/MM8 support planned (compatible data formats).

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

### Rules

## Keeping Documentation Updated

> **⚠️ CRITICAL: This section is mandatory, not aspirational.** Stale docs are worse than no docs — they actively mislead developers and AI agents into building on false assumptions. A recent audit found that 4 out of 6 feature docs had drifted so far from reality that they required complete rewrites. This happens when documentation updates are treated as optional. **They are not optional.**

### Rule 1: Every Behaviour Change Includes a Doc Update

Every code change that alters behaviour must include a documentation update in the same commit or PR. This is not optional. A PR that changes behaviour without updating docs is incomplete and should not be merged.

### Rule 2: Keep This Index in Sync

When any doc is added, renamed, or removed in `docs/`, update the Documentation Index section in this file in the same commit. A doc that exists but isn't indexed here is invisible to Claude Code.

### Rule 3: No hardcoded magic numbers or inline data tables

Never hardcode format-specific constants, range boundaries, enum mappings, or data tables inline in game code. All format knowledge belongs in the `lod` crate as proper parsing functions that expose clean APIs. Game code should call a function (e.g., `dtile.tileset_for_tile(id)`) — never re-derive format logic with raw ranges or magic numbers. If the data comes from a binary file, the parser owns the logic.

### Rule 4: Use best practices for game development

When implementing a feature, always separate concerns and organize logic into well-defined, modular components, functions, and services. Write clean, maintainable code with clear naming, minimal duplication, and low coupling. Prefer simplicity, readability, and extensibility over quick but messy solutions

### Rule 4: It's ok to log debug noisy stuff and, or info,warn,error. The logger level can be adjusted and it will help debugging.

### Rule 5: Document and keep updated all the relevant findings about the original engine

Document what each file,format does how it is structured in terms of lod raw data and what each field is for.
Add the complete details in the docs folder.

### Rule 6: Create tests instead of Examples

Creating tests will help keeping the code working and enhances the confidence that our changes did not break the functionality.
Once you understand something or know for sure some detail of some data asset from the game like npc_id to npc.name, sprite name, number of variants palette_id also for other entities, you must write a test that will ensure bugs will be caught right away.

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

- `HudView` resource controls the active view: `World`, `Building`, `Chest`, `Inventory`, `Stats`, `Rest`
- When `HudView` is not `World`: game time freezes (`Time<Virtual>` paused), player input disabled
- Gate gameplay systems with `.run_if(resource_equals(HudView::World))`
- Use `OverlayImage` resource to display a background image in the viewport inner area
- `viewport_inner_rect()` returns the area inside all four HUD borders (for overlay positioning)
- `viewport_rect()` returns the 3D camera viewport area (extends behind border4 on the left)

### Event dispatch

- `GameEvent` enum in `lod::evt` (renamed from EventAction): SpeakInHouse, MoveToMap, OpenChest, Hint, ChangeDoorState
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

### Player

- Player entity with Transform, terrain-following movement (bilinear heightmap interpolation)
- Camera is a child entity with independent pitch control
- Movement: arrow keys for forward/back/rotate, A/D for strafe, mouse for look
- Escape toggles cursor grab

### Sprites and actors

- NPC sprites are resolved from the DSFT table at runtime (no hardcoded sprite list). Preloaded during loading screen via `build_npc_sprite_table()`.
- Sprite variant system: monsters have difficulty variants (1=A base, 2=B blue tint, 3=C red tint). The `tint_variant()` function in `lod::image` applies color shifts.
- Cache key format: `"root"`, `"root@v2"`, `"root@64x128"`, or `"root@64x128@v2"` — encodes sprite root, optional minimum dimensions, and optional variant.
- Entity spawning is lazy: `spawn_world` creates terrain/models immediately, then `lazy_spawn` spawns billboards, NPCs, and monsters in batches per frame (time-budgeted) sorted by distance from player.

### lod crate structure

- `lod.rs` — LOD archive reader (MM6's container format)
- `odm.rs` — Outdoor map parser (heightmap, tiles, models, billboards, spawn points), `mm6_to_bevy()` coordinate helper
- `bsp_model.rs` — BSP model geometry (buildings, structures)
- `dtile.rs` — Tile table and texture atlas generation
- `palette.rs` — Color palette handling (8-bit indexed color)
- `image.rs` — Sprite/texture image decoding, `tint_variant()` for monster color variants
- `billboard.rs` — Billboard/decoration sprite manager
- `ddeclist.rs`, `dsft.rs` — Decoration and sprite frame tables
- `dlv.rs` — DLV file parser (indoor delta: actors and doors per BLV map)
- `ddm.rs` — DDM file parser (actors/NPCs per map)
- `monlist.rs` — Monster list (dmonlist.bin) with sprite name resolution
- `mapstats.rs` — Map statistics (monster groups per map zone)
- `game/actors.rs` — `Actor`/`Actors`: per-map DDM actor roster with pre-resolved sprites and palette variants
- `game/decorations.rs` — `DecorationEntry`/`Decorations`: per-map ODM billboard roster with pre-resolved sprite names, dimensions, and DSFT metadata
- `game/monster.rs` — `Monster`/`Monsters`: per-map spawn resolution (MapStats + monlist + DSFT → one `Monster` per group member); also `resolve_entry()` and `resolve_sprite_group()` for DDM actor sprite resolution
- `dsounds.rs` — Sound descriptor table (dsounds.bin): sound ID -> filename mapping
- `snd.rs` — Audio.snd container reader: extracts/decompresses WAV files

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

## Future Ideas

- Lazy LOD loading with LRU eviction cache — raw LOD bytes are ~50-100MB total, manageable for now but could be optimized if memory becomes a concern
- Async map loading via Bevy AsyncComputeTaskPool — current step-based sync loading is fast enough for now
- Bitmap/atlas caching in GameAssets — cache decoded bitmaps and tile atlases to avoid re-decoding on map changes
