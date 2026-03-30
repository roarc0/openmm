# Sound System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse MM6 sound data (dsounds.bin + audio.snd), extract WAV files, and play them in-game with 3D positional audio via Bevy.

**Architecture:** Two new modules in the `lod` crate (`dsounds.rs` for the sound descriptor table, `snd.rs` for the audio container). One new `sound/` module in the `openmm` crate that consolidates all audio (music + effects) behind a `SoundManager` resource with spatial audio support.

**Tech Stack:** Rust, Bevy 0.18 (built-in audio + spatial), flate2 (zlib, already a dependency), byteorder (already a dependency)

---

## File Map

### lod crate (new files)
- `lod/src/dsounds.rs` — Parse dsounds.bin sound descriptor table
- `lod/src/snd.rs` — Read audio.snd container, extract/decompress WAV files

### lod crate (modify)
- `lod/src/lib.rs` — Add `pub mod dsounds; pub mod snd;` exports

### openmm crate (new files)
- `openmm/src/game/sound/mod.rs` — SoundPlugin, SoundManager resource, SpatialListener setup
- `openmm/src/game/sound/music.rs` — Music playback (extracted from odm.rs)
- `openmm/src/game/sound/effects.rs` — Spatial sound effect playback

### openmm crate (modify)
- `openmm/src/game/mod.rs` — Add `pub(crate) mod sound;`, register SoundPlugin
- `openmm/src/game/odm.rs` — Remove MapMusic, music playback code; call SoundManager instead
- `openmm/src/game/player.rs` — Add `SpatialListener` to player camera

---

### Task 1: Parse dsounds.bin in lod crate

**Files:**
- Create: `lod/src/dsounds.rs`
- Modify: `lod/src/lib.rs`

**Context:** `dsounds.bin` lives in `icons.lod`. Format: `u32` count, then 1355 records of 112 bytes each. Each record has a 32-byte name, u32 sound_id, u32 type, u32 attributes, 68 bytes padding (runtime pointers, zeros on disk). The file is accessed via `LodManager::try_get_bytes("icons/dsounds.bin")` and may need decompression via `LodData::try_from()`. Follow the exact pattern in `lod/src/ddeclist.rs`.

- [ ] **Step 1: Write the failing test**

Add to `lod/src/dsounds.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::{get_lod_path, LodManager};
    use super::DSounds;

    #[test]
    fn read_dsounds_data_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsounds = DSounds::new(&lod_manager).unwrap();
        assert_eq!(dsounds.items.len(), 1355);
        // Record [1] should be "campfire" with id=4
        let campfire = &dsounds.items[1];
        assert_eq!(campfire.name(), Some("campfire".to_string()));
        assert_eq!(campfire.sound_id, 4);
    }

    #[test]
    fn get_by_id_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let dsounds = DSounds::new(&lod_manager).unwrap();
        let campfire = dsounds.get_by_id(4).expect("sound id 4 should exist");
        assert_eq!(campfire.name(), Some("campfire".to_string()));
        assert!(dsounds.get_by_id(9999).is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd lod && cargo test dsounds -- --nocapture 2>&1 | tail -5`
Expected: FAIL — module doesn't exist yet

- [ ] **Step 3: Write the dsounds.rs module**

Create `lod/src/dsounds.rs`:

```rust
use std::{
    error::Error,
    io::{Cursor, Read},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{lod_data::LodData, utils::try_read_name, LodManager};

pub struct DSounds {
    pub items: Vec<DSoundInfo>,
}

#[allow(dead_code)]
#[repr(C)]
#[derive(Clone)]
pub struct DSoundInfo {
    name_bytes: [u8; 32],
    pub sound_id: u32,
    pub sound_type: u32,
    pub attributes: u32,
    _runtime: [u8; 68],
}

impl Default for DSoundInfo {
    fn default() -> Self {
        Self {
            name_bytes: [0; 32],
            sound_id: 0,
            sound_type: 0,
            attributes: 0,
            _runtime: [0; 68],
        }
    }
}

impl DSoundInfo {
    pub fn name(&self) -> Option<String> {
        try_read_name(&self.name_bytes)
    }

    pub fn is_3d(&self) -> bool {
        (self.attributes & 0x0002) != 0
    }
}

impl DSounds {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes("icons/dsounds.bin")?)?;
        let data = data.data.as_slice();

        let mut cursor = Cursor::new(data);
        let item_count = cursor.read_u32::<LittleEndian>()?;
        let item_size = std::mem::size_of::<DSoundInfo>();
        let mut items = Vec::with_capacity(item_count as usize);

        for _ in 0..item_count {
            let mut item = DSoundInfo::default();
            cursor.read_exact(unsafe {
                std::slice::from_raw_parts_mut(&mut item as *mut _ as *mut u8, item_size)
            })?;
            items.push(item);
        }

        Ok(Self { items })
    }

    pub fn get_by_id(&self, id: u32) -> Option<&DSoundInfo> {
        self.items.iter().find(|s| s.sound_id == id)
    }
}
```

