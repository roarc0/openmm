//! Enum re-exports — domain-specific enums live in dedicated files,
//! re-exported here so `use crate::assets::enums::*` keeps working.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

// ── Domain modules (re-exported) ─────────────────────────────────────────

pub use super::actor_enums::*;
pub use super::door_enums::*;
pub use super::event_enums::*;
pub use super::face_enums::*;
pub use super::sprite_enums::*;

// ── Tile Flags ───────────────────────────────────────────────────────────

bitflags! {
    /// Tile attribute flags from dtile.bin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

pub use super::sound_enums::*;

#[cfg(test)]
#[path = "enums_tests.rs"]
mod tests;
