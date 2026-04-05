# lod crate structure

Library crate for reading all MM6 data formats. No Bevy dependency; pure parsing, no side effects.

## Top-level modules

- `lod.rs` — LOD archive reader (MM6's container format)
- `lod_data.rs` — raw LOD entry data helpers
- `odm.rs` — Outdoor map parser (heightmap, tiles, models, billboards, spawn points), `mm6_to_bevy()` coordinate helper
- `blv.rs` — Indoor map parser (BLV): vertices, faces, sectors, BSP nodes, lights, decorations, doors
- `bsp_model.rs` — BSP model geometry (buildings, structures)
- `dtile.rs` — Tile table and texture atlas generation
- `terrain.rs` — `TerrainLookup`: tileset queries by world position
- `palette.rs` — Color palette handling (8-bit indexed color)
- `image.rs` — Sprite/texture image decoding, `tint_variant()` for monster color variants
- `billboard.rs` — Billboard/decoration sprite manager
- `ddeclist.rs`, `dsft.rs` — Decoration and sprite frame tables
- `dlv.rs` — DLV file parser (indoor delta: actors and doors per BLV map)
- `ddm.rs` — DDM file parser (actors/NPCs per map)
- `dchest.rs` — Chest descriptor table
- `dobjlist.rs` — Object list descriptor table
- `doverlay.rs` — Overlay descriptor table
- `monlist.rs` — Monster list (dmonlist.bin) with sprite name resolution
- `monsters.rs` — Per-variant monster display names from monsters.txt (e.g. "PeasantM2A" → "Apprentice Mage")
- `mapstats.rs` — Map statistics (monster groups per map zone)
- `evt.rs` — EVT event script parser → `GameEvent` enum
- `twodevents.rs` — 2DEvents.txt parser (house/building event table)
- `enums.rs` — Shared MM6 enums (face flags, object types, etc.)
- `tft.rs` — TFT (tile frame table) parser
- `dsounds.rs` — Sound descriptor table (dsounds.bin): sound ID → filename mapping
- `snd.rs` — Audio.snd container reader: extracts/decompresses WAV files
- `smk.rs` — `SmkDecoder`: safe Rust wrapper around vendored libsmacker C library; decodes SMK2/SMK4 video frames to RGBA pixels one frame at a time
- `vid.rs` — `Vid`: parses MM6 VID archives (Anims1.vid, Anims2.vid); provides index of embedded SMK files with byte-range access

## game/ sub-modules

- `game/actors.rs` — `Actor`/`Actors`: per-map DDM actor roster with pre-resolved sprites and palette variants
- `game/decorations.rs` — `DecorationEntry`/`Decorations`: per-map ODM billboard roster with pre-resolved sprite names, dimensions, and DSFT metadata
- `game/monster.rs` — `Monster`/`Monsters`: per-map spawn resolution (MapStats + monlist + DSFT → one `Monster` per group member); also `resolve_entry()` and `resolve_sprite_group()` for DDM actor sprite resolution
- `game/npc.rs` — `NpcEntry`/`StreetNpcs`: street NPC roster with generated names
- `game/font.rs` — Font loading from LOD bitmaps
- `game/global.rs` — `GameData`: top-level container for all global game tables
