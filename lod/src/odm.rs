use std::{
    error::Error,
    io::{Cursor, Read, Seek},
};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::{
    billboard::{read_billboards, Billboard},
    bsp_model::{read_bsp_models, BSPModel},
    dtile::{Dtile, TileTable},
    lod_data::LodData,
    utils::try_read_string_block,
    LodManager,
};

pub const ODM_SIZE: usize = 128;
pub const ODM_PLAY_SIZE: usize = 88;
pub const ODM_AREA: usize = ODM_SIZE * ODM_SIZE;

pub const ODM_TILE_SCALE: f32 = 512.;
pub const ODM_HEIGHT_SCALE: f32 = 32.;

const HEIGHT_MAP_OFFSET: u64 = 176;
const HEIGHT_MAP_SIZE: usize = ODM_AREA;

const TILE_MAP_OFFSET: u64 = HEIGHT_MAP_OFFSET + HEIGHT_MAP_SIZE as u64;
const TILEMAP_SIZE: usize = ODM_AREA;

const ATTRIBUTE_MAP_OFFSET: u64 = TILE_MAP_OFFSET + ATTRIBUTE_MAP_SIZE as u64;
const ATTRIBUTE_MAP_SIZE: usize = ODM_AREA;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Odm {
    pub name: String,
    pub odm_version: String,
    pub sky_texture: String,
    pub ground_texture: String,
    pub tile_data: [u16; 8],
    pub height_map: [u8; HEIGHT_MAP_SIZE],
    pub tile_map: [u8; TILEMAP_SIZE],
    pub attribute_map: [u8; ATTRIBUTE_MAP_SIZE],
    pub bsp_models: Vec<BSPModel>,
    pub billboards: Vec<Billboard>,
    pub spawn_points: Vec<SpawnPoint>,
}

/// A spawn point for monsters/NPCs/items in the map.
#[derive(Debug)]
pub struct SpawnPoint {
    /// Position in MM6 coordinates (x, y, z).
    pub position: [i32; 3],
    /// Wander radius around spawn position.
    pub radius: u16,
    /// 1 = monster, 2 = item/treasure.
    pub spawn_type: u16,
    /// Monster index or treasure level.
    pub monster_index: u16,
    pub attributes: u16,
}

impl Odm {
    pub fn new(lod_manager: &LodManager, name: &str) -> Result<Self, Box<dyn Error>> {
        let data = LodData::try_from(lod_manager.try_get_bytes(&format!("games/{}", name))?)?;
        let data = data.data.as_slice();

        let mut cursor = Cursor::new(data);
        cursor.seek(std::io::SeekFrom::Start(2 * 32))?;
        let odm_version = try_read_string_block(&mut cursor, 32)?;
        let sky_texture = try_read_string_block(&mut cursor, 32)?;
        let ground_texture = try_read_string_block(&mut cursor, 32)?;
        let tile_data: [u16; 8] = [
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
            cursor.read_u16::<LittleEndian>()?,
        ];

        cursor.seek(std::io::SeekFrom::Start(HEIGHT_MAP_OFFSET))?;
        let mut height_map: [u8; HEIGHT_MAP_SIZE] = [0; HEIGHT_MAP_SIZE];
        cursor.read_exact(&mut height_map)?;

        cursor.seek(std::io::SeekFrom::Start(TILE_MAP_OFFSET))?;
        let mut tile_map: [u8; TILEMAP_SIZE] = [0; TILEMAP_SIZE];
        cursor.read_exact(&mut tile_map)?;

        cursor.seek(std::io::SeekFrom::Start(ATTRIBUTE_MAP_OFFSET))?;
        let mut attribute_map: [u8; ATTRIBUTE_MAP_SIZE] = [0; ATTRIBUTE_MAP_SIZE];
        cursor.read_exact(&mut attribute_map)?;

        let bsp_model_count = cursor.read_u32::<LittleEndian>()? as usize;
        let bsp_models: Vec<BSPModel> = read_bsp_models(&mut cursor, bsp_model_count)?;

        let billboard_count = cursor.read_u32::<LittleEndian>()? as usize;
        let billboards: Vec<Billboard> = read_billboards(&mut cursor, billboard_count)?;

        // Spawn points / actors are in the DDM delta file, not the ODM.
        // For now, spawn_points is empty — will be loaded from DDM later.
        let spawn_points = Vec::new();

        Ok(Self {
            name: "test".into(),
            odm_version,
            sky_texture,
            ground_texture,
            tile_data,
            height_map,
            tile_map,
            attribute_map,
            bsp_models,
            billboards,
            spawn_points,
        })
    }
}

impl Odm {
    pub fn size(&self) -> (usize, usize) {
        (ODM_SIZE, ODM_SIZE)
    }

    pub fn tile_table(&self, lod_manager: &LodManager) -> Result<TileTable, Box<dyn Error>> {
        Dtile::new(lod_manager)?
            .table(self.tile_data)
            .ok_or("could not get the tile table".into())
    }
}

pub struct OdmData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
}

impl OdmData {
    pub fn new(odm: &Odm, tile_table: &TileTable) -> Self {
        let (width, depth) = odm.size();
        let width_u32 = width as u32;
        let (width_half, depth_half) = (width as f32 / 2., depth as f32 / 2.);

        // Build vertex positions on the heightmap grid
        let vertices_count: usize = width * depth;
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertices_count);

