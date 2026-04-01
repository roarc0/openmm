# LOD Complete Data Structures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ensure all MM6 binary formats in the `lod` crate are fully parsed with proper enums, bitflags, and no skipped fields — cross-referenced against MMExtension and OpenEnroth.

**Architecture:** Add a new `enums.rs` module for shared enums/bitflags. Expand existing parsers to read all fields. Add new simple format parsers. All changes confined to the `lod` crate.

**Tech Stack:** Rust, byteorder, bitflags (add to lod/Cargo.toml)

---

### Task 1: Add bitflags dependency and create enums.rs

**Files:**
- Modify: `lod/Cargo.toml`
- Create: `lod/src/enums.rs`
- Modify: `lod/src/lib.rs`

- [ ] **Step 1: Add bitflags to lod/Cargo.toml**

Add `bitflags = "2"` to the `[dependencies]` section of `lod/Cargo.toml`.

- [ ] **Step 2: Create lod/src/enums.rs with all shared enums and bitflags**

This file contains enums and bitflags shared across multiple parsers. Each type includes doc comments with the source reference (MMExtension/OpenEnroth).

```rust
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
    MouseOver = 4,        // "Hint" in our code
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
    // 20 is unused
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
/// From OpenEnroth ActorEnums.h.
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
    Bored = 7,  // fidget
}

// ── Actor Attributes (bitflags) ──────────────────────────────────────────

bitflags! {
    /// MapMonster.Bits flags (u32 at offset 0x24).
    /// From OpenEnroth ActorEnums.h and MMExtension const.MonsterBits.
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

// ── Actor Buffs ──────────────────────────────────────────────────────────

/// Actor spell buff indices. 14 buffs stored as SpellBuff[14] at offset 0xC4.
/// From OpenEnroth ActorEnums.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ActorBuff {
    Charm = 1,
    Summoned = 2,
    Shrink = 3,
    Afraid = 4,
    Stoned = 5,
    Paralyzed = 6,
    Slowed = 7,
    HalvesAC = 8,
    Berserk = 9,
    MassDistortion = 10,
    Fate = 11,
    Enslaved = 12,
    DayOfProtection = 13,
    HourOfPower = 14,
    Shield = 15,
    Stoneskin = 16,
    Bless = 17,
    Heroism = 18,
    Haste = 19,
    PainReflection = 20,
    Hammerhands = 21,
}

// ── Tile Flags ───────────────────────────────────────────────────────────

bitflags! {
    /// Tile attribute flags from dtile.bin.
    /// From OpenEnroth TileEnums.h and MMExtension TileItem.Bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TileFlags: u16 {
        const BURN             = 0x0001;
        const WATER            = 0x0002;
        const BLOCK            = 0x0004;
        const REPULSE          = 0x0008;
        const FLAT             = 0x0010;
        const WAVY             = 0x0020;
        const DONT_DRAW        = 0x0040;
        // 0x0080 unused
        const SHORE            = 0x0100; // water transition
        const TRANSITION       = 0x0200;
        const SCROLL_DOWN      = 0x0400;
        const SCROLL_UP        = 0x0800;
        const SCROLL_LEFT      = 0x1000;
        const SCROLL_RIGHT     = 0x2000;
    }
}

// ── Decoration Flags ─────────────────────────────────────────────────────

bitflags! {
    /// Decoration description flags from ddeclist.bin.
    /// From OpenEnroth DecorationEnums.h.
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
    /// From OpenEnroth DecorationEnums.h.
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
    /// From MMExtension MapFacet.Bits and OpenEnroth FaceAttribute.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FaceAttributes: u32 {
        const PORTAL               = 0x00000001;
        const SECRET               = 0x00000002;
        const SCROLL_DOWN          = 0x00000004;
        const ALIGN_TOP            = 0x00000008;
        const FLUID                = 0x00000010; // water face
        const SCROLL_UP            = 0x00000020;
        const SCROLL_LEFT          = 0x00000040;
        const SCROLL_RIGHT         = 0x00000080; // projected to byte boundary
        const ALIGN_LEFT           = 0x00000100;
        const INVISIBLE            = 0x00002000;
        const ANIMATED             = 0x00004000;
        const ALIGN_RIGHT          = 0x00008000;
        const ALIGN_BOTTOM         = 0x00010000;
        const MOVES_BY_DOOR        = 0x00010000; // same bit: context-dependent
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
    /// Outdoor BSP model face attribute flags (u32). Same bits as BLV faces.
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
    /// BLV door attribute flags (u32 at offset 0x0 of MapDoor).
    /// From MMExtension MapDoor.Bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DoorAttributes: u32 {
        const START_STATE_2    = 0x01; // starts in state 2 (closed)
        const SILENT_MOVE      = 0x02;
        const NO_SOUND         = 0x04;
        const STOPPED          = 0x08;
    }
}

// ── Sound Type ───────────────────────────────────────────────────────────

/// Sound type from dsounds.bin.
/// From MMExtension SoundsItem.Type.
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
    /// Sprite frame table (dsft.bin) frame attribute flags (u16).
    /// From MMExtension SFTItem.Bits and OpenEnroth SpriteFrame.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SpriteFrameFlags: u16 {
        const NOT_GROUP_END = 0x0001;
        const LUMINOUS      = 0x0002;
        const GROUP_START   = 0x0004;
        // 0x0008 unused
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

/// Monster movement type from MonstersTxt (CommonMonsterProps.MoveType).
/// From OpenEnroth MonsterEnums.h.
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

/// Monster AI type from MonstersTxt (CommonMonsterProps.AIType).
/// From OpenEnroth MonsterEnums.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MonsterAIType {
    Suicide = 0,   // never runs
    Wimp = 1,      // always runs (peasants)
    Normal = 2,    // runs at 20% HP
    Aggressive = 3, // runs at 10% HP
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

/// Monster hostility type from MonstersTxt.
/// From OpenEnroth MonsterEnums.h.
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
/// From OpenEnroth MonsterEnums.h.
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
/// From OpenEnroth MonsterEnums.h.
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
/// From OpenEnroth and MMExtension.
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
}

/// Target character selection for EVT events.
/// From OpenEnroth EvtEnums.h.
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
```

