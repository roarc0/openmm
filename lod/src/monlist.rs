use std::error::Error;

use crate::{lod_data::LodData, utils::try_read_name, LodManager};

const RECORD_SIZE: usize = 148;

/// A monster/NPC description from dmonlist.bin.
pub struct MonsterDesc {
    pub height: u16,
    pub radius: u16,
    pub move_speed: u16,
    pub to_hit_radius: u16,
    pub sound_ids: [u16; 4],
    pub internal_name: String,
    /// Sprite names for each animation state:
    /// [0]=standing, [1]=walking, [2]=attack1, [3]=attack2,
    /// [4]=hit, [5]=dying, [6]=dead, [7]=fidget
    pub sprite_names: [String; 8],
}

/// The full monster description table loaded from dmonlist.bin.
pub struct MonsterList {
    pub monsters: Vec<MonsterDesc>,
}

impl MonsterList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dmonlist.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        if data.len() < 4 {
            return Err("dmonlist.bin too short".into());
        }
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + count * RECORD_SIZE {
            return Err("dmonlist.bin truncated".into());
        }

        let mut monsters = Vec::with_capacity(count);
        for i in 0..count {
            let off = 4 + i * RECORD_SIZE;
            let rec = &data[off..off + RECORD_SIZE];
            monsters.push(Self::parse_record(rec));
        }

        Ok(MonsterList { monsters })
    }

    fn parse_record(rec: &[u8]) -> MonsterDesc {
        let height = u16::from_le_bytes([rec[0], rec[1]]);
        let radius = u16::from_le_bytes([rec[2], rec[3]]);
        let move_speed = u16::from_le_bytes([rec[4], rec[5]]);
        let to_hit_radius = u16::from_le_bytes([rec[6], rec[7]]);
        let sound_ids = [
            u16::from_le_bytes([rec[8], rec[9]]),
            u16::from_le_bytes([rec[10], rec[11]]),
            u16::from_le_bytes([rec[12], rec[13]]),
            u16::from_le_bytes([rec[14], rec[15]]),
        ];

        let name_bytes = &rec[16..48];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(32);
        let internal_name = String::from_utf8_lossy(&name_bytes[..name_end]).to_string();

        let mut sprite_names: [String; 8] = Default::default();
        for (j, sn) in sprite_names.iter_mut().enumerate() {
            let s = 48 + j * 10;
            let bytes = &rec[s..s + 10];
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(10);
            *sn = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
        }

        MonsterDesc {
            height,
            radius,
            move_speed,
            to_hit_radius,
            sound_ids,
            internal_name,
            sprite_names,
        }
    }

    /// Get a monster description by 0-based index.
    pub fn get(&self, id: usize) -> Option<&MonsterDesc> {
        self.monsters.get(id)
    }

    /// Find a monster by internal name prefix + difficulty (1=A, 2=B, 3=C).
    /// e.g. find_by_name("Goblin", 1) → GoblinA
    pub fn find_by_name(&self, name: &str, difficulty: u8) -> Option<&MonsterDesc> {
        let suffix = match difficulty {
            1 => "A",
            2 => "B",
            3 | _ => "C",
        };
        let target = format!("{}{}", name, suffix);
        self.monsters.iter().find(|m| m.internal_name.eq_ignore_ascii_case(&target))
    }
}
