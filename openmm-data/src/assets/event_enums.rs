//! EVT script opcodes, variable IDs, and target character selectors.

// ── EVT Opcodes ──────────────────────────────────────────────────────────

/// EVT script opcodes (from OpenEnroth EvtEnums.h).
/// Each opcode is a single u8 in the binary .evt file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EvtOpcode {
    Invalid = 0,
    Exit = 1,
    SpeakInHouse = 2,
    PlaySound = 3,
    MouseOver = 4,
    LocationName = 5,
    MoveToMap = 6,
    OpenChest = 7,
    ShowFace = 8,
    ReceiveDamage = 9,
    SetSnow = 10,
    SetTexture = 11,
    /// In MM6: SetTextureOutdoors(model u32, facet u32, name). In MM7+: ShowMovie.
    SetTextureOutdoors = 12,
    SetSprite = 13,
    Compare = 14,
    ChangeDoorState = 15,
    Add = 16,
    Subtract = 17,
    Set = 18,
    SummonMonsters = 19,
    /// Purpose unknown. Observed in some EVT files; params preserved in Unhandled.
    Unknown20 = 20,
    CastSpell = 21,
    SpeakNPC = 22,
    SetFacesBit = 23,
    ToggleActorFlag = 24,
    RandomGoTo = 25,
    InputString = 26,
    /// Purpose unknown. Observed in some EVT files; params preserved in Unhandled.
    Unknown27 = 27,
    /// Purpose unknown. Observed in some EVT files; params preserved in Unhandled.
    Unknown28 = 28,
    StatusText = 29,
    ShowMessage = 30,
    OnTimer = 31,
    ToggleIndoorLight = 32,
    PressAnyKey = 33,
    SummonItem = 34,
    ForPartyMember = 35,
    Jmp = 36,
    OnMapReload = 37,
    OnLongTimer = 38,
    SetNPCTopic = 39,
    MoveNPC = 40,
    GiveItem = 41,
    ChangeEvent = 42,
    CheckSkill = 43,
    OnCanShowDialogItemCmp = 44,
    EndCanShowDialogItem = 45,
    SetCanShowDialogItem = 46,
    SetNPCGroupNews = 47,
    SetActorGroup = 48,
    NPCSetItem = 49,
    SetNPCGreeting = 50,
    IsActorKilled = 51,
    CanShowTopicIsActorKilled = 52,
    OnMapLeave = 53,
    ChangeGroup = 54,
    ChangeGroupAlly = 55,
    CheckSeason = 56,
    ToggleActorGroupFlag = 57,
    ToggleChestFlag = 58,
    CharacterAnimation = 59,
    SetActorItem = 60,
    OnDateTimer = 61,
    EnableDateTimer = 62,
    StopAnimation = 63,
    CheckItemsCount = 64,
    RemoveItems = 65,
    SpecialJump = 66,
    IsTotalBountyHuntingAwardInRange = 67,
    IsNPCInParty = 68,
}

