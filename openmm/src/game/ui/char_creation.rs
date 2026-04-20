//! Character creation state — tracks portrait, class, and selected stat per party member.
//!
//! Exposes four objects via [`PropertySource`]: `char0`, `char1`, `char2`, `char3`.
//! Each resolves:
//!   - `portrait`       → texture key, e.g. `"icons/CCMALEA"` (male A–H, female A–D)
//!   - `class`          → class name, e.g. `"Knight"`
//!   - `selected_stat`  → stat name, e.g. `"might"`
//!   - `gender`         → `"male"` or `"female"`
//!   - `class_icon`     → class icon texture key, e.g. `"icons/IC_PALAD"`

use bevy::prelude::*;

use crate::game::player::party::creation;
use crate::game::player::party::member::CharacterClass;
use crate::screens::PropertySource;
use crate::screens::runtime::RuntimeElement;

/// Portrait variants — male has A–H (8), female A–D (4).
const MALE_PORTRAITS: &[&str] = &[
    "icons/CCMALEA",
    "icons/CCMALEB",
    "icons/CCMALEC",
    "icons/CCMALED",
    "icons/CCMALEE",
    "icons/CCMALEF",
    "icons/CCMALEG",
    "icons/CCMALEH",
];
const FEMALE_PORTRAITS: &[&str] = &["icons/CCGIRLA", "icons/CCGIRLB", "icons/CCGIRLC", "icons/CCGIRLD"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharGender {
    #[default]
    Male,
    Female,
}

/// All selectable classes in MM6 character creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharClass {
    #[default]
    Knight,
    Paladin,
    Archer,
    Cleric,
    Druid,
    Sorcerer,
}

impl CharClass {
    pub fn name(self) -> &'static str {
        match self {
            CharClass::Knight => "Knight",
            CharClass::Paladin => "Paladin",
            CharClass::Archer => "Archer",
            CharClass::Cleric => "Cleric",
            CharClass::Druid => "Druid",
            CharClass::Sorcerer => "Sorcerer",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            CharClass::Knight => "icons/IC_KNIG",
            CharClass::Paladin => "icons/IC_PALAD",
            CharClass::Archer => "icons/IC_ARCH",
            CharClass::Cleric => "icons/IC_CLER",
            CharClass::Druid => "icons/IC_DRUID",
            CharClass::Sorcerer => "icons/IC_SORC",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Knight" => Some(CharClass::Knight),
            "Paladin" => Some(CharClass::Paladin),
            "Archer" => Some(CharClass::Archer),
            "Cleric" => Some(CharClass::Cleric),
            "Druid" => Some(CharClass::Druid),
            "Sorcerer" => Some(CharClass::Sorcerer),
            _ => None,
        }
    }

    pub fn from_party_class(class: CharacterClass) -> Self {
        match class {
            CharacterClass::Knight => CharClass::Knight,
            CharacterClass::Paladin => CharClass::Paladin,
            CharacterClass::Archer => CharClass::Archer,
            CharacterClass::Cleric => CharClass::Cleric,
            CharacterClass::Druid => CharClass::Druid,
            CharacterClass::Sorcerer => CharClass::Sorcerer,
        }
    }

    pub fn to_party_class(self) -> CharacterClass {
        match self {
            CharClass::Knight => CharacterClass::Knight,
            CharClass::Paladin => CharacterClass::Paladin,
            CharClass::Archer => CharacterClass::Archer,
            CharClass::Cleric => CharacterClass::Cleric,
            CharClass::Druid => CharacterClass::Druid,
            CharClass::Sorcerer => CharacterClass::Sorcerer,
        }
    }
}

/// All selectable primary stats for distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CharStat {
    #[default]
    Might,
    Intellect,
    Personality,
    Endurance,
    Accuracy,
    Speed,
    Luck,
}

