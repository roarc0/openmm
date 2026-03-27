# OpenMM

Open-source reimplementation of the Might and Magic VI engine in Rust. Targeting MM6 first, with MM7/MM8 support planned (compatible data formats).

## Current State

- Terrain rendering with textures (outdoor maps / ODM files)
- BSP model rendering (buildings) without textures
- Billboards partially implemented (commented out due to library issues)
- Fly camera with basic movement controls
- Splash screen and menu scaffolding
- Map switching between outdoor zones (H/J/K/L keys)

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
- **`map_viewer`** — Bevy 0.13.2 application that renders the world

### map_viewer structure

- `main.rs` — App entry point
- `lib.rs` — GamePlugin, state machine (Splash → Menu → Game)
- `world.rs` — WorldPlugin, loads LodManager, manages map state
- `odm.rs` — OdmPlugin, terrain mesh generation, BSP model spawning, map switching
- `player.rs` — Fly camera with keyboard/mouse controls
- `dev.rs` — Debug/dev tools
- `menu.rs` — Main menu UI
- `splash.rs` — Splash screen
- `world/sky.rs`, `world/sun.rs` — Sky and sun rendering

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

- Rust 2021 edition
- Bevy ECS patterns: plugins, systems, components, resources
- MM6 coordinate system: X right, Y forward, Z up → Bevy: X right, Z = -Y, Y = Z
- Terrain is a 128x128 heightmap grid with 512-unit tile scale
