//! Shared enums and bitflags for MM6 data formats.
//! Cross-referenced against MMExtension (Scripts/Structs/01 common structs.lua)
//! and OpenEnroth (src/Engine/).

use bitflags::bitflags;

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
    ShowMovie = 12,
    SetSprite = 13,
    Compare = 14,
    ChangeDoorState = 15,
    Add = 16,
    Subtract = 17,
    Set = 18,
    SummonMonsters = 19,
    // 20 unused
    CastSpell = 21,
    SpeakNPC = 22,
    SetFacesBit = 23,
    ToggleActorFlag = 24,
    RandomGoTo = 25,
    InputString = 26,
    // 27, 28 unused
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
            12 => Some(Self::ShowMovie),
            13 => Some(Self::SetSprite),
            14 => Some(Self::Compare),
            15 => Some(Self::ChangeDoorState),
            16 => Some(Self::Add),
            17 => Some(Self::Subtract),
            18 => Some(Self::Set),
            19 => Some(Self::SummonMonsters),
            21 => Some(Self::CastSpell),
            22 => Some(Self::SpeakNPC),
            23 => Some(Self::SetFacesBit),
            24 => Some(Self::ToggleActorFlag),
            25 => Some(Self::RandomGoTo),
            26 => Some(Self::InputString),
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
            Self::ShowMovie => "ShowMovie",
            Self::SetSprite => "SetSprite",
            Self::Compare => "Compare",
            Self::ChangeDoorState => "ChangeDoorState",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Set => "Set",
            Self::SummonMonsters => "SummonMonsters",
            Self::CastSpell => "CastSpell",
            Self::SpeakNPC => "SpeakNPC",
            Self::SetFacesBit => "SetFacesBit",
            Self::ToggleActorFlag => "ToggleActorFlag",
            Self::RandomGoTo => "RandomGoTo",
            Self::InputString => "InputString",
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

// ── AI State ─────────────────────────────────────────────────────────────

/// Actor AI state (from OpenEnroth ActorEnums.h).
/// Stored as u16 in MapMonster at offset 0xA0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum AIState {
    Standing = 0,
    Tethered = 1,
    AttackingMelee = 2,
    AttackingRanged1 = 3,
    Dying = 4,
    Dead = 5,
    Pursuing = 6,
    Fleeing = 7,
    Stunned = 8,
    Fidgeting = 9,
    Interacting = 10,
    Removed = 11,
    AttackingRanged2 = 12,
    AttackingRanged3 = 13,
    Stoned = 14,
    Paralyzed = 15,
    Resurrected = 16,
    Summoned = 17,
    AttackingRanged4 = 18,
    Disabled = 19,
}

impl AIState {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0 => Some(Self::Standing),
            1 => Some(Self::Tethered),
            2 => Some(Self::AttackingMelee),
            3 => Some(Self::AttackingRanged1),
            4 => Some(Self::Dying),
            5 => Some(Self::Dead),
            6 => Some(Self::Pursuing),
            7 => Some(Self::Fleeing),
            8 => Some(Self::Stunned),
            9 => Some(Self::Fidgeting),
            10 => Some(Self::Interacting),
            11 => Some(Self::Removed),
            12 => Some(Self::AttackingRanged2),
            13 => Some(Self::AttackingRanged3),
            14 => Some(Self::Stoned),
            15 => Some(Self::Paralyzed),
            16 => Some(Self::Resurrected),
            17 => Some(Self::Summoned),
            18 => Some(Self::AttackingRanged4),
            19 => Some(Self::Disabled),
            _ => None,
        }
    }
}

// ── Actor Animation ──────────────────────────────────────────────────────

/// Actor animation type indices (8 animation slots in MapMonster.Frames[]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ActorAnimation {
    Standing = 0,
    Walking = 1,
    AtkMelee = 2,
    AtkRanged = 3,
    GotHit = 4,
    Dying = 5,
    Dead = 6,
    Bored = 7,
}

// ── Actor Attributes (bitflags) ──────────────────────────────────────────

bitflags! {
    /// MapMonster.Bits flags (u32 at offset 0x24).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ActorAttributes: u32 {
        const UNKNOWN_4        = 0x00000004;
        const VISIBLE          = 0x00000008;
        const STAND_IN_QUEUE   = 0x00000080;
        const FULL_AI_STATE    = 0x00000400;
        const ACTIVE           = 0x00004000;
        const NEARBY           = 0x00008000;
        const AI_DISABLED      = 0x00010000;
        const FLEEING          = 0x00020000;
        const LAST_SPELL_MISSED = 0x00040000;
        const AGGRESSOR        = 0x00080000;
        const ALERT_STATUS     = 0x00100000;
        const ANIMATION        = 0x00200000;
        const HAS_JOB          = 0x00400000;
        const HAS_ITEM         = 0x00800000;
        const HOSTILE          = 0x01000000;
    }
}

// ── Tile Flags ───────────────────────────────────────────────────────────