- [ ] **Step 4: Add module export to lib.rs**

In `lod/src/lib.rs`, add after the `pub mod dtile;` line:

```rust
pub mod dsounds;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd lod && cargo test dsounds -- --nocapture`
Expected: 2 tests PASS

- [ ] **Step 6: Commit**

```bash
git add lod/src/dsounds.rs lod/src/lib.rs
git commit --no-gpg-sign -m "feat(lod): add dsounds.bin parser for MM6 sound descriptor table"
```

---

### Task 2: Read audio.snd container in lod crate

**Files:**
- Create: `lod/src/snd.rs`
- Modify: `lod/src/lib.rs`

**Context:** `audio.snd` is at `{OPENMM_6_PATH}/../Sounds/Audio.snd` (path: `target/mm6/Sounds/Audio.snd`). It's a standalone file, NOT inside a LOD archive. Format: `u32` entry count (1526 entries), then 52-byte index entries (`[u8; 40]` name, `u32` offset, `u32` compressed_size, `u32` decompressed_size`), then raw data. Files are zlib-compressed when `decompressed_size > 0`. Decompressed data is standard WAV (RIFF header). Use `flate2` for decompression (already in Cargo.toml).

- [ ] **Step 1: Write the failing test**

Add to `lod/src/snd.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::SndArchive;
    use std::path::Path;

    fn snd_path() -> String {
        let data_path = crate::get_data_path();
        let base = Path::new(&data_path);
        // Audio.snd is at mm6/Sounds/Audio.snd, data_path is mm6 or mm6/data
        // Try common locations
        for candidate in &[
            base.join("../Sounds/Audio.snd"),
            base.join("Sounds/Audio.snd"),
        ] {
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
        // Fallback for test environment
        String::from("../target/mm6/Sounds/Audio.snd")
    }

    #[test]
    fn read_snd_archive_works() {
        let archive = SndArchive::open(snd_path()).unwrap();
        let entries = archive.list();
        assert!(entries.len() > 1000, "should have >1000 sound entries, got {}", entries.len());
        assert!(archive.exists("01archera_attack"), "should find 01archera_attack");
    }

    #[test]
    fn extract_wav_works() {
        let archive = SndArchive::open(snd_path()).unwrap();
        let wav = archive.get("01archera_attack").expect("should extract sound");
        // WAV files start with RIFF header
        assert!(wav.len() > 44, "WAV should be longer than header");
        assert_eq!(&wav[0..4], b"RIFF", "should start with RIFF");
        assert_eq!(&wav[8..12], b"WAVE", "should contain WAVE marker");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd lod && cargo test snd -- --nocapture 2>&1 | tail -5`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Write the snd.rs module**

Create `lod/src/snd.rs`:

```rust
use std::{
    collections::HashMap,
    error::Error,
    fs,
    io::{Cursor, Read},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::utils::try_read_name;

struct SndEntry {
    offset: u32,
    size: u32,
    decompressed_size: u32,
}

pub struct SndArchive {
    entries: HashMap<String, SndEntry>,
    data: Vec<u8>,
}

impl SndArchive {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let data = fs::read(path)?;
        let mut cursor = Cursor::new(&data);
        let entry_count = cursor.read_u32::<LittleEndian>()?;

        let mut entries = HashMap::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            let mut name_buf = [0u8; 40];
            cursor.read_exact(&mut name_buf)?;
            let name = try_read_name(&name_buf).unwrap_or_default();
            let offset = cursor.read_u32::<LittleEndian>()?;
            let size = cursor.read_u32::<LittleEndian>()?;
            let decompressed_size = cursor.read_u32::<LittleEndian>()?;

            if !name.is_empty() {
                entries.insert(name, SndEntry { offset, size, decompressed_size });
            }
        }

        Ok(Self { entries, data })
    }

    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        let entry = self.entries.get(&name.to_lowercase())?;
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        if end > self.data.len() {
            return None;
        }
        let raw = &self.data[start..end];

        if entry.decompressed_size > 0 {
            crate::zlib::decompress(raw, entry.size as usize, entry.decompressed_size as usize).ok()
        } else {
            Some(raw.to_vec())
        }
    }

    pub fn list(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    pub fn exists(&self, name: &str) -> bool {
        self.entries.contains_key(&name.to_lowercase())
    }
}
```

