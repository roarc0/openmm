//! OBJ → BSP model importer.
//!
//! Parse a Wavefront OBJ file and produce a [`BSPModel`] that can be placed
//! in an ODM outdoor map. This lets you author buildings in any 3-D tool
//! (Blender, Maya, …), export as OBJ, and load them directly into the engine
//! without needing to touch the MM6 binary format by hand.
//!
//! # Coordinate conventions
//!
//! MM6 uses a right-handed system: X right, Y forward, Z up.
//! Most exporters default to Y-up. The import function accepts a `CoordConv`
//! parameter so you can match whatever convention your exporter uses.
//!
//! # Limitations
//!
//! - Only triangles and quads are supported (faces with more vertices are
//!   split into quads from the first four indices).
//! - No UV-map import yet — UVs default to zero.
//! - MTL material files are ignored; a single `texture` name is applied to all
//!   faces.
//!
//! # Example
//! ```no_run
//! use openmm_data::generator::obj::{import_obj, CoordConv};
//!
//! let bytes = std::fs::read("house.obj").unwrap();
//! let model = import_obj(&bytes, "brick", CoordConv::BlenderYUp, [0, 0, 512])
//!     .expect("failed to parse OBJ");
//! assert!(model.header.faces_count > 0);
//! ```

use crate::assets::bsp_model::{BSPModel, BSPModelFace, BSPModelHeader, BSPNode, BoundingBox, Plane};
use std::error::Error;

/// How to convert from the OBJ/exporter coordinate system to MM6 world coords.
///
/// MM6: X right, **Y forward, Z up** (right-handed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordConv {
    /// Blender default export: X right, Y up, -Z forward → remap to MM6.
    BlenderYUp,
    /// Already in MM6 convention: X right, Y forward, Z up — no remap.
    Mm6Native,
}

impl CoordConv {
    fn apply(self, x: f64, y: f64, z: f64) -> [i32; 3] {
        let (mx, my, mz) = match self {
            // Blender Y-up: swap Y/Z and negate new Y (which was +Z in Blender)
            CoordConv::BlenderYUp => (x, -z, y),
            CoordConv::Mm6Native => (x, y, z),
        };
        [mx as i32, my as i32, mz as i32]
    }
}

