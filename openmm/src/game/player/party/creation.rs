use std::time::{SystemTime, UNIX_EPOCH};

use super::member::{ATTR_COUNT, Class, Skill};
use super::portrait::PortraitId;

/// Male character names — MM6-style mix of fantasy and classic.
const MALE_NAMES: &[&str] = &[
    "Zoltan", "Harry", "Roderick", "Henry", "George", "Trent", "Randall", "Franklyn", "Newt", "Armand", "Zorex",
    "Quentin", "Dal", "Bo", "Frank", "Barney", "Abe", "Albert", "Joseph", "Trevor", "Roland", "Drake", "Stefan",
    "Marcus", "Cain", "Bors", "Edmund", "Gareth", "Owen", "Morton", "Warren", "Cecil", "Percival", "Victor", "Duncan",
    "Erik", "Leopold", "Conrad", "Gilbert", "Wade", "Aldric", "Brennan", "Corvin", "Desmond", "Evander",
];

/// Female character names — MM6-style mix of fantasy and classic.
const FEMALE_NAMES: &[&str] = &[
    "Serena",
    "Alexis",
    "Cornelia",
    "Emma",
    "Sharon",
    "Marcia",
    "Jenny",
    "Patricia",
    "Mamie",
    "Linda",
    "Regina",
    "Sylvia",
    "Meredith",
    "Vivian",
    "Agnes",
    "Hildegard",
    "Miriam",
    "Winifred",
    "Claire",
    "Dorothea",
    "Elspeth",
    "Florence",
    "Irene",
    "Lavinia",
    "Octavia",
    "Penelope",
    "Roberta",
    "Theresa",
    "Ursula",
    "Vera",
    "Wilma",
    "Adela",
    "Brenna",
    "Cora",
    "Della",
    "Evangeline",
];

const ALL_CLASSES: &[Class] = &[
    Class::Knight,
    Class::Paladin,
    Class::Archer,
    Class::Cleric,
    Class::Druid,
    Class::Sorcerer,
];

#[derive(Debug, Clone, Copy)]
pub struct CharCreationSeed {
    pub class: Class,
    pub portrait: PortraitId,
    pub name: &'static str,
    pub base_attrs: [i16; ATTR_COUNT],
}

pub fn class_base_attrs(class: Class) -> [i16; ATTR_COUNT] {
    // [Might, Intellect, Personality, Endurance, Speed, Accuracy, Luck]
    match class {
        Class::Knight => [15, 7, 7, 15, 9, 15, 7],
        Class::Paladin => [13, 7, 11, 13, 9, 13, 7],
        Class::Archer => [11, 9, 7, 11, 13, 15, 9],
        Class::Cleric => [9, 7, 15, 11, 9, 7, 13],
        Class::Druid => [9, 11, 11, 9, 9, 9, 13],
        Class::Sorcerer => [7, 15, 7, 9, 11, 7, 9],
    }
}

/// Fixed starting skills a class always begins with (2 per class in MM6).
pub fn class_starting_skills(class: Class) -> &'static [Skill] {
    use Skill::*;
    match class {
        Class::Knight => &[Sword, Leather],
        Class::Paladin => &[Sword, SpiritMagic],
        Class::Archer => &[Bow, AirMagic],
        Class::Cleric => &[Mace, BodyMagic],
        Class::Sorcerer => &[Dagger, FireMagic],
        Class::Druid => &[Staff, EarthMagic],
    }
}

/// Selectable skill pool during party creation (pick 2 additional from these).
pub fn class_available_skills(class: Class) -> &'static [Skill] {
    use Skill::*;
    match class {
        Class::Knight => &[
            Dagger,
            Bow,
            Bodybuilding,
            Axe,
            Shield,
            Perception,
            Spear,
            Chain,
            DisarmTrap,
        ],
        Class::Paladin => &[
            Dagger,
            Shield,
            Perception,
            Spear,
            Leather,
            Diplomacy,
            Mace,
            Chain,
            DisarmTrap,
        ],
        Class::Archer => &[
            Sword,
            Leather,
            Perception,
            Dagger,
            FireMagic,
            Diplomacy,
            Axe,
            IdentifyItem,
            DisarmTrap,
        ],
        Class::Cleric => &[
            Staff,
            SpiritMagic,
            RepairItem,
            Shield,
            MindMagic,
            Meditation,
            Leather,
            IdentifyItem,
            Diplomacy,
        ],
        Class::Sorcerer => &[
            Staff,
            WaterMagic,
            RepairItem,
            Leather,
            EarthMagic,
            Meditation,
            AirMagic,
            IdentifyItem,
            Diplomacy,
        ],
        Class::Druid => &[
            Mace,
            SpiritMagic,
            RepairItem,
            Leather,
            BodyMagic,
            Meditation,
            WaterMagic,
            IdentifyItem,
            Learning,
        ],
    }
}

pub fn random_unique_party_creation_seeds() -> [CharCreationSeed; 4] {
    let mut classes = ALL_CLASSES.to_vec();
    let mut portraits = PortraitId::ALL.to_vec();
    let mut male_names = MALE_NAMES.to_vec();
    let mut female_names = FEMALE_NAMES.to_vec();

    let mut rng = SplitMix64::seeded();
    shuffle(&mut classes, &mut rng);
    shuffle(&mut portraits, &mut rng);
    shuffle(&mut male_names, &mut rng);
    shuffle(&mut female_names, &mut rng);

    let mut male_idx = 0usize;
    let mut female_idx = 0usize;

    std::array::from_fn(|i| {
        let class = classes[i];
        let portrait = portraits[i];
        let name = if portrait.is_male() {
            let n = male_names[male_idx % male_names.len()];
            male_idx += 1;
            n
        } else {
            let n = female_names[female_idx % female_names.len()];
            female_idx += 1;
            n
        };
        CharCreationSeed {
            class,
            portrait,
            name,
            base_attrs: class_base_attrs(class),
        }
    })
}

// ── RNG ─────────────────────────────────────────────────────────────────────

pub(crate) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn seeded() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self {
            state: nanos ^ 0xA076_1D64_78BD_642F,
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Pick a random index in `0..len`.
    pub fn index(&mut self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        self.next_u64() as usize % len
    }
}

fn shuffle<T>(items: &mut [T], rng: &mut SplitMix64) {
    for i in (1..items.len()).rev() {
        let j = (rng.next_u64() as usize) % (i + 1);
        items.swap(i, j);
    }
}

/// Pick a random name from the male or female pool.
pub fn random_name(is_male: bool) -> &'static str {
    let mut rng = SplitMix64::seeded();
    let pool = if is_male { MALE_NAMES } else { FEMALE_NAMES };
    pool[rng.index(pool.len())]
}