- [ ] **Step 4: Make zlib module accessible to snd**

In `lod/src/lib.rs`, change:

```rust
mod zlib;
```

to:

```rust
pub(crate) mod zlib;
```

And add after the `pub mod dsounds;` line:

```rust
pub mod snd;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd lod && cargo test snd -- --nocapture`
Expected: 2 tests PASS

- [ ] **Step 6: Commit**

```bash
git add lod/src/snd.rs lod/src/lib.rs
git commit --no-gpg-sign -m "feat(lod): add audio.snd container reader for MM6 sound files"
```

---

### Task 3: Create sound/mod.rs with SoundPlugin and SoundManager

**Files:**
- Create: `openmm/src/game/sound/mod.rs`
- Modify: `openmm/src/game/mod.rs`

**Context:** The `SoundManager` resource is the central point for all audio. It holds the `DSounds` table (sound ID -> name mapping) and the `SndArchive` (extracts WAV bytes). It caches loaded sounds as Bevy `Handle<AudioSource>` to avoid re-adding duplicates. The `SoundPlugin` registers this resource, the sub-plugins for music and effects, and adds the `SpatialListener` to the player camera.

The `SndArchive` file path is: the MM6 data path goes up one level then into `Sounds/Audio.snd`. The data path is `lod::get_data_path()` which returns `OPENMM_6_PATH` env or `./target/mm6`. Audio.snd is at `{base}/Sounds/Audio.snd` where `{base}` is the parent of the LOD data directory.

- [ ] **Step 1: Create sound/mod.rs with SoundPlugin and SoundManager**

Create `openmm/src/game/sound/mod.rs`:

```rust
pub(crate) mod effects;
pub(crate) mod music;

use bevy::prelude::*;
use lod::{dsounds::DSounds, snd::SndArchive};
use std::collections::HashMap;
use std::path::Path;

use crate::assets::GameAssets;

/// Central resource for all audio: music and sound effects.
#[derive(Resource)]
pub struct SoundManager {
    pub dsounds: DSounds,
    pub snd_archive: SndArchive,
    cache: HashMap<u32, Handle<AudioSource>>,
}

impl SoundManager {
    /// Load a sound by dsounds ID into Bevy assets, caching the handle.
    /// Returns None if the sound ID doesn't exist or the WAV can't be extracted.
    pub fn load_sound(
        &mut self,
        sound_id: u32,
        audio_sources: &mut Assets<AudioSource>,
    ) -> Option<Handle<AudioSource>> {
        if let Some(handle) = self.cache.get(&sound_id) {
            return Some(handle.clone());
        }

        let info = self.dsounds.get_by_id(sound_id)?;
        let name = info.name()?;
        let wav_bytes = self.snd_archive.get(&name)?;

        let source = AudioSource {
            bytes: wav_bytes.into(),
        };
        let handle = audio_sources.add(source);
        self.cache.insert(sound_id, handle.clone());
        Some(handle)
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((music::MusicPlugin, effects::EffectsPlugin))
            .add_systems(Startup, init_sound_manager);
    }
}

fn init_sound_manager(mut commands: Commands, game_assets: Res<GameAssets>) {
    let dsounds = match DSounds::new(&game_assets.lod_manager) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to load dsounds.bin: {e}");
            return;
        }
    };

    let data_path = lod::get_data_path();
    let base = Path::new(&data_path);
    // Audio.snd is at the MM6 root/Sounds/Audio.snd
    // data_path might be mm6/data or mm6, try both
    let snd_path = [
        base.join("../Sounds/Audio.snd"),
        base.join("Sounds/Audio.snd"),
    ]
    .into_iter()
    .find(|p| p.exists());

    let Some(snd_path) = snd_path else {
        warn!("Audio.snd not found — sound effects disabled");
        return;
    };

    let snd_archive = match SndArchive::open(&snd_path) {
        Ok(a) => a,
        Err(e) => {
            warn!("Failed to open Audio.snd: {e}");
            return;
        }
    };

    info!(
        "Sound system initialized: {} descriptors, {} audio files",
        dsounds.items.len(),
        snd_archive.list().len()
    );
    commands.insert_resource(SoundManager {
        dsounds,
        snd_archive,
        cache: HashMap::new(),
    });
}
```

