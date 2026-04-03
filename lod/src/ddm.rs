use std::error::Error;
use std::io::{Cursor, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::enums::ActorAttributes;
use crate::{LodManager, lod_data::LodData};

/// MM6 MapMonster struct size = 0x224 = 548 bytes.
/// Layout from MMExtension: Scripts/Structs/01 common structs.lua (MapMonster).
const ACTOR_SIZE_MM6: usize = 548;

/// A spell buff on an actor (16 bytes each, 14 per actor).
///
/// Layout: ExpireTime(8) + Power(2) + Skill(2) + OverlayId(2) + Caster(1) + Bits(1)
#[derive(Debug, Clone, Copy, Default)]
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
#[derive(Debug, Clone, Copy, Default)]
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
#[derive(Debug, Clone)]
pub struct DdmActor {
    /// Actor name. Offset 0x00 (32 bytes, null-terminated).
    pub name: String,
    /// Index into dmonlist.bin (MonsterList). Contains both NPCs and monsters.
    /// Offset 0x34 (CommonMonsterProps.Id, u8). 1-indexed in file, stored as 0-indexed.
    pub monlist_id: u8,
    /// NPC dialogue index (Game.StreetNPC + 1). Zero for monsters.
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
    /// CommonMonsterProps raw bytes (72 bytes at offset 0x2C).
    /// Stored raw for round-tripping; monlist_id is extracted separately.
    pub common_props: [u8; 72],
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
}

/// Parsed DDM delta file.
#[derive(Debug)]
pub struct Ddm {
    pub actors: Vec<DdmActor>,
}

impl Ddm {
    pub fn new(lod_manager: &LodManager, map_name: &str) -> Result<Self, Box<dyn Error>> {
        let ddm_name = map_name.replace(".odm", ".ddm");
        let raw = lod_manager.try_get_bytes(format!("games/{}", ddm_name))?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
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

        Ok(Ddm { actors })
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

        // CommonMonsterProps: 72 bytes at offset 0x2C. Store raw for round-tripping.
        let mut common_props = [0u8; 72];
        common_props.copy_from_slice(&data[0x2C..0x2C + 72]);

        // The Id field is 1-indexed in the game engine; subtract 1 for our 0-based monlist array.
        let monlist_id = data[0x34].saturating_sub(1);

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

#[cfg(test)]
#[path = "ddm_tests.rs"]
mod tests;
