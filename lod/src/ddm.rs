use std::error::Error;
use std::io::{Cursor, Seek};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{lod_data::LodData, LodManager};

/// MM6 MapMonster struct size = 0x224 = 548 bytes.
/// Layout from MMExtension: Scripts/Structs/01 common structs.lua (MapMonster).
const ACTOR_SIZE_MM6: usize = 548;

/// An actor (monster/NPC) from the DDM delta file.
///
/// Field offsets verified against MMExtension's MapMonster struct (MM6, o=0):
///   0x00: Name[32], 0x20: NPC_ID, 0x24: Bits, 0x28: HP,
///   0x2C: CommonMonsterProps (skip 8 + Id @ 0x34, Level @ 0x35, ...),
///   0x78: BodyRadius, 0x7A: BodyHeight, 0x7C: Velocity,
///   0x7E: Pos[3], 0x84: Vel[3], 0x8A: Direction, 0x8E: Room,
///   0x92: Start[3], 0x98: Guard[3], 0x9E: GuardRadius,
///   0xA0: AIState, 0xA2: GraphicState, 0xA4: Item,
///   0xA8: CurrentActionStep, 0xAC: Frames[8], 0xBC: Sounds[4],
///   0xC4: SpellBuffs[14], ...
#[derive(Debug, Clone)]
pub struct DdmActor {
    pub name: String,
    /// Index into dmonlist.bin (MonsterList). Contains both NPCs and monsters.
    /// Offset 0x34 (CommonMonsterProps.Id, u8).
    pub monlist_id: u8,
    /// NPC dialogue index (Game.StreetNPC + 1). Zero for monsters.
    /// Offset 0x20 (i16).
    pub npc_id: i16,
    pub hp: i16,
    pub radius: u16,
    pub height: u16,
    pub move_speed: u16,
    /// Current position in MM6 coordinates (x, y, z as i16). Offset 0x7E.
    pub position: [i16; 3],
    /// Velocity. Offset 0x84.
    pub velocity: [i16; 3],
    /// Facing angle (0-65535 for 360 degrees). Offset 0x8A.
    pub yaw: u16,
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
    /// DSFT frame table indices for 8 animation states. Offset 0xAC.
    /// Zero in DDM files — populated at runtime by LoadFrames(). Use monlist_id instead.
    pub sprite_ids: [u16; 8],
}

/// Parsed DDM delta file.
pub struct Ddm {
    pub actors: Vec<DdmActor>,
}