impl CharStat {
    pub fn name(self) -> &'static str {
        match self {
            CharStat::Might => "might",
            CharStat::Intellect => "intellect",
            CharStat::Personality => "personality",
            CharStat::Endurance => "endurance",
            CharStat::Accuracy => "accuracy",
            CharStat::Speed => "speed",
            CharStat::Luck => "luck",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "might" => Some(CharStat::Might),
            "intellect" => Some(CharStat::Intellect),
            "personality" => Some(CharStat::Personality),
            "endurance" => Some(CharStat::Endurance),
            "accuracy" => Some(CharStat::Accuracy),
            "speed" => Some(CharStat::Speed),
            "luck" => Some(CharStat::Luck),
            _ => None,
        }
    }
}

/// Per-member character creation state.
#[derive(Debug, Clone, Default)]
pub struct CharMember {
    pub gender: CharGender,
    pub portrait_index: usize,
    pub class: CharClass,
    pub selected_stat: CharStat,
    pub base_attrs: [i16; 7],
    pub name: String,
}

impl CharMember {
    pub fn set_class_with_defaults(&mut self, class: CharClass) {
        self.class = class;
        self.base_attrs = creation::class_base_attrs(class.to_party_class());
    }

    fn selected_stat_color(&self, stat: CharStat) -> &'static str {
        if self.selected_stat == stat { "green" } else { "white" }
    }

    pub fn set_portrait_from_texture(&mut self, portrait: &str) {
        if let Some(i) = MALE_PORTRAITS.iter().position(|p| *p == portrait) {
            self.gender = CharGender::Male;
            self.portrait_index = i;
            return;
        }
        if let Some(i) = FEMALE_PORTRAITS.iter().position(|p| *p == portrait) {
            self.gender = CharGender::Female;
            self.portrait_index = i;
        }
    }

    fn selected_class_color(&self, class: CharClass) -> &'static str {
        if self.class == class { "cyan" } else { "white" }
    }

    pub fn portrait_texture(&self) -> &'static str {
        match self.gender {
            CharGender::Male => MALE_PORTRAITS[self.portrait_index % MALE_PORTRAITS.len()],
            CharGender::Female => FEMALE_PORTRAITS[self.portrait_index % FEMALE_PORTRAITS.len()],
        }
    }

    /// Advance through all portraits (male A–H then female A–D) as one flat sequence.
    /// Gender updates automatically when crossing the boundary; name regenerates to match.
    pub fn cycle_portrait(&mut self, delta: i32) {
        const TOTAL: usize = MALE_PORTRAITS.len() + FEMALE_PORTRAITS.len();
        let flat = match self.gender {
            CharGender::Male => self.portrait_index,
            CharGender::Female => MALE_PORTRAITS.len() + self.portrait_index,
        };
        let next = (flat as i32 + delta).rem_euclid(TOTAL as i32) as usize;
        if next < MALE_PORTRAITS.len() {
            self.gender = CharGender::Male;
            self.portrait_index = next;
        } else {
            self.gender = CharGender::Female;
            self.portrait_index = next - MALE_PORTRAITS.len();
        }
        self.name = creation::random_name(self.gender == CharGender::Male).to_string();
    }

    fn resolve(&self, path: &str) -> Option<String> {
        match path {
            "portrait" => Some(self.portrait_texture().to_string()),
            "class" => Some(self.class.name().to_string()),
            "class_icon" => Some(self.class.icon().to_string()),
            "class_color_knight" => Some(self.selected_class_color(CharClass::Knight).to_string()),
            "class_color_paladin" => Some(self.selected_class_color(CharClass::Paladin).to_string()),
            "class_color_archer" => Some(self.selected_class_color(CharClass::Archer).to_string()),
            "class_color_cleric" => Some(self.selected_class_color(CharClass::Cleric).to_string()),
            "class_color_druid" => Some(self.selected_class_color(CharClass::Druid).to_string()),
            "class_color_sorcerer" => Some(self.selected_class_color(CharClass::Sorcerer).to_string()),
            "gender" => Some(
                match self.gender {
                    CharGender::Male => "male",
                    CharGender::Female => "female",
                }
                .to_string(),
            ),
            "selected_stat" => Some(self.selected_stat.name().to_string()),
            "stat_color_might" => Some(self.selected_stat_color(CharStat::Might).to_string()),
            "stat_color_intellect" => Some(self.selected_stat_color(CharStat::Intellect).to_string()),
            "stat_color_personality" => Some(self.selected_stat_color(CharStat::Personality).to_string()),
            "stat_color_endurance" => Some(self.selected_stat_color(CharStat::Endurance).to_string()),
            "stat_color_accuracy" => Some(self.selected_stat_color(CharStat::Accuracy).to_string()),
            "stat_color_speed" => Some(self.selected_stat_color(CharStat::Speed).to_string()),
            "stat_color_luck" => Some(self.selected_stat_color(CharStat::Luck).to_string()),
            "name" => Some(self.name.clone()),
            "might" => Some(self.base_attrs[0].to_string()),
            "intellect" => Some(self.base_attrs[1].to_string()),
            "personality" => Some(self.base_attrs[2].to_string()),
            "endurance" => Some(self.base_attrs[3].to_string()),
            "speed" => Some(self.base_attrs[4].to_string()),
            "accuracy" => Some(self.base_attrs[5].to_string()),
            "luck" => Some(self.base_attrs[6].to_string()),
            _ => None,
        }
    }
}

