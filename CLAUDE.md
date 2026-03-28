# OpenMM

Open-source reimplementation of the Might and Magic VI engine in Rust. Targeting MM6 first, with MM7/MM8 support planned (compatible data formats).

## Current State

- Terrain rendering with textures (outdoor maps / ODM files)
- BSP model rendering (buildings) without textures
- Billboards partially implemented (commented out due to library issues)
- Player entity with terrain-following movement and first-person camera
- Loading screen with step-based map loader
- Splash screen and menu scaffolding
- Debug map switching between outdoor zones (H/J/K/L keys)

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
  lib.rs               — GamePlugin, GameState enum (Splash → Menu → Loading → Game)
  config.rs            — BevyConfigPlugin (window, vsync, diagnostics)
  assets/mod.rs        — GameAssets resource wrapping LodManager
  states/
    mod.rs             — State plugins wiring
    splash.rs          — SplashPlugin (1-second splash screen)
    menu.rs            — MenuPlugin (main menu, settings)
    loading.rs         — LoadingPlugin (step-based map loader with progress UI)
  game/
    mod.rs             — InGamePlugin, InGame marker component
    world.rs           — WorldPlugin (sky, sun sub-plugins)
    world/sky.rs       — Sky cylinder with bitmap texture
    world/sun.rs       — Directional light + animated fake sun
    odm.rs             — OdmPlugin (spawns terrain/models from PreparedWorld)
    player.rs          — PlayerPlugin (Player entity, terrain following, camera, controls)
    dev.rs             — DevPlugin (wireframe, FPS/position HUD, debug map switching)
    utils.rs           — Helpers (random_color)
```

### Game states

- `Splash` → `Menu` → `Loading` → `Game`
- Loading state runs a step-based pipeline: ParseMap → BuildTerrain → BuildAtlas → BuildModels → Done
- Map switching (dev H/J/K/L) transitions Game → Loading → Game

### Player

- Player entity with Transform, terrain-following movement (bilinear heightmap interpolation)
- Camera is a child entity with independent pitch control
- Movement: arrow keys for forward/back/rotate, A/D for strafe, mouse for look
- Escape toggles cursor grab

### lod crate structure

- `lod.rs` — LOD archive reader (MM6's container format)
- `odm.rs` — Outdoor map parser (heightmap, tiles, models, billboards)
- `bsp_model.rs` — BSP model geometry (buildings, structures)
- `dtile.rs` — Tile table and texture atlas generation
- `palette.rs` — Color palette handling (8-bit indexed color)
- `image.rs` — Sprite/texture image decoding
- `billboard.rs` — Billboard/decoration sprite manager
- `ddeclist.rs`, `dsft.rs` — Decoration and sprite frame tables

## Conventions

- Rust 2024 edition
- Bevy 0.18 ECS patterns: plugins, systems, components, resources
- MM6 coordinate system: X right, Y forward, Z up → Bevy: X right, Z = -Y, Y = Z
- Terrain is a 128x128 heightmap grid with 512-unit tile scale (u8 height values, multiplied by ODM_HEIGHT_SCALE=32)
- Per-state entity markers (InGame, InLoading, etc.) for automatic cleanup on state exit
- Keep this CLAUDE.md up to date when dependency versions, architecture, or conventions change
- Document notable engine findings in `docs/` — see `docs/actors-and-sprites.md` for the actor/sprite system, but create more sections based on the topic.
- Reference OpenEnroth (C++ MM7 decompilation) when investigating MM6 formats

## Future Ideas

- Lazy LOD loading with LRU eviction cache — raw LOD bytes are ~50-100MB total, manageable for now but could be optimized if memory becomes a concern
- Async map loading via Bevy AsyncComputeTaskPool — current step-based sync loading is fast enough for now
- Bitmap/atlas caching in GameAssets — cache decoded bitmaps and tile atlases to avoid re-decoding on map changes
