// MM6 SaveParty parser for party.bin (64720 bytes).
//
// Contains party position, calendar, gold/food, quest bits, autonotes,
// and 4 character records. Keeps raw bytes for round-trip fidelity.

use super::character::{CHARACTER_SIZE, SaveCharacter};

/// Total size of party.bin in bytes.
pub const PARTY_BIN_SIZE: usize = 64720;

/// Offset where the 4 character records begin.
const PLAYERS_OFFSET: usize = 0x02C4;

/// Parsed party.bin from an MM6 save file.
#[derive(Debug, Clone)]
pub struct SaveParty {
    /// MM6 coordinates (x, y, z).
    pub position: [i32; 3],
    /// Facing direction, 0-2047.
    pub direction: i32,
    /// Vertical look angle, -512 to 512.
    pub look_angle: i32,
    /// Calendar year.
    pub year: i32,
    /// Calendar month (0-11).
    pub month: i32,
    /// Week of month (0-3).
    pub week: i32,
    /// Day of month (0-27).
    pub day: i32,
    /// Hour (0-23).
    pub hour: i32,
    /// Minute (0-59).
    pub minute: i32,
    /// Second (0-59).
    pub second: i32,
    /// Gold carried.
    pub gold: i32,
    /// Gold in bank.
    pub bank_gold: i32,
    /// Food rations.
    pub food: i32,
    /// Current map index into MapStats.txt (1-based). Offset 0x88.
    pub current_map_index: i32,
    /// Party reputation.
    pub reputation: i32,
    /// Total party deaths.
    pub deaths: i32,
    /// Indices of set quest bits (from 512-bit field, LSB-first).
    pub quest_bits: Vec<i32>,
    /// Indices of set autonote bits (from 128-bit field, LSB-first).
    pub autonote_bits: Vec<i32>,
    /// The four party members.
    pub characters: [SaveCharacter; 4],
    /// Raw bytes for round-trip serialization.
    raw: Vec<u8>,
}