- [ ] **Step 2: Register SoundPlugin in game/mod.rs**

In `openmm/src/game/mod.rs`, add the module declaration after the existing modules:

```rust
pub(crate) mod sound;
```

And add `sound::SoundPlugin` to the plugin tuple in `InGamePlugin::build`:

```rust
app.add_plugins((
    lighting::LightingPlugin,
    sky::SkyPlugin,
    player::PlayerPlugin,
    physics::PhysicsPlugin,
    odm::OdmPlugin,
    blv::BlvPlugin,
    entities::EntitiesPlugin,
    debug::DebugPlugin,
    hud::HudPlugin,
    interaction::InteractionPlugin,
    event_dispatch::EventDispatchPlugin,
    console::ConsolePlugin,
    world_state::WorldStatePlugin,
    sound::SoundPlugin,
    MaterialPlugin::<terrain_material::TerrainMaterial>::default(),
))
```

- [ ] **Step 3: Build to verify it compiles**

Run: `make build 2>&1 | tail -10`
Expected: Compiles (music and effects modules are empty stubs so far — create them as minimal files)

Create `openmm/src/game/sound/music.rs`:

```rust
use bevy::prelude::*;

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, _app: &mut App) {}
}
```

Create `openmm/src/game/sound/effects.rs`:

```rust
use bevy::prelude::*;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, _app: &mut App) {}
}
```

- [ ] **Step 4: Commit**

```bash
git add openmm/src/game/sound/ openmm/src/game/mod.rs
git commit --no-gpg-sign -m "feat: add SoundPlugin with SoundManager resource and stub sub-plugins"
```

---

### Task 4: Extract music playback from odm.rs into sound/music.rs

**Files:**
- Modify: `openmm/src/game/sound/music.rs`
- Modify: `openmm/src/game/odm.rs`

**Context:** Currently `odm.rs` has `MapMusic` component (line 10-12), and `spawn_world` (line 201) takes `audio_sources: ResMut<Assets<AudioSource>>` and `existing_music: Query<Entity, With<MapMusic>>` to handle music. Lines 373-399 stop old music and start new music. This needs to move to `sound/music.rs`. The `spawn_world` function should call a public function from the music module instead.

Music files are MP3 at `{data_path}/Music/{track}.mp3` where `data_path` is `lod::get_data_path()`. Track number comes from `prepared.music_track` (u8, 0 = no music). Volume comes from `cfg.music_volume` (f32).

- [ ] **Step 1: Implement music.rs**

Replace `openmm/src/game/sound/music.rs` with:

```rust
use bevy::prelude::*;

use crate::game::InGame;

/// Marker for the map music entity, so we can despawn it on map change.
#[derive(Component)]
pub struct MapMusic;

/// Event to request music playback. Sent by odm.rs or blv.rs when a map loads.
#[derive(Event)]
pub struct PlayMusicEvent {
    pub track: u8,
    pub volume: f32,
}

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlayMusicEvent>()
            .add_systems(Update, handle_play_music);
    }
}

fn handle_play_music(
    mut commands: Commands,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: EventReader<PlayMusicEvent>,
    existing_music: Query<Entity, With<MapMusic>>,
) {
    for ev in events.read() {
        // Stop any existing music
        for entity in existing_music.iter() {
            commands.entity(entity).despawn();
        }

        if ev.track == 0 || ev.volume <= 0.0 {
            continue;
        }

        let data_path = lod::get_data_path();
        let music_path =
            std::path::Path::new(&data_path).join(format!("Music/{}.mp3", ev.track));

        if let Ok(bytes) = std::fs::read(&music_path) {
            let source = AudioSource {
                bytes: bytes.into(),
            };
            let handle = audio_sources.add(source);
            commands.spawn((
                AudioPlayer(handle),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::Linear(ev.volume),
                    ..default()
                },
                MapMusic,
                InGame,
            ));
            info!("Playing music track {} (vol={:.1})", ev.track, ev.volume);
        } else {
            warn!("Music file not found: {:?}", music_path);
        }
    }
}
```

- [ ] **Step 2: Update odm.rs to use PlayMusicEvent**

In `openmm/src/game/odm.rs`:

