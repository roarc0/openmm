use lod::enums::EvtVariable;

pub const SKILL_COUNT: usize = 31; // EvtVariable 0x38..=0x56

/// MM6 character classes (6 classes, matching the original game).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterClass {
    Knight,
    Paladin,
    Archer,
    Cleric,
    Sorcerer,
    Druid,
}

impl std::fmt::Display for CharacterClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Knight   => "Knight",
            Self::Paladin  => "Paladin",
            Self::Archer   => "Archer",
            Self::Cleric   => "Cleric",
            Self::Sorcerer => "Sorcerer",
            Self::Druid    => "Druid",
        };
        write!(f, "{}", s)
    }
}

/// A single party member. Skills are stored as raw levels (0 = untrained).
#[derive(Debug, Clone)]
pub struct PartyMember {
    pub name: &'static str,
    pub class: CharacterClass,
    pub level: u8,
    /// Raw skill levels, indexed by `EvtVariable::skill_index()`.
    pub skills: [u8; SKILL_COUNT],
}

impl PartyMember {
    pub fn new(name: &'static str, class: CharacterClass, level: u8) -> Self {
        Self {
            name,
            class,
            level,
            skills: [0; SKILL_COUNT],
        }
    }

    /// Set a skill by its EvtVariable. No-op if variable is not a skill.
    pub fn set_skill(&mut self, var: EvtVariable, level: u8) {
        if let Some(idx) = var.skill_index() {
            self.skills[idx as usize] = level;
        }
    }

    /// Get skill level for a given EvtVariable (0 if not a skill variable).
    pub fn get_skill(&self, var: EvtVariable) -> u8 {
        var.skill_index()
            .map(|idx| self.skills[idx as usize])
            .unwrap_or(0)
    }
}