- [ ] **Step 3: Register enums.rs in lib.rs**

Add `pub mod enums;` to `lod/src/lib.rs` after the existing module declarations.

- [ ] **Step 4: Build to verify compilation**

Run: `cd /home/roarc/repos/openmm && cargo build -p lod`
Expected: compiles successfully.

- [ ] **Step 5: Commit**

```bash
git add lod/Cargo.toml lod/src/enums.rs lod/src/lib.rs
git commit -m "feat(lod): add enums.rs with complete MM6 bitflags and enums

Adds EvtOpcode (69 opcodes), AIState, ActorAttributes, ActorBuff,
TileFlags, DecorationDescFlags, FaceAttributes, DoorAttributes,
SoundType, SpriteFrameFlags, MonsterMovementType, MonsterAIType,
MonsterHostility, MonsterSpecialAttack, PolygonType, and more.
All cross-referenced against MMExtension and OpenEnroth."
```

---

### Task 2: Wire all skipped DdmActor fields

**Files:**
- Modify: `lod/src/ddm.rs`

The current DdmActor skips ~15 fields. Wire them all per MMExtension MapMonster struct.

- [ ] **Step 1: Add new fields to DdmActor struct**

Add these fields to the `DdmActor` struct (after existing fields, before closing brace):

```rust
    /// Monster attribute flags (offset 0x24).
    pub attributes: ActorAttributes,
    /// Pitch/look angle (offset 0x8C).
    pub pitch: u16,
    /// Room/sector for indoor maps (offset 0x8E).
    pub room: i16,
    /// Current action length/timer (offset 0x90).
    pub current_action_length: u16,
    /// Item being carried (offset 0xA4).
    pub carried_item: u16,
    /// Current action step/time (offset 0xA8).
    pub current_action_step: u32,
    /// Sound IDs: [attack, die, got_hit, fidget] (offset 0xBC).
    pub sound_ids: [u16; 4],
    /// Spell buffs (14 entries, each 16 bytes) at offset 0xC4.
    pub spell_buffs: [SpellBuff; 14],
    /// Group ID (offset 0x1A4).
    pub group: i32,
    /// Ally monster class (offset 0x1A8).
    pub ally: i32,
    /// Summoner actor ID (offset 0x20C).
    pub summoner: i32,
    /// Last attacker actor ID (offset 0x210).
    pub last_attacker: i32,
```