Remove the `MapMusic` component (lines 10-12):
```rust
// DELETE these lines:
/// Marker for the map music entity, so we can despawn it on map change.
#[derive(Component)]
struct MapMusic;
```

Remove from `spawn_world` function signature:
- `mut audio_sources: ResMut<Assets<AudioSource>>,` (line 207)
- `existing_music: Query<Entity, With<MapMusic>>,` (line 213)

Add to `spawn_world` signature:
- `mut music_events: EventWriter<crate::game::sound::music::PlayMusicEvent>,`

Replace lines 373-399 (the music block) with:

```rust
    // Play map music
    music_events.write(crate::game::sound::music::PlayMusicEvent {
        track: prepared.music_track,
        volume: cfg.music_volume,
    });
```

Remove the `use bevy::audio::PlaybackMode;` import if it becomes unused (check).

- [ ] **Step 3: Build and run to verify music still works**

Run: `make build 2>&1 | tail -10`
Expected: Compiles clean

Run: `make run` — verify music plays on map load (same as before)

- [ ] **Step 4: Commit**

```bash
git add openmm/src/game/sound/music.rs openmm/src/game/odm.rs
git commit --no-gpg-sign -m "refactor: extract music playback from odm.rs into sound/music.rs"
```

---

### Task 5: Add SpatialListener to player camera

**Files:**
- Modify: `openmm/src/game/player.rs`

**Context:** In `player.rs`, the camera entity is spawned at line 210-224 as a child of the player entity. We need to add `SpatialListener::default()` to this camera entity so Bevy knows where the "ears" are for 3D audio. The `SpatialListener` component uses the entity's `GlobalTransform` to compute audio attenuation.

- [ ] **Step 1: Add SpatialListener to camera spawn**

In `openmm/src/game/player.rs`, find the camera spawn block (around line 211-224):

```rust
    player_entity.with_children(|parent| {
        let mut cam = parent.spawn((
            Name::new("player_camera"),
            PlayerCamera,
            Camera3d::default(),
            crate::bevy_config::camera_msaa(&cfg),
            Transform::from_rotation(Quat::from_rotation_x(-8.0_f32.to_radians())),
            Projection::Perspective(PerspectiveProjection {
```

Add `SpatialListener::default()` to the camera's component tuple:

```rust
    player_entity.with_children(|parent| {
        let mut cam = parent.spawn((
            Name::new("player_camera"),
            PlayerCamera,
            Camera3d::default(),
            SpatialListener::default(),
            crate::bevy_config::camera_msaa(&cfg),
            Transform::from_rotation(Quat::from_rotation_x(-8.0_f32.to_radians())),
            Projection::Perspective(PerspectiveProjection {
```

- [ ] **Step 2: Build to verify it compiles**

Run: `make build 2>&1 | tail -10`
Expected: Compiles clean

- [ ] **Step 3: Commit**

```bash
git add openmm/src/game/player.rs
git commit --no-gpg-sign -m "feat: add SpatialListener to player camera for 3D positional audio"
```

---

### Task 6: Implement spatial sound effects in effects.rs

**Files:**
- Modify: `openmm/src/game/sound/effects.rs`

**Context:** Sound effects are played at 3D positions in the world. The `SoundManager` resolves sound_id -> WAV handle. We spawn an entity with `AudioPlayer`, `PlaybackSettings`, `Transform`, and Bevy's `SpatialBundle` for 3D attenuation. For UI sounds (non-positional), we skip the spatial components.

We use events so any system can request sound playback without needing direct access to `SoundManager`.

- [ ] **Step 1: Implement effects.rs**

Replace `openmm/src/game/sound/effects.rs` with:

