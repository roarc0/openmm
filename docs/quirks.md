# OpenMM Quirks

This document tracks interesting findings, bugs, and data inconsistencies discovered within the original Might and Magic data files and engine behaviors.

## Sound System

### Empty Sound Descriptors in `dsounds.bin`

Various entries in the `dsounds.bin` data table (found in `icons.lod`) resolve to empty strings instead of valid WAV filenames. These function as silent placeholders in the original data.

- **Peasant Fidget Sounds**: Every peasant variant in the game (F1–F4, M1–M4) is assigned a "fidget" sound ID that points to an entry with an empty name.
  - **IDs**: 1403, 1413, 1423, 1433, 1443, 1453, 1463, 1473.
- **Engine Behavior**: The `monster_ai_system` attempts to play these sounds periodically when a monster is in `Wander` mode.
- **Logging**: The OpenMM engine logs a `WARN` when these IDs are encountered to assist in identifying missing assets or data bugs.
- **Discovery**: This was identified while debugging why `sound_id 1453` (used by `PeasantM2`) was failing to load.