/// Read an i32 (little-endian) from a byte slice at the given offset.
fn read_i32(data: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

/// Write an i32 (little-endian) into a byte slice at the given offset.
fn write_i32(buf: &mut [u8], offset: usize, val: i32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
}

/// Extract indices of set bits from a byte array (LSB-first ordering).
fn extract_set_bits(bytes: &[u8]) -> Vec<i32> {
    let mut result = Vec::new();
    for (byte_idx, &byte) in bytes.iter().enumerate() {
        for bit in 0..8 {
            if byte & (1 << bit) != 0 {
                result.push((byte_idx * 8 + bit) as i32);
            }
        }
    }
    result
}

/// Pack set-bit indices back into a zeroed byte array of the given length.
fn pack_bits(indices: &[i32], len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    for &idx in indices {
        let byte_idx = idx as usize / 8;
        let bit = idx as usize % 8;
        if byte_idx < len {
            bytes[byte_idx] |= 1 << bit;
        }
    }
    bytes
}

impl SaveParty {
    /// Parse party.bin from raw bytes. Panics if data is too short.
    pub fn parse(data: &[u8]) -> Self {
        assert!(
            data.len() >= PARTY_BIN_SIZE,
            "party.bin too short: {} bytes, need {}",
            data.len(),
            PARTY_BIN_SIZE,
        );

        let raw = data[..PARTY_BIN_SIZE].to_vec();

        let position = [read_i32(data, 0x0028), read_i32(data, 0x002C), read_i32(data, 0x0030)];
        let direction = read_i32(data, 0x0034);
        let look_angle = read_i32(data, 0x0038);

        let year = read_i32(data, 0x00A0);
        let month = read_i32(data, 0x00A4);
        let week = read_i32(data, 0x00A8);
        let day = read_i32(data, 0x00AC);
        let hour = read_i32(data, 0x00B0);
        let minute = read_i32(data, 0x00B4);
        let second = read_i32(data, 0x00B8);

        let current_map_index = data[0x008B] as i32;

        let food = read_i32(data, 0x00BC);
        let reputation = read_i32(data, 0x00D8);
        let gold = read_i32(data, 0x00E0);
        let bank_gold = read_i32(data, 0x00E4);
        let deaths = read_i32(data, 0x00E8);

        let quest_bits = extract_set_bits(&data[0x00FD..0x00FD + 64]);
        let autonote_bits = extract_set_bits(&data[0x013D..0x013D + 16]);

        let characters = std::array::from_fn(|i| {
            let start = PLAYERS_OFFSET + i * CHARACTER_SIZE;
            SaveCharacter::parse(&data[start..start + CHARACTER_SIZE])
        });

        Self {
            position,
            direction,
            look_angle,
            year,
            month,
            week,
            day,
            hour,
            minute,
            second,
            gold,
            bank_gold,
            current_map_index,
            food,
            reputation,
            deaths,
            quest_bits,
            autonote_bits,
            characters,
            raw,
        }
    }

    /// Serialize back to bytes, patching parsed fields into the raw copy.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = self.raw.clone();

        write_i32(&mut buf, 0x0028, self.position[0]);
        write_i32(&mut buf, 0x002C, self.position[1]);
        write_i32(&mut buf, 0x0030, self.position[2]);
        write_i32(&mut buf, 0x0034, self.direction);
        write_i32(&mut buf, 0x0038, self.look_angle);

        write_i32(&mut buf, 0x00A0, self.year);
        write_i32(&mut buf, 0x00A4, self.month);
        write_i32(&mut buf, 0x00A8, self.week);
        write_i32(&mut buf, 0x00AC, self.day);
        write_i32(&mut buf, 0x00B0, self.hour);
        write_i32(&mut buf, 0x00B4, self.minute);
        write_i32(&mut buf, 0x00B8, self.second);

        write_i32(&mut buf, 0x00BC, self.food);
        write_i32(&mut buf, 0x00D8, self.reputation);
        write_i32(&mut buf, 0x00E0, self.gold);
        write_i32(&mut buf, 0x00E4, self.bank_gold);
        write_i32(&mut buf, 0x00E8, self.deaths);

        // Pack quest bits and autonotes back.
        let qbits = pack_bits(&self.quest_bits, 64);
        buf[0x00FD..0x00FD + 64].copy_from_slice(&qbits);

        let anotes = pack_bits(&self.autonote_bits, 16);
        buf[0x013D..0x013D + 16].copy_from_slice(&anotes);

        // Patch character data back.
        for i in 0..4 {
            let start = PLAYERS_OFFSET + i * CHARACTER_SIZE;
            let char_bytes = self.characters[i].to_bytes();
            buf[start..start + CHARACTER_SIZE].copy_from_slice(&char_bytes);
        }

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Load party.bin from new.lod test data.
    fn load_party_bin() -> Vec<u8> {
        let save = crate::save::file::SaveFile::open("../data/mm6/data/new.lod").expect("open new.lod");
        save.get_file("party.bin").expect("party.bin missing")
    }

    #[test]
    fn parse_party_position() {
        let data = load_party_bin();
        let party = SaveParty::parse(&data);
        assert_eq!(party.position, [-9507, -8711, 161]);
        assert_eq!(party.direction, 1536);
        assert_eq!(party.look_angle, 0);
    }

    #[test]
    fn parse_party_calendar() {
        let data = load_party_bin();
        let party = SaveParty::parse(&data);
        assert_eq!(party.year, 1165);
        assert_eq!(party.month, 0);
        assert!(party.day <= 27, "day in 0-27 range");
        assert!(party.hour <= 23, "hour in 0-23 range");
        assert!(party.minute <= 59, "minute in 0-59 range");
    }

    #[test]
    fn parse_party_resources() {
        let data = load_party_bin();
        let party = SaveParty::parse(&data);
        assert_eq!(party.gold, 1067);
        assert_eq!(party.food, 16);
        assert_eq!(party.bank_gold, 0);
    }

    #[test]
    fn parse_party_characters() {
        let data = load_party_bin();
        let party = SaveParty::parse(&data);
        assert_eq!(party.characters[0].name, "Roderick");
        assert_eq!(party.characters[1].name, "Alexis");
        assert_eq!(party.characters[2].name, "Serena");
        assert_eq!(party.characters[3].name, "Zoltan");
    }

    #[test]
    fn roundtrip() {
        let data = load_party_bin();
        let party = SaveParty::parse(&data);
        let bytes = party.to_bytes();
        let reparsed = SaveParty::parse(&bytes);

        assert_eq!(reparsed.position, party.position);
        assert_eq!(reparsed.direction, party.direction);
        assert_eq!(reparsed.year, party.year);
        assert_eq!(reparsed.month, party.month);
        assert_eq!(reparsed.day, party.day);
        assert_eq!(reparsed.hour, party.hour);
        assert_eq!(reparsed.gold, party.gold);
        assert_eq!(reparsed.food, party.food);
        assert_eq!(reparsed.quest_bits, party.quest_bits);
        assert_eq!(reparsed.autonote_bits, party.autonote_bits);
        for i in 0..4 {
            assert_eq!(reparsed.characters[i].name, party.characters[i].name);
        }
        // Full byte-level round-trip.
        assert_eq!(bytes, data[..PARTY_BIN_SIZE]);
    }
}