impl EvtOpcode {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Invalid),
            1 => Some(Self::Exit),
            2 => Some(Self::SpeakInHouse),
            3 => Some(Self::PlaySound),
            4 => Some(Self::MouseOver),
            5 => Some(Self::LocationName),
            6 => Some(Self::MoveToMap),
            7 => Some(Self::OpenChest),
            8 => Some(Self::ShowFace),
            9 => Some(Self::ReceiveDamage),
            10 => Some(Self::SetSnow),
            11 => Some(Self::SetTexture),
            12 => Some(Self::SetTextureOutdoors),
            13 => Some(Self::SetSprite),
            14 => Some(Self::Compare),
            15 => Some(Self::ChangeDoorState),
            16 => Some(Self::Add),
            17 => Some(Self::Subtract),
            18 => Some(Self::Set),
            19 => Some(Self::SummonMonsters),
            20 => Some(Self::Unknown20),
            21 => Some(Self::CastSpell),
            22 => Some(Self::SpeakNPC),
            23 => Some(Self::SetFacesBit),
            24 => Some(Self::ToggleActorFlag),
            25 => Some(Self::RandomGoTo),
            26 => Some(Self::InputString),
            27 => Some(Self::Unknown27),
            28 => Some(Self::Unknown28),
            29 => Some(Self::StatusText),
            30 => Some(Self::ShowMessage),
            31 => Some(Self::OnTimer),
            32 => Some(Self::ToggleIndoorLight),
            33 => Some(Self::PressAnyKey),
            34 => Some(Self::SummonItem),
            35 => Some(Self::ForPartyMember),
            36 => Some(Self::Jmp),
            37 => Some(Self::OnMapReload),
            38 => Some(Self::OnLongTimer),
            39 => Some(Self::SetNPCTopic),
            40 => Some(Self::MoveNPC),
            41 => Some(Self::GiveItem),
            42 => Some(Self::ChangeEvent),
            43 => Some(Self::CheckSkill),
            44 => Some(Self::OnCanShowDialogItemCmp),
            45 => Some(Self::EndCanShowDialogItem),
            46 => Some(Self::SetCanShowDialogItem),
            47 => Some(Self::SetNPCGroupNews),
            48 => Some(Self::SetActorGroup),
            49 => Some(Self::NPCSetItem),
            50 => Some(Self::SetNPCGreeting),
            51 => Some(Self::IsActorKilled),
            52 => Some(Self::CanShowTopicIsActorKilled),
            53 => Some(Self::OnMapLeave),
            54 => Some(Self::ChangeGroup),
            55 => Some(Self::ChangeGroupAlly),
            56 => Some(Self::CheckSeason),
            57 => Some(Self::ToggleActorGroupFlag),
            58 => Some(Self::ToggleChestFlag),
            59 => Some(Self::CharacterAnimation),
            60 => Some(Self::SetActorItem),
            61 => Some(Self::OnDateTimer),
            62 => Some(Self::EnableDateTimer),
            63 => Some(Self::StopAnimation),
            64 => Some(Self::CheckItemsCount),
            65 => Some(Self::RemoveItems),
            66 => Some(Self::SpecialJump),
            67 => Some(Self::IsTotalBountyHuntingAwardInRange),
            68 => Some(Self::IsNPCInParty),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Exit => "Exit",
            Self::SpeakInHouse => "SpeakInHouse",
            Self::PlaySound => "PlaySound",
            Self::MouseOver => "MouseOver",
            Self::LocationName => "LocationName",
            Self::MoveToMap => "MoveToMap",
            Self::OpenChest => "OpenChest",
            Self::ShowFace => "ShowFace",
            Self::ReceiveDamage => "ReceiveDamage",
            Self::SetSnow => "SetSnow",
            Self::SetTexture => "SetTexture",
            Self::SetTextureOutdoors => "SetTextureOutdoors",
            Self::SetSprite => "SetSprite",
            Self::Compare => "Compare",
            Self::ChangeDoorState => "ChangeDoorState",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Set => "Set",
            Self::SummonMonsters => "SummonMonsters",
            Self::Unknown20 => "Unknown20",
            Self::CastSpell => "CastSpell",
            Self::SpeakNPC => "SpeakNPC",
            Self::SetFacesBit => "SetFacesBit",
            Self::ToggleActorFlag => "ToggleActorFlag",
            Self::RandomGoTo => "RandomGoTo",
            Self::InputString => "InputString",
            Self::Unknown27 => "Unknown27",
            Self::Unknown28 => "Unknown28",
            Self::StatusText => "StatusText",
            Self::ShowMessage => "ShowMessage",
            Self::OnTimer => "OnTimer",
            Self::ToggleIndoorLight => "ToggleIndoorLight",
            Self::PressAnyKey => "PressAnyKey",
            Self::SummonItem => "SummonItem",
            Self::ForPartyMember => "ForPartyMember",
            Self::Jmp => "Jmp",
            Self::OnMapReload => "OnMapReload",
            Self::OnLongTimer => "OnLongTimer",
            Self::SetNPCTopic => "SetNPCTopic",
            Self::MoveNPC => "MoveNPC",
            Self::GiveItem => "GiveItem",
            Self::ChangeEvent => "ChangeEvent",
            Self::CheckSkill => "CheckSkill",
            Self::OnCanShowDialogItemCmp => "OnCanShowDialogItemCmp",
            Self::EndCanShowDialogItem => "EndCanShowDialogItem",
            Self::SetCanShowDialogItem => "SetCanShowDialogItem",
            Self::SetNPCGroupNews => "SetNPCGroupNews",
            Self::SetActorGroup => "SetActorGroup",
            Self::NPCSetItem => "NPCSetItem",
            Self::SetNPCGreeting => "SetNPCGreeting",
            Self::IsActorKilled => "IsActorKilled",
            Self::CanShowTopicIsActorKilled => "CanShowTopicIsActorKilled",
            Self::OnMapLeave => "OnMapLeave",
            Self::ChangeGroup => "ChangeGroup",
            Self::ChangeGroupAlly => "ChangeGroupAlly",
            Self::CheckSeason => "CheckSeason",
            Self::ToggleActorGroupFlag => "ToggleActorGroupFlag",
            Self::ToggleChestFlag => "ToggleChestFlag",
            Self::CharacterAnimation => "CharacterAnimation",
            Self::SetActorItem => "SetActorItem",
            Self::OnDateTimer => "OnDateTimer",
            Self::EnableDateTimer => "EnableDateTimer",
            Self::StopAnimation => "StopAnimation",
            Self::CheckItemsCount => "CheckItemsCount",
            Self::RemoveItems => "RemoveItems",
            Self::SpecialJump => "SpecialJump",
            Self::IsTotalBountyHuntingAwardInRange => "IsTotalBountyHuntingAwardInRange",
            Self::IsNPCInParty => "IsNPCInParty",
        }
    }
}

