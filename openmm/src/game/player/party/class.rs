/// MM6 character classes (6 classes, matching the original game).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Class {
    #[default]
    Knight,
    Paladin,
    Archer,
    Cleric,
    Sorcerer,
    Druid,
}

impl Class {
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

    pub fn icon(self) -> &'static str {
        match self {
            Self::Knight => "icons/IC_KNIG",
            Self::Paladin => "icons/IC_PALAD",
            Self::Archer => "icons/IC_ARCH",
            Self::Cleric => "icons/IC_CLER",
            Self::Druid => "icons/IC_DRUID",
            Self::Sorcerer => "icons/IC_SORC",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Knight" => Some(Self::Knight),
            "Paladin" => Some(Self::Paladin),
            "Archer" => Some(Self::Archer),
            "Cleric" => Some(Self::Cleric),
            "Druid" => Some(Self::Druid),
            "Sorcerer" => Some(Self::Sorcerer),
            _ => None,
        }
    }
}

impl std::fmt::Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}
