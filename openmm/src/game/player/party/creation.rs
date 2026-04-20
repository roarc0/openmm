use std::time::{SystemTime, UNIX_EPOCH};

use super::member::{ATTR_COUNT, CharacterClass};

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

const ALL_PORTRAITS: &[&str] = &[
    "icons/CCMALEA",
    "icons/CCMALEB",
    "icons/CCMALEC",
    "icons/CCMALED",
    "icons/CCMALEE",
    "icons/CCMALEF",
    "icons/CCMALEG",
    "icons/CCMALEH",
    "icons/CCGIRLA",
    "icons/CCGIRLB",
    "icons/CCGIRLC",
    "icons/CCGIRLD",
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
    pub portrait: &'static str,
    pub name: &'static str,
    pub base_attrs: [i16; ATTR_COUNT],
}

fn is_male_portrait(portrait: &str) -> bool {
    portrait.contains("CCMALE")
}

pub fn class_base_attrs(class: CharacterClass) -> [i16; ATTR_COUNT] {
    // [Might, Intellect, Personality, Endurance, Speed, Accuracy, Luck]
    match class {
        CharacterClass::Knight => [18, 10, 10, 16, 13, 15, 10],
        CharacterClass::Paladin => [16, 10, 13, 14, 13, 13, 10],
        CharacterClass::Archer => [13, 13, 10, 13, 14, 16, 10],
        CharacterClass::Cleric => [12, 10, 17, 13, 12, 12, 12],
        CharacterClass::Druid => [11, 15, 15, 12, 12, 11, 12],
        CharacterClass::Sorcerer => [10, 18, 13, 10, 13, 11, 10],
    }
}

pub fn random_unique_char_creation_seeds() -> [CharCreationSeed; 4] {
    let mut classes = ALL_CLASSES.to_vec();
    let mut portraits = ALL_PORTRAITS.to_vec();
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
        let name = if is_male_portrait(portrait) {
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

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn seeded() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self {
            state: nanos ^ 0xA076_1D64_78BD_642F,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
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
    pool[rng.next_u64() as usize % pool.len()]
}

/// Pick a random index into a slice of `len` elements.
pub fn random_name_index(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let mut rng = SplitMix64::seeded();
    rng.next_u64() as usize % len
}

/// Return the sound variants (without extension) that exist for a given portrait texture key.
/// Only variants confirmed present in Audio.snd are listed — missing variants are omitted.
pub fn portrait_sound_variants(portrait: &str) -> &'static [&'static str] {
    // portrait key is e.g. "icons/CCMALEA" — match the last segment case-insensitively
    let key = portrait.trim_start_matches("icons/").to_uppercase();
    match key.as_str() {
        "CCMALEA" => &["MaleA42a", "MaleA42c"],
        "CCMALEB" => &["MaleB42a", "MaleB42b"],
        "CCMALEC" => &["MaleC42b", "MaleC42c"],
        "CCMALED" => &["MaleD42a", "MaleD42c"],
        "CCMALEE" => &["MaleE42a", "MaleE42b"],
        "CCMALEF" => &["MaleF42a"],
        "CCMALEG" => &["MaleG42a", "MaleG42b"],
        "CCMALEH" => &["MaleH42a", "MaleH42b"],
        "CCGIRLA" => &["GirlA42a", "GirlA42c"],
        "CCGIRLB" => &["GirlB42a", "GirlB42c"],
        "CCGIRLC" => &["GirlC42b", "GirlC42c"],
        "CCGIRLD" => &["GirlD42a", "GirlD42c"],
        _ => &[],
    }
}
