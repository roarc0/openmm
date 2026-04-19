//! Shared spawning helpers for monsters and decorations.
//!
//! Both indoor (BLV) and outdoor (ODM) maps funnel through these functions
//! so entity construction stays consistent — same components, same light
//! attachment strategy, same naming.

pub mod actor;
pub mod decoration;

use bevy::prelude::*;

/// Cylindrical collision obstacle used for player-vs-world XZ pushout.
///
/// Attached to actors and blocking decorations. The player movement system
/// queries all `WorldObstacle` entities and pushes the player out of any
/// cylinder it overlaps (circle-circle rejection in XZ, with a Y height
/// guard to skip obstacles on distant floors/platforms).
#[derive(Component, Clone, Copy)]
pub struct WorldObstacle {
    /// Horizontal collision radius in world units.
    pub radius: f32,
}

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
