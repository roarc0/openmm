//! Door attribute flags and action types for BLV indoor maps.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

// ── Door Attributes ──────────────────────────────────────────────────────

bitflags! {
    /// BLV door attribute flags (u32).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct DoorAttributes: u32 {
        const START_STATE_2    = 0x01;
        const SILENT_MOVE      = 0x02;
        const NO_SOUND         = 0x04;
        const STOPPED          = 0x08;
    }
}

// ── Door Action ──────────────────────────────────────────────────────────

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