- [ ] **Step 2: Add SpellBuff struct**

Add at the top of `ddm.rs` (or in a sub-section):

```rust
use crate::enums::ActorAttributes;

/// MM6 SpellBuff struct (16 bytes). From MMExtension.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpellBuff {
    pub expire_time: i64,
    pub power: i16,
    pub skill: i16,
    pub overlay_id: i16,
    pub caster: u8,
    pub bits: u8,
}
```

- [ ] **Step 3: Update read_actor to populate all fields**

Replace the `read_actor` function body to read ALL MapMonster fields at their correct offsets. Key changes:
- Read `attributes` (u32) at offset 0x24 and convert to `ActorAttributes`
- Read pitch at 0x8C, room at 0x8E, current_action_length at 0x90
- Read carried_item at 0xA4, current_action_step at 0xA8
- Read sound_ids[4] at 0xBC
- Read 14 SpellBuff entries (16 bytes each) at 0xC4
- Read group at 0x1A4, ally at 0x1A8
- Read summoner at 0x20C, last_attacker at 0x210

- [ ] **Step 4: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod`
Expected: all existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add lod/src/ddm.rs
git commit -m "feat(lod): wire all DdmActor fields from MapMonster struct

Adds attributes, pitch, room, carried_item, sound_ids, spell_buffs,
group, ally, summoner, last_attacker. No fields skipped."
```

---

### Task 3: Complete MonsterDesc with stats from MonstersTxt

**Files:**
- Modify: `lod/src/monlist.rs`

The monlist.bin parser reads the visual/audio descriptor correctly. However, the full monster stats (level, HP, AC, resistances, attacks, spells) come from `MonstersTxt` which is loaded from the text table, not from the binary. The binary `dmonlist.bin` is the visual descriptor only (148 bytes/record: height, radius, speed, to_hit_radius, 4 sounds, name, 8 sprite names, 20 bytes skip).

The 20 "skip" bytes at the end of each record are undocumented padding. No action needed on monlist.rs — it's already complete for the binary format.

- [ ] **Step 1: Document the skip bytes**

Add a comment in `parse_record` documenting the 20-byte tail:

```rust
        // Bytes 128-147: 20 bytes of padding/unused data at end of each record.
        // The full monster stats (level, HP, AC, resistances, attacks, spells)
        // come from monstxt.txt, not from this binary file.
```

- [ ] **Step 2: Commit**

```bash
git add lod/src/monlist.rs
git commit -m "docs(lod): document monlist.bin record padding bytes"
```

---

### Task 4: Complete MapStats with all fields

**Files:**
- Modify: `lod/src/mapstats.rs`

Currently only reads monster names, difficulty, and music track. Missing: map name, respawn days, lock, trap, treasure level, encounter chance, encounter counts per slot, steal fine, perception difficulty.

- [ ] **Step 1: Expand MapMonsterConfig to MapInfo**

Rename `MapMonsterConfig` to `MapInfo` and add all fields from OpenEnroth's MapInfo struct:

```rust
pub struct MapInfo {
    /// Display name (e.g. "New Sorpigal").
    pub name: String,
    /// File name (e.g. "oute3.odm").
    pub filename: String,
    /// Monster picture/dmonlist prefix for each of 3 slots.
    pub monster_names: [String; 3],
    /// Difficulty level for each monster.
    pub difficulty: [u8; 3],
    /// Respawn interval in days.
    pub respawn_days: u16,
    /// Base stealing fine (actual fine = 100 * this).
    pub steal_fine: u16,
    /// Perception difficulty (roll needs 2x this).
    pub perception: u16,
    /// Lock difficulty ("x5 Lock" from mapstats.txt).
    pub lock: u8,
    /// Trap damage (this many d20).
    pub trap_d20_count: u8,
    /// Map treasure level (0-6).
    pub treasure_level: u8,
    /// Encounter chance when resting [0, 100].
    pub encounter_chance: u8,
    /// Per-encounter chance weights (should add to 100 or all 0).
    pub encounter_chances: [u8; 3],
    /// Min/max monster count per encounter slot.
    pub encounter_min: [u8; 3],
    pub encounter_max: [u8; 3],
    /// Music track ID (maps to Music/{track}.mp3). 0 = no music.
    pub music_track: u8,
}
```

