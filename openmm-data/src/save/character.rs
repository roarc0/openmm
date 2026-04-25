// MM6 SaveCharacter parser.
//
// Each character occupies 0x161C (5660) bytes inside `party.bin`.
// Four characters start at offset 0x02C4 in party.bin.
//
// Stores a full copy of the raw bytes for round-trip fidelity --
// fields we don't parse stay intact through parse -> to_bytes cycles.

/// Size of a single character record in bytes.
pub const CHARACTER_SIZE: usize = 0x161C; // 5660

/// Parsed MM6 character from a save file.
#[derive(Debug, Clone)]
pub struct SaveCharacter {
    /// Raw bytes for round-trip serialization.
    raw: Vec<u8>,

    pub face: u8,
    pub name: String,
    /// 0 = male, 1 = female.
    pub sex: u8,
    /// Class index (e.g. 9 = Paladin, 12 = ?, 3 = ?, 6 = Knight).
    pub class: u8,
    /// Base character level.
    pub level: i16,
    /// Base stats: Might, Intellect, Personality, Endurance, Accuracy, Speed, Luck.
    pub base_stats: [i16; 7],
    /// Bonus stats (same order as base_stats).
    pub stat_bonuses: [i16; 7],
    /// Skill levels (31 skills).
    pub skills: [u8; 31],
    /// Base resistances: Fire, Elec, Cold, Poison, Magic.
    pub resistances: [i16; 5],
    /// Bonus resistances (same order).
    pub resistance_bonuses: [i16; 5],
    /// Recovery delay in ticks.
    pub recovery_delay: i16,
    /// Current hit points.
    pub hp: i32,
    /// Current spell points.
    pub sp: i32,
    /// Character birth year.
    pub birth_year: i32,
    /// Total experience points.
    pub experience: i64,
    /// Unspent skill points.
    pub skill_points: i32,
}

impl SaveCharacter {
    /// Parse a character from raw bytes. Panics if `data.len() < CHARACTER_SIZE`.
    pub fn parse(data: &[u8]) -> Self {
        assert!(
            data.len() >= CHARACTER_SIZE,
            "SaveCharacter::parse: need {} bytes, got {}",
            CHARACTER_SIZE,
            data.len()
        );

        let raw = data[..CHARACTER_SIZE].to_vec();

        let face = data[0x0000];

        // Name: 16 bytes at 0x0001, null-terminated.
        let name_bytes = &data[0x0001..0x0011];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(16);
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();

        let sex = data[0x0011];
        let class = data[0x0012];

        // Base level at 0x001C (i16 LE).
        let level = i16::from_le_bytes([data[0x001C], data[0x001D]]);

        // Stats at 0x0014: 7 pairs of (base i16, bonus i16) = 28 bytes.
        let mut base_stats = [0i16; 7];
        let mut stat_bonuses = [0i16; 7];
        for i in 0..7 {
            let off = 0x0014 + i * 4;
            base_stats[i] = i16::from_le_bytes([data[off], data[off + 1]]);
            stat_bonuses[i] = i16::from_le_bytes([data[off + 2], data[off + 3]]);
        }

        // Skills at 0x0060: 31 bytes.
        let mut skills = [0u8; 31];
        skills.copy_from_slice(&data[0x0060..0x007F]);

        // Resistances at 0x1254: 5 pairs of (base i16, bonus i16) = 20 bytes.
        let mut resistances = [0i16; 5];
        let mut resistance_bonuses = [0i16; 5];
        for i in 0..5 {
            let off = 0x1254 + i * 4;
            resistances[i] = i16::from_le_bytes([data[off], data[off + 1]]);
            resistance_bonuses[i] = i16::from_le_bytes([data[off + 2], data[off + 3]]);
        }

        let recovery_delay = i16::from_le_bytes([data[0x137C], data[0x137D]]);

        let hp = i32::from_le_bytes([data[0x1414], data[0x1415], data[0x1416], data[0x1417]]);
        let sp = i32::from_le_bytes([data[0x1418], data[0x1419], data[0x141A], data[0x141B]]);
        let birth_year = i32::from_le_bytes([data[0x141C], data[0x141D], data[0x141E], data[0x141F]]);
        let experience = i64::from_le_bytes([
            data[0x1420],
            data[0x1421],
            data[0x1422],
            data[0x1423],
            data[0x1424],
            data[0x1425],
            data[0x1426],
            data[0x1427],
        ]);
        let skill_points = i32::from_le_bytes([data[0x1410], data[0x1411], data[0x1412], data[0x1413]]);

        Self {
            raw,
            face,
            name,
            sex,
            class,
            level,
            base_stats,
            stat_bonuses,
            skills,
            resistances,
            resistance_bonuses,
            recovery_delay,
            hp,
            sp,
            birth_year,
            experience,
            skill_points,
        }
    }

