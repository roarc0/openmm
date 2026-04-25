// MM6 SaveCharacter parser.
//
// Each character occupies 0x161C (5660) bytes inside `party.bin`.
// Four characters start at offset 0x02C4 in party.bin.
//
// Stores a full copy of the raw bytes for round-trip fidelity --
// fields we don't parse stay intact through parse -> to_bytes cycles.

/// Size of a single character record in bytes.
pub const CHARACTER_SIZE: usize = 0x161C; // 5660

// ── Binary layout offsets ──────────────────────────────────────────
const FACE_OFFSET: usize = 0x0000;
const NAME_OFFSET: usize = 0x0001;
const NAME_LEN: usize = 16; // null-terminated, max 15 chars + null
const SEX_OFFSET: usize = 0x0011;
const CLASS_OFFSET: usize = 0x0012;
const STATS_OFFSET: usize = 0x0014; // 7 × (base i16, bonus i16) = 28 bytes
const STAT_COUNT: usize = 7;
const STAT_PAIR_SIZE: usize = 4; // base i16 + bonus i16
const LEVEL_OFFSET: usize = 0x001C;
const SKILLS_OFFSET: usize = 0x0060;
const SKILLS_LEN: usize = 31;
const RESISTANCES_OFFSET: usize = 0x1254; // 5 × (base i16, bonus i16) = 20 bytes
const RESISTANCE_COUNT: usize = 5;
const RECOVERY_DELAY_OFFSET: usize = 0x137C;
const SKILL_POINTS_OFFSET: usize = 0x1410;
const HP_OFFSET: usize = 0x1414;
const SP_OFFSET: usize = 0x1418;
const BIRTH_YEAR_OFFSET: usize = 0x141C;
const EXPERIENCE_OFFSET: usize = 0x1420;

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
    pub base_stats: [i16; STAT_COUNT],
    /// Bonus stats (same order as base_stats).
    pub stat_bonuses: [i16; STAT_COUNT],
    /// Skill levels (31 skills).
    pub skills: [u8; SKILLS_LEN],
    /// Base resistances: Fire, Elec, Cold, Poison, Magic.
    pub resistances: [i16; RESISTANCE_COUNT],
    /// Bonus resistances (same order).
    pub resistance_bonuses: [i16; RESISTANCE_COUNT],
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

/// Read stat or resistance pairs (base i16, bonus i16) from contiguous memory.
fn read_i16_pairs<const N: usize>(data: &[u8], offset: usize) -> ([i16; N], [i16; N]) {
    let mut base = [0i16; N];
    let mut bonus = [0i16; N];
    for i in 0..N {
        let off = offset + i * STAT_PAIR_SIZE;
        base[i] = i16::from_le_bytes([data[off], data[off + 1]]);
        bonus[i] = i16::from_le_bytes([data[off + 2], data[off + 3]]);
    }
    (base, bonus)
}

/// Write stat or resistance pairs back into a buffer.
fn write_i16_pairs<const N: usize>(buf: &mut [u8], offset: usize, base: &[i16; N], bonus: &[i16; N]) {
    for i in 0..N {
        let off = offset + i * STAT_PAIR_SIZE;
        buf[off..off + 2].copy_from_slice(&base[i].to_le_bytes());
        buf[off + 2..off + 4].copy_from_slice(&bonus[i].to_le_bytes());
    }
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

        let face = data[FACE_OFFSET];

        // Name: 16 bytes, null-terminated.
        let name_bytes = &data[NAME_OFFSET..NAME_OFFSET + NAME_LEN];
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
        let name = String::from_utf8_lossy(&name_bytes[..name_end]).into_owned();

        let sex = data[SEX_OFFSET];
        let class = data[CLASS_OFFSET];
        let level = i16::from_le_bytes([data[LEVEL_OFFSET], data[LEVEL_OFFSET + 1]]);

        let (base_stats, stat_bonuses) = read_i16_pairs::<STAT_COUNT>(data, STATS_OFFSET);

        let mut skills = [0u8; SKILLS_LEN];
        skills.copy_from_slice(&data[SKILLS_OFFSET..SKILLS_OFFSET + SKILLS_LEN]);

        let (resistances, resistance_bonuses) = read_i16_pairs::<RESISTANCE_COUNT>(data, RESISTANCES_OFFSET);

        let recovery_delay = i16::from_le_bytes([data[RECOVERY_DELAY_OFFSET], data[RECOVERY_DELAY_OFFSET + 1]]);

        let hp = i32::from_le_bytes(data[HP_OFFSET..HP_OFFSET + 4].try_into().unwrap());
        let sp = i32::from_le_bytes(data[SP_OFFSET..SP_OFFSET + 4].try_into().unwrap());
        let birth_year = i32::from_le_bytes(data[BIRTH_YEAR_OFFSET..BIRTH_YEAR_OFFSET + 4].try_into().unwrap());
        let experience = i64::from_le_bytes(data[EXPERIENCE_OFFSET..EXPERIENCE_OFFSET + 8].try_into().unwrap());
        let skill_points = i32::from_le_bytes(data[SKILL_POINTS_OFFSET..SKILL_POINTS_OFFSET + 4].try_into().unwrap());

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

        buf[FACE_OFFSET] = self.face;

        // Name: write up to 15 chars, pad with zeroes + null terminator.
        let name_bytes = self.name.as_bytes();
        let copy_len = name_bytes.len().min(NAME_LEN - 1);
        buf[NAME_OFFSET..NAME_OFFSET + NAME_LEN].fill(0);
        buf[NAME_OFFSET..NAME_OFFSET + copy_len].copy_from_slice(&name_bytes[..copy_len]);

        buf[SEX_OFFSET] = self.sex;
        buf[CLASS_OFFSET] = self.class;
        buf[LEVEL_OFFSET..LEVEL_OFFSET + 2].copy_from_slice(&self.level.to_le_bytes());

        write_i16_pairs(&mut buf, STATS_OFFSET, &self.base_stats, &self.stat_bonuses);

        buf[SKILLS_OFFSET..SKILLS_OFFSET + SKILLS_LEN].copy_from_slice(&self.skills);

        write_i16_pairs(
            &mut buf,
            RESISTANCES_OFFSET,
            &self.resistances,
            &self.resistance_bonuses,
        );

        buf[RECOVERY_DELAY_OFFSET..RECOVERY_DELAY_OFFSET + 2].copy_from_slice(&self.recovery_delay.to_le_bytes());
        buf[SKILL_POINTS_OFFSET..SKILL_POINTS_OFFSET + 4].copy_from_slice(&self.skill_points.to_le_bytes());
        buf[HP_OFFSET..HP_OFFSET + 4].copy_from_slice(&self.hp.to_le_bytes());
        buf[SP_OFFSET..SP_OFFSET + 4].copy_from_slice(&self.sp.to_le_bytes());
        buf[BIRTH_YEAR_OFFSET..BIRTH_YEAR_OFFSET + 4].copy_from_slice(&self.birth_year.to_le_bytes());
        buf[EXPERIENCE_OFFSET..EXPERIENCE_OFFSET + 8].copy_from_slice(&self.experience.to_le_bytes());

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
