# Sound System Design

## Overview

Add sound support to OpenMM: parse the MM6 sound descriptor table (`dsounds.bin`), read the sound file container (`audio.snd`), and play sounds in-game with 3D positional audio via Bevy.

## openmm-data crate: Two New Modules

### `dsounds.rs` — Sound Descriptor Table

Parses `dsounds.bin` from `icons.openmm-data`. This is a binary table mapping sound IDs to filenames, similar to `dtile.rs` for tiles.

**MM6 record format (112 bytes):**

| Field | Type | Size | Description |
|-------|------|------|-------------|
| name | `[u8; 32]` | 32 | Filename in audio.snd (e.g., "walkgrass") |
| sound_id | `u32` | 4 | Unique sound identifier |
| sound_type | `u32` | 4 | 0=level, 1=system, 2=swap, 3=unknown, 4=lock |
| attributes | `u32` | 4 | Flags: 0x1=locked, 0x2=3D positional |
| _runtime | `[u8; 68]` | 68 | Runtime pointers, always zero on disk |

**File format:**
```
[u32] sound_count
[DSoundRecord * sound_count] records
```

**Public API:**
```rust
pub struct DSounds {
    pub items: Vec<DSoundInfo>,
}

pub struct DSoundInfo {
    pub name: String,        // cleaned filename
    pub sound_id: u16,       // unique ID (u16 matches monlist/ddeclist references)
    pub sound_type: u32,
    pub is_3d: bool,         // attribute flag 0x2
}

impl DSounds {
    pub fn new(lod_manager: &LodManager) -> Result<Self, ...>;
    pub fn get_by_id(&self, id: u16) -> Option<&DSoundInfo>;
}
```

Follows the existing `repr(C)` struct + `Cursor` + `read_exact` unsafe pattern from `dtile.rs`/`dsft.rs`.

### `snd.rs` — Audio Container Reader

Reads `audio.snd`, a standalone container format (NOT a LOD archive). Contains WAV files, optionally zlib-compressed.

**Container format:**
```
[u32] entry_count
[SndEntry * entry_count] index
[...] file data (concatenated)
```

**Entry format (52 bytes):**

| Field | Type | Size | Description |
|-------|------|------|-------------|
| name | `[u8; 40]` | 40 | Filename (e.g., "walkgrass.wav") |
| offset | `u32` | 4 | Byte offset to data in container |
| size | `u32` | 4 | Compressed size |
| decompressed_size | `u32` | 4 | Original size (0 = not compressed) |

**Public API:**
```rust
pub struct SndArchive {
    entries: HashMap<String, SndEntry>,
    data: Vec<u8>,  // full file bytes
}

impl SndArchive {
    pub fn open(path: &Path) -> Result<Self, ...>;
    pub fn get(&self, name: &str) -> Option<Vec<u8>>;  // returns decompressed WAV bytes
    pub fn list(&self) -> Vec<&str>;  // list all filenames
    pub fn exists(&self, name: &str) -> bool;
}
```

- Filenames are lowercased for case-insensitive lookup
- If `decompressed_size > 0`, data is zlib-decompressed before returning
- Returns raw WAV bytes ready for Bevy's `AudioSource`

## openmm crate: `game/sound/` Module

### Structure

```
game/sound/
  mod.rs       — SoundPlugin, SoundManager resource
  music.rs     — Music playback (moved from odm.rs)
  effects.rs   — Sound effect playback with spatial audio
```

### `SoundManager` Resource

Central resource holding sound data and a cache:

```rust
#[derive(Resource)]
pub struct SoundManager {
    snd_archive: SndArchive,          // audio.snd reader
    dsounds: DSounds,                  // sound ID -> name mapping
    cache: HashMap<u16, Handle<AudioSource>>,  // avoid re-adding same WAV
}

impl SoundManager {
    /// Load a sound by ID into Bevy assets, returning the handle.
    /// Caches on first load.
    pub fn load_sound(&mut self, id: u16, audio_sources: &mut Assets<AudioSource>) -> Option<Handle<AudioSource>>;
}
```

### `music.rs` — Music Playback

Extracted from `odm.rs` lines 373-399:

- `MapMusic` marker component (moved from odm.rs)
- `play_music(commands, sound_manager, track: u8, volume: f32)` — loads MP3 from `Music/{track}.mp3`, spawns `AudioPlayer` entity with `PlaybackMode::Loop`
- `stop_music(commands, query)` — despawns `MapMusic` entities
- `odm.rs` `spawn_world` calls these instead of inlining audio logic

### `effects.rs` — Sound Effects with Spatial Audio

- `SpatialListener` component on the player camera
- `play_sound_at(commands, sound_manager, id: u16, position: Vec3)` — spawns spatial audio entity at world position
- `play_ui_sound(commands, sound_manager, id: u16)` — plays non-positional UI sound
- Entities with `sound_id` (decorations from `ddeclist`, monsters from `monlist`) can have sounds attached via their `Transform`

### SpatialListener Setup

The player camera entity gets `SpatialListener::default()` added in `player.rs`. Bevy uses this to compute 3D attenuation based on distance from sound sources.

## Loading Flow

1. During `LoadingPlugin` (or app startup), `SndArchive::open()` reads `sounds/audio.snd` from the game data directory
2. `DSounds::new()` parses `dsounds.bin` from `icons.openmm-data` via `LodManager`
3. Both are stored in `SoundManager` resource
4. Sounds are loaded on-demand: first `play_sound_at(id)` call looks up name via `DSounds`, extracts WAV via `SndArchive`, adds to Bevy `Assets<AudioSource>`, caches the handle

## What Moves from `odm.rs`

- `MapMusic` component definition (line 10-12)
- Music spawn/despawn logic (lines 373-399)
- `audio_sources: ResMut<Assets<AudioSource>>` and `existing_music: Query` params removed from `spawn_world`
- `spawn_world` calls `SoundManager` / music functions instead

## Testing Strategy

1. **openmm-data crate unit tests**: Parse `dsounds.bin`, verify record count and spot-check known sound IDs/names
2. **openmm-data crate unit tests**: Open `audio.snd`, list entries, extract a known WAV file, verify it starts with RIFF header
3. **Integration**: Load a map, verify decorations with `sound_id` produce audio at their positions

## Future Considerations

- Sound pooling (limit simultaneous sounds) — not needed initially, Bevy handles basic mixing
- Looping ambient sounds for decorations (fountains, etc.) — check `ddeclist` attributes for loop flags
- Monster sounds (4 IDs per monster: attack, hit, die, fidget) — wire up when combat exists
- Volume falloff tuning for spatial audio (Bevy defaults should be reasonable to start)
