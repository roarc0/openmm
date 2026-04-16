//! Sound-domain enums: sound type classification and attribute flags.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct SoundAttributes: u32 {
        const LOCKED = 0x01;
        const IS_3D  = 0x02;
    }
}