impl Ddm {
    pub fn new(lod_manager: &LodManager, map_name: &str) -> Result<Self, Box<dyn Error>> {
        let ddm_name = map_name.replace(".odm", ".ddm");
        let raw = lod_manager.try_get_bytes(&format!("games/{}", ddm_name))?;
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
            if start + ACTOR_SIZE_MM6 > data.len() { break; }
            if let Some(actor) = Self::read_actor(&data[start..start + ACTOR_SIZE_MM6]) {
                actors.push(actor);
            }
        }
        actors
    }

    fn find_actors(data: &[u8]) -> Result<usize, Box<dyn Error>> {
        for offset in (0..data.len().saturating_sub(40)).step_by(2) {
            let val = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            if val >= 1 && val <= 500 {
                let name_start = offset + 4;
                if name_start + 32 > data.len() {
                    continue;
                }
                let name_bytes = &data[name_start..name_start + 32];
                let is_name = name_bytes.iter().take(20).all(|&b| b == 0 || (b >= 0x20 && b <= 0x7E));
                let has_alpha = name_bytes.iter().take(10).any(|&b| b.is_ascii_alphabetic());
                if is_name && has_alpha {
                    // Verify second actor name too
                    let second = name_start + ACTOR_SIZE_MM6;
                    if second + 32 <= data.len() && val > 1 {
                        let name2 = &data[second..second + 32];
                        let is_name2 = name2.iter().take(20).all(|&b| b == 0 || (b >= 0x20 && b <= 0x7E));
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

    fn read_actor(data: &[u8]) -> Option<DdmActor> {
        let name_end = data[..32].iter().position(|&b| b == 0).unwrap_or(32);
        let name = String::from_utf8_lossy(&data[..name_end]).to_string();

        let mut c = Cursor::new(data);
        c.seek(std::io::SeekFrom::Start(32)).ok()?; // skip name

        // Offset 0x20: NPC_ID(2) + pad(2) + Bits(4) + HP(2) + pad(2) = 12 bytes
        let npc_id = c.read_i16::<LittleEndian>().ok()?;
        let _pad = c.read_i16::<LittleEndian>().ok()?;
        let _attrs = c.read_u32::<LittleEndian>().ok()?;
        let hp = c.read_i16::<LittleEndian>().ok()?;
        let _pad2 = c.read_i16::<LittleEndian>().ok()?;

        // CommonMonsterProps starts at 0x2C: skip(8) + Id(u8) @ 0x34.
        // Confirmed via MMExtension MapMonster struct definition.
        // The Id field is 1-indexed in the game engine; subtract 1 for our 0-based monlist array.
        let monlist_id = data[0x34].saturating_sub(1);

        // Skip to position fields (MM6 offsets, verified to produce correct coordinates)
        c.seek(std::io::SeekFrom::Start(0x74)).ok()?;
        let _field84 = c.read_i16::<LittleEndian>().ok()?;
        let _monster_id2 = c.read_i16::<LittleEndian>().ok()?;

        // Offset 0x78: radius(2) + height(2) + moveSpeed(2)
        let radius = c.read_u16::<LittleEndian>().ok()?;
        let height = c.read_u16::<LittleEndian>().ok()?;
        let move_speed = c.read_u16::<LittleEndian>().ok()?;

        // Offset 0x7E: position (i16 x 3)
        let px = c.read_i16::<LittleEndian>().ok()?;
        let py = c.read_i16::<LittleEndian>().ok()?;
        let pz = c.read_i16::<LittleEndian>().ok()?;

        // Velocity (i16 x 3)
        let vx = c.read_i16::<LittleEndian>().ok()?;
        let vy = c.read_i16::<LittleEndian>().ok()?;
        let vz = c.read_i16::<LittleEndian>().ok()?;

        // Yaw + pitch
        let yaw = c.read_u16::<LittleEndian>().ok()?;
        let _pitch = c.read_u16::<LittleEndian>().ok()?;
        let _sector = c.read_i16::<LittleEndian>().ok()?;
        let _action_len = c.read_u16::<LittleEndian>().ok()?;

        // Initial position
        let ix = c.read_i16::<LittleEndian>().ok()?;
        let iy = c.read_i16::<LittleEndian>().ok()?;
        let iz = c.read_i16::<LittleEndian>().ok()?;

        // Guarding position
        let gx = c.read_i16::<LittleEndian>().ok()?;
        let gy = c.read_i16::<LittleEndian>().ok()?;
        let gz = c.read_i16::<LittleEndian>().ok()?;

        let tether = c.read_u16::<LittleEndian>().ok()?;
        let ai_state = c.read_u16::<LittleEndian>().ok()?; // actually i16 in MM7
        let current_animation = c.read_u16::<LittleEndian>().ok()?;
        let _carried_item = c.read_u16::<LittleEndian>().ok()?;
        let _pad3 = c.read_u16::<LittleEndian>().ok()?;
        let _action_time = c.read_u32::<LittleEndian>().ok()?;

        // Sprite frame table IDs (DSFT indices) at offset 0xAC.
        // 8 entries: standing, walking, attack, shoot, stun, dying, dead, fidget.
        // (0xBC is the Sounds array, not Frames — confirmed via MMExtension structs.)
        c.seek(std::io::SeekFrom::Start(0xAC)).ok()?;
        let mut sprite_ids = [0u16; 8];
        for sid in &mut sprite_ids {
            *sid = c.read_u16::<LittleEndian>().ok()?;
        }

        Some(DdmActor {
            name,
            monlist_id,
            npc_id,
            hp,
            radius,
            height,
            move_speed,
            position: [px, py, pz],
            velocity: [vx, vy, vz],
            yaw,
            initial_position: [ix, iy, iz],
            guarding_position: [gx, gy, gz],
            tether_distance: tether,
            ai_state,
            current_animation,
            sprite_ids,
        })
    }
}
