/// All selectable primary attributes for point distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Attribute {
    #[default]
    Might,
    Intellect,
    Personality,
    Endurance,
    Accuracy,
    Speed,
    Luck,
}

impl Attribute {
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
