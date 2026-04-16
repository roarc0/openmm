//! Actor AI state, animation, sound slot, and attribute enums.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

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

// ── Actor Sound Slots ────────────────────────────────────────────────────

/// Actor sound effect indices (recorded as u16[4] in MapMonster).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum ActorSoundSlot {
    Attack = 0,
    Die = 1,
    GotHit = 2,
    Fidget = 3,
}

// ── Actor Attributes (bitflags) ──────────────────────────────────────────

bitflags! {
    /// MapMonster.Bits flags (u32 at offset 0x24).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