bitflags! {
    /// Tile attribute flags from dtile.bin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TileFlags: u16 {
        const BURN             = 0x0001;
        const WATER            = 0x0002;
        const BLOCK            = 0x0004;
        const REPULSE          = 0x0008;
        const FLAT             = 0x0010;
        const WAVY             = 0x0020;
        const DONT_DRAW        = 0x0040;
        const SHORE            = 0x0100;
        const TRANSITION       = 0x0200;
        const SCROLL_DOWN      = 0x0400;
        const SCROLL_UP        = 0x0800;
        const SCROLL_LEFT      = 0x1000;
        const SCROLL_RIGHT     = 0x2000;
    }
}

impl Default for TileFlags {
    fn default() -> Self {
        Self::empty()
    }
}

// ── Decoration Flags ─────────────────────────────────────────────────────

bitflags! {
    /// Decoration description flags from ddeclist.bin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DecorationDescFlags: u16 {
        const MOVE_THROUGH     = 0x0001;
        const DONT_DRAW        = 0x0002;
        const FLICKER_SLOW     = 0x0004;
        const FLICKER_MEDIUM   = 0x0008;
        const FLICKER_FAST     = 0x0010;
        const MARKER           = 0x0020;
        const SLOW_LOOP        = 0x0040;
        const EMITS_FIRE       = 0x0080;
        const SOUND_ON_DAWN    = 0x0100;
        const SOUND_ON_DUSK    = 0x0200;
        const EMITS_SMOKE      = 0x0400;
    }
}

bitflags! {
    /// Level decoration (billboard) instance flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct LevelDecorationFlags: u16 {
        const TRIGGERED_BY_TOUCH   = 0x01;
        const TRIGGERED_BY_MONSTER = 0x02;
        const TRIGGERED_BY_OBJECT  = 0x04;
        const VISIBLE_ON_MAP       = 0x08;
        const CHEST                = 0x10;
        const INVISIBLE            = 0x20;
        const OBELISK_CHEST        = 0x40;
    }
}

// ── BLV Face Attributes ──────────────────────────────────────────────────

bitflags! {
    /// BLV indoor face attribute flags (u32).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FaceAttributes: u32 {
        const PORTAL               = 0x00000001;
        const SECRET               = 0x00000002;
        const SCROLL_DOWN          = 0x00000004;
        const ALIGN_TOP            = 0x00000008;
        const FLUID                = 0x00000010;
        const SCROLL_UP            = 0x00000020;
        const SCROLL_LEFT          = 0x00000040;
        const SCROLL_RIGHT         = 0x00000080;
        const ALIGN_LEFT           = 0x00000100;
        const INVISIBLE            = 0x00002000;
        const ANIMATED             = 0x00004000;
        const ALIGN_RIGHT          = 0x00008000;
        const MOVES_BY_DOOR        = 0x00010000;
        const ALTERNATE_SOUND      = 0x00020000;
        const SKY                  = 0x00040000;
        const CLICKABLE            = 0x02000000;
        const EVENT_BY_TOUCH       = 0x04000000;
        const EVENT_BY_MONSTER     = 0x08000000;
        const EVENT_BY_OBJECT      = 0x10000000;
        const DONT_BLOCK           = 0x20000000;
        const LAVA                 = 0x40000000;
    }
}

// ── BSP Model Face Attributes ────────────────────────────────────────────

bitflags! {
    /// Outdoor BSP model face attribute flags (u32).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ModelFaceAttributes: u32 {
        const PORTAL               = 0x00000001;
        const FLUID                = 0x00000010;
        const INVISIBLE            = 0x00002000;
        const ANIMATED             = 0x00004000;
        const MOVES_BY_DOOR        = 0x00010000;
        const SKY                  = 0x00040000;
        const CLICKABLE            = 0x02000000;
        const EVENT_BY_TOUCH       = 0x04000000;
        const DONT_BLOCK           = 0x20000000;
        const LAVA                 = 0x40000000;
    }
}

// ── Door Attributes ──────────────────────────────────────────────────────

bitflags! {
    /// BLV door attribute flags (u32).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DoorAttributes: u32 {
        const START_STATE_2    = 0x01;
        const SILENT_MOVE      = 0x02;
        const NO_SOUND         = 0x04;
        const STOPPED          = 0x08;
    }
}

// ── Sound Type ───────────────────────────────────────────────────────────

/// Sound type from dsounds.bin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SoundType {
    LevelSpecific = 0,
    System = 1,
    Swap = 2,
    Unknown3 = 3,
    Lock = 4,
}

impl SoundType {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::LevelSpecific),
            1 => Some(Self::System),
            2 => Some(Self::Swap),
            3 => Some(Self::Unknown3),
            4 => Some(Self::Lock),
            _ => None,
        }
    }
}

bitflags! {
    /// Sound attribute flags from dsounds.bin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SoundAttributes: u32 {
        const LOCKED = 0x01;
        const IS_3D  = 0x02;
    }
}

// ── SFT Frame Attributes ─────────────────────────────────────────────────

