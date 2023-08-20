use bevy::{prelude::*, render::render_resource::PrimitiveTopology};
use lod::{
    dtile::DtileTable,
    odm::{Odm, ODM_MAP_HEIGHT_SIZE, ODM_MAP_SIZE, ODM_MAP_TILE_SIZE},
};
use nalgebra::Vector3;

const HEIGHT_SCALE: f32 = ODM_MAP_HEIGHT_SIZE as f32;
const TILE_SCALE: f32 = ODM_MAP_TILE_SIZE as f32;

// It needs a refactor but for now I want to see the terrain render properly.
pub fn odm_to_mesh(
    odm: &Odm,
    primitive_topology: PrimitiveTopology,
    dtile_table: &DtileTable,
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
            uvs.push(calculate_uv_coordinate(w, d, dtile_table, odm.tile_map[i]).into());
        }
    }

    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertices_count);
    // Calculate normals
    for d in 0..depth {
        for w in 0..width {
            let i = d * width + w;
            let mut normal = Vector3::new(0.0, 0.0, 0.0);
            if w > 0 && d > 0 {
                let prev_row = d * width + (w - 1);
                let prev_col = (d - 1) * width + w;
                let edge1 = Vector3::new(
                    positions[i][0] - positions[prev_row][0],
                    positions[i][1] - positions[prev_row][1],
                    positions[i][2] - positions[prev_row][2],
                );
                let edge2 = Vector3::new(
                    positions[i][0] - positions[prev_col][0],
                    positions[i][1] - positions[prev_col][1],
                    positions[i][2] - positions[prev_col][2],
                );
                normal = edge1.cross(&edge2);
            }
            normal = normal.normalize();
            normals.push([normal.x, normal.y, normal.z]);
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
    let _ = write(format!("normals.txt"), format!("{:?}", &normals));
    let _ = write(format!("uvs.txt"), print_chunks(uvs.as_slice()));
    let dtile_set = dtile_table.names();
    let _ = write(
        format!("dtile.txt"),
        format!(
            "set_count:{}\nset:{:?}\n\n{}",
            dtile_set.len(),
            dtile_set,
            print_table(dtile_table, odm.tile_map)
        ),
    );
    let _ = write(format!("tile_map.txt"), print_square_map(odm.tile_map));

    let mut mesh = Mesh::new(primitive_topology);
    mesh.set_indices(Some(bevy::render::mesh::Indices::U32(indices)));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    if primitive_topology != PrimitiveTopology::LineList {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }

    mesh
}

fn calculate_uv_coordinate(col: usize, row: usize, tile_table: &DtileTable, idx: u8) -> (f32, f32) {
    let (atlas_x, atlas_y) = tile_table.atlas_index(idx);
    let (atlas_x, atlas_y) = (atlas_x as f32, atlas_y as f32);
    //let atlas_count = tile_table.names().len() as f32;
    let atlas_width = 1.0 / 12.0;
    let atlas_height = 1.0 / 10.0;

    let block_w_start = atlas_x * atlas_width;
    let block_w_end = block_w_start + atlas_width;
    let block_h_start = atlas_y * atlas_height;
    let block_h_end = block_h_start + atlas_height;

    match (col % 2 == 0, row % 2 == 0) {
        (true, true) => (block_w_start, block_h_end),
        (false, true) => (block_w_end, block_h_end),
        (true, false) => (block_w_start, block_h_start),
        (false, false) => (block_w_end, block_h_start),
    }
}

fn print_table(table: &DtileTable, data: [u8; 16384]) -> String {
    let mut output = String::new();
    output.push_str(&format!("indexes in the table: \n"));

    for i in 0..=255 {
        output.push_str(&format!("{:03} -> {}\n", i, table.name(i)));
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
