//! Minimal BSP model builder — produces a valid axis-aligned box `BSPModel`.
//!
//! The box has 8 vertices and 6 quad faces.  A single BSP node covers all faces;
//! no spatial splitting is needed for a convex box.

use crate::assets::bsp_model::{BSPModel, BSPModelFace, BSPModelHeader, BSPNode, BoundingBox, Plane};

/// Build a minimal valid `BSPModel` representing an axis-aligned box.
///
/// # Arguments
/// * `pos`     — world position in MM6 coordinates (x right, y forward, z up)
/// * `half`    — half-extents (size_x/2, size_y/2, size_z/2) in MM6 units
/// * `texture` — texture name (max 9 chars) to assign to every face
pub fn make_box_bsp(pos: [i32; 3], half: [i32; 3], texture: &str) -> BSPModel {
    let [cx, cy, cz] = pos;
    let [hx, hy, hz] = half;

    // 8 vertices — stored in Bevy space (x, z, -y after decode_vertices).
    // We write raw MM6 coords; the reader applies the swap, so we must pre-invert.
    // Bevy stored as [mm6_x, mm6_z, -mm6_y] → invert: [vx, -vz, vy]
    let verts_mm6: [[i32; 3]; 8] = [
        [cx - hx, cy - hy, cz - hz], // 0 bottom-front-left
        [cx + hx, cy - hy, cz - hz], // 1 bottom-front-right
        [cx + hx, cy + hy, cz - hz], // 2 bottom-back-right
        [cx - hx, cy + hy, cz - hz], // 3 bottom-back-left
        [cx - hx, cy - hy, cz + hz], // 4 top-front-left
        [cx + hx, cy - hy, cz + hz], // 5 top-front-right
        [cx + hx, cy + hy, cz + hz], // 6 top-back-right
        [cx - hx, cy + hy, cz + hz], // 7 top-back-left
    ];

    // Convert to Bevy [f32;3]: decode_vertices does [x, z, -y]
    let vertices: Vec<[f32; 3]> = verts_mm6
        .iter()
        .map(|v| [v[0] as f32, v[2] as f32, -(v[1] as f32)])
        .collect();

    // 6 faces: quads (vertices_count = 4)
    // Each face: vertex indices (CCW), normal, and texture
    let face_defs: &[(u16, u16, u16, u16, [i32; 3])] = &[
        // (v0, v1, v2, v3, normal_mm6)
        (0, 1, 5, 4, [0, -65536, 0]),  // front  (-Y)
        (2, 3, 7, 6, [0,  65536, 0]),  // back   (+Y)
        (1, 2, 6, 5, [65536, 0, 0]),   // right  (+X)
        (3, 0, 4, 7, [-65536, 0, 0]),  // left   (-X)
        (4, 5, 6, 7, [0, 0,  65536]),  // top    (+Z)
        (0, 3, 2, 1, [0, 0, -65536]),  // bottom (-Z)
    ];

    let faces_count = face_defs.len() as i32;

    let mut faces: Vec<BSPModelFace> = Vec::with_capacity(face_defs.len());
    let mut texture_names: Vec<String> = Vec::new();

    for &(v0, v1, v2, v3, normal) in face_defs {
        let mut face = BSPModelFace::default();
        face.vertices_count = 4;
        face.polygon_type = 4; // flat poly
        face.shade_type = 1;
        face.visible = 1;
        face.plane = Plane {
            normal,
            distance: dot_i32(normal, [cx, cy, cz]),
        };
        face.vertices_ids[0] = v0;
        face.vertices_ids[1] = v1;
        face.vertices_ids[2] = v2;
        face.vertices_ids[3] = v3;
        // Simple UV: map world coords to texture pixels
        face.bounding_box = BoundingBox {
            min_x: -(hx as i16),
            max_x: hx as i16,
            min_y: -(hy as i16),
            max_y: hy as i16,
            min_z: 0,
            max_z: hz as i16,
        };
        faces.push(face);
        texture_names.push(texture.to_string());
    }

    // Face order indices — identity order (software renderer z-sort, we keep simple)
    let face_order_indices: Vec<i16> = (0..faces_count as i16).collect();

    // Single BSP node covering all faces
    let bsp_nodes = vec![BSPNode {
        front: -1,
        back: -1,
        face_id_offset: 0,
        faces_count: faces_count as i16,
    }];

    // Bounding box in model-local coords
    let bb = BoundingBox {
        min_x: cx - hx,
        max_x: cx + hx,
        min_y: cy - hy,
        max_y: cy + hy,
        min_z: cz - hz,
        max_z: cz + hz,
    };
    let bb_i16 = BoundingBox {
        min_x: (cx - hx) as i16,
        max_x: (cx + hx) as i16,
        min_y: (cy - hy) as i16,
        max_y: (cy + hy) as i16,
        min_z: (cz - hz) as i16,
        max_z: (cz + hz) as i16,
    };
    let _ = bb_i16; // used by face bounding_box above

    let radius = (*[hx, hy, hz].iter().max().unwrap_or(&0)) as i32;

    let header = BSPModelHeader {
        name: "testbox".to_string(),
        name2: "testbox".to_string(),
        attributes: 0x0001, // shown on minimap
        vertex_count: vertices.len() as i32,
        faces_count,
        bsp_nodes_count: 1,
        grid: [8, 8],
        position: pos,
        bounding_box: bb.clone(),
        bounding_box_bf: bb,
        position_box: pos,
        bounding_radius: radius,
    };

    // Build triangle indices for the Bevy renderer (fan triangulation)
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

    BSPModel {
        header,
        vertices,
        faces,
        face_order_indices,
        texture_names,
        bsp_nodes,
        indices,
    }
}

fn dot_i32(a: [i32; 3], b: [i32; 3]) -> i32 {
    ((a[0] as i64 * b[0] as i64 + a[1] as i64 * b[1] as i64 + a[2] as i64 * b[2] as i64)
        >> 16) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_box_has_correct_structure() {
        let m = make_box_bsp([0, 0, 512], [1024, 1024, 512], "grass");
        assert_eq!(m.header.vertex_count, 8);
        assert_eq!(m.header.faces_count, 6);
        assert_eq!(m.header.bsp_nodes_count, 1);
        assert_eq!(m.vertices.len(), 8);
        assert_eq!(m.faces.len(), 6);
        assert_eq!(m.texture_names.len(), 6);
        assert!(m.texture_names.iter().all(|n| n == "grass"));
    }
}