bitflags! {
    /// Sprite frame table frame attribute flags (u16).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SpriteFrameFlags: u16 {
        const NOT_GROUP_END = 0x0001;
        const LUMINOUS      = 0x0002;
        const GROUP_START   = 0x0004;
        const IMAGE1        = 0x0010;
        const CENTER        = 0x0020;
        const FIDGET        = 0x0040;
        const LOADED        = 0x0080;
        const MIRROR0       = 0x0100;
        const MIRROR1       = 0x0200;
        const MIRROR2       = 0x0400;
        const MIRROR3       = 0x0800;
        const MIRROR4       = 0x1000;
        const MIRROR5       = 0x2000;
        const MIRROR6       = 0x4000;
        const MIRROR7       = 0x8000;
    }
}

// ── Monster Enums ────────────────────────────────────────────────────────

/// Monster movement type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MonsterMovementType {
    Short = 0,
    Medium = 1,
    Long = 2,
    Global = 3,
    Free = 4,
    Stationary = 5,
}

impl MonsterMovementType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Short),
            1 => Some(Self::Medium),
            2 => Some(Self::Long),
            3 => Some(Self::Global),
            4 => Some(Self::Free),
            5 => Some(Self::Stationary),
            _ => None,
        }
    }
}

/// Monster AI type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MonsterAIType {
    Suicide = 0,
    Wimp = 1,
    Normal = 2,
    Aggressive = 3,
}

impl MonsterAIType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Suicide),
            1 => Some(Self::Wimp),
            2 => Some(Self::Normal),
            3 => Some(Self::Aggressive),
            _ => None,
        }
    }
}

/// Monster hostility type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MonsterHostility {
    Friendly = 0,
    Close = 1,
    Short = 2,
    Medium = 3,
    Long = 4,
}

impl MonsterHostility {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Friendly),
            1 => Some(Self::Close),
            2 => Some(Self::Short),
            3 => Some(Self::Medium),
            4 => Some(Self::Long),
            _ => None,
        }
    }
}

/// Monster special attack types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MonsterSpecialAttack {
    None = 0,
    Curse = 1,
    Weak = 2,
    Sleep = 3,
    Drunk = 4,
    Insane = 5,
    PoisonWeak = 6,
    PoisonMedium = 7,
    PoisonSevere = 8,
    DiseaseWeak = 9,
    DiseaseMedium = 10,
    DiseaseSevere = 11,
    Paralyzed = 12,
    Unconscious = 13,
    Dead = 14,
    Petrified = 15,
    Eradicated = 16,
    BreakAny = 17,
    BreakArmor = 18,
    BreakWeapon = 19,
    Steal = 20,
    Aging = 21,
    ManaDrain = 22,
    Fear = 23,
}

impl MonsterSpecialAttack {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Curse),
            2 => Some(Self::Weak),
            3 => Some(Self::Sleep),
            4 => Some(Self::Drunk),
            5 => Some(Self::Insane),
            6 => Some(Self::PoisonWeak),
            7 => Some(Self::PoisonMedium),
            8 => Some(Self::PoisonSevere),
            9 => Some(Self::DiseaseWeak),
            10 => Some(Self::DiseaseMedium),
            11 => Some(Self::DiseaseSevere),
            12 => Some(Self::Paralyzed),
            13 => Some(Self::Unconscious),
            14 => Some(Self::Dead),
            15 => Some(Self::Petrified),
            16 => Some(Self::Eradicated),
            17 => Some(Self::BreakAny),
            18 => Some(Self::BreakArmor),
            19 => Some(Self::BreakWeapon),
            20 => Some(Self::Steal),
            21 => Some(Self::Aging),
            22 => Some(Self::ManaDrain),
            23 => Some(Self::Fear),
            _ => None,
        }
    }
}

/// Monster special ability type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MonsterSpecialAbility {
    None = 0,
    MultiShot = 1,
    Summon = 2,
    Explode = 3,
}

impl MonsterSpecialAbility {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::MultiShot),
            2 => Some(Self::Summon),
            3 => Some(Self::Explode),
            _ => None,
        }
    }
}

/// Polygon type (for both BLV faces and BSP model faces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PolygonType {
    Invalid = 0,
    VerticalWall = 1,
    Unknown2 = 2,
    Floor = 3,
    InBetweenFloorAndWall = 4,
    Ceiling = 5,
    InBetweenCeilingAndWall = 6,
}

impl PolygonType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Invalid),
            1 => Some(Self::VerticalWall),
            2 => Some(Self::Unknown2),
            3 => Some(Self::Floor),
            4 => Some(Self::InBetweenFloorAndWall),
            5 => Some(Self::Ceiling),
            6 => Some(Self::InBetweenCeilingAndWall),
            _ => None,
        }
    }

    pub fn is_ceiling(self) -> bool {
        matches!(self, Self::Ceiling | Self::InBetweenCeilingAndWall)
    }
}

/// Door action type used in ChangeDoorState EVT event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DoorAction {
    Open = 0,
    Close = 1,
    Toggle = 2,
}

impl DoorAction {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Open),
            1 => Some(Self::Close),
            2 => Some(Self::Toggle),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl std::fmt::Display for DoorAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "Open"),
            Self::Close => write!(f, "Close"),
            Self::Toggle => write!(f, "Toggle"),
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
