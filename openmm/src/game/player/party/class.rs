/// The six core class types in MM6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BaseClass {
    #[default]
    Knight,
    Paladin,
    Archer,
    Cleric,
    Sorcerer,
    Druid,
}

impl BaseClass {
    pub fn name(self) -> &'static str {
        match self {
            Self::Knight => "Knight",
            Self::Paladin => "Paladin",
            Self::Archer => "Archer",
            Self::Cleric => "Cleric",
            Self::Sorcerer => "Sorcerer",
            Self::Druid => "Druid",
        }
    }
}

/// MM6 character classes with their two promotion tiers (tier 0, 1, or 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Knight(u8),
    Paladin(u8),
    Archer(u8),
    Cleric(u8),
    Sorcerer(u8),
    Druid(u8),
}

impl Default for Class {
    fn default() -> Self {
        Self::Knight(0)
    }
}

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Self::Knight(0) => "Knight",
            Self::Knight(1) => "Cavalier",
            Self::Knight(2) => "Champion",
            Self::Paladin(0) => "Paladin",
            Self::Paladin(1) => "Crusader",
            Self::Paladin(2) => "Hero",
            Self::Archer(0) => "Archer",
            Self::Archer(1) => "Battle Mage",
            Self::Archer(2) => "Warrior Mage",
            Self::Cleric(0) => "Cleric",
            Self::Cleric(1) => "Priest",
            Self::Cleric(2) => "High Priest",
            Self::Sorcerer(0) => "Sorcerer",
            Self::Sorcerer(1) => "Wizard",
            Self::Sorcerer(2) => "Arch Mage",
            Self::Druid(0) => "Druid",
            Self::Druid(1) => "Great Druid",
            Self::Druid(2) => "Arch Druid",
            _ => "Unknown",
        }
    }

    pub fn icon(self) -> &'static str {
        match self.base_class() {
            BaseClass::Knight => "icons/IC_KNIG",
            BaseClass::Paladin => "icons/IC_PALAD",
            BaseClass::Archer => "icons/IC_ARCH",
            BaseClass::Cleric => "icons/IC_CLER",
            BaseClass::Druid => "icons/IC_DRUID",
            BaseClass::Sorcerer => "icons/IC_SORC",
        }
    }

    pub fn base_class(self) -> BaseClass {
        match self {
            Self::Knight(_) => BaseClass::Knight,
            Self::Paladin(_) => BaseClass::Paladin,
            Self::Archer(_) => BaseClass::Archer,
            Self::Cleric(_) => BaseClass::Cleric,
            Self::Sorcerer(_) => BaseClass::Sorcerer,
            Self::Druid(_) => BaseClass::Druid,
        }
    }

    pub fn tier(self) -> u8 {
        match self {
            Self::Knight(t) | Self::Paladin(t) | Self::Archer(t) | Self::Cleric(t) | Self::Sorcerer(t) | Self::Druid(t) => t,
        }
    }

    pub fn id(self) -> u8 {
        let base_offset = match self.base_class() {
            BaseClass::Knight => 0,
            BaseClass::Paladin => 3,
            BaseClass::Archer => 6,
            BaseClass::Cleric => 9,
            BaseClass::Sorcerer => 12,
            BaseClass::Druid => 15,
        };
        base_offset + self.tier().min(2)
    }

    pub fn from_id(id: u8) -> Option<Self> {
        let base_idx = id / 3;
        let tier = id % 3;
        match base_idx {
            0 => Some(Self::Knight(tier)),
            1 => Some(Self::Paladin(tier)),
            2 => Some(Self::Archer(tier)),
            3 => Some(Self::Cleric(tier)),
            4 => Some(Self::Sorcerer(tier)),
            5 => Some(Self::Druid(tier)),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Knight" => Some(Self::Knight(0)),
            "Cavalier" => Some(Self::Knight(1)),
            "Champion" => Some(Self::Knight(2)),
            "Paladin" => Some(Self::Paladin(0)),
            "Crusader" => Some(Self::Paladin(1)),
            "Hero" => Some(Self::Paladin(2)),
            "Archer" => Some(Self::Archer(0)),
            "Battle Mage" => Some(Self::Archer(1)),
            "Warrior Mage" => Some(Self::Archer(2)),
            "Cleric" => Some(Self::Cleric(0)),
            "Priest" => Some(Self::Cleric(1)),
            "High Priest" => Some(Self::Cleric(2)),
            "Sorcerer" => Some(Self::Sorcerer(0)),
            "Wizard" => Some(Self::Sorcerer(1)),
            "Arch Mage" => Some(Self::Sorcerer(2)),
            "Druid" => Some(Self::Druid(0)),
            "Great Druid" => Some(Self::Druid(1)),
            "Arch Druid" => Some(Self::Druid(2)),
            _ => None,
        }
    }
}

impl std::fmt::Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
