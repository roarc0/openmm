use std::time::{SystemTime, UNIX_EPOCH};

use super::member::{ATTR_COUNT, CharacterClass};
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

const ALL_CLASSES: &[CharacterClass] = &[
    CharacterClass::Knight,
    CharacterClass::Paladin,
    CharacterClass::Archer,
    CharacterClass::Cleric,
    CharacterClass::Druid,
    CharacterClass::Sorcerer,
];

#[derive(Debug, Clone, Copy)]
pub struct CharCreationSeed {
    pub class: CharacterClass,
    pub portrait: PortraitId,
    pub name: &'static str,
    pub base_attrs: [i16; ATTR_COUNT],
}

pub fn class_base_attrs(class: CharacterClass) -> [i16; ATTR_COUNT] {
    // [Might, Intellect, Personality, Endurance, Speed, Accuracy, Luck]
    match class {
        CharacterClass::Knight => [14, 7, 7, 14, 11, 11, 9],
        CharacterClass::Paladin => [14, 7, 14, 11, 11, 9, 7],
        CharacterClass::Archer => [9, 14, 7, 11, 14, 11, 7],
        CharacterClass::Cleric => [7, 9, 14, 11, 11, 7, 14],
        CharacterClass::Druid => [11, 15, 15, 12, 12, 11, 12],
        CharacterClass::Sorcerer => [7, 14, 14, 11, 7, 11, 9],
    }
}

/// Fixed starting skills a class always begins with (2 per class in MM6).
pub fn class_starting_skills(class: CharacterClass) -> &'static [&'static str] {
    match class {
        CharacterClass::Knight => &["Sword", "Leather"],
        CharacterClass::Paladin => &["Sword", "Spirit Magic"],
        CharacterClass::Archer => &["Bow", "Air Magic"],
        CharacterClass::Cleric => &["Mace", "Body Magic"],
        CharacterClass::Sorcerer => &["Dagger", "Fire Magic"],
        CharacterClass::Druid => &["Staff", "Earth Magic"],
    }
}

/// Selectable skill pool during party creation (pick 2 additional from these).
pub fn class_available_skills(class: CharacterClass) -> &'static [&'static str] {
    match class {
        CharacterClass::Knight => &[
            "Dagger",
            "Bow",
            "Body",
            "Axe",
            "Shield",
            "Perception",
            "Spear",
            "Chain",
            "Disarm",
        ],
        CharacterClass::Paladin => &[
            "Dagger",
            "Shield",
            "Perception",
            "Spear",
            "Leather",
            "Diplomacy",
            "Mace",
            "Chain",
            "Disarm",
        ],
        CharacterClass::Archer => &[
            "Sword",
            "Dagger",
            "Axe",
            "Leather",
            "Fire Magic",
            "Identify Item",
            "Perception",
            "Diplomacy",
            "Disarm Trap",
        ],
        CharacterClass::Cleric => &[
            "Staff",
            "Shield",
            "Leather",
            "Spirit Magic",
            "Mind Magic",
            "Identify Item",
            "Repair",
            "Meditation",
            "Diplomacy",
        ],
        CharacterClass::Sorcerer => &[
            "Dagger",
            "Leather",
            "Water Magic",
            "Earth Magic",
            "Air Magic",
            "Identify Item",
            "Repair",
            "Meditation",
            "Learning",
        ],
        CharacterClass::Druid => &[
            "Dagger",
            "Staff",
            "Leather",
            "Water Magic",
            "Earth Magic",
            "Air Magic",
            "Spirit Magic",
            "Mind Magic",
            "Body Magic",
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
