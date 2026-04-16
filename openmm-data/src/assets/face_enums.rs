//! BLV/BSP face attribute flags and polygon type classification.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

// ── BLV Face Attributes ──────────────────────────────────────────────────

bitflags! {
    /// BLV indoor face attribute flags (u32).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

// ── Polygon Type ─────────────────────────────────────────────────────────

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
