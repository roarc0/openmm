use byteorder::{LittleEndian, ReadBytesExt};
use std::error::Error;
use std::io::{Cursor, Seek};

use serde::{Deserialize, Serialize};

use crate::Assets;
use crate::LodSerialise;
use crate::assets::enums::ActorAttributes;

/// MM6 MapMonster struct size = 0x224 = 548 bytes.
/// Layout from MMExtension: Scripts/Structs/01 common structs.lua (MapMonster).
const ACTOR_SIZE_MM6: usize = 548;

/// One monster attack definition (5 bytes). Used in `CommonMonsterProps`.
///
/// Layout: Type(1) + DamageDiceCount(1) + DamageDiceSides(1) + DamageAdd(1) + Missile(1)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MonsterAttackInfo {
    /// Attack type (damage kind enum). Offset 0x00.
    pub attack_type: u8,
    /// Number of damage dice. Offset 0x01.
    pub damage_dice_count: u8,
    /// Sides per damage die. Offset 0x02.
    pub damage_dice_sides: u8,
    /// Flat damage bonus added after dice roll. Offset 0x03.
    pub damage_add: u8,
    /// Missile/projectile type (0 = melee). Offset 0x04.
    pub missile: u8,
}

/// Per-actor combat and loot properties embedded in each `DdmActor` at offset 0x2C.
///
/// MM6 `CommonMonsterProps` struct, 72 bytes (0x48). Fields verified against
/// MMExtension `Scripts/Structs/01 common structs.lua` — MM6 branch only.
///
/// Layout (relative to struct start):
///   0x00: skip(8) [Name/Picture runtime pointers, always 0 in file]
///   0x08: monlist_id(1), level(1), treasure_item_pct(1), treasure_dice_count(1)
///   0x0C: treasure_dice_sides(1), treasure_item_level(1), treasure_item_type(1), fly(1)
///   0x10: move_type(1), ai_type(1), hostile_type(1), pref_class(1)
///   0x14: bonus(1), bonus_mul(1)
///   0x16: attack1(5), attack2_chance(1), attack2(5)
///   0x21: spell_chance(1), spell(1), spell_skill(1)
///   0x24: fire_res(1), elec_res(1), cold_res(1), poison_res(1), phys_res(1), magic_res(1)
///   0x2A: pref_num(1), pad(1), quest_item(2), skip(2)[pad]
///   0x30: full_hp(4), armor_class(4), experience(4), move_speed(4), attack_recovery(4)
///   0x44: _unknown(4) [MM6-only trailing field]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct CommonMonsterProps {
    /// Index into dmonlist.bin (1-based in file, stored 0-based here). Offset 0x08.
    pub monlist_id: u8,
    /// Monster level used for scaling. Offset 0x09.
    pub level: u8,
    /// Chance (0-100) that this monster drops a treasure item. Offset 0x0A.
    pub treasure_item_percent: u8,
    /// Number of gold dice rolled on death. Offset 0x0B.
    pub treasure_dice_count: u8,
    /// Sides on each gold die. Offset 0x0C.
    pub treasure_dice_sides: u8,
    /// Item level for random loot table. Offset 0x0D.
    pub treasure_item_level: u8,
    /// Item type for random loot table. Offset 0x0E.
    pub treasure_item_type: u8,
    /// Non-zero if this monster can fly. Offset 0x0F.
    pub fly: u8,
    /// Movement style (0=stand, 1=walk, 2=fly, 3=water). Offset 0x10.
    pub move_type: u8,
    /// AI archetype (0=wimp, 1=normal, 2=aggressive, 3=suicide). Offset 0x11.
    pub ai_type: u8,
    /// How the monster chooses targets. Offset 0x12.
    pub hostile_type: u8,
    /// Preferred target class (MM6 only). Offset 0x13.
    pub pref_class: u8,
    /// Special on-hit bonus effect type (steal, curse, etc.). Offset 0x14.
    pub bonus: u8,
    /// Chance multiplier for bonus effect (`level * bonus_mul`). Offset 0x15.
    pub bonus_mul: u8,
    /// Primary melee/ranged attack definition. Offset 0x16.
    pub attack1: MonsterAttackInfo,
    /// Percentage chance to use attack2 instead of attack1. Offset 0x1B.
    pub attack2_chance: u8,
    /// Secondary attack definition. Offset 0x1C.
    pub attack2: MonsterAttackInfo,
    /// Percentage chance to cast the assigned spell. Offset 0x21.
    pub spell_chance: u8,
    /// Spell ID to cast (0 = none). Offset 0x22.
    pub spell: u8,
    /// Skill rank/mastery for the spell. Offset 0x23.
    pub spell_skill: u8,
    /// Fire resistance (0-200, 200=immune). Offset 0x24.
    pub fire_resistance: u8,
    /// Electrical resistance. Offset 0x25.
    pub elec_resistance: u8,
    /// Cold resistance. Offset 0x26.
    pub cold_resistance: u8,
    /// Poison resistance. Offset 0x27.
    pub poison_resistance: u8,
    /// Physical resistance. Offset 0x28.
    pub phys_resistance: u8,
    /// Magic resistance. Offset 0x29.
    pub magic_resistance: u8,
    /// Number of party members targeted per attack. Offset 0x2A.
    pub pref_num: u8,
    /// Quest item index (carried quest item ID, 0 = none). Offset 0x2C.
    pub quest_item: i16,
    /// Maximum HP for this actor instance. Offset 0x30.
    pub full_hp: i32,
    /// Armor class. Offset 0x34.
    pub armor_class: i32,
    /// Experience awarded on death. Offset 0x38.
    pub experience: i32,
    /// Movement speed in MM6 units/tick. Offset 0x3C.
    pub move_speed: i32,
    /// Recovery time between attacks in game ticks. Offset 0x40.
    pub attack_recovery: i32,
    /// Unknown MM6-only trailing field. Offset 0x44.
    pub _unknown: i32,
}

