use openmm_data::enums::EvtVariable;

pub const SKILL_COUNT: usize = 31; // EvtVariable 0x38..=0x56

pub use super::class::Class;
/// Index mapping for base/cur attribute arrays: [Might, Intellect, Personality, Endurance, Speed, Accuracy, Luck]
pub const ATTR_COUNT: usize = 7;
/// Index mapping for resistance arrays: [Fire, Elec, Cold, Poison, Magic]
pub const RESIST_COUNT: usize = 5;
/// Number of condition bits (CondCursed=0 .. CondMain=17), matching EvtVariable 0x57..=0x68.
pub const COND_COUNT: usize = 18;

/// All selectable primary stats for point distribution.
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
            Self::Might => "might",
            Self::Intellect => "intellect",
            Self::Personality => "personality",
            Self::Endurance => "endurance",
            Self::Accuracy => "accuracy",
            Self::Speed => "speed",
            Self::Luck => "luck",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "might" => Some(Self::Might),
            "intellect" => Some(Self::Intellect),
            "personality" => Some(Self::Personality),
            "endurance" => Some(Self::Endurance),
            "accuracy" => Some(Self::Accuracy),
            "speed" => Some(Self::Speed),
            "luck" => Some(Self::Luck),
            _ => None,
        }
    }

    /// Index into PartyMember.base_attrs array.
    pub fn attr_index(self) -> usize {
        match self {
            Self::Might => 0,
            Self::Intellect => 1,
            Self::Personality => 2,
            Self::Endurance => 3,
            Self::Speed => 4,
            Self::Accuracy => 5,
            Self::Luck => 6,
        }
    }
}

pub use super::skills::Skill;

/// A single party member. Skills are stored as raw levels (0 = untrained).
#[derive(Debug, Clone)]
pub struct PartyMember {
    pub name: String,
    pub class: Class,
    pub portrait: super::portrait::PortraitId,
    pub level: u8,
    /// Raw skill levels, indexed by `EvtVariable::skill_index()`.
    pub skills: [u8; SKILL_COUNT],

    // ── Combat stats ────────────────────────────────────────────────────
    pub hp: i16,
    pub max_hp: i16,
    pub sp: i16,
    pub max_sp: i16,
    pub ac_bonus: i16,
    pub level_bonus: u8,
    pub age_bonus: i16,

    // ── Attributes ──────────────────────────────────────────────────────
    /// Base (permanent) attributes: [Might, Intellect, Personality, Endurance, Speed, Accuracy, Luck].
    pub base_attrs: [i16; ATTR_COUNT],
    /// Temporary bonus attributes (same index order as base_attrs).
    pub attr_bonuses: [i16; ATTR_COUNT],

    // ── Resistances ─────────────────────────────────────────────────────
    /// Base resistances: [Fire, Elec, Cold, Poison, Magic].
    pub resistances: [i16; RESIST_COUNT],
    /// Temporary resistance bonuses (same order as resistances).
    pub resistance_bonuses: [i16; RESIST_COUNT],

    // ── Progress ────────────────────────────────────────────────────────
    pub experience: i64,
    pub awards: i32,
    pub skill_points: i32,

    // ── Conditions ──────────────────────────────────────────────────────
    /// Bitmask of active conditions.
    /// Bit N corresponds to EvtVariable(0x57 + N): bit 0=Cursed, 1=Weak, …, 17=CondMain.
    pub conditions: u32,
}

