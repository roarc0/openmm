//! Terrain type queries for outdoor maps.

use crate::dtile::Tileset;
use crate::odm::{Odm, ODM_SIZE, ODM_TILE_SCALE};

/// Get the terrain tileset at a Bevy world position.
/// Uses tile_data even indices (tileset IDs) and tile_map value ranges.
pub fn tileset_at(odm: &Odm, x: f32, z: f32) -> Option<Tileset> {
    let col = (x / ODM_TILE_SCALE) as i32;
    let row = (-z / ODM_TILE_SCALE) as i32;

    if col < 0 || row < 0 || col >= ODM_SIZE as i32 || row >= ODM_SIZE as i32 {
        return None;
    }

    let idx = row as usize * ODM_SIZE + col as usize;
    let tile_id = *odm.tile_map.get(idx)? as u16;

    // tile_data layout: [dirt_tileset, dirt_start, water_tileset, water_start,
    //                    secondary_tileset, secondary_start, road_tileset, road_start]
    // tile_map ranges: 0-89=dirt, 90-125=primary(=dirt), 126-161=water, 162-197=secondary, 198+=roads
    let tileset_id = match tile_id {
        0..=125 => odm.tile_data[0],    // dirt and primary terrain
        126..=161 => odm.tile_data[2],  // water
        162..=197 => odm.tile_data[4],  // secondary terrain
        198.. => odm.tile_data[6],      // roads
    };

    Tileset::from_raw(tileset_id as i16)
}