```rust
use bevy::prelude::*;

use super::SoundManager;
use crate::game::InGame;

/// Event to play a sound effect at a 3D position.
#[derive(Event)]
pub struct PlaySoundEvent {
    pub sound_id: u32,
    pub position: Vec3,
}

/// Event to play a non-positional UI sound.
#[derive(Event)]
pub struct PlayUiSoundEvent {
    pub sound_id: u32,
}

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlaySoundEvent>()
            .add_event::<PlayUiSoundEvent>()
            .add_systems(Update, (handle_play_sound, handle_play_ui_sound));
    }
}

fn handle_play_sound(
    mut commands: Commands,
    mut sound_manager: Option<ResMut<SoundManager>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: EventReader<PlaySoundEvent>,
) {
    let Some(ref mut sound_manager) = sound_manager else {
        events.clear();
        return;
    };

    for ev in events.read() {
        let Some(handle) = sound_manager.load_sound(ev.sound_id, &mut audio_sources) else {
            debug!("Sound id {} not found or failed to load", ev.sound_id);
            continue;
        };

        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                spatial: true,
                ..default()
            },
            Transform::from_translation(ev.position),
            InGame,
        ));
    }
}

fn handle_play_ui_sound(
    mut commands: Commands,
    mut sound_manager: Option<ResMut<SoundManager>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut events: EventReader<PlayUiSoundEvent>,
) {
    let Some(ref mut sound_manager) = sound_manager else {
        events.clear();
        return;
    };

    for ev in events.read() {
        let Some(handle) = sound_manager.load_sound(ev.sound_id, &mut audio_sources) else {
            debug!("UI sound id {} not found", ev.sound_id);
            continue;
        };

        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                ..default()
            },
            InGame,
        ));
    }
}
```

- [ ] **Step 2: Build to verify it compiles**

Run: `make build 2>&1 | tail -10`
Expected: Compiles clean

- [ ] **Step 3: Commit**

```bash
git add openmm/src/game/sound/effects.rs
git commit --no-gpg-sign -m "feat: add spatial sound effect playback via PlaySoundEvent"
```

---

### Task 7: Wire up decoration sounds

**Files:**
- Modify: `openmm/src/game/odm.rs` (or wherever decorations are spawned)

**Context:** Decorations from `ddeclist.rs` have a `sound_id: u16` field. When a decoration is spawned with `sound_id > 0`, we should fire a `PlaySoundEvent` at that decoration's position. For now, we play the sound once on spawn. Looping ambient sounds (fountains etc.) can be added later by checking ddeclist attributes.

First, find where decoration entities are spawned to add the sound trigger.

- [ ] **Step 1: Find decoration spawn code**

Search for where `DDecListItem` or `sound_id` is used during entity spawning in the openmm crate. This is likely in `odm.rs` `lazy_spawn` or the billboard/decoration spawning code. Read that code to understand the spawn flow.

- [ ] **Step 2: Add PlaySoundEvent for decorations with sound_id**

At the point where a decoration entity is spawned (with its world position), add:

```rust
if dec_item.sound_id > 0 {
    sound_events.write(crate::game::sound::effects::PlaySoundEvent {
        sound_id: dec_item.sound_id as u32,
        position: entity_transform.translation,
    });
}
```

Add `mut sound_events: EventWriter<crate::game::sound::effects::PlaySoundEvent>` to the spawning system's parameters.

- [ ] **Step 3: Build and run to verify decoration sounds play**

Run: `make build 2>&1 | tail -10`
Expected: Compiles clean

Run: `make run` — listen for ambient sounds near decorations (fountains, campfires)

- [ ] **Step 4: Commit**

```bash
git add -A
git commit --no-gpg-sign -m "feat: play decoration sounds on spawn via sound_id from ddeclist"
```

---

### Task 8: Update documentation

**Files:**
- Modify: `CLAUDE.md`

**Context:** Update the architecture section to document the new sound system modules.

- [ ] **Step 1: Update CLAUDE.md**

Add to the `openmm crate structure` section:

```
    sound/
      mod.rs             — SoundPlugin, SoundManager resource (DSounds + SndArchive + cache)
      music.rs           — MusicPlugin, PlayMusicEvent, map music playback
      effects.rs         — EffectsPlugin, PlaySoundEvent, PlayUiSoundEvent, spatial audio
```

Add to the `lod crate structure` section:

```
- `dsounds.rs` — Sound descriptor table (dsounds.bin): sound ID -> filename mapping
- `snd.rs` — Audio.snd container reader: extracts/decompresses WAV files
```

Add a new section after `### Indoor maps (BLV) and doors`:

```
### Sound system

- `SoundManager` resource holds DSounds table + SndArchive + cached Bevy audio handles
- `PlayMusicEvent` — any system can request map music (track number + volume)
- `PlaySoundEvent` — plays a sound at a 3D world position (spatial audio via SpatialListener on player camera)
- `PlayUiSoundEvent` — plays a non-positional UI sound
- Sound files are WAV stored in `Sounds/Audio.snd` (zlib-compressed), resolved by name from dsounds.bin
- Sounds are loaded on-demand and cached by sound_id
- Decorations with `sound_id > 0` trigger PlaySoundEvent on spawn
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit --no-gpg-sign -m "docs: add sound system to CLAUDE.md architecture"
```
