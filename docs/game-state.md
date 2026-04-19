# Game State, Party, and Save System

## WorldState

`WorldState` resource is the single source of truth for all runtime player and map state.

`GameSave` is the persistent serialization form (JSON). Transfer with:
- `WorldState::write_to_save(&self) -> GameSave`
- `WorldState::read_from_save(&mut self, save: &GameSave)`

`GameSave` is written to `data/saves/{slot}.json`.

### Time of Day

`WorldState.time_of_day` drives lighting, sky color, and HUD tap frame:

| Value | Meaning |
|-------|---------|
| 0.0 | Midnight |
| 0.25 | Sunrise |
| 0.375 | 9am (default) |
| 0.5 | Noon |
| 0.75 | Sunset |

Advances each frame while `HudView::World`. Full day cycle = `DAY_CYCLE_SECS = 1800` (30 min real time).

## GameVariables

- `map_vars: [i32; 100]` — reset on every map change
- `quest_bits: HashSet<i32>` — permanent quest flags
- `autonotes: HashSet<i32>` — collected autonotes
- `gold: i32` — starts at 200
- `food: i32` — starts at 7
- `reputation: i32`

Use `set_qbit` / `clear_qbit` / `has_qbit` and `add_autonote` — these log every change at info level.

## Party

`Party` resource holds exactly 4 `PartyMember`s (indices 0–3 = Player1–4 in EVT).

Default party:
| Index | Name | Class |
|-------|------|-------|
| 0 | Zoltan | Knight |
| 1 | Roderick | Paladin |
| 2 | Alexei | Archer |
| 3 | Serena | Cleric |

All start at level 1.

- `Party::active_target` — set by the `ForPartyMember` EVT opcode, used by subsequent variable reads/writes
- `Party::max_skill(target, var)` — returns the highest skill level across all members matching `target`
- `PartyMember::skills` is `[u8; 31]` indexed by `EvtVariable::skill_index()` (covers skills 0x38–0x56)
- Use `set_skill` / `get_skill` with `EvtVariable`

## Map Events

`MapEvents` resource fields:
- `evt` — map-specific EVT script
- `houses: Option<TwoDEvents>` — building/house metadata from `2devents.txt` (None for indoor maps)
- `npc_table: StreetNpcs` — street NPC roster
- `name_pool: NpcNamePool` — name generation pool
- `generated_npcs: HashMap<u32, GeneratedNpc>` — lazily-populated for peasant actors with npc_id ≥ 5000

Event loading order (entries from later files extend, not override, per event_id):
1. Map-specific `.evt` file (e.g., `oute3.evt`, `d01.evt`)
2. `out.evt` (outdoor maps only; shared events for all outdoor maps)
3. `global.evt` (always loaded; shared events for all maps)

`TwoDEvents` (2devents.txt) is **NOT loaded for indoor maps** (`indoor=true` → `houses = None`). Building `SpeakInHouse` events still work but have no metadata.

`generated_npcs` is populated lazily at actor spawn time for generic peasant/actor entries.

### EventQueue Internals

`EventQueue` uses `VecDeque<EventSequence>` where each `EventSequence` is the full step list for one event_id.

- `push_all(event_id, evt)` — push a full EVT script sequence
- `push_single(GameEvent)` — push a synthesized single event
- `push_front()` — for sub-events (depth-first, preserves ordering within a script)
- `clear()` — abort the entire queue

## Map Names and Save

- `MapName` enum: `Outdoor(OdmName)` or `Indoor(String)`
- `TryFrom<&str>` parses `"oute3"` (5 chars, starts with `"out"`) as outdoor; anything else as indoor (with or without `.odm`/`.blv` extension)
- `OdmName` supports directional navigation: `go_north/go_south/go_east/go_west` return `Option<OdmName>` (None at boundary)
- Valid column range: `'a'–'e'`, row range: `'1'–'3'`
- Default spawn position: `[-10178, 340, 11206]` at yaw -38.7° (MM6 starting area of oute3)

## GameAssets

`GameAssets` resource (from `assets/mod.rs`) wraps `LodManager` + `GameData` + `BillboardManager`.

`game_lod()` returns a `GameLod<'_>` view for decoded sprites, bitmaps, icons, and fonts.
