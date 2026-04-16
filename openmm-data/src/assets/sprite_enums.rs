//! Sprite frame attribute flags (luminous, centering, grouping).

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

// ── SFT Frame Attributes ─────────────────────────────────────────────────

bitflags! {
    /// Sprite frame table frame attribute flags (u16).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