- [ ] **Step 2: Update the parser to read all columns**

The mapstats.txt column layout (tab-separated, 0-indexed):
- 1: Name
- 2: Filename
- 3: NumResets (skip)
- 4: FirstVisitDay (skip)
- 5: RespawnDays
- 6: AlertDays (skip, MM7+ only)
- 7: StealFine
- 8: Perception
- 9: field_2C (skip)
- 10: Lock
- 11: Trap
- 12: Treasure
- 13: Mon1Pic, 14: Mon1Low, 15: Mon1Dif, 16: Mon1Hi (verify column positions)
- Continue pattern for Mon2, Mon3
- 25: RedbookTrack

Update `parse()` to extract all fields from the correct columns. Use `.trim().parse().unwrap_or(0)` for numeric fields.

- [ ] **Step 3: Update MapStats to use new struct**

Change `pub maps: Vec<(String, MapMonsterConfig)>` to `pub maps: Vec<MapInfo>` and update `get()` to search by filename.

- [ ] **Step 4: Update game code references**

Search for all uses of `MapMonsterConfig` in the `openmm` crate and update to `MapInfo`. The `monster_for_index` method stays on `MapInfo`.

- [ ] **Step 5: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod && cargo build -p openmm`
Expected: all tests pass, game compiles.

- [ ] **Step 6: Commit**

```bash
git add lod/src/mapstats.rs
git commit -m "feat(lod): complete MapStats with all mapstats.txt fields

