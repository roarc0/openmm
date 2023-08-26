use bevy::{prelude::*, render::render_resource::PrimitiveTopology};
use lod::{
    dtile::TileTable,
    odm::{Odm, ODM_MAP_HEIGHT_SIZE, ODM_MAP_TILE_SIZE},
};

const HEIGHT_SCALE: f32 = ODM_MAP_HEIGHT_SIZE as f32;
const TILE_SCALE: f32 = ODM_MAP_TILE_SIZE as f32;

pub fn odm_to_mesh(
    odm: &Odm,
    tile_table: &TileTable,
    primitive_topology: PrimitiveTopology,
) -> Mesh {
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
                (w as f32 - width_half) * TILE_SCALE,
                odm.height_map[i] as f32 * HEIGHT_SCALE,
                (d as f32 - depth_half) * TILE_SCALE,
            ]);
            if w < (depth - 1) && d < (depth - 1) {
                push_uvs(&mut uvs, tile_table, odm.tile_map[i]);
                push_triangle_indices(&mut indices, i as u32, width_u32);
            }
        }
    }

    let mut mesh = Mesh::new(primitive_topology);
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);

    if primitive_topology == PrimitiveTopology::TriangleList {
        mesh.duplicate_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.compute_flat_normals();
        mesh.compute_aabb();
    }

    mesh
}

fn push_uvs(uvs: &mut Vec<[f32; 2]>, tile_table: &TileTable, tile_index: u8) {
    let (tile_x, tile_y) = tile_table.coordinate(tile_index);
    let (tile_x, tile_y) = (tile_x as f32, tile_y as f32);
    let (tile_table_size_x, tile_table_size_y) = tile_table.size();
    let (tile_table_size_x, tile_table_size_y) =
        (tile_table_size_x as f32, tile_table_size_y as f32);

    let uv_scale = 1.;
    let w_start = (tile_x / tile_table_size_x) * uv_scale;
    let w_end = ((tile_x + 1.0) / tile_table_size_x) * uv_scale;
    let h_start = (tile_y / tile_table_size_y) * uv_scale;
    let h_end = ((tile_y + 1.0) / tile_table_size_y) * uv_scale;

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
    indices.push(i + width_u32);
    indices.push(i + 1);
    // Second triangle
    indices.push(i + 1);
    indices.push(i + width_u32);
    indices.push(i + width_u32 + 1);
}