/// A spell buff on an actor (16 bytes each, 14 per actor).
///
/// Layout: ExpireTime(8) + Power(2) + Skill(2) + OverlayId(2) + Caster(1) + Bits(1)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct SpellBuff {
    pub expire_time: i64,
    pub power: i16,
    pub skill: i16,
    pub overlay_id: i16,
    pub caster: u8,
    pub bits: u8,
}

/// A monster schedule entry (12 bytes each, 8 per actor).
///
/// Layout: X(2) + Y(2) + Z(2) + Bits(2) + Action(1) + Hour(1) + Day(1) + Month(1)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct MonsterSchedule {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub bits: u16,
    pub action: u8,
    pub hour: u8,
    pub day: u8,
    pub month: u8,
}

/// An actor (monster/NPC) from the DDM delta file.
///
/// Field offsets verified against MMExtension's MapMonster struct (MM6, o=0):
///   0x00: Name[32], 0x20: NPC_ID, 0x22: pad, 0x24: Bits, 0x28: HP, 0x2A: pad,
///   0x2C: CommonMonsterProps (72 bytes, Id @ 0x34),
///   0x74: RangeAttack, 0x76: MonsterIdType,
///   0x78: BodyRadius, 0x7A: BodyHeight, 0x7C: MoveSpeed,
///   0x7E: Pos[3], 0x84: Vel[3], 0x8A: Yaw, 0x8C: Pitch, 0x8E: Room,
///   0x90: CurrentActionLength, 0x92: Start[3], 0x98: Guard[3],
///   0x9E: GuardRadius, 0xA0: AIState, 0xA2: GraphicState, 0xA4: CarriedItem,
///   0xA6: pad, 0xA8: CurrentActionStep, 0xAC: Frames[8], 0xBC: Sounds[4],
///   0xC4: SpellBuffs[14], 0x1A4: Group, 0x1A8: Ally,
///   0x1AC: Schedules[8], 0x20C: Summoner, 0x210: LastAttacker,
///   0x214: remaining padding to 0x224
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DdmActor {
    /// Actor name. Offset 0x00 (32 bytes, null-terminated).
    pub name: String,
    /// Index into dmonlist.bin (MonsterList). Mirrors `common_props.monlist_id`.
    /// Convenience shortcut — both are 0-indexed (file stores 1-indexed, we subtract 1).
    pub monlist_id: u8,
    /// NPC dialogue index into npcdata.txt (1-based). Zero for monsters.
    /// For peasant actors: 1 = female, 2 = male (sex flag, not a real npcdata index).
    /// Offset 0x20 (i16).
    pub npc_id: i16,
    /// Padding at offset 0x22.
    pub _pad0x22: i16,
    /// Actor attribute bitflags. Offset 0x24 (u32).
    /// Use `actor_attributes()` for typed access.
    pub attributes: u32,
    /// Current hit points. Offset 0x28 (i16).
    pub hp: i16,
    /// Padding at offset 0x2A.
    pub _pad0x2a: i16,
    /// Per-actor combat and loot properties. Offset 0x2C (72 bytes).
    /// Contains level, HP, AC, XP, attacks, resistances, loot table, etc.
    /// The `monlist_id` field here matches the separately-extracted top-level field.
    pub common_props: CommonMonsterProps,
    /// Range attack type. Offset 0x74 (i16).
    pub range_attack: i16,
    /// Monster ID type (Id2). Offset 0x76 (i16).
    pub monster_id_type: i16,
    /// Body collision radius. Offset 0x78 (u16).
    pub radius: u16,
    /// Body height. Offset 0x7A (u16).
    pub height: u16,
    /// Movement speed. Offset 0x7C (u16).
    pub move_speed: u16,
    /// Current position in MM6 coordinates (x, y, z as i16). Offset 0x7E.
    pub position: [i16; 3],
    /// Velocity vector. Offset 0x84.
    pub velocity: [i16; 3],
    /// Facing angle (0-65535 for 360 degrees). Offset 0x8A.
    pub yaw: u16,
    /// Look angle / pitch. Offset 0x8C (u16).
    pub pitch: u16,
    /// Room / sector index (for indoor maps). Offset 0x8E (i16).
    pub room: i16,
    /// Current action length (timer). Offset 0x90 (u16).
    pub current_action_length: u16,
    /// Spawn/initial position. Offset 0x92.
    pub initial_position: [i16; 3],
    /// Guarding position (patrol center). Offset 0x98.
    pub guarding_position: [i16; 3],
    /// Max wander distance from guarding position. Offset 0x9E.
    pub tether_distance: u16,
    /// AI state (0=standing, 1=tethered, 4=dying, 5=dead, 6=pursuing, etc.). Offset 0xA0.
    pub ai_state: u16,
    /// Current animation/graphic state. Offset 0xA2.
    pub current_animation: u16,
    /// Item carried by this actor. Offset 0xA4 (u16).
    pub carried_item: u16,
    /// Padding at offset 0xA6.
    pub _pad0xa6: u16,
    /// Current action step / timer. Offset 0xA8 (u32).
    pub current_action_step: u32,
    /// DSFT frame table indices for 8 animation states. Offset 0xAC.
    /// standing, walk, attack, shoot, stun, die, dead, fidget.
    /// Zero in DDM files -- populated at runtime by LoadFrames(). Use monlist_id instead.
    pub sprite_ids: [u16; 8],
    /// Sound IDs for 4 sound types. Offset 0xBC.
    /// attack, die, got_hit, fidget.
    pub sound_ids: [u16; 4],
    /// Active spell buffs (14 slots). Offset 0xC4 (14 x 16 = 224 bytes).
    pub spell_buffs: [SpellBuff; 14],
    /// Group ID for faction. Offset 0x1A4 (i32).
    pub group: i32,
    /// Ally faction ID. Offset 0x1A8 (i32).
    pub ally: i32,
    /// AI schedules (8 entries). Offset 0x1AC (8 x 12 = 96 bytes).
    pub schedules: [MonsterSchedule; 8],
    /// Summoner actor index. Offset 0x20C (i32).
    pub summoner: i32,
    /// Last attacker actor index. Offset 0x210 (i32).
    pub last_attacker: i32,
    /// Remaining padding bytes (16 bytes, offset 0x214 to 0x224).
    pub _pad0x214: [u8; 16],
}