// ── EVT Variable IDs (MM6) ──────────────────────────────────────────────
// From MMExtension Scripts/Structs/evt.lua — MM6 uses 1-byte variable IDs.
// These are the variable identifiers used by Compare/Add/Subtract/Set opcodes.

/// MM6 EVT variable identifier (1-byte). Used in Compare, Add, Subtract, Set opcodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvtVariable(pub u8);

impl EvtVariable {
    // Player attributes
    pub const SEX_IS: Self = Self(0x01);
    pub const CLASS_IS: Self = Self(0x02);
    pub const HP: Self = Self(0x03);
    pub const HAS_FULL_HP: Self = Self(0x04);
    pub const SP: Self = Self(0x05);
    pub const HAS_FULL_SP: Self = Self(0x06);
    pub const AC: Self = Self(0x07);
    pub const AC_BONUS: Self = Self(0x08);
    pub const BASE_LEVEL: Self = Self(0x09);
    pub const LEVEL_BONUS: Self = Self(0x0A);
    pub const AGE_BONUS: Self = Self(0x0B);
    pub const AWARDS: Self = Self(0x0C);
    pub const EXPERIENCE: Self = Self(0x0D);

    // Quest / inventory
    pub const QBITS: Self = Self(0x10);
    pub const INVENTORY: Self = Self(0x11);

    // Time
    pub const HOUR_IS: Self = Self(0x12);
    pub const DAY_OF_YEAR_IS: Self = Self(0x13);
    pub const DAY_OF_WEEK_IS: Self = Self(0x14);

    // Gold / food
    pub const GOLD: Self = Self(0x15);
    pub const GOLD_ADD_RANDOM: Self = Self(0x16);
    pub const FOOD: Self = Self(0x17);
    pub const FOOD_ADD_RANDOM: Self = Self(0x18);

    // Stat bonuses
    pub const MIGHT_BONUS: Self = Self(0x19);
    pub const INTELLECT_BONUS: Self = Self(0x1A);
    pub const PERSONALITY_BONUS: Self = Self(0x1B);
    pub const ENDURANCE_BONUS: Self = Self(0x1C);
    pub const SPEED_BONUS: Self = Self(0x1D);
    pub const ACCURACY_BONUS: Self = Self(0x1E);
    pub const LUCK_BONUS: Self = Self(0x1F);

    // Base stats
    pub const BASE_MIGHT: Self = Self(0x20);
    pub const BASE_INTELLECT: Self = Self(0x21);
    pub const BASE_PERSONALITY: Self = Self(0x22);
    pub const BASE_ENDURANCE: Self = Self(0x23);
    pub const BASE_SPEED: Self = Self(0x24);
    pub const BASE_ACCURACY: Self = Self(0x25);
    pub const BASE_LUCK: Self = Self(0x26);

    // Current stats
    pub const CUR_MIGHT: Self = Self(0x27);
    pub const CUR_INTELLECT: Self = Self(0x28);
    pub const CUR_PERSONALITY: Self = Self(0x29);
    pub const CUR_ENDURANCE: Self = Self(0x2A);
    pub const CUR_SPEED: Self = Self(0x2B);
    pub const CUR_ACCURACY: Self = Self(0x2C);
    pub const CUR_LUCK: Self = Self(0x2D);