Adds respawn_days, steal_fine, perception, lock, trap, treasure_level,
encounter_chance, encounter counts. Renamed MapMonsterConfig to MapInfo."
```

---

### Task 5: Wire EVT opcodes enum into evt.rs

**Files:**
- Modify: `lod/src/evt.rs`

Replace the `const OP_*` constants and `opcode_name()` function with the `EvtOpcode` enum from `enums.rs`.

- [ ] **Step 1: Replace opcode constants with enum**

Remove all `const OP_*` lines and the `opcode_name()` function. Update parsing to use `EvtOpcode::from_u8()`. Keep the `Unhandled` variant but use `EvtOpcode::from_u8(opcode).map(|o| o.name()).unwrap_or("Unknown")` for the name.

- [ ] **Step 2: Update GameEvent to use DoorAction enum**

Change `ChangeDoorState { door_id: u8, action: u8 }` to `ChangeDoorState { door_id: u8, action: DoorAction }` and parse accordingly.

- [ ] **Step 3: Store opcode enum in Unhandled variant**

Change `Unhandled { opcode: u8, opcode_name: &'static str, params: Vec<u8> }` to use the proper enum or raw value.

- [ ] **Step 4: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod && cargo build -p openmm`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add lod/src/evt.rs
git commit -m "refactor(lod): use EvtOpcode enum in evt.rs, remove raw constants"
```

---

### Task 6: Wire TileFlags into dtile.rs

**Files:**
- Modify: `lod/src/dtile.rs`

- [ ] **Step 1: Use TileFlags bitflags in Tile struct**

Change `attributes: u16` to `pub attributes: TileFlags` and update the `is_*` methods to use `.contains()`. Add `use crate::enums::TileFlags;`.

- [ ] **Step 2: Make Tile struct public with pub fields**

Remove `#[allow(dead_code)]` and make the Tile struct and relevant fields public so game code can inspect tile properties.

- [ ] **Step 3: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod -- dtile`
Expected: all dtile tests pass.

- [ ] **Step 4: Commit**

```bash
git add lod/src/dtile.rs
git commit -m "refactor(lod): use TileFlags bitflags in dtile.rs"
```

---

### Task 7: Wire FaceAttributes into blv.rs

**Files:**
- Modify: `lod/src/blv.rs`

- [ ] **Step 1: Replace raw attribute constants with FaceAttributes**

Change `pub attributes: u32` to `pub attributes: FaceAttributes` on `BlvFace`. Update `is_portal()`, `is_invisible()`, `is_clickable()`, `moves_by_door()` to use `.contains()`. Remove the `const FACE_ATTR_*` constants.

- [ ] **Step 2: Use PolygonType enum**

Change `pub polygon_type: u8` to use the `PolygonType` enum. Update ceiling detection logic to use `polygon_type.is_ceiling()`.

- [ ] **Step 3: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod -- blv && cargo build -p openmm`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add lod/src/blv.rs
git commit -m "refactor(lod): use FaceAttributes and PolygonType in blv.rs"
```

---

### Task 8: Wire DoorAttributes and complete BLV face extras

**Files:**
- Modify: `lod/src/blv.rs`
- Modify: `lod/src/dlv.rs`

- [ ] **Step 1: Use DoorAttributes on BlvDoor**

Change `pub attributes: u32` on `BlvDoor` to `pub attributes: DoorAttributes`.

- [ ] **Step 2: Parse complete face extras (36 bytes each)**

Currently skips the first 0x14 bytes and last 12 bytes of each face extra. Parse all fields per MMExtension FacetData struct:

```rust
struct FaceExtra {
    z_fade: i32,
    x_fade: i32,
    y_fade: i32,
    facet_index: i16,
    bitmap_index: i16,
    tft_index: i16,      // TextureFrameTable index (animated textures)
    tft_cog_index: i16,
    bitmap_u: i16,       // texture U offset
    bitmap_v: i16,       // texture V offset
    cog_number: i16,
    event_id: u16,       // was already parsed
    cog_trigger_type: i16,
    fade_base_x_index: i16,
    fade_base_y: i16,
    light_level: i16,
}
```

Store the relevant extra fields (tft_index, light_level) on BlvFace.

- [ ] **Step 3: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod -- blv && cargo build -p openmm`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add lod/src/blv.rs lod/src/dlv.rs
git commit -m "feat(lod): complete BLV face extras parsing, wire DoorAttributes"
```

---

### Task 9: Wire ModelFaceAttributes into bsp_model.rs

**Files:**
- Modify: `lod/src/bsp_model.rs`

- [ ] **Step 1: Use ModelFaceAttributes and PolygonType**

Change `pub attributes: u32` on `BSPModelFace` to `pub attributes: ModelFaceAttributes`.
Change `pub polygon_type: u8` to use `PolygonType` enum.
Update any code that checks attributes with raw masks.

- [ ] **Step 2: Complete BSPModelHeader skipped fields**

Currently skips 4+12+16 bytes. Parse them properly per MMExtension MapModel:
- After vertex_count: `convex_facets_count: i16`, pad `i16`
- After faces_count: facets pointer (skip 4), ordering pointer (skip 4), bsp_nodes_count already read, bsp_nodes pointer (skip 4)
- The 16 bytes before position: decoration count (skip 4), grid_x, grid_y

Actually most of these are runtime pointers. Just read and name them properly rather than raw seeks.

- [ ] **Step 3: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod && cargo build -p openmm`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add lod/src/bsp_model.rs
git commit -m "refactor(lod): use ModelFaceAttributes and PolygonType in BSP models"
```

---

### Task 10: Wire SpriteFrameFlags into dsft.rs

**Files:**
- Modify: `lod/src/dsft.rs`

- [ ] **Step 1: Use SpriteFrameFlags**

Change `pub attributes: u16` on `DSFTFrame` to `pub attributes: SpriteFrameFlags`. Update the `is_*` methods to use `.contains()`. The repr(C) raw read still works because bitflags wraps a u16.

- [ ] **Step 2: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod -- dsft`
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add lod/src/dsft.rs
git commit -m "refactor(lod): use SpriteFrameFlags in dsft.rs"
```

---

### Task 11: Wire DecorationDescFlags and SoundAttributes

**Files:**
- Modify: `lod/src/ddeclist.rs`
- Modify: `lod/src/dsounds.rs`

- [ ] **Step 1: Use DecorationDescFlags in ddeclist.rs**

Change `pub attributes: u16` on `DDecListItem` to `pub attributes: DecorationDescFlags`. Update `is_*` methods to use `.contains()`.

- [ ] **Step 2: Use SoundType and SoundAttributes in dsounds.rs**

Add helper methods to `DSoundInfo` that return typed enums:

```rust
pub fn sound_type_enum(&self) -> Option<SoundType> {
    SoundType::from_u32(self.sound_type)
}
```

Don't change the raw field types since the struct uses repr(C) raw read.

- [ ] **Step 3: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod -- ddeclist && cargo test -p lod -- dsounds`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add lod/src/ddeclist.rs lod/src/dsounds.rs
git commit -m "refactor(lod): use DecorationDescFlags and SoundType enums"
```

---

### Task 12: Add dobjlist.bin parser (Object/Projectile List)

**Files:**
- Create: `lod/src/dobjlist.rs`
- Modify: `lod/src/lib.rs`

- [ ] **Step 1: Create dobjlist.rs**

Parse dobjlist.bin (Object/projectile list). MM6 record size = 52 bytes per MMExtension ObjListItem.

```rust
//! Parser for dobjlist.bin — object/projectile visual descriptors.

use std::error::Error;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use bitflags::bitflags;
use crate::{lod_data::LodData, utils::try_read_name, LodManager};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ObjectDescFlags: u16 {
        const INVISIBLE        = 0x0001;
        const UNTOUCHABLE      = 0x0002;
        const TEMPORARY        = 0x0004;
        const LIFETIME_IN_SFT  = 0x0008;
        const NO_PICKUP        = 0x0010;
        const NO_GRAVITY       = 0x0020;
        const INTERCEPT_ACTION = 0x0040;
        const BOUNCE           = 0x0080;
        const TRAIL_PARTICLES  = 0x0100;
        const TRAIL_FIRE       = 0x0200;
        const TRAIL_LINE       = 0x0400;
    }
}