impl DdmActor {
    /// Returns typed actor attribute flags.
    pub fn actor_attributes(&self) -> ActorAttributes {
        ActorAttributes::from_bits_truncate(self.attributes)
    }

    /// For peasant actors, `npc_id` encodes sex: 1 = female, 2 = male.
    /// Returns true if this actor is flagged as a female peasant.
    pub fn is_female_peasant(&self) -> bool {
        self.npc_id == 1
    }
}

/// Parsed DDM (outdoor) delta file.
///
/// On-disk layout (sections in order):
///   1. MapVars     — 200 × u8 event/barrel state variables (no count prefix)
///   2. MapObjects  — u32 count + count × 0x64 bytes each (projectiles / dropped items)
///   3. MapSprites  — u32 count + count × 0x1C bytes each (decoration instances)
///   4. SoundSprites — 10 × i32 (ambient sound positions, no count prefix)
///   5. MapChests   — u32 count + count × ~4204 bytes each (chest contents)
///   6. MapMonsters — u32 count + count × 548 bytes each (actors)
///
/// **Save support:** sections 1–5 are currently skipped by the heuristic actor-finder.
/// To implement round-trip saving, parse all sections sequentially and store their raw
/// bytes (or typed structs). Every section must be re-serialised in order with the
/// correct C struct layout — all padding fields in `DdmActor` are already preserved for
/// this purpose.
#[derive(Debug, Serialize, Deserialize)]
pub struct Ddm {
    pub actors: Vec<DdmActor>,
    /// Sections before actors (MapVars, MapObjects, MapSprites, etc.)
    #[serde(skip)]
    pub prefix_data: Vec<u8>,
}

