//! Terrain type queries for outdoor maps.

use crate::dtile::{Dtile, Tileset};
use crate::odm::{ODM_SIZE, ODM_TILE_SCALE, Odm};

/// Precomputed terrain lookup for a specific map.
/// Built once from Dtile + Odm tile_data, maps tile_map values (0-255) to Tileset.
pub struct TerrainLookup {
    tilesets: Vec<i16>, // 256 entries: tile_map value -> tile_set
}

impl TerrainLookup {
    /// Empty lookup (all dirt). Used as fallback.
    pub fn empty() -> Self {
        Self { tilesets: vec![4; 256] } // 4 = dirt in MM6 dtile.bin
    }

    /// Build from dtile and the per-map tile_data offsets.
    pub fn new(dtile: &Dtile, tile_data: [u16; 8]) -> Self {
        Self {
            tilesets: dtile.tileset_lookup(tile_data),
        }
    }

    /// Get the terrain tileset at a Bevy world position.
    /// The terrain grid is centered: world origin is at grid center (64, 64).
    pub fn tileset_at(&self, odm: &Odm, x: f32, z: f32) -> Option<Tileset> {
        let half = ODM_SIZE as f32 / 2.0;
        let col = (x / ODM_TILE_SCALE + half) as i32;
        let row = (z / ODM_TILE_SCALE + half) as i32;

        if col < 0 || row < 0 || col >= ODM_SIZE as i32 || row >= ODM_SIZE as i32 {
            return None;
        }

        let idx = row as usize * ODM_SIZE + col as usize;
        let tile_id = *odm.tile_map.get(idx)? as usize;
        let tile_set = *self.tilesets.get(tile_id)?;
        Tileset::from_raw(tile_set)
    }
}