/// Import a Wavefront OBJ file and produce a `BSPModel`.
///
/// * `data`    — raw bytes of the `.obj` file
/// * `texture` — texture name to assign to every face (max 9 chars for MM6)
/// * `conv`    — coordinate remapping from exporter space to MM6 space
/// * `origin`  — world-space position offset applied to every vertex
pub fn import_obj(data: &[u8], texture: &str, conv: CoordConv, origin: [i32; 3]) -> Result<BSPModel, Box<dyn Error>> {
    let text = std::str::from_utf8(data)?;

    let mut raw_verts: Vec<[f64; 3]> = Vec::new();
    let mut face_indices: Vec<Vec<usize>> = Vec::new(); // 0-based into raw_verts

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("v ") {
            let parts: Vec<&str> = line[2..].split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            let x: f64 = parts[0].parse().unwrap_or(0.0);
            let y: f64 = parts[1].parse().unwrap_or(0.0);
            let z: f64 = parts[2].parse().unwrap_or(0.0);
            raw_verts.push([x, y, z]);
        } else if line.starts_with("f ") {
            // Parse face — vertex refs may be "v", "v/vt", or "v/vt/vn"
            let indices: Vec<usize> = line[2..]
                .split_whitespace()
                .filter_map(|tok| {
                    let v = tok.split('/').next().unwrap_or("0");
                    let i: i32 = v.parse().unwrap_or(0);
                    if i > 0 {
                        Some((i - 1) as usize) // OBJ is 1-based
                    } else if i < 0 {
                        // Negative = relative to end of vertex list
                        Some((raw_verts.len() as i32 + i) as usize)
                    } else {
                        None
                    }
                })
                .collect();
            if indices.len() >= 3 {
                face_indices.push(indices);
            }
        }
        // Everything else (mtllib, usemtl, vt, vn, s, o, g) is ignored.
    }

    if raw_verts.is_empty() {
        return Err("OBJ file contains no vertices".into());
    }

    // Convert vertices to MM6 world space (apply coord convention + origin)
    let vertices_mm6: Vec<[i32; 3]> = raw_verts
        .iter()
        .map(|&[x, y, z]| {
            let [mx, my, mz] = conv.apply(x, y, z);
            [mx + origin[0], my + origin[1], mz + origin[2]]
        })
        .collect();

    // Also prepare Bevy-space vertices for the `BSPModel::vertices` field
    // (the reader applies decode_vertices: [mm6_x, mm6_z, -mm6_y])
    let vertices_bevy: Vec<[f32; 3]> = vertices_mm6
        .iter()
        .map(|&[x, y, z]| [x as f32, z as f32, -(y as f32)])
        .collect();

    // Compute bounding box (MM6 coords)
    let mut bb_min = [i32::MAX; 3];
    let mut bb_max = [i32::MIN; 3];
    for v in &vertices_mm6 {
        for i in 0..3 {
            bb_min[i] = bb_min[i].min(v[i]);
            bb_max[i] = bb_max[i].max(v[i]);
        }
    }
    let center = [
        (bb_min[0] + bb_max[0]) / 2,
        (bb_min[1] + bb_max[1]) / 2,
        (bb_min[2] + bb_max[2]) / 2,
    ];
    let radius = (0..3).map(|i| (bb_max[i] - bb_min[i]) / 2).max().unwrap_or(0);

    let bounding_box = BoundingBox {
        min_x: bb_min[0],
        max_x: bb_max[0],
        min_y: bb_min[1],
        max_y: bb_max[1],
        min_z: bb_min[2],
        max_z: bb_max[2],
    };

    // Build BSPModelFace list
    let mut faces: Vec<BSPModelFace> = Vec::with_capacity(face_indices.len());
    let mut texture_names: Vec<String> = Vec::new();

    for vis in &face_indices {
        // Cap at 4 (MM6 max useful verts for a billboard-style face; up to 20 supported)
        let count = vis.len().min(20) as u8;
        let mut face = BSPModelFace::default();
        face.vertices_count = count;
        face.polygon_type = 4; // flat
        face.shade_type = 1;
        face.visible = 1;

        for (slot, &vi) in vis.iter().take(20).enumerate() {
            face.vertices_ids[slot] = vi as u16;
        }

        // Compute face normal from first 3 vertices (cross product)
        if vis.len() >= 3 {
            let v0 = vertices_mm6[vis[0]];
            let v1 = vertices_mm6[vis[1]];
            let v2 = vertices_mm6[vis[2]];
            let ab = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
            let ac = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
            let nx = ab[1] * ac[2] - ab[2] * ac[1];
            let ny = ab[2] * ac[0] - ab[0] * ac[2];
            let nz = ab[0] * ac[1] - ab[1] * ac[0];
            let len = ((nx as f64).powi(2) + (ny as f64).powi(2) + (nz as f64).powi(2))
                .sqrt()
                .max(1.0);
            let scale = 65536.0 / len;
            let normal = [
                (nx as f64 * scale) as i32,
                (ny as f64 * scale) as i32,
                (nz as f64 * scale) as i32,
            ];
            let dist =
                (normal[0] as i64 * v0[0] as i64 + normal[1] as i64 * v0[1] as i64 + normal[2] as i64 * v0[2] as i64)
                    >> 16;
            face.plane = Plane {
                normal,
                distance: dist as i32,
            };
        }

        faces.push(face);
        texture_names.push(texture.to_string());
    }

    // Single BSP node covering all faces
    let bsp_nodes = vec![BSPNode {
        front: -1,
        back: -1,
        face_id_offset: 0,
        faces_count: faces.len() as i16,
    }];

    let face_order_indices: Vec<i16> = (0..faces.len() as i16).collect();

    let header = BSPModelHeader {
        name: "obj_import".to_string(),
        name2: "obj_import".to_string(),
        attributes: 0x0001,
        vertex_count: vertices_mm6.len() as i32,
        faces_count: faces.len() as i32,
        bsp_nodes_count: 1,
        grid: [8, 8],
        position: center,
        bounding_box: bounding_box.clone(),
        bounding_box_bf: bounding_box,
        position_box: center,
        bounding_radius: radius,
    };

    // Triangle indices for renderer
    let indices: Vec<u32> = faces
        .iter()
        .flat_map(|f| {
            (0..(f.vertices_count as usize).saturating_sub(2))
                .flat_map(|i| {
                    [
                        f.vertices_ids[0] as u32,
                        f.vertices_ids[i + 1] as u32,
                        f.vertices_ids[i + 2] as u32,
                    ]
                })
                .collect::<Vec<_>>()
        })
        .collect();

    Ok(BSPModel {
        header,
        vertices: vertices_bevy,
        faces,
        face_order_indices,
        texture_names,
        bsp_nodes,
        indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE_OBJ: &[u8] = b"\
o Cube
v -1 -1 -1
v  1 -1 -1
v  1  1 -1
v -1  1 -1
v -1 -1  1
v  1 -1  1
v  1  1  1
v -1  1  1
f 1 2 3 4
f 5 6 7 8
f 1 2 6 5
f 2 3 7 6
f 3 4 8 7
f 4 1 5 8
";

    #[test]
    fn import_cube_obj() {
        let m = import_obj(CUBE_OBJ, "brick", CoordConv::Mm6Native, [0, 0, 512]).unwrap();
        assert_eq!(m.header.vertex_count, 8);
        assert_eq!(m.header.faces_count, 6);
        assert_eq!(m.vertices.len(), 8);
        assert_eq!(m.faces.len(), 6);
        assert!(m.texture_names.iter().all(|n| n == "brick"));
    }

    #[test]
    fn import_empty_fails() {
        let result = import_obj(b"# empty\n", "x", CoordConv::Mm6Native, [0, 0, 0]);
        assert!(result.is_err());
    }
}