impl PartyMember {
    pub fn new(
        name: impl Into<String>,
        class: Class,
        portrait: super::portrait::PortraitId,
        level: u8,
    ) -> Self {
        Self {
            name: name.into(),
            class,
            portrait,
            level,
            skills: [0; SKILL_COUNT],
            hp: 50,
            max_hp: 50,
            sp: 20,
            max_sp: 20,
            ac_bonus: 0,
            level_bonus: 0,
            age_bonus: 0,
            base_attrs: [15; ATTR_COUNT],
            attr_bonuses: [0; ATTR_COUNT],
            resistances: [0; RESIST_COUNT],
            resistance_bonuses: [0; RESIST_COUNT],
            experience: 0,
            awards: 0,
            skill_points: 0,
            conditions: 0,
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
        var.skill_index().map(|idx| self.skills[idx as usize]).unwrap_or(0)
    }

    /// Read a per-character EvtVariable value (0 if unrecognised).
    pub fn get_var(&self, var: EvtVariable) -> i32 {
        match var.0 {
            0x01 => {
                if self.portrait.is_male() {
                    0
                } else {
                    1
                }
            }
            0x02 => self.class as i32,
            0x03 => self.hp as i32,
            0x04 => (self.hp >= self.max_hp) as i32,
            0x05 => self.sp as i32,
            0x06 => (self.sp >= self.max_sp) as i32,
            0x08 => self.ac_bonus as i32,
            0x09 => self.level as i32,
            0x0A => self.level_bonus as i32,
            0x0B => self.age_bonus as i32,
            0x0C => self.awards,
            0x0D => self.experience as i32,
            0xE1 => self.skill_points,
            // Attr bonuses: 0x19..=0x1F → index 0..6
            0x19..=0x1F => self.attr_bonuses[(var.0 - 0x19) as usize] as i32,
            // Base attrs: 0x20..=0x26 → index 0..6
            0x20..=0x26 => self.base_attrs[(var.0 - 0x20) as usize] as i32,
            // Cur attrs (base + bonus): 0x27..=0x2D → index 0..6
            0x27..=0x2D => {
                let idx = (var.0 - 0x27) as usize;
                (self.base_attrs[idx] + self.attr_bonuses[idx]) as i32
            }
            // Resistances: 0x2E..=0x32 → index 0..4
            0x2E..=0x32 => self.resistances[(var.0 - 0x2E) as usize] as i32,
            // Resistance bonuses: 0x33..=0x37 → index 0..4
            0x33..=0x37 => self.resistance_bonuses[(var.0 - 0x33) as usize] as i32,
            // Skills: 0x38..=0x56
            0x38..=0x56 => self.get_skill(var) as i32,
            // Conditions: 0x57..=0x68
            0x57..=0x68 => ((self.conditions >> (var.0 - 0x57)) & 1) as i32,
            _ => 0,
        }
    }

    /// Write a per-character EvtVariable value.
    pub fn set_var(&mut self, var: EvtVariable, value: i32) {
        match var.0 {
            0x03 => self.hp = value as i16,
            0x05 => self.sp = value as i16,
            0x08 => self.ac_bonus = value as i16,
            0x09 => self.level = value as u8,
            0x0A => self.level_bonus = value as u8,
            0x0B => self.age_bonus = value as i16,
            0x0C => self.awards = value,
            0x0D => self.experience = value as i64,
            0xE1 => self.skill_points = value,
            0x19..=0x1F => self.attr_bonuses[(var.0 - 0x19) as usize] = value as i16,
            0x20..=0x26 => self.base_attrs[(var.0 - 0x20) as usize] = value as i16,
            // cur attrs — write to base (no separate cur storage)
            0x27..=0x2D => self.base_attrs[(var.0 - 0x27) as usize] = value as i16,
            0x2E..=0x32 => self.resistances[(var.0 - 0x2E) as usize] = value as i16,
            0x33..=0x37 => self.resistance_bonuses[(var.0 - 0x33) as usize] = value as i16,
            0x38..=0x56 => self.set_skill(var, value as u8),
            // Conditions: set bit
            0x57..=0x68 => {
                let bit = var.0 - 0x57;
                if value != 0 {
                    self.conditions |= 1 << bit;
                } else {
                    self.conditions &= !(1 << bit);
                }
            }
            _ => {}
        }
    }

    /// Add delta to a per-character EvtVariable.
    pub fn add_var(&mut self, var: EvtVariable, delta: i32) {
        match var.0 {
            0x03 => self.hp = self.hp.saturating_add(delta as i16),
            0x05 => self.sp = self.sp.saturating_add(delta as i16),
            0x09 => self.level = self.level.saturating_add(delta as u8),
            0x0C => self.awards = self.awards.wrapping_add(delta),
            0x0D => self.experience = self.experience.wrapping_add(delta as i64),
            0xE1 => self.skill_points = self.skill_points.wrapping_add(delta),
            0x19..=0x1F => self.attr_bonuses[(var.0 - 0x19) as usize] += delta as i16,
            0x20..=0x26 => self.base_attrs[(var.0 - 0x20) as usize] += delta as i16,
            0x27..=0x2D => self.base_attrs[(var.0 - 0x27) as usize] += delta as i16,
            0x2E..=0x32 => self.resistances[(var.0 - 0x2E) as usize] += delta as i16,
            0x33..=0x37 => self.resistance_bonuses[(var.0 - 0x33) as usize] += delta as i16,
            0x38..=0x56 => {
                if let Some(idx) = var.skill_index() {
                    self.skills[idx as usize] = self.skills[idx as usize].saturating_add(delta as u8);
                }
            }
            _ => {}
        }
    }
}