pub struct ObjectDesc {
    pub name: String,
    pub id: i16,
    pub radius: i16,
    pub height: i16,
    pub flags: ObjectDescFlags,
    pub sft_index: i16,
    pub lifetime: i16,
    pub particles_color: u16, // MM6: u16 (MM7+: u32)
    pub speed: u16,
    pub particle_r: u8,
    pub particle_g: u8,
    pub particle_b: u8,
}

pub struct ObjectList {
    pub objects: Vec<ObjectDesc>,
}

impl ObjectList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dobjlist.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut objects = Vec::with_capacity(count);

        for _ in 0..count {
            let mut name_buf = [0u8; 32];
            cursor.read_exact(&mut name_buf)?;
            let name_end = name_buf.iter().position(|&b| b == 0).unwrap_or(32);
            let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();

            let id = cursor.read_i16::<LittleEndian>()?;
            let radius = cursor.read_i16::<LittleEndian>()?;
            let height = cursor.read_i16::<LittleEndian>()?;
            let flags_raw = cursor.read_u16::<LittleEndian>()?;
            let flags = ObjectDescFlags::from_bits_truncate(flags_raw);
            let sft_index = cursor.read_i16::<LittleEndian>()?;
            let lifetime = cursor.read_i16::<LittleEndian>()?;
            let particles_color = cursor.read_u16::<LittleEndian>()?;
            let speed = cursor.read_u16::<LittleEndian>()?;
            let particle_r = cursor.read_u8()?;
            let particle_g = cursor.read_u8()?;
            let particle_b = cursor.read_u8()?;
            let _pad = cursor.read_u8()?; // 1 byte padding to align to 52

            objects.push(ObjectDesc {
                name, id, radius, height, flags, sft_index,
                lifetime, particles_color, speed,
                particle_r, particle_g, particle_b,
            });
        }

        Ok(ObjectList { objects })
    }

    pub fn get_by_id(&self, id: i16) -> Option<&ObjectDesc> {
        self.objects.iter().find(|o| o.id == id)
    }
}
```

- [ ] **Step 2: Register in lib.rs**

Add `pub mod dobjlist;` to `lod/src/lib.rs`.

- [ ] **Step 3: Add test**

```rust
#[cfg(test)]
mod tests {
    use crate::{get_lod_path, LodManager};
    use super::ObjectList;