    // Resistances
    pub const FIRE_RESISTANCE: Self = Self(0x2E);
    pub const ELEC_RESISTANCE: Self = Self(0x2F);
    pub const COLD_RESISTANCE: Self = Self(0x30);
    pub const POISON_RESISTANCE: Self = Self(0x31);
    pub const MAGIC_RESISTANCE: Self = Self(0x32);

    // Resistance bonuses
    pub const FIRE_RESISTANCE_BONUS: Self = Self(0x33);
    pub const ELEC_RESISTANCE_BONUS: Self = Self(0x34);
    pub const COLD_RESISTANCE_BONUS: Self = Self(0x35);
    pub const POISON_RESISTANCE_BONUS: Self = Self(0x36);
    pub const MAGIC_RESISTANCE_BONUS: Self = Self(0x37);

    // Skills (0x38..=0x56)
    pub const SKILL_STAFF: Self = Self(0x38);
    pub const SKILL_SWORD: Self = Self(0x39);
    pub const SKILL_DAGGER: Self = Self(0x3A);
    pub const SKILL_AXE: Self = Self(0x3B);
    pub const SKILL_SPEAR: Self = Self(0x3C);
    pub const SKILL_BOW: Self = Self(0x3D);
    pub const SKILL_MACE: Self = Self(0x3E);
    pub const SKILL_BLASTER: Self = Self(0x3F);
    pub const SKILL_SHIELD: Self = Self(0x40);
    pub const SKILL_LEATHER: Self = Self(0x41);
    pub const SKILL_CHAIN: Self = Self(0x42);
    pub const SKILL_PLATE: Self = Self(0x43);
    pub const SKILL_FIRE_MAGIC: Self = Self(0x44);
    pub const SKILL_AIR_MAGIC: Self = Self(0x45);
    pub const SKILL_WATER_MAGIC: Self = Self(0x46);
    pub const SKILL_EARTH_MAGIC: Self = Self(0x47);
    pub const SKILL_SPIRIT_MAGIC: Self = Self(0x48);
    pub const SKILL_MIND_MAGIC: Self = Self(0x49);
    pub const SKILL_BODY_MAGIC: Self = Self(0x4A);
    pub const SKILL_LIGHT_MAGIC: Self = Self(0x4B);
    pub const SKILL_DARK_MAGIC: Self = Self(0x4C);
    pub const SKILL_IDENTIFY_ITEM: Self = Self(0x4D);
    pub const SKILL_MERCHANT: Self = Self(0x4E);
    pub const SKILL_REPAIR: Self = Self(0x4F);
    pub const SKILL_BODY_BUILDING: Self = Self(0x50);
    pub const SKILL_MEDITATION: Self = Self(0x51);
    pub const SKILL_PERCEPTION: Self = Self(0x52);
    pub const SKILL_DIPLOMACY: Self = Self(0x53);
    pub const SKILL_DISARM_TRAP: Self = Self(0x54);
    pub const SKILL_LEARNING: Self = Self(0x55);
    pub const SKILL_MISC: Self = Self(0x56);

    // Conditions (0x57..=0x68)
    pub const COND_CURSED: Self = Self(0x57);
    pub const COND_WEAK: Self = Self(0x58);
    pub const COND_ASLEEP: Self = Self(0x59);
    pub const COND_AFRAID: Self = Self(0x5A);
    pub const COND_DRUNK: Self = Self(0x5B);
    pub const COND_INSANE: Self = Self(0x5C);
    pub const COND_POISONED1: Self = Self(0x5D);
    pub const COND_DISEASED1: Self = Self(0x5E);
    pub const COND_POISONED2: Self = Self(0x5F);
    pub const COND_DISEASED2: Self = Self(0x60);
    pub const COND_POISONED3: Self = Self(0x61);
    pub const COND_DISEASED3: Self = Self(0x62);
    pub const COND_PARALYZED: Self = Self(0x63);
    pub const COND_UNCONSCIOUS: Self = Self(0x64);
    pub const COND_DEAD: Self = Self(0x65);
    pub const COND_PETRIFIED: Self = Self(0x66);
    pub const COND_ERADICATED: Self = Self(0x67);
    pub const COND_MAIN: Self = Self(0x68);

