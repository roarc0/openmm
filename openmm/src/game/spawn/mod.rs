//! Shared spawning helpers for monsters and decorations.
//!
//! Both indoor (BLV) and outdoor (ODM) maps funnel through these functions
//! so entity construction stays consistent — same components, same light
//! attachment strategy, same naming.

pub mod decoration;
pub mod monster;

use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::game::sprites::loading::SpriteCache;
use crate::game::sprites::material::SpriteMaterial;

/// Common context for sprite-based entity spawning.
///
/// Holds mutable refs to asset stores and config flags. Callers construct
/// this from whatever system params they have (Bevy system args, or manual
/// refs during sync indoor spawn).
pub struct SpawnCtx<'a> {
    pub game_assets: &'a GameAssets,
    pub images: &'a mut Assets<Image>,
    pub meshes: &'a mut Assets<Mesh>,
    pub sprite_materials: &'a mut Assets<SpriteMaterial>,
    pub sprite_cache: &'a mut SpriteCache,
    pub shadows: bool,
    pub billboard_shadows: bool,
    pub actor_shadows: bool,
}