    #[test]
    fn parse_dobjlist() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let objlist = ObjectList::new(&lod_manager).unwrap();
        assert!(!objlist.objects.is_empty(), "should have objects");
        // First entry should be a known object
        println!("dobjlist: {} entries", objlist.objects.len());
        for obj in objlist.objects.iter().take(5) {
            println!("  {} id={} radius={} height={}", obj.name, obj.id, obj.radius, obj.height);
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod -- dobjlist`
Expected: test passes.

- [ ] **Step 5: Commit**

```bash
git add lod/src/dobjlist.rs lod/src/lib.rs
git commit -m "feat(lod): add dobjlist.bin parser (object/projectile list)"
```

---

### Task 13: Add dchest.bin parser

**Files:**
- Create: `lod/src/dchest.rs`
- Modify: `lod/src/lib.rs`

- [ ] **Step 1: Create dchest.rs**

Parse dchest.bin (chest visual descriptors). 36 bytes per record.

```rust
//! Parser for dchest.bin — chest visual descriptors.

use std::error::Error;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::{lod_data::LodData, LodManager};

pub struct ChestDesc {
    pub name: String,
    pub width: u8,
    pub height: u8,
    pub image_index: i16,
}

pub struct ChestList {
    pub chests: Vec<ChestDesc>,
}

impl ChestList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dchest.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut chests = Vec::with_capacity(count);

        for _ in 0..count {
            let mut name_buf = [0u8; 32];
            cursor.read_exact(&mut name_buf)?;
            let name_end = name_buf.iter().position(|&b| b == 0).unwrap_or(32);
            let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();
            let width = cursor.read_u8()?;
            let height = cursor.read_u8()?;
            let image_index = cursor.read_i16::<LittleEndian>()?;

            chests.push(ChestDesc { name, width, height, image_index });
        }

        Ok(ChestList { chests })
    }
}
```

- [ ] **Step 2: Register in lib.rs and add test**

- [ ] **Step 3: Commit**

```bash
git add lod/src/dchest.rs lod/src/lib.rs
git commit -m "feat(lod): add dchest.bin parser (chest visual descriptors)"
```

---

### Task 14: Add overlay.bin and TFT parsers

**Files:**
- Create: `lod/src/doverlay.rs`
- Create: `lod/src/tft.rs`
- Modify: `lod/src/lib.rs`

- [ ] **Step 1: Create doverlay.rs**

Parse doverlay.bin (spell overlay list). 8 bytes per record:

```rust
//! Parser for doverlay.bin — spell/buff overlay descriptors.

use std::error::Error;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use crate::{lod_data::LodData, LodManager};

pub struct OverlayDesc {
    pub id: i16,
    pub overlay_type: i16,
    pub sft_index: i16,
}

pub struct OverlayList {
    pub overlays: Vec<OverlayDesc>,
}

impl OverlayList {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/doverlay.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut overlays = Vec::with_capacity(count);

        for _ in 0..count {
            let id = cursor.read_i16::<LittleEndian>()?;
            let overlay_type = cursor.read_i16::<LittleEndian>()?;
            let sft_index = cursor.read_i16::<LittleEndian>()?;
            let _pad = cursor.read_i16::<LittleEndian>()?;
            overlays.push(OverlayDesc { id, overlay_type, sft_index });
        }

        Ok(OverlayList { overlays })
    }
}
```

- [ ] **Step 2: Create tft.rs**

Parse TFT (Texture Frame Table) for animated textures. From MMExtension TFTItem: 20 bytes per entry.

```rust
//! Parser for dtft.bin — texture frame table (animated textures).

use std::error::Error;
use std::io::{Cursor, Read};
use byteorder::{LittleEndian, ReadBytesExt};
use bitflags::bitflags;
use crate::{lod_data::LodData, LodManager};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TFTFlags: u16 {
        const NOT_GROUP_END = 0x0001;
        const GROUP_START   = 0x0002;
    }
}

pub struct TFTEntry {
    pub name: String,
    pub index: i16,
    pub time: i16,          // 1/16 second
    pub total_time: i16,
    pub flags: TFTFlags,
}

pub struct TextureFrameTable {
    pub entries: Vec<TFTEntry>,
}

