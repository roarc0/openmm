use serde::{Deserialize, Serialize};
use std::error::Error;

use crate::LodSerialise;
use crate::{Assets, assets::lod_data::LodData};

const RECORD_SIZE: usize = 148;

/// A monster/NPC description from dmonlist.bin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterDesc {
    /// Sprite height in MM6 units. Offset 0x00.
    pub height: u16,
    /// Body collision radius in MM6 units. Used for `attack_range * 2` in game. Offset 0x02.
    pub radius: u16,
    /// Movement speed in MM6 units/tick. Offset 0x04.
    pub move_speed: u16,
    /// Bytes 6-7. Always 0 in MM6 (this is the MM7+ `Radius2` field). Do not use for attack range.
    /// Use `radius * 2` for melee reach instead. Offset 0x06.
    pub to_hit_radius: u16,
    /// Sound IDs: [0]=attack, [1]=die, [2]=got_hit, [3]=fidget. Offset 0x08.
    pub sound_ids: [u16; 4],
    /// Internal monster name from dmonlist.bin, e.g. "GoblinA". Null-terminated, 32 bytes. Offset 0x10.
    pub internal_name: String,
    /// DSFT group names for each animation state (10 bytes each, null-terminated). Offset 0x30.
    /// [0]=standing, [1]=walking, [2]=attack1, [3]=attack2,
    /// [4]=hit, [5]=dying, [6]=dead, [7]=fidget
    pub sprite_names: [String; 8],
}

/// The full monster description table loaded from dmonlist.bin.
#[derive(Debug, Serialize, Deserialize)]
pub struct MonsterList {
    pub monsters: Vec<MonsterDesc>,
}

impl MonsterList {
    pub fn load(assets: &Assets) -> Result<Self, Box<dyn Error>> {
        let raw = assets.get_bytes("icons/dmonlist.bin")?;
        Self::try_from(raw.as_slice())
    }

    pub fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
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

        // Bytes 128-147: two unused 10-byte frame-name slots (FramesUnk1/FramesUnk2).
        // MMExtension's MonListItem struct also skips these with `.skip(20)` — they are
        // intentionally empty even in the original engine.

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

    pub fn get_by_id(&self, id: usize) -> Option<&MonsterDesc> {
        self.monsters.get(id)
    }

    /// Get a monster description by 0-based index.
    pub fn get(&self, id: usize) -> Option<&MonsterDesc> {
        self.monsters.get(id)
    }

    /// Returns true if the given 0-based monlist_id is a peasant type.
    /// Checks the actual internal_name from dmonlist.bin for a "Peasant" prefix.
    pub fn is_peasant(&self, monlist_id: u8) -> bool {
        self.monsters
            .get(monlist_id as usize)
            .is_some_and(|m| m.internal_name.to_ascii_lowercase().starts_with("peasant"))
    }

    /// Returns true if the given peasant monlist_id is female (PeasantF types).
    /// Checks the internal_name for the "F" marker after "Peasant".
    pub fn is_female_peasant(&self, monlist_id: u8) -> bool {
        self.monsters.get(monlist_id as usize).is_some_and(|m| {
            let lower = m.internal_name.to_ascii_lowercase();
            lower.starts_with("peasantf")
        })
    }

    /// Find a monster by internal name prefix + difficulty (1=A, 2=B, 3=C).
    /// Tries the requested variant first, then falls back to A, B, C.
    pub fn find_by_name(&self, name: &str, difficulty: u8) -> Option<&MonsterDesc> {
        let preferred = match difficulty {
            1 => "A",
            2 => "B",
            _ => "C",
        };
        // Try preferred variant first, then all variants
        for suffix in &[preferred, "A", "B", "C"] {
            let target = format!("{}{}", name, suffix);
            if let Some(m) = self
                .monsters
                .iter()
                .find(|m| m.internal_name.eq_ignore_ascii_case(&target))
            {
                return Some(m);
            }
        }
        None
    }

    /// Find a monster whose sprite actually exists in the LOD.
    /// Uses get_bytes (cheap) instead of sprite() (expensive decode).
    pub fn find_with_sprite(&self, name: &str, difficulty: u8, assets: &crate::Assets) -> Option<&MonsterDesc> {
        let preferred = match difficulty {
            1 => "A",
            2 => "B",
            _ => "C",
        };
        for suffix in &[preferred, "A", "B", "C"] {
            let target = format!("{}{}", name, suffix);
            if let Some(m) = self
                .monsters
                .iter()
                .find(|m| m.internal_name.eq_ignore_ascii_case(&target))
            {
                let sprite = &m.sprite_names[0];
                if !sprite.is_empty() {
                    // Cheap check: just see if the raw bytes exist in the LOD
                    let root = sprite.trim_end_matches(|c: char| c.is_ascii_digit());
                    let check_name = if root.ends_with('a') {
                        format!("{}0", root)
                    } else {
                        format!("{}a0", root)
                    };
                    if assets.get_bytes(format!("sprites/{}", check_name)).is_ok() {
                        return Some(m);
                    }
                    // Try stripped root
                    if root.len() > 1 {
                        let shorter = &root[..root.len() - 1];
                        let check2 = format!("{}a0", shorter);
                        if assets.get_bytes(format!("sprites/{}", check2)).is_ok() {
                            return Some(m);
                        }
                    }
                }
            }
        }
        self.find_by_name(name, difficulty)
    }
}

impl TryFrom<&[u8]> for MonsterList {
    type Error = Box<dyn Error>;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let data = LodData::try_from(data)?;
        Self::parse(&data.data)
    }
}

impl LodSerialise for MonsterList {
    /// Serialize the full monster list back to dmonlist.bin binary format.
    fn to_bytes(&self) -> Vec<u8> {
        use byteorder::{LittleEndian, WriteBytesExt};
        let mut out = Vec::with_capacity(4 + self.monsters.len() * RECORD_SIZE);
        out.write_u32::<LittleEndian>(self.monsters.len() as u32).unwrap();

        for m in &self.monsters {
            out.write_u16::<LittleEndian>(m.height).unwrap();
            out.write_u16::<LittleEndian>(m.radius).unwrap();
            out.write_u16::<LittleEndian>(m.move_speed).unwrap();
            out.write_u16::<LittleEndian>(m.to_hit_radius).unwrap();
            for &sid in &m.sound_ids {
                out.write_u16::<LittleEndian>(sid).unwrap();
            }

            // Name (32 bytes)
            let mut name_bytes = [0u8; 32];
            let n = m.internal_name.len().min(31);
            name_bytes[..n].copy_from_slice(&m.internal_name.as_bytes()[..n]);
            out.extend_from_slice(&name_bytes);

            // Sprite Names (8 x 10 bytes)
            for sn in &m.sprite_names {
                let mut sn_bytes = [0u8; 10];
                let n = sn.len().min(9);
                sn_bytes[..n].copy_from_slice(&sn.as_bytes()[..n]);
                out.extend_from_slice(&sn_bytes);
            }

            // Unused padding (20 bytes)
            out.extend_from_slice(&[0u8; 20]);
        }
        out
    }
}

#[cfg(test)]
#[path = "monlist_tests.rs"]
mod tests;