impl LodSerialise for Ddm {
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.prefix_data.clone();
        use byteorder::{LittleEndian, WriteBytesExt};
        out.write_u32::<LittleEndian>(self.actors.len() as u32).unwrap();
        for actor in &self.actors {
            out.extend_from_slice(&actor.to_bytes());
        }
        out
    }
}

impl TryFrom<&[u8]> for Ddm {
    type Error = Box<dyn Error>;
    fn try_from(raw: &[u8]) -> Result<Self, Self::Error> {
        Self::parse(raw)
    }
}

impl Ddm {
    pub fn load(assets: &Assets, map_name: &str) -> Result<Self, Box<dyn Error>> {
        let ddm_name = map_name.replace(".odm", ".ddm");
        let raw = assets.get_decompressed(format!("games/{}", ddm_name))?;
        Self::try_from(raw.as_slice())
    }

    /// Parse actors from raw (decompressed) delta file data.
    /// Usable by both DDM (outdoor) and DLV (indoor) parsers since they share the same MapMonster struct.
    pub fn parse_from_data(data: &[u8]) -> Result<Vec<DdmActor>, Box<dyn Error>> {
        let ddm = Self::parse(data)?;
        Ok(ddm.actors)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        // Scan for actor count: a u32 followed by ASCII name bytes
        let actor_start = Self::find_actors(data)?;
        let actor_count = u32::from_le_bytes([
            data[actor_start],
            data[actor_start + 1],
            data[actor_start + 2],
            data[actor_start + 3],
        ]) as usize;

        let mut actors = Vec::with_capacity(actor_count);
        let base = actor_start + 4;

        for i in 0..actor_count {
            let offset = base + i * ACTOR_SIZE_MM6;
            if offset + ACTOR_SIZE_MM6 > data.len() {
                break;
            }
            if let Some(actor) = Self::read_actor(&data[offset..offset + ACTOR_SIZE_MM6]) {
                actors.push(actor);
            }
        }

        Ok(Ddm {
            actors,
            prefix_data: data[..actor_start].to_vec(),
        })
    }

    /// Parse actors starting at a known byte offset (for DLV sequential parsing).
    pub fn parse_actors_at(data: &[u8], offset: usize, count: usize) -> Vec<DdmActor> {
        let mut actors = Vec::with_capacity(count);
        for i in 0..count {
            let start = offset + i * ACTOR_SIZE_MM6;
            if start + ACTOR_SIZE_MM6 > data.len() {
                break;
            }
            if let Some(actor) = Self::read_actor(&data[start..start + ACTOR_SIZE_MM6]) {
                actors.push(actor);
            }
        }
        actors
    }

