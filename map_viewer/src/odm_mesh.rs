use bevy::{prelude::*, render::render_resource::PrimitiveTopology};
use lod::{
    dtile::TileTable,
    odm::{Odm, ODM_MAP_HEIGHT_SIZE, ODM_MAP_SIZE, ODM_MAP_TILE_SIZE},
};

const HEIGHT_SCALE: f32 = ODM_MAP_HEIGHT_SIZE as f32;
const TILE_SCALE: f32 = ODM_MAP_TILE_SIZE as f32;

// It needs a refactor but for now I want to see the terrain render properly.
pub fn odm_to_mesh(
    odm: &Odm,
    primitive_topology: PrimitiveTopology,
    tile_table: &TileTable,
) -> Mesh {
    let width: usize = ODM_MAP_SIZE;
    let depth: usize = ODM_MAP_SIZE;
    let (width_u32, depth_u32) = (width as u32, depth as u32);
    let (width_f32, depth_f32) = (width as f32, depth as f32);

    let vertices_count: usize = width * depth;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertices_count);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vertices_count);

    for d in 0..depth {
        for w in 0..width {
            let i = d * width + w;
            let pos = [
                (w as f32 - width_f32 / 2.) * TILE_SCALE,
                HEIGHT_SCALE * odm.height_map[i] as f32,
                (d as f32 - depth_f32 / 2.) * TILE_SCALE,
            ];
            positions.push(pos);
            uvs.push(calculate_uv_coordinate(w, d, tile_table, odm.tile_map[i]).into());
        }
    }

    // Defining triangles indices.
    let indices_count: usize = (width - 1) * (depth - 1) * 6;
    let mut indices: Vec<u32> = Vec::with_capacity(indices_count);
    for d in 0..(depth_u32 - 1) {
        for w in 0..(width_u32 - 1) {
            let i = (d * width_u32) + w;
            // First triangle
            indices.push(i);
            indices.push(i + width_u32);
            indices.push(i + 1);
            // Second triangle
            indices.push(i + 1);
            indices.push(i + width_u32);
            indices.push(i + width_u32 + 1);
        }
    }

    /// debug
    use std::fs::write;
    let _ = write(format!("positions.txt"), format!("{:?}", &positions));
    let _ = write(format!("uvs.txt"), print_chunks(uvs.as_slice()));
    let _ = write(
        format!("dtile.txt"),
        format!("{}", print_table(tile_table, odm.tile_map)),
    );
    let _ = write(format!("tile_map.txt"), print_square_map(odm.tile_map));

    let mut mesh = Mesh::new(primitive_topology);
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);

    if primitive_topology == PrimitiveTopology::TriangleList {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

        mesh.duplicate_vertices();
        mesh.compute_flat_normals();
        mesh.compute_aabb();
    }

    mesh
}

fn calculate_uv_coordinate(col: usize, row: usize, tile_table: &TileTable, idx: u8) -> (f32, f32) {
    let (atlas_x, atlas_y) = tile_table.coordinate(idx);
    let (atlas_x, atlas_y) = (atlas_x as f32, atlas_y as f32);
    let (atlas_size_x, atlas_size_y) = tile_table.size();
    let (atlas_size_x, atlas_size_y) = (atlas_size_x as f32, atlas_size_y as f32);

    let block_w_start = atlas_x / atlas_size_x;
    let block_w_end = (atlas_x + 1.0) / atlas_size_x;
    let block_h_start = atlas_y / atlas_size_y;
    let block_h_end = (atlas_y + 1.0) / atlas_size_y;

    let even_col = col % 2 == 0;
    let even_row = row % 2 == 0;
    match (even_col, even_row) {
        (true, true) => (block_w_start, block_h_start),
        (false, true) => (block_w_end, block_h_start),
        (true, false) => (block_w_start, block_h_end),
        (false, false) => (block_w_end, block_h_end),
    }
}

fn print_table(table: &TileTable, data: [u8; 16384]) -> String {
    let mut output = String::new();
    output.push_str(&format!("indexes in the table: \n"));

    for i in 0..=255 {
        output.push_str(&format!(
            "{:03} | {:?} -> {}\n",
            i,
            table.coordinate(i),
            table.name(i)
        ));
    }

    output.push_str(&format!("\nindexes that the map uses: \n"));

    let mut uniq = data.clone().to_vec();
    uniq.sort();
    uniq.dedup();
    for i in uniq {
        output.push_str(&format!("{:03} -> {}\n", i, table.name(i)));
    }

    output
}

fn print_chunks(data: &[[f32; 2]]) -> String {
    let mut output = String::new();
    for chunk in data.chunks(4) {
        for &element in chunk {
            output.push_str(&format!("{:?} ", element));
        }
        output.push('\n');
    }
    output
}

fn print_square_map(data: [u8; 16384]) -> String {
    let mut output = String::new();
    for chunk in data.chunks(128) {
        for &element in chunk {
            output.push_str(&format!("{:03} ", element));
        }
        output.push('\n');
    }

    output
}