    // Map variables (0x69..=0xCC = MapVar0..MapVar99)
    pub const MAP_VAR_BASE: Self = Self(0x69);

    // Misc
    pub const AUTONOTES_BITS: Self = Self(0xCD);
    pub const IS_MIGHT_MORE_THAN_BASE: Self = Self(0xCE);
    pub const IS_INTELLECT_MORE_THAN_BASE: Self = Self(0xCF);
    pub const IS_PERSONALITY_MORE_THAN_BASE: Self = Self(0xD0);
    pub const IS_ENDURANCE_MORE_THAN_BASE: Self = Self(0xD1);
    pub const IS_SPEED_MORE_THAN_BASE: Self = Self(0xD2);
    pub const IS_ACCURACY_MORE_THAN_BASE: Self = Self(0xD3);
    pub const IS_LUCK_MORE_THAN_BASE: Self = Self(0xD4);
    pub const PLAYER_BITS: Self = Self(0xD5);
    pub const NPCS: Self = Self(0xD6);
    pub const REPUTATION_IS: Self = Self(0xD7);
    pub const DAYS_COUNTER1: Self = Self(0xD8);
    pub const DAYS_COUNTER2: Self = Self(0xD9);
    pub const DAYS_COUNTER3: Self = Self(0xDA);
    pub const DAYS_COUNTER4: Self = Self(0xDB);
    pub const DAYS_COUNTER5: Self = Self(0xDC);
    pub const DAYS_COUNTER6: Self = Self(0xDD);
    pub const FLYING: Self = Self(0xDE);
    pub const HAS_NPC_PROFESSION: Self = Self(0xDF);
    pub const TOTAL_CIRCUS_PRIZE: Self = Self(0xE0);
    pub const SKILL_POINTS: Self = Self(0xE1);
    pub const MONTH_IS: Self = Self(0xE2);

