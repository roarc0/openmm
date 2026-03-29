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

## Goal

Reach a playable state: character movement on terrain, NPCs, monsters, combat, indoor maps (BLV), UI overlays, dialogue, and quests.

## Build

```
cargo build
```

Requires MM6 game data files. Set `OPENMM_6_PATH` env var to the game data directory (defaults to `./target/mm6/data` for LOD files).

## Architecture

Cargo workspace with two crates:

- **`lod`** — Library for reading MM6 data formats: LOD archives, ODM (outdoor maps), BSP models, tile tables, palettes, sprites/billboards, images
- **`openmm`** — Bevy 0.18 game engine application

### openmm crate structure

```
src/
  main.rs              — App entry point
  lib.rs               — GamePlugin, GameState enum (Splash -> Menu -> Loading -> Game)
  config.rs            — BevyConfigPlugin (window, vsync, diagnostics)
  assets/mod.rs        — GameAssets resource, shared image/sampler helpers
  ui_assets.rs         — UiAssets resource (cached UI textures from LOD)
  save.rs              — GameSave resource (player position, map state)
  states/
    mod.rs             — State plugins wiring
    splash.rs          — SplashPlugin (1-second splash screen)
    menu.rs            — MenuPlugin (main menu, settings)
    loading.rs         — LoadingPlugin (step-based map loader with progress UI)
  game/
    mod.rs             — InGamePlugin, InGame marker component
    world.rs           — WorldPlugin (sky, sun sub-plugins)
    world/sky.rs       — Sky plane with bitmap texture
    world/sun.rs       — Directional light + animated fake sun
    odm.rs             — OdmPlugin (spawns terrain/models, lazy entity spawning)
    player.rs          — PlayerPlugin (Player entity, terrain following, camera, controls)
    collision.rs       — Ground height probing, building colliders
    interaction.rs     — InteractionPlugin (trigger-only: pushes events to queue, exit input, hover hints)
    event_dispatch.rs  — EventDispatchPlugin, EventQueue, process_events system
    dev.rs             — DevPlugin (wireframe, FPS/position HUD, debug map switching)
    utils.rs           — Helpers (random_color)
    hud/
      mod.rs           — HudPlugin, HudView resource, freeze system
      borders.rs       — Border layout, HudDimensions, letterbox, viewport_rect
      minimap.rs       — Minimap, compass strip, tap frames
      footer.rs        — FooterText resource + rendering
      overlay.rs       — OverlayImage, viewport_inner_rect, overlay spawn/despawn
    entities/
      mod.rs           — EntitiesPlugin, shared components (WorldEntity, EntityKind, Billboard, AnimationState)
      actor.rs         — Actor component, NPC_SPRITES constant
      sprites.rs       — SpriteCache, SpriteSheet, directional sprite loading and animation
      decoration.rs    — Decoration-related types
    terrain_material/  — Custom terrain shader with water extension
```

## Useful Resources


### Rules

## Keeping Documentation Updated

> **⚠️ CRITICAL: This section is mandatory, not aspirational.** Stale docs are worse than no docs — they actively mislead developers and AI agents into building on false assumptions. A recent audit found that 4 out of 6 feature docs had drifted so far from reality that they required complete rewrites. This happens when documentation updates are treated as optional. **They are not optional.**

### Rule 1: Every Behaviour Change Includes a Doc Update

Every code change that alters behaviour must include a documentation update in the same commit or PR. This is not optional. A PR that changes behaviour without updating docs is incomplete and should not be merged.

### Rule 2: Keep This Index in Sync

When any doc is added, renamed, or removed in `docs/`, update the Documentation Index section in this file in the same commit. A doc that exists but isn't indexed here is invisible to Claude Code.

### Rule 3: Use best practices for game development

When implementing a feature, always separate concerns and organize logic into well-defined, modular components, functions, and services. Write clean, maintainable code with clear naming, minimal duplication, and low coupling. Prefer simplicity, readability, and extensibility over quick but messy solutions

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

- `GameEvent` enum in `lod::evt` (renamed from EventAction): SpeakInHouse, MoveToMap, OpenChest, Hint
- `EventQueue` resource — any system can push events, processed one per frame by `process_events`
- Sub-events use `push_front()` for depth-first processing
- UI-opening events (SpeakInHouse, OpenChest) block the queue until HudView returns to World
- MoveToMap uses `LoadRequest` + `GameState::Loading` pipeline (same as boundary crossing and debug map switch)
- `interaction.rs` is trigger-only — detects player interaction, pushes events to queue
- `event_dispatch.rs` handles all event logic (image loading, view switching, map transitions)

### Player

- Player entity with Transform, terrain-following movement (bilinear heightmap interpolation)
- Camera is a child entity with independent pitch control
- Movement: arrow keys for forward/back/rotate, A/D for strafe, mouse for look
- Escape toggles cursor grab

### Sprites and actors

- `NPC_SPRITES` constant in `actor.rs` — the single source of truth for peasant sprite prefixes (standing/walking pairs). Used by odm.rs (lazy_spawn fallback) and loading.rs (preloading).
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
- `ddm.rs` — DDM file parser (actors/NPCs per map)
- `monlist.rs` — Monster list (dmonlist.bin) with sprite name resolution
- `mapstats.rs` — Map statistics (monster groups per map zone)

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
- Reference OpenEnroth (C++ MM7 decompilation) when investigating MM6 formats
- Use gpg no sign when you commit

## Future Ideas

- Lazy LOD loading with LRU eviction cache — raw LOD bytes are ~50-100MB total, manageable for now but could be optimized if memory becomes a concern
- Async map loading via Bevy AsyncComputeTaskPool — current step-based sync loading is fast enough for now
- Bitmap/atlas caching in GameAssets — cache decoded bitmaps and tile atlases to avoid re-decoding on map changes
