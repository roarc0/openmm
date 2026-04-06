# OpenMM Engine Architecture Redesign

## Overview

Restructure the project from a map viewer into a proper game engine foundation. Rename `map_viewer` to `openmm`, add loading states, player entity with terrain-following movement, asset caching, and clean module organization.

## Crate Structure

Two crates (unchanged boundary):
- **`openmm-data`** — Pure Rust data parsing library (no Bevy dependency). Unchanged.
- **`openmm`** (renamed from `map_viewer`) — Bevy 0.18 game engine application.

## Module Structure

```
openmm/src/
  main.rs              — App entry point
  lib.rs               — GamePlugin, GameState enum, shared helpers
  config.rs            — BevyConfigPlugin (window, vsync, diagnostics)

  states/
    mod.rs             — State plugins wiring
    splash.rs          — SplashPlugin
    menu.rs            — MenuPlugin
    loading.rs         — LoadingPlugin, step-based map loader, loading screen UI

  game/
    mod.rs             — InGamePlugin (wires sub-plugins for Game state)
    world.rs           — WorldPlugin — sky, sun, ambient light
    odm.rs             — OdmPlugin — terrain mesh, BSP models
    player.rs          — PlayerPlugin — Player entity, terrain-following, camera
    dev.rs             — DevPlugin — wireframe, FPS, debug fly camera, map switching

  assets/
    mod.rs             — GameAssets resource: LodManager wrapper with caching
```

## Game States

```
Splash → Menu → Loading → Game
                  ↑          |
                  └──────────┘  (map change via dev keys or future gameplay)
```

- `Splash`: splash image, 1-second timer, transition to Menu
- `Menu`: main menu UI (New Game button → Loading)
- `Loading`: step-based map loader with progress display
- `Game`: active gameplay with player, world, dev tools

## Loading Pipeline

`LoadRequest` resource specifies what to load (map name). Steps run one per frame:

1. `ParseMap` — read & decompress ODM from GameAssets
2. `BuildTerrain` — generate mesh vertices, indices, UVs
3. `BuildAtlas` — decode tile textures, assemble atlas (uses cache)
4. `BuildModels` — process BSP models into meshes + materials
5. `Done` — store prepared data, transition to Game

On `OnEnter(Game)`, systems read prepared data and spawn entities.

Map switching (dev H/J/K/L) transitions Game → Loading → Game.

## Player Entity

```
Player (marker)
├── Transform — position/rotation in world
└── child: PlayerCamera
    └── Camera3d with fog settings
```

Components:
- `Player` — marker
- `PlayerSettings` — speed, sensitivity, eye_height
- `GroundHeight` — cached terrain height at player position

Movement:
- WASD/arrows for forward/back/strafe
- Mouse X for yaw, mouse Y for pitch (clamped)
- Terrain following: bilinear interpolation of heightmap, lerp to target height
- Bounded to playable area (88x88 tiles)
- Cursor grabbed in Game state, Escape releases

Debug fly camera (F1 toggle) in dev.rs — detaches from player, enables free flight.

## Asset Management

`GameAssets` resource wraps `LodManager`:

```rust
pub struct GameAssets {
    lod_manager: LodManager,
    palettes: Option<Palettes>,
    bitmap_cache: HashMap<String, DynamicImage>,
    atlas_cache: HashMap<[u16; 8], DynamicImage>,
}
```

- Lazy palette loading (cached on first access)
- Decoded bitmap/sprite caching
- Atlas caching keyed by tile_data configuration
- Replaces WorldSettings.lod_manager

## Entity Cleanup

Per-state marker components:
- `InSplash` — despawned on OnExit(Splash)
- `InMenu` — despawned on OnExit(Menu)
- `InLoading` — despawned on OnExit(Loading)
- `InGame` — despawned on OnExit(Game)

Single `despawn_all::<InGame>` handles terrain, models, sky, sun, player, debug overlays.

## What's NOT in scope

- Character creation / stat allocation
- Settings menus beyond current placeholder
- Indoor maps (BLV format)
- Billboard/sprite rendering
- Combat, NPCs, dialogue, quests
- Async loading (may upgrade later)
- Lazy LOD loading with LRU eviction (may upgrade later)