/// Runtime state for the character creation screen.
#[derive(Resource)]
pub struct CharCreationState {
    pub members: [CharMember; 4],
    /// Which member is currently focused in the editor (0–3).
    pub active_member: usize,
}

impl Default for CharCreationState {
    fn default() -> Self {
        let seeds = creation::random_unique_char_creation_seeds();
        let members = std::array::from_fn(|i| {
            let mut member = CharMember::default();
            member.set_class_with_defaults(CharClass::from_party_class(seeds[i].class));
            member.set_portrait_from_texture(seeds[i].portrait);
            member.base_attrs = seeds[i].base_attrs;
            member.name = seeds[i].name.to_string();
            member
        });

        Self {
            members,
            active_member: 0,
        }
    }
}

/// One `PropertySource` per member so RON can use `"char0.portrait"`, `"char1.class"`, etc.
struct CharMemberSource {
    index: usize,
    member: CharMember,
}

impl PropertySource for CharMemberSource {
    fn source_name(&self) -> &str {
        // &'static str required — use a fixed table
        match self.index {
            0 => "char0",
            1 => "char1",
            2 => "char2",
            _ => "char3",
        }
    }

    fn resolve(&self, path: &str) -> Option<String> {
        self.member.resolve(path)
    }
}

/// System: push all four `CharMemberSource` entries into `PropertyRegistry` each frame.
pub fn update_char_creation_registry(
    state: Res<CharCreationState>,
    mut registry: ResMut<crate::screens::PropertyRegistry>,
) {
    for (i, member) in state.members.iter().enumerate() {
        registry.register(Box::new(CharMemberSource {
            index: i,
            member: member.clone(),
        }));
    }
}

/// Move the stat selector arrows to the selected stat row for member 0.
/// This mirrors the classic MM6 layout in `makeme.ron`.
pub fn sync_char_creation_arrows(state: Res<CharCreationState>, mut query: Query<(&RuntimeElement, &mut Node)>) {
    let y = match state.members[0].selected_stat {
        CharStat::Might => 154.0,
        CharStat::Intellect => 171.0,
        CharStat::Personality => 188.0,
        CharStat::Endurance => 205.0,
        CharStat::Accuracy => 222.0,
        CharStat::Speed => 239.0,
        CharStat::Luck => 256.0,
    };

    for (elem, mut node) in &mut query {
        if elem.element_id == "icons/ARROWL0" || elem.element_id == "icons/ARROWR0" {
            node.top = Val::Percent(y / crate::screens::REF_H * 100.0);
        }
    }
}