    /// Serialize back to bytes, patching parsed fields into the raw copy.
    /// Fields we don't parse stay exactly as they were in the original data.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = self.raw.clone();

        buf[0x0000] = self.face;

        // Name: write up to 16 bytes, pad with zeroes.
        let name_bytes = self.name.as_bytes();
        let copy_len = name_bytes.len().min(15); // leave room for null terminator
        buf[0x0001..0x0011].fill(0);
        buf[0x0001..0x0001 + copy_len].copy_from_slice(&name_bytes[..copy_len]);

        buf[0x0011] = self.sex;
        buf[0x0012] = self.class;

        buf[0x001C..0x001E].copy_from_slice(&self.level.to_le_bytes());

        for i in 0..7 {
            let off = 0x0014 + i * 4;
            buf[off..off + 2].copy_from_slice(&self.base_stats[i].to_le_bytes());
            buf[off + 2..off + 4].copy_from_slice(&self.stat_bonuses[i].to_le_bytes());
        }

        buf[0x0060..0x007F].copy_from_slice(&self.skills);

        for i in 0..5 {
            let off = 0x1254 + i * 4;
            buf[off..off + 2].copy_from_slice(&self.resistances[i].to_le_bytes());
            buf[off + 2..off + 4].copy_from_slice(&self.resistance_bonuses[i].to_le_bytes());
        }

        buf[0x137C..0x137E].copy_from_slice(&self.recovery_delay.to_le_bytes());
        buf[0x1410..0x1414].copy_from_slice(&self.skill_points.to_le_bytes());
        buf[0x1414..0x1418].copy_from_slice(&self.hp.to_le_bytes());
        buf[0x1418..0x141C].copy_from_slice(&self.sp.to_le_bytes());
        buf[0x141C..0x1420].copy_from_slice(&self.birth_year.to_le_bytes());
        buf[0x1420..0x1428].copy_from_slice(&self.experience.to_le_bytes());

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Offset where characters start in party.bin.
    const PARTY_CHARS_OFFSET: usize = 0x02C4;

    /// Load party.bin from new.lod test data.
    fn load_party_bin() -> Vec<u8> {
        let save = crate::save::file::SaveFile::open("../data/mm6/data/new.lod").expect("open new.lod");
        save.get_file("party.bin").expect("party.bin missing")
    }

    /// Extract character N (0-based) from party.bin bytes.
    fn char_slice(party: &[u8], index: usize) -> &[u8] {
        let start = PARTY_CHARS_OFFSET + index * CHARACTER_SIZE;
        &party[start..start + CHARACTER_SIZE]
    }

    #[test]
    fn parse_roderick() {
        let party = load_party_bin();
        let rod = SaveCharacter::parse(char_slice(&party, 0));
        assert_eq!(rod.name, "Roderick");
        assert_eq!(rod.sex, 0, "Roderick is male");
        assert_eq!(rod.class, 9, "Roderick is Paladin (class 9)");
        assert_eq!(rod.hp, 31, "Roderick starts with 31 HP");
    }

    #[test]
    fn parse_alexis() {
        let party = load_party_bin();
        let alex = SaveCharacter::parse(char_slice(&party, 1));
        assert_eq!(alex.name, "Alexis");
        assert_eq!(alex.sex, 1, "Alexis is female");
        assert_eq!(alex.class, 12);
    }

    #[test]
    fn parse_serena() {
        let party = load_party_bin();
        let serena = SaveCharacter::parse(char_slice(&party, 2));
        assert_eq!(serena.name, "Serena");
        assert_eq!(serena.sex, 1, "Serena is female");
        assert_eq!(serena.class, 3);
    }

    #[test]
    fn parse_zoltan() {
        let party = load_party_bin();
        let zoltan = SaveCharacter::parse(char_slice(&party, 3));
        assert_eq!(zoltan.name, "Zoltan");
        assert_eq!(zoltan.sex, 0, "Zoltan is male");
        assert_eq!(zoltan.class, 6, "Zoltan is Knight (class 6)");
    }

    #[test]
    fn round_trip_all_characters() {
        let party = load_party_bin();
        for i in 0..4 {
            let original = char_slice(&party, i);
            let parsed = SaveCharacter::parse(original);
            let serialized = parsed.to_bytes();
            assert_eq!(
                serialized.as_slice(),
                original,
                "round-trip mismatch for character {}",
                i
            );
        }
    }
}
