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
            12 => Some(Self::ShowMovie),
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
            Self::ShowMovie => "ShowMovie",
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
///
/// MM6 semantics (from MMExtension evt.SetDoorState):
///   0 = go to state (0) = initial/open position
///   1 = go to state (1) = alternate/closed position
///   2 = toggle if door isn't moving
///   3 = toggle always
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DoorAction {
    GoToOpen = 0,
    GoToClosed = 1,
    ToggleIfStopped = 2,
    Toggle = 3,
}

impl DoorAction {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::GoToOpen),
            1 => Some(Self::GoToClosed),
            2 => Some(Self::ToggleIfStopped),
            3 => Some(Self::Toggle),
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
            Self::GoToOpen => write!(f, "GoToOpen"),
            Self::GoToClosed => write!(f, "GoToClosed"),
            Self::ToggleIfStopped => write!(f, "ToggleIfStopped"),
            Self::Toggle => write!(f, "Toggle"),
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

#[cfg(test)]
#[path = "enums_tests.rs"]
mod tests;