    fn find_actors(data: &[u8]) -> Result<usize, Box<dyn Error>> {
        for offset in (0..data.len().saturating_sub(40)).step_by(2) {
            let val = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            if (1..=500).contains(&val) {
                let name_start = offset + 4;
                if name_start + 32 > data.len() {
                    continue;
                }
                let name_bytes = &data[name_start..name_start + 32];
                let is_name = name_bytes
                    .iter()
                    .take(20)
                    .all(|&b| b == 0 || (0x20..=0x7E).contains(&b));
                let has_alpha = name_bytes.iter().take(10).any(|&b| b.is_ascii_alphabetic());
                if is_name && has_alpha {
                    // Verify second actor name too
                    let second = name_start + ACTOR_SIZE_MM6;
                    if second + 32 <= data.len() && val > 1 {
                        let name2 = &data[second..second + 32];
                        let is_name2 = name2.iter().take(20).all(|&b| b == 0 || (0x20..=0x7E).contains(&b));
                        let has_alpha2 = name2.iter().take(10).any(|&b| b.is_ascii_alphabetic());
                        if is_name2 && has_alpha2 {
                            return Ok(offset);
                        }
                    } else if val == 1 {
                        return Ok(offset);
                    }
                }
            }
        }
        Err("Could not find actor data in DDM".into())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn actor_count(&self) -> usize {
        self.actors.len()
    }

    fn read_actor(data: &[u8]) -> Option<DdmActor> {
        let name_end = data[..32].iter().position(|&b| b == 0).unwrap_or(32);
        let name = String::from_utf8_lossy(&data[..name_end]).to_string();

        let mut c = Cursor::new(data);
        c.seek(std::io::SeekFrom::Start(0x20)).ok()?;

        // Offset 0x20: NPC_ID(2) + pad(2) + Bits(4) + HP(2) + pad(2)
        let npc_id = c.read_i16::<LittleEndian>().ok()?;
        let _pad0x22 = c.read_i16::<LittleEndian>().ok()?;
        let attributes = c.read_u32::<LittleEndian>().ok()?;
        let hp = c.read_i16::<LittleEndian>().ok()?;
        let _pad0x2a = c.read_i16::<LittleEndian>().ok()?;

        // CommonMonsterProps at offset 0x2C (72 bytes).
        // Offsets within props are relative to 0x2C.
        // 0x00-0x07: Name/Picture runtime pointers — always 0 in file, skip.
        let p = &data[0x2C..0x2C + 72];
        let common_props = CommonMonsterProps {
            monlist_id: p[0x08].saturating_sub(1), // 1-indexed → 0-indexed
            level: p[0x09],
            treasure_item_percent: p[0x0A],
            treasure_dice_count: p[0x0B],
            treasure_dice_sides: p[0x0C],
            treasure_item_level: p[0x0D],
            treasure_item_type: p[0x0E],
            fly: p[0x0F],
            move_type: p[0x10],
            ai_type: p[0x11],
            hostile_type: p[0x12],
            pref_class: p[0x13],
            bonus: p[0x14],
            bonus_mul: p[0x15],
            attack1: MonsterAttackInfo {
                attack_type: p[0x16],
                damage_dice_count: p[0x17],
                damage_dice_sides: p[0x18],
                damage_add: p[0x19],
                missile: p[0x1A],
            },
            attack2_chance: p[0x1B],
            attack2: MonsterAttackInfo {
                attack_type: p[0x1C],
                damage_dice_count: p[0x1D],
                damage_dice_sides: p[0x1E],
                damage_add: p[0x1F],
                missile: p[0x20],
            },
            spell_chance: p[0x21],
            spell: p[0x22],
            spell_skill: p[0x23],
            fire_resistance: p[0x24],
            elec_resistance: p[0x25],
            cold_resistance: p[0x26],
            poison_resistance: p[0x27],
            phys_resistance: p[0x28],
            magic_resistance: p[0x29],
            pref_num: p[0x2A],
            // 0x2B: padding byte
            quest_item: i16::from_le_bytes([p[0x2C], p[0x2D]]),
            // 0x2E-0x2F: padding
            full_hp: i32::from_le_bytes([p[0x30], p[0x31], p[0x32], p[0x33]]),
            armor_class: i32::from_le_bytes([p[0x34], p[0x35], p[0x36], p[0x37]]),
            experience: i32::from_le_bytes([p[0x38], p[0x39], p[0x3A], p[0x3B]]),
            move_speed: i32::from_le_bytes([p[0x3C], p[0x3D], p[0x3E], p[0x3F]]),
            attack_recovery: i32::from_le_bytes([p[0x40], p[0x41], p[0x42], p[0x43]]),
            _unknown: i32::from_le_bytes([p[0x44], p[0x45], p[0x46], p[0x47]]),
        };
        // The Id field is 1-indexed in the game engine; subtract 1 for our 0-based monlist array.
        let monlist_id = common_props.monlist_id;

        // Offset 0x74: RangeAttack(2) + MonsterIdType(2)
        c.seek(std::io::SeekFrom::Start(0x74)).ok()?;
        let range_attack = c.read_i16::<LittleEndian>().ok()?;
        let monster_id_type = c.read_i16::<LittleEndian>().ok()?;

        // Offset 0x78: radius(2) + height(2) + moveSpeed(2)
        let radius = c.read_u16::<LittleEndian>().ok()?;
        let height = c.read_u16::<LittleEndian>().ok()?;
        let move_speed = c.read_u16::<LittleEndian>().ok()?;

        // Offset 0x7E: position (i16 x 3)
        let px = c.read_i16::<LittleEndian>().ok()?;
        let py = c.read_i16::<LittleEndian>().ok()?;
        let pz = c.read_i16::<LittleEndian>().ok()?;

        // Offset 0x84: velocity (i16 x 3)
        let vx = c.read_i16::<LittleEndian>().ok()?;
        let vy = c.read_i16::<LittleEndian>().ok()?;
        let vz = c.read_i16::<LittleEndian>().ok()?;

        // Offset 0x8A: yaw(2) + pitch(2) + room(2) + action_length(2)
        let yaw = c.read_u16::<LittleEndian>().ok()?;
        let pitch = c.read_u16::<LittleEndian>().ok()?;
        let room = c.read_i16::<LittleEndian>().ok()?;
        let current_action_length = c.read_u16::<LittleEndian>().ok()?;

        // Offset 0x92: initial position (i16 x 3)
        let ix = c.read_i16::<LittleEndian>().ok()?;
        let iy = c.read_i16::<LittleEndian>().ok()?;
        let iz = c.read_i16::<LittleEndian>().ok()?;

        // Offset 0x98: guarding position (i16 x 3)
        let gx = c.read_i16::<LittleEndian>().ok()?;
        let gy = c.read_i16::<LittleEndian>().ok()?;
        let gz = c.read_i16::<LittleEndian>().ok()?;

        // Offset 0x9E..0xA8
        let tether_distance = c.read_u16::<LittleEndian>().ok()?;
        let ai_state = c.read_u16::<LittleEndian>().ok()?;
        let current_animation = c.read_u16::<LittleEndian>().ok()?;
        let carried_item = c.read_u16::<LittleEndian>().ok()?;
        let _pad0xa6 = c.read_u16::<LittleEndian>().ok()?;
        let current_action_step = c.read_u32::<LittleEndian>().ok()?;

        // Offset 0xAC: sprite frame table IDs (8 x u16)
        // standing, walk, attack, shoot, stun, die, dead, fidget
        let mut sprite_ids = [0u16; 8];
        for sid in &mut sprite_ids {
            *sid = c.read_u16::<LittleEndian>().ok()?;
        }

        // Offset 0xBC: sound IDs (4 x u16)
        // attack, die, got_hit, fidget
        let mut sound_ids = [0u16; 4];
        for sid in &mut sound_ids {
            *sid = c.read_u16::<LittleEndian>().ok()?;
        }

        // Offset 0xC4: spell buffs (14 x 16 = 224 bytes)
        let mut spell_buffs = [SpellBuff::default(); 14];
        for buff in &mut spell_buffs {
            buff.expire_time = c.read_i64::<LittleEndian>().ok()?;
            buff.power = c.read_i16::<LittleEndian>().ok()?;
            buff.skill = c.read_i16::<LittleEndian>().ok()?;
            buff.overlay_id = c.read_i16::<LittleEndian>().ok()?;
            buff.caster = c.read_u8().ok()?;
            buff.bits = c.read_u8().ok()?;
        }

        // Offset 0x1A4: group(4) + ally(4)
        let group = c.read_i32::<LittleEndian>().ok()?;
        let ally = c.read_i32::<LittleEndian>().ok()?;

        // Offset 0x1AC: schedules (8 x 12 = 96 bytes)
        let mut schedules = [MonsterSchedule::default(); 8];
        for sched in &mut schedules {
            sched.x = c.read_i16::<LittleEndian>().ok()?;
            sched.y = c.read_i16::<LittleEndian>().ok()?;
            sched.z = c.read_i16::<LittleEndian>().ok()?;
            sched.bits = c.read_u16::<LittleEndian>().ok()?;
            sched.action = c.read_u8().ok()?;
            sched.hour = c.read_u8().ok()?;
            sched.day = c.read_u8().ok()?;
            sched.month = c.read_u8().ok()?;
        }

        // Offset 0x20C: summoner(4) + last_attacker(4)
        let summoner = c.read_i32::<LittleEndian>().ok()?;
        let last_attacker = c.read_i32::<LittleEndian>().ok()?;

        // Offset 0x214: remaining padding (16 bytes to 0x224)
        let mut _pad0x214 = [0u8; 16];
        _pad0x214.copy_from_slice(&data[0x214..0x224]);

        Some(DdmActor {
            name,
            monlist_id,
            npc_id,
            _pad0x22,
            attributes,
            hp,
            _pad0x2a,
            common_props,
            range_attack,
            monster_id_type,
            radius,
            height,
            move_speed,
            position: [px, py, pz],
            velocity: [vx, vy, vz],
            yaw,
            pitch,
            room,
            current_action_length,
            initial_position: [ix, iy, iz],
            guarding_position: [gx, gy, gz],
            tether_distance,
            ai_state,
            current_animation,
            carried_item,
            _pad0xa6,
            current_action_step,
            sprite_ids,
            sound_ids,
            spell_buffs,
            group,
            ally,
            schedules,
            summoner,
            last_attacker,
            _pad0x214,
        })
    }
}

impl DdmActor {
    pub fn to_bytes(&self) -> [u8; ACTOR_SIZE_MM6] {
        let mut out = [0u8; ACTOR_SIZE_MM6];
        let mut cursor = Cursor::new(&mut out[..]);
        use byteorder::{LittleEndian, WriteBytesExt};
        use std::io::Write;

        // Name (32 bytes)
        let mut name_bytes = [0u8; 32];
        let n = self.name.len().min(31);
        name_bytes[..n].copy_from_slice(&self.name.as_bytes()[..n]);
        cursor.write_all(&name_bytes).unwrap();

        // 0x20: NPC_ID(2) + pad(2) + Bits(4) + HP(2) + pad(2)
        cursor.set_position(0x20);
        cursor.write_i16::<LittleEndian>(self.npc_id).unwrap();
        cursor.write_i16::<LittleEndian>(self._pad0x22).unwrap();
        cursor.write_u32::<LittleEndian>(self.attributes).unwrap();
        cursor.write_i16::<LittleEndian>(self.hp).unwrap();
        cursor.write_i16::<LittleEndian>(self._pad0x2a).unwrap();

        // CommonMonsterProps at 0x2C (72 bytes)
        // We'll write it field-by-field.
        cursor.set_position(0x2C);
        let p = &self.common_props;
        cursor.write_all(&[0u8; 8]).unwrap(); // runtime pointers
        cursor.write_u8(p.monlist_id.saturating_add(1)).unwrap();
        cursor.write_u8(p.level).unwrap();
        cursor.write_u8(p.treasure_item_percent).unwrap();
        cursor.write_u8(p.treasure_dice_count).unwrap();
        cursor.write_u8(p.treasure_dice_sides).unwrap();
        cursor.write_u8(p.treasure_item_level).unwrap();
        cursor.write_u8(p.treasure_item_type).unwrap();
        cursor.write_u8(p.fly).unwrap();
        cursor.write_u8(p.move_type).unwrap();
        cursor.write_u8(p.ai_type).unwrap();
        cursor.write_u8(p.hostile_type).unwrap();
        cursor.write_u8(p.pref_class).unwrap();
        cursor.write_u8(p.bonus).unwrap();
        cursor.write_u8(p.bonus_mul).unwrap();
        cursor.write_u8(p.attack1.attack_type).unwrap();
        cursor.write_u8(p.attack1.damage_dice_count).unwrap();
        cursor.write_u8(p.attack1.damage_dice_sides).unwrap();
        cursor.write_u8(p.attack1.damage_add).unwrap();
        cursor.write_u8(p.attack1.missile).unwrap();
        cursor.write_u8(p.attack2_chance).unwrap();
        cursor.write_u8(p.attack2.attack_type).unwrap();
        cursor.write_u8(p.attack2.damage_dice_count).unwrap();
        cursor.write_u8(p.attack2.damage_dice_sides).unwrap();
        cursor.write_u8(p.attack2.damage_add).unwrap();
        cursor.write_u8(p.attack2.missile).unwrap();
        cursor.write_u8(p.spell_chance).unwrap();
        cursor.write_u8(p.spell).unwrap();
        cursor.write_u8(p.spell_skill).unwrap();
        cursor.write_u8(p.fire_resistance).unwrap();
        cursor.write_u8(p.elec_resistance).unwrap();
        cursor.write_u8(p.cold_resistance).unwrap();
        cursor.write_u8(p.poison_resistance).unwrap();
        cursor.write_u8(p.phys_resistance).unwrap();
        cursor.write_u8(p.magic_resistance).unwrap();
        cursor.write_u8(p.pref_num).unwrap();
        cursor.write_u8(0).unwrap(); // pad 0x2B
        cursor.write_i16::<LittleEndian>(p.quest_item).unwrap();
        cursor.write_all(&[0u8; 2]).unwrap(); // pad 0x2E
        cursor.write_i32::<LittleEndian>(p.full_hp).unwrap();
        cursor.write_i32::<LittleEndian>(p.armor_class).unwrap();
        cursor.write_i32::<LittleEndian>(p.experience).unwrap();
        cursor.write_i32::<LittleEndian>(p.move_speed).unwrap();
        cursor.write_i32::<LittleEndian>(p.attack_recovery).unwrap();
        cursor.write_i32::<LittleEndian>(p._unknown).unwrap();

        // 0x74: RangeAttack(2) + MonsterIdType(2)
        cursor.set_position(0x74);
        cursor.write_i16::<LittleEndian>(self.range_attack).unwrap();
        cursor.write_i16::<LittleEndian>(self.monster_id_type).unwrap();
        cursor.write_u16::<LittleEndian>(self.radius).unwrap();
        cursor.write_u16::<LittleEndian>(self.height).unwrap();
        cursor.write_u16::<LittleEndian>(self.move_speed).unwrap();
        for &p in &self.position {
            cursor.write_i16::<LittleEndian>(p).unwrap();
        }
        for &v in &self.velocity {
            cursor.write_i16::<LittleEndian>(v).unwrap();
        }
        cursor.write_u16::<LittleEndian>(self.yaw).unwrap();
        cursor.write_u16::<LittleEndian>(self.pitch).unwrap();
        cursor.write_i16::<LittleEndian>(self.room).unwrap();
        cursor.write_u16::<LittleEndian>(self.current_action_length).unwrap();
        for &p in &self.initial_position {
            cursor.write_i16::<LittleEndian>(p).unwrap();
        }
        for &p in &self.guarding_position {
            cursor.write_i16::<LittleEndian>(p).unwrap();
        }
        cursor.write_u16::<LittleEndian>(self.tether_distance).unwrap();
        cursor.write_u16::<LittleEndian>(self.ai_state).unwrap();
        cursor.write_u16::<LittleEndian>(self.current_animation).unwrap();
        cursor.write_u16::<LittleEndian>(self.carried_item).unwrap();
        cursor.write_u16::<LittleEndian>(self._pad0xa6).unwrap();
        cursor.write_u32::<LittleEndian>(self.current_action_step).unwrap();
        for &sid in &self.sprite_ids {
            cursor.write_u16::<LittleEndian>(sid).unwrap();
        }
        for &sid in &self.sound_ids {
            cursor.write_u16::<LittleEndian>(sid).unwrap();
        }
        for buff in &self.spell_buffs {
            cursor.write_i64::<LittleEndian>(buff.expire_time).unwrap();
            cursor.write_i16::<LittleEndian>(buff.power).unwrap();
            cursor.write_i16::<LittleEndian>(buff.skill).unwrap();
            cursor.write_i16::<LittleEndian>(buff.overlay_id).unwrap();
            cursor.write_u8(buff.caster).unwrap();
            cursor.write_u8(buff.bits).unwrap();
        }
        cursor.write_i32::<LittleEndian>(self.group).unwrap();
        cursor.write_i32::<LittleEndian>(self.ally).unwrap();
        for s in &self.schedules {
            cursor.write_i16::<LittleEndian>(s.x).unwrap();
            cursor.write_i16::<LittleEndian>(s.y).unwrap();
            cursor.write_i16::<LittleEndian>(s.z).unwrap();
            cursor.write_u16::<LittleEndian>(s.bits).unwrap();
            cursor.write_u8(s.action).unwrap();
            cursor.write_u8(s.hour).unwrap();
            cursor.write_u8(s.day).unwrap();
            cursor.write_u8(s.month).unwrap();
        }
        cursor.write_i32::<LittleEndian>(self.summoner).unwrap();
        cursor.write_i32::<LittleEndian>(self.last_attacker).unwrap();
        cursor.write_all(&self._pad0x214).unwrap();

        out
    }
}

#[cfg(test)]
#[path = "ddm_tests.rs"]
mod tests;