    /// Human-readable name for this variable ID.
    pub fn name(self) -> &'static str {
        match self.0 {
            0x01 => "SexIs",
            0x02 => "ClassIs",
            0x03 => "HP",
            0x04 => "HasFullHP",
            0x05 => "SP",
            0x06 => "HasFullSP",
            0x07 => "AC",
            0x08 => "ACBonus",
            0x09 => "BaseLevel",
            0x0A => "LevelBonus",
            0x0B => "AgeBonus",
            0x0C => "Awards",
            0x0D => "Experience",
            0x10 => "QBits",
            0x11 => "Inventory",
            0x12 => "HourIs",
            0x13 => "DayOfYearIs",
            0x14 => "DayOfWeekIs",
            0x15 => "Gold",
            0x16 => "GoldAddRandom",
            0x17 => "Food",
            0x18 => "FoodAddRandom",
            0x19 => "MightBonus",
            0x1A => "IntellectBonus",
            0x1B => "PersonalityBonus",
            0x1C => "EnduranceBonus",
            0x1D => "SpeedBonus",
            0x1E => "AccuracyBonus",
            0x1F => "LuckBonus",
            0x20 => "BaseMight",
            0x21 => "BaseIntellect",
            0x22 => "BasePersonality",
            0x23 => "BaseEndurance",
            0x24 => "BaseSpeed",
            0x25 => "BaseAccuracy",
            0x26 => "BaseLuck",
            0x27 => "CurMight",
            0x28 => "CurIntellect",
            0x29 => "CurPersonality",
            0x2A => "CurEndurance",
            0x2B => "CurSpeed",
            0x2C => "CurAccuracy",
            0x2D => "CurLuck",
            0x2E => "FireResistance",
            0x2F => "ElecResistance",
            0x30 => "ColdResistance",
            0x31 => "PoisonResistance",
            0x32 => "MagicResistance",
            0x33 => "FireResistanceBonus",
            0x34 => "ElecResistanceBonus",
            0x35 => "ColdResistanceBonus",
            0x36 => "PoisonResistanceBonus",
            0x37 => "MagicResistanceBonus",
            0x38 => "SkillStaff",
            0x39 => "SkillSword",
            0x3A => "SkillDagger",
            0x3B => "SkillAxe",
            0x3C => "SkillSpear",
            0x3D => "SkillBow",
            0x3E => "SkillMace",
            0x3F => "SkillBlaster",
            0x40 => "SkillShield",
            0x41 => "SkillLeather",
            0x42 => "SkillChain",
            0x43 => "SkillPlate",
            0x44 => "SkillFireMagic",
            0x45 => "SkillAirMagic",
            0x46 => "SkillWaterMagic",
            0x47 => "SkillEarthMagic",
            0x48 => "SkillSpiritMagic",
            0x49 => "SkillMindMagic",
            0x4A => "SkillBodyMagic",
            0x4B => "SkillLightMagic",
            0x4C => "SkillDarkMagic",
            0x4D => "SkillIdentifyItem",
            0x4E => "SkillMerchant",
            0x4F => "SkillRepair",
            0x50 => "SkillBodyBuilding",
            0x51 => "SkillMeditation",
            0x52 => "SkillPerception",
            0x53 => "SkillDiplomacy",
            0x54 => "SkillDisarmTrap",
            0x55 => "SkillLearning",
            0x56 => "SkillMisc",
            0x57 => "CondCursed",
            0x58 => "CondWeak",
            0x59 => "CondAsleep",
            0x5A => "CondAfraid",
            0x5B => "CondDrunk",
            0x5C => "CondInsane",
            0x5D => "CondPoisoned1",
            0x5E => "CondDiseased1",
            0x5F => "CondPoisoned2",
            0x60 => "CondDiseased2",
            0x61 => "CondPoisoned3",
            0x62 => "CondDiseased3",
            0x63 => "CondParalyzed",
            0x64 => "CondUnconscious",
            0x65 => "CondDead",
            0x66 => "CondPetrified",
            0x67 => "CondEradicated",
            0x68 => "CondMain",
            0x69..=0xCC => "MapVar", // caller should subtract 0x69 for index
            0xCD => "AutonotesBits",
            0xCE => "IsMightMoreThanBase",
            0xCF => "IsIntellectMoreThanBase",
            0xD0 => "IsPersonalityMoreThanBase",
            0xD1 => "IsEnduranceMoreThanBase",
            0xD2 => "IsSpeedMoreThanBase",
            0xD3 => "IsAccuracyMoreThanBase",
            0xD4 => "IsLuckMoreThanBase",
            0xD5 => "PlayerBits",
            0xD6 => "NPCs",
            0xD7 => "ReputationIs",
            0xD8 => "DaysCounter1",
            0xD9 => "DaysCounter2",
            0xDA => "DaysCounter3",
            0xDB => "DaysCounter4",
            0xDC => "DaysCounter5",
            0xDD => "DaysCounter6",
            0xDE => "Flying",
            0xDF => "HasNPCProfession",
            0xE0 => "TotalCircusPrize",
            0xE1 => "SkillPoints",
            0xE2 => "MonthIs",
            _ => "Unknown",
        }
    }

    /// Returns true if this is a map-local variable (MapVar0..MapVar99).
    pub fn is_map_var(self) -> bool {
        (0x69..=0xCC).contains(&self.0)
    }

    /// For map variables, returns the index (0-99).
    pub fn map_var_index(self) -> Option<u8> {
        if self.is_map_var() { Some(self.0 - 0x69) } else { None }
    }

    /// Returns true if this variable ID refers to a skill (SkillStaff..SkillMisc).
    pub fn is_skill(self) -> bool {
        (0x38..=0x56).contains(&self.0)
    }

    /// For skill variables, returns the skill index (0 = Staff, 30 = Misc).
    pub fn skill_index(self) -> Option<u8> {
        if self.is_skill() { Some(self.0 - 0x38) } else { None }
    }

    /// Returns true if this variable ID refers to a character-scoped variable (0x01..=0x68).
    pub fn is_character_scoped(self) -> bool {
        (0x01..=0x68).contains(&self.0)
    }
}

impl std::fmt::Display for EvtVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_map_var() {
            write!(f, "MapVar{}", self.0 - 0x69)
        } else {
            write!(f, "{}", self.name())
        }
    }
}

/// Target character selection for EVT events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EvtTargetCharacter {
    Player1 = 0,
    Player2 = 1,
    Player3 = 2,
    Player4 = 3,
    Active = 4,
    Party = 5,
    Random = 6,
}

impl EvtTargetCharacter {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Player1),
            1 => Some(Self::Player2),
            2 => Some(Self::Player3),
            3 => Some(Self::Player4),
            4 => Some(Self::Active),
            5 => Some(Self::Party),
            6 => Some(Self::Random),
            _ => None,
        }
    }
}