        for d in 0..depth {
            for w in 0..width {
                let i = d * width + w;
                positions.push([
                    (w as f32 - width_half) * ODM_TILE_SCALE,
                    odm.height_map[i] as f32 * ODM_HEIGHT_SCALE,
                    (d as f32 - depth_half) * ODM_TILE_SCALE,
                ]);
            }
        }

        // Compute smooth normals from the heightmap using central differences
        let normals = Self::compute_smooth_normals(&odm.height_map[..], width, depth);

        // Build indices with adaptive triangulation and matching UVs
        let quad_count = (width - 1) * (depth - 1);
        let mut indices: Vec<u32> = Vec::with_capacity(quad_count * 6);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(quad_count * 6);

        for d in 0..(depth - 1) {
            for w in 0..(width - 1) {
                let i = (d * width + w) as u32;
                let tl = i;                    // top-left
                let tr = i + 1;                // top-right
                let bl = i + width_u32;        // bottom-left
                let br = i + width_u32 + 1;    // bottom-right

                // Adaptive diagonal: choose the split that minimizes height difference
                let h_tl = positions[tl as usize][1];
                let h_tr = positions[tr as usize][1];
                let h_bl = positions[bl as usize][1];
                let h_br = positions[br as usize][1];

                let diag1 = (h_tl - h_br).abs(); // TL-BR diagonal
                let diag2 = (h_tr - h_bl).abs(); // TR-BL diagonal

                let tile_index = d * width + w;
                let (uv_tl, uv_tr, uv_bl, uv_br) =
                    Self::tile_uvs(tile_table, odm.tile_map[tile_index]);

                if diag1 <= diag2 {
                    // Split along TL-BR: triangles (TL,BL,BR) and (TL,BR,TR)
                    indices.extend_from_slice(&[tl, bl, br, tl, br, tr]);
                    uvs.extend_from_slice(&[uv_tl, uv_bl, uv_br, uv_tl, uv_br, uv_tr]);
                } else {
                    // Split along TR-BL: triangles (TL,BL,TR) and (TR,BL,BR)
                    indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
                    uvs.extend_from_slice(&[uv_tl, uv_bl, uv_tr, uv_tr, uv_bl, uv_br]);
                }
            }
        }

        Self {
            positions,
            normals,
            indices,
            uvs,
        }
    }

    /// Compute smooth per-vertex normals using central differences on the heightmap.
    ///
    /// Terrain vertex positions are: (x, y, z) = ((w-64)*512, height*32, (d-64)*512)
    /// where Y is up (Bevy convention).
    ///
    /// Tangent along w (x-axis): T_w = (tile_scale, dh_dw, 0)
    /// Tangent along d (z-axis): T_d = (0, dh_dd, tile_scale)
    /// Normal = T_d × T_w = (-dh_dw * tile_scale, tile_scale², -dh_dd * tile_scale)
    /// Simplified: (-dh_dw, tile_scale, -dh_dd) then normalize
    fn compute_smooth_normals(
        height_map: &[u8],
        width: usize,
        depth: usize,
    ) -> Vec<[f32; 3]> {
        let mut normals = Vec::with_capacity(width * depth);
        for d in 0..depth {
            for w in 0..width {
                let left = if w > 0 { w - 1 } else { 0 };
                let right = if w < width - 1 { w + 1 } else { width - 1 };
                let up = if d > 0 { d - 1 } else { 0 };
                let down = if d < depth - 1 { d + 1 } else { depth - 1 };

                let h_left = height_map[d * width + left] as f32 * ODM_HEIGHT_SCALE;
                let h_right = height_map[d * width + right] as f32 * ODM_HEIGHT_SCALE;
                let h_up = height_map[up * width + w] as f32 * ODM_HEIGHT_SCALE;
                let h_down = height_map[down * width + w] as f32 * ODM_HEIGHT_SCALE;

                // Height gradients (central differences)
                let dh_dw = (h_right - h_left) / ((right - left) as f32);
                let dh_dd = (h_down - h_up) / ((down - up) as f32);

                // Normal from cross product T_d × T_w (Y-up convention)
                let nx = -dh_dw;
                let ny = ODM_TILE_SCALE;
                let nz = -dh_dd;
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                normals.push([nx / len, ny / len, nz / len]);
            }
        }
        normals
    }

    fn tile_uvs(
        tile_table: &TileTable,
        tile_index: u8,
    ) -> ([f32; 2], [f32; 2], [f32; 2], [f32; 2]) {
        let (tile_x, tile_y) = tile_table.coordinate(tile_index);
        let (tile_x, tile_y) = (tile_x as f32, tile_y as f32);
        let (size_x, size_y) = tile_table.size();
        let (size_x, size_y) = (size_x as f32, size_y as f32);

        let u0 = tile_x / size_x;
        let u1 = (tile_x + 1.0) / size_x;
        let v0 = tile_y / size_y;
        let v1 = (tile_y + 1.0) / size_y;

        // TL, TR, BL, BR
        ([u0, v0], [u1, v0], [u0, v1], [u1, v1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_lod_path, LodManager};

    #[test]
    fn get_map_works() {
        let lod_manager = LodManager::new(get_lod_path()).unwrap();
        let map = Odm::new(&lod_manager, "oute3.odm").unwrap();
        assert_eq!(map.bsp_models.len(), 85)
    }
}