impl TextureFrameTable {
    pub fn new(lod_manager: &LodManager) -> Result<Self, Box<dyn Error>> {
        let raw = lod_manager.try_get_bytes("icons/dtft.bin")?;
        let data = LodData::try_from(raw)?;
        Self::parse(&data.data)
    }

    fn parse(data: &[u8]) -> Result<Self, Box<dyn Error>> {
        let mut cursor = Cursor::new(data);
        let count = cursor.read_u32::<LittleEndian>()? as usize;
        let mut entries = Vec::with_capacity(count);

        for _ in 0..count {
            let mut name_buf = [0u8; 12];
            cursor.read_exact(&mut name_buf)?;
            let name_end = name_buf.iter().position(|&b| b == 0).unwrap_or(12);
            let name = String::from_utf8_lossy(&name_buf[..name_end]).to_string();
            let index = cursor.read_i16::<LittleEndian>()?;
            let time = cursor.read_i16::<LittleEndian>()?;
            let total_time = cursor.read_i16::<LittleEndian>()?;
            let flags = TFTFlags::from_bits_truncate(cursor.read_u16::<LittleEndian>()?);

            entries.push(TFTEntry { name, index, time, total_time, flags });
        }

        Ok(TextureFrameTable { entries })
    }

    /// Find the animation group for a texture name.
    /// Returns all frames in the group (from GROUP_START to the frame without NOT_GROUP_END).
    pub fn find_group(&self, texture_name: &str) -> Option<&[TFTEntry]> {
        let start = self.entries.iter().position(|e| {
            e.name.eq_ignore_ascii_case(texture_name) && e.flags.contains(TFTFlags::GROUP_START)
        })?;
        let end = self.entries[start..].iter().position(|e| {
            !e.flags.contains(TFTFlags::NOT_GROUP_END)
        })? + start + 1;
        Some(&self.entries[start..end])
    }
}
```

- [ ] **Step 3: Register both in lib.rs and add tests**

- [ ] **Step 4: Commit**

```bash
git add lod/src/doverlay.rs lod/src/tft.rs lod/src/lib.rs
git commit -m "feat(lod): add overlay.bin and TFT (animated texture) parsers"
```

---

### Task 15: Add BLV decoration complete fields and ODM billboard flags

**Files:**
- Modify: `lod/src/blv.rs`
- Modify: `lod/src/billboard.rs`

- [ ] **Step 1: Complete BlvDecoration fields**

Currently skips 12 bytes at the end of each decoration. Parse them:

```rust
pub struct BlvDecoration {
    pub decoration_desc_id: u16,
    pub flags: LevelDecorationFlags,  // was raw u16
    pub position: [i32; 3],
    pub yaw: i32,
    pub cog_number: i16,     // NEW
    pub event_id: u16,       // NEW
    pub trigger_radius: i16, // NEW
    pub field_1a: i16,       // NEW (unknown)
    pub event_var_id: i16,   // NEW
    pub field_1e: i16,       // NEW (unknown)
    pub name: String,
}
```

- [ ] **Step 2: Use LevelDecorationFlags in BillboardData**

In `billboard.rs`, the `attributes` field on `BillboardData` should use `LevelDecorationFlags` type.

- [ ] **Step 3: Run tests**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod && cargo build -p openmm`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add lod/src/blv.rs lod/src/billboard.rs
git commit -m "feat(lod): complete BLV decoration fields, use LevelDecorationFlags"
```

---

### Task 16: Final integration test and cleanup

**Files:**
- Modify: `lod/src/lib.rs` (if needed)

- [ ] **Step 1: Run full test suite**

Run: `cd /home/roarc/repos/openmm && cargo test -p lod`
Expected: all tests pass.

- [ ] **Step 2: Run clippy**

Run: `cd /home/roarc/repos/openmm && cargo clippy -p lod`
Expected: no warnings.

- [ ] **Step 3: Build full game**

Run: `cd /home/roarc/repos/openmm && cargo build -p openmm`
Expected: compiles successfully.

- [ ] **Step 4: Commit any fixups**

```bash
git add -A
git commit -m "fix(lod): address clippy warnings and fix integration issues"
```
