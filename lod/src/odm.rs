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
    pub indices: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
}

impl OdmData {
    pub fn new(odm: &Odm, tile_table: &TileTable) -> Self {
        let (width, depth) = odm.size();
        let width_u32 = width as u32;
        let (width_half, depth_half) = (width as f32 / 2., depth as f32 / 2.);

        let vertices_count: usize = width * depth;
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertices_count);
        let indices_count: usize = (width - 1) * (depth - 1) * 6;
        let mut indices: Vec<u32> = Vec::with_capacity(indices_count);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(indices_count); // vertices will be duplicated so we have as much as the indices

        for d in 0..depth {
            for w in 0..width {
                let i = d * width + w;
                positions.push([
                    (w as f32 - width_half) * ODM_TILE_SCALE,
                    odm.height_map[i] as f32 * ODM_HEIGHT_SCALE,
                    (d as f32 - depth_half) * ODM_TILE_SCALE,
                ]);
                if w < (depth - 1) && d < (depth - 1) {
                    Self::push_uvs(&mut uvs, tile_table, odm.tile_map[i]);
                    Self::push_triangle_indices(&mut indices, i as u32, width_u32);
                }
            }
        }

        Self {
            positions,
            indices,
            uvs,
        }
    }

    fn push_uvs(uvs: &mut Vec<[f32; 2]>, tile_table: &TileTable, tile_index: u8) {
        let (tile_x, tile_y) = tile_table.coordinate(tile_index);
        let (tile_x, tile_y) = (tile_x as f32, tile_y as f32);
        let (tile_table_size_x, tile_table_size_y) = tile_table.size();
        let (tile_table_size_x, tile_table_size_y) =
            (tile_table_size_x as f32, tile_table_size_y as f32);

        let uv_scale: f32 = 1.0;
        let w_start = (tile_x / tile_table_size_x) / uv_scale;
        let w_end = ((tile_x + 1.0) / tile_table_size_x) / uv_scale;
        let h_start = (tile_y / tile_table_size_y) / uv_scale;
        let h_end = ((tile_y + 1.0) / tile_table_size_y) / uv_scale;

        uvs.push([w_start, h_start]);
        uvs.push([w_start, h_end]);
        uvs.push([w_end, h_start]);
        uvs.push([w_end, h_start]);
        uvs.push([w_start, h_end]);
        uvs.push([w_end, h_end]);
    }

    fn push_triangle_indices(indices: &mut Vec<u32>, i: u32, width_u32: u32) {
        // First triangle
        indices.push(i);
        indices.push(i + (width_u32));
        indices.push(i + 1);
        // Second triangle
        indices.push(i + 1);
        indices.push(i + width_u32);
        indices.push(i + width_u32 + 1);

        // // First triangle (in CW order)
        // indices.push(i);
        // indices.push(i + 1);
        // indices.push(i + (width_u32));

        // // Second triangle (in CW order)
        // indices.push(i + 1);
        // indices.push(i + width_u32 + 1);
        // indices.push(i + width_u32);
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
