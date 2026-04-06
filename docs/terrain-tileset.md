# Terrain Tileset System

## Overview

MM6 outdoor maps use a tile-based terrain system. Each map has a 128x128 grid of tiles. Each tile references a terrain type via a tile_map value (0-255).

## Data Structures

### tile_data ([u16; 8]) — per-map terrain configuration

Stored in the ODM header. Even indices are tileset IDs, odd indices are starting tile indices in dtile.bin:

| Index | Meaning | Example (oute3) |
|-------|---------|-----------------|
| [0] | Primary terrain tileset ID | 0 (dirt) |
| [1] | Primary terrain start index in dtile | 90 |
| [2] | Water tileset ID | 5 (water) |
| [3] | Water start index in dtile | 126 |
| [4] | Secondary terrain tileset ID | 3 (desert) |
| [5] | Secondary terrain start index in dtile | 198 |
| [6] | Road tileset ID | 22 (road variant) |
| [7] | Road start index in dtile | 774 |

### tile_map ([u8; 16384]) — terrain grid

128x128 array. Each value (0-255) falls into a range that determines the terrain group:

| Range | Terrain Group | tile_data index for tileset |
|-------|---------------|----------------------------|
| 0-89 | Dirt/base terrain | tile_data[0] |
| 90-125 | Primary terrain (= same as base) | tile_data[0] |
| 126-161 | Water tiles | tile_data[2] |
| 162-197 | Secondary terrain | tile_data[4] |
| 198-255 | Road tiles | tile_data[6] |

### Tileset enum values

From OpenEnroth (confirmed by raw data inspection):

| Value | Tileset |
|-------|---------|
| 0 | Dirt (base/default) |
| 1 | Grass |
| 2 | Snow |
| 3 | Desert |
| 4 | Dirt |
| 5 | Water |
| 6 | Badlands |
| 7 | Swamp |
| 8+ | Road (various variants, e.g. 22 = cobble) |

## Coordinate Conversion

World position to tile grid:
- `col = (bevy_x / 512.0) as i32`
- `row = (-bevy_z / 512.0) as i32`  (Z is negated due to MM6→Bevy Y-flip)
- `tile_index = row * 128 + col`

## API

```rust
// In openmm-data crate:
openmm_data::terrain::tileset_at(&odm, bevy_x, bevy_z) -> Option<Tileset>

// In openmm crate (via PreparedWorld):
prepared.terrain_at(bevy_x, bevy_z) -> Option<Tileset>
```

## Walking Sounds

Each tileset maps to a walking sound ID (from OpenEnroth SoundEnums.h):

| Tileset | Walk Sound ID | Sound Name |
|---------|--------------|------------|
| Grass | 93 | walkgrass |
| Dirt | 92 | walkdirt |
| Desert | 91 | walkdesert |
| Snow | 97 | walksnow |
| Water | 101 | walkwater |
| Badlands | 88 | walkbadlands |
| Swamp | 100 | walkswamp |
| Road | 96 | walkroad |

Sound files are stored as IMA-ADPCM WAV in `Sounds/Audio.snd`, decoded to 16-bit PCM at runtime by `SndArchive::get()`.
