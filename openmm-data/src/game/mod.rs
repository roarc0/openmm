//! Backward-compatibility re-exports. All types now live in `assets::provider`.
//!
//! External paths like `openmm_data::game::monster::Monsters` remain valid.

pub use crate::assets::font;
pub use crate::assets::npc;
pub use crate::assets::provider::actors;
pub use crate::assets::provider::decorations;
pub use crate::assets::provider::monster;

/// Backward-compatible alias. New code should use `Assets::game()` which returns `LodDecoder`.
pub use crate::assets::provider::lod_decoder::LodDecoder as GameLod;
