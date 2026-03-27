# MM6 Actor & Sprite System

## File Structure

- **ODM** (`oute3.odm`) — terrain, BSP models, decorations (billboards), spawn point definitions
- **DDM** (`oute3.ddm`) — runtime delta: actors, sprite objects, chests, face attributes, decoration flags

## Actor Data (DDM file)

### Actor_MM7 struct (836 bytes)
```
offset  size  field
0x000   32    name (char[32])
0x020   2     npcId (i16)
0x022   2     padding
0x024   4     attributes (u32, ActorAttributes flags)
0x028   2     hp (i16)
0x02A   2     padding
0x02C   88    monsterInfo (MonsterInfo_MM7)
0x084   2     field_84 (i16)
0x086   2     monsterId (i16)
0x088   2     radius (u16, collision)
0x08A   2     height (u16, sprite height)
0x08C   2     moveSpeed (u16)
0x08E   6     position (Vec3s: x,y,z as i16)
0x094   6     velocity (Vec3s)
0x09A   2     yawAngle (u16, 0-65535 = 360°)
0x09C   2     pitchAngle (u16)
0x09E   2     sectorId (i16)
0x0A0   2     currentActionLength (u16)
0x0A2   6     initialPosition (Vec3s, spawn point)
0x0A8   6     guardingPosition (Vec3s, patrol center)
0x0AE   2     tetherDistance (u16, max wander range)
0x0B0   2     aiState (i16, AIState enum)
0x0B2   2     currentActionAnimation (u16, ActorAnimation enum)
0x0B4   2     carriedItemId (u16)
0x0B6   2     padding
0x0B8   4     currentActionTime (u32)
0x0BC   16    spriteIds (u16[8], one per animation state)
0x0CC   8     soundSampleIds (u16[4])
...           buffs, items, group, scheduled jobs, etc.
```

### MM6 vs MM7 differences
- MM6 Actor struct is smaller (no `group` field, 20-byte spawn points vs 24)
- MonsterInfo is similar but MM6 may have different padding
- Core fields (position, hp, spriteIds, aiState) are the same concept

## Actor Animation System

### 8 Animation States (ActorAnimation enum)
```
0 = Standing    — idle stance
1 = Walking     — movement
2 = AtkMelee    — melee attack
3 = AtkRanged   — ranged/spell attack
4 = GotHit      — hurt reaction
5 = Dying       — death animation
6 = Dead        — corpse (static, lootable)
7 = Bored       — idle fidget
```

Each actor has `spriteIds[8]` — one sprite frame table index per animation state.

### AI States → Animation Mapping
```
Standing/Tethered → ANIM_Standing (or ANIM_Bored if idle long)
Pursuing/Fleeing  → ANIM_Walking
AttackingMelee    → ANIM_AtkMelee
AttackingRanged   → ANIM_AtkRanged
Dying             → ANIM_Dying
Dead              → ANIM_Dead
Stunned           → ANIM_Standing
```

## Directional Sprites (8 directions)

Each animation frame can have up to 8 directional variants:
```
0 = Front (facing camera)
1 = Front-Right
2 = Right
3 = Back-Right
4 = Back (facing away)
5 = Back-Left
6 = Left
7 = Front-Left
```

### Sprite Name Convention
Base texture name + direction digit suffix:
- `goblinA0` = goblin attack, front view
- `goblinA2` = goblin attack, right view
- `goblinA4` = goblin attack, back view

### Loading Strategies (SpriteFrameFlags)
1. **Single image** — same sprite for all 8 directions (trees, decorations)
2. **3-view mirrored** — only front(0), side(2), back(4); others mirror
3. **Full 8 directions** — unique sprite per direction, with optional mirror flags
4. **Fidget** — special idle animation variant

### Direction Calculation
```
angle = atan2(camera.x - actor.x, camera.z - actor.z)
relative_angle = angle - actor.yaw
direction_index = ((relative_angle + PI/8) / (PI/4)) mod 8
```

## Monster Description (MonsterDesc)

Loaded from `monsters.txt` or monster list files:
```
- monsterHeight, monsterRadius — collision dimensions
- movementSpeed — how fast it moves
- soundSampleIds[4] — attack, hurt, death, idle sounds
- spriteNames[8] — one sprite group name per animation state
  e.g. ["goblinSt", "goblinWa", "goblinAt", "goblinSh",
        "goblinHi", "goblinDi", "goblinDe", "goblinBo"]
```

## Spawn Points (ODM file)

### SpawnPoint_MM6 (20 bytes)
```
position: Vec3i (12 bytes)
radius: u16 (wander range)
type: u16 (1=monster, 2=item)
monsterIndex: u16
attributes: u16
```

Spawn points define WHERE actors appear. The DDM contains the actual live actors
with their current state. On first map load, spawn points generate actors; on
subsequent loads, the DDM state is restored.
