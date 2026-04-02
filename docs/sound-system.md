# Sound System

## Architecture

- `SoundManager` resource holds DSounds table + SndArchive + cached Bevy audio handles
- Sound files are WAV stored in `Sounds/Audio.snd` (zlib-compressed), resolved by name from `dsounds.bin`
- Sounds are loaded on-demand and cached by `sound_id`

## Events

- `PlaySoundEvent` — looping spatial audio at a 3D world position (use for decoration ambience, footsteps)
- `PlayUiSoundEvent` — once-and-despawn non-positional audio (UI clicks, menu sounds)
- `PlayMusicEvent` — requests map music by track number + volume

## Music

- Music is loaded **from the filesystem** (`Music/{track}.mp3` relative to the game data directory), NOT from any LOD archive
- The `MapMusic` component marks the music entity so it is despawned on map change
- Volume syncs from `cfg.music_volume` whenever `GameConfig` changes (e.g. after a console command)

## Spatial Audio

- `SpatialListener` (ear gap=4.0) is on the `PlayerCamera` child entity
- Spatial scale: `1.0 / 1000.0` — approximately 2 terrain tiles (1024 units) maps to normal attenuation distance

## Sound Loading

- `SoundManager::load_sound` validates WAV: checks RIFF header, WAVE tag, and PCM format byte (`audio_fmt == 1`). Unsupported formats are skipped with a warning.
- `chest_open_sound_id` is pre-cached at startup by looking up `"openchest0101"` in DSounds

## Footsteps

Footstep sound IDs by terrain tileset (from OpenEnroth `SoundEnums.h`):

| Tileset | Sound ID |
|---------|----------|
| Grass | 93 |
| Snow | 97 |
| Desert | 91 |
| Volcanic/Badlands | 88 |
| Dirt | 92 |
| Water | 101 |
| CrackedSwamp/Swamp | 100 |
| Road | 96 |

- Footsteps use a looping audio entity that is despawned and respawned when the player transitions to a different tileset
- No footstep sound plays in fly mode
- Decorations with `sound_id > 0` trigger `PlaySoundEvent` on spawn
