use std::collections::HashMap;

use log::info;

use super::blv::Blv;
use super::blv_types::{BlvDoor, BlvDoorFaceMesh, BlvFace, BlvTexturedMesh, BlvVertex};

/// Swap MM6 vertex coordinates (X right, Y forward, Z up) into the
/// right-handed Y-up convention used by the renderer (X right, Y up, -Z forward).
///
/// `openmm-data` deliberately stays free of engine types, but the BLV mesh
/// builders below pre-bake render-ready vertex positions, so this purely
/// numerical helper lives here as a private utility.
fn vertex_to_yup(x: i32, y: i32, z: i32) -> [f32; 3] {
    [x as f32, z as f32, -(y as f32)]
}

impl Blv {
    /// Ear-clipping triangulation for coplanar polygons.
    /// Projects vertices to the best-fit 2D plane based on the face normal,
    /// then performs ear clipping to handle concave polygons (arches, doorframes).
    /// Triangulate a BLV face into triangles.
    ///
    /// MM6 BLV faces are almost always convex (quads, pentagons, etc.), so simple
    /// fan triangulation from vertex 0 works correctly and avoids the edge cases
    /// that plague ear-clipping on near-degenerate or floating-point-sensitive polygons.
    pub(crate) fn triangulate_face(face: &BlvFace, vertices: &[BlvVertex]) -> Vec<[usize; 3]> {
        let n = face.num_vertices as usize;
        if n < 3 {
            return vec![];
        }
        if n == 3 {
            return vec![[0, 1, 2]];
        }

        // Project 3D vertices to 2D by dropping the axis with the largest normal component.
        let normal = face.normal_f32();
        let abs_n = [normal[0].abs(), normal[1].abs(), normal[2].abs()];
        // Choose which two axes to keep (drop the dominant one).
        let (ax_u, ax_v) = if abs_n[0] >= abs_n[1] && abs_n[0] >= abs_n[2] {
            (1, 2) // drop X
        } else if abs_n[1] >= abs_n[0] && abs_n[1] >= abs_n[2] {
            (0, 2) // drop Y
        } else {
            (0, 1) // drop Z
        };

        let coords_3d = |idx: usize| -> [f32; 3] {
            let vid = face.vertex_ids[idx] as usize;
            let v = &vertices[vid];
            [v.x as f32, v.y as f32, v.z as f32]
        };
        let project = |idx: usize| -> [f32; 2] {
            let c = coords_3d(idx);
            [c[ax_u], c[ax_v]]
        };

        let pts: Vec<[f32; 2]> = (0..n).map(project).collect();

        // Compute signed area to determine winding.
        let signed_area: f32 = (0..n)
            .map(|i| {
                let j = (i + 1) % n;
                pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1]
            })
            .sum();

        // If area is essentially zero, fall back to fan.
        if signed_area.abs() < 1e-6 {
            return (1..n - 1).map(|i| [0, i, i + 1]).collect();
        }

        // For CCW winding (positive area), a convex ear has positive cross product.
        // For CW winding (negative area), a convex ear has negative cross product.
        // We want the cross product sign to match the sign of signed_area.
        let winding_sign = signed_area.signum();

        fn cross_2d(o: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
            (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
        }

        fn point_in_triangle(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
            let d1 = cross_2d(p, a, b);
            let d2 = cross_2d(p, b, c);
            let d3 = cross_2d(p, c, a);
            let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
            let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
            !(has_neg && has_pos)
        }

        let mut indices: Vec<usize> = (0..n).collect();
        let mut triangles: Vec<[usize; 3]> = Vec::with_capacity(n - 2);
        let mut fail_count = 0;

        while indices.len() > 3 {
            let len = indices.len();
            let mut ear_found = false;

            for i in 0..len {
                let prev = indices[(i + len - 1) % len];
                let curr = indices[i];
                let next = indices[(i + 1) % len];

                let cross = cross_2d(pts[prev], pts[curr], pts[next]);

                // Check convexity: cross product sign must match winding.
                if cross * winding_sign <= 0.0 {
                    continue;
                }

                // Check no other vertex is inside this triangle.
                let mut contains_point = false;
                for &idx in indices.iter().take(len) {
                    if idx == prev || idx == curr || idx == next {
                        continue;
                    }
                    if point_in_triangle(pts[idx], pts[prev], pts[curr], pts[next]) {
                        contains_point = true;
                        break;
                    }
                }

                if !contains_point {
                    triangles.push([prev, curr, next]);
                    indices.remove(i);
                    ear_found = true;
                    break;
                }
            }

            if !ear_found {
                fail_count += 1;
                if fail_count > indices.len() {
                    // Degenerate polygon — fall back to fan triangulation.
                    return (1..n - 1).map(|i| [0, i, i + 1]).collect();
                }
                // Try removing the vertex with the smallest absolute cross product
                // to make progress on near-degenerate polygons.
                let len = indices.len();
                let mut best = 0;
                let mut best_abs = f32::MAX;
                for i in 0..len {
                    let prev = indices[(i + len - 1) % len];
                    let curr = indices[i];
                    let next = indices[(i + 1) % len];
                    let abs_cross = cross_2d(pts[prev], pts[curr], pts[next]).abs();
                    if abs_cross < best_abs {
                        best_abs = abs_cross;
                        best = i;
                    }
                }
                let prev = indices[(best + len - 1) % len];
                let curr = indices[best];
                let next = indices[(best + 1) % len];
                triangles.push([prev, curr, next]);
                indices.remove(best);
            }
        }

        if indices.len() == 3 {
            triangles.push([indices[0], indices[1], indices[2]]);
        }

        triangles
    }

    /// Fill in door face/vertex/offset data from BLV geometry for doors
    /// that are missing this data. DLV files usually have this populated,
    /// but as a fallback we compute it from face cog_numbers.
    pub fn initialize_doors(&self, doors: &mut [BlvDoor]) {
        for door in doors.iter_mut() {
            if door.door_id == 0 || !door.face_ids.is_empty() {
                continue; // Already has data or unused slot
            }

            let mut face_ids = Vec::new();
            let mut vertex_id_set = std::collections::BTreeSet::new();

            for (fi, face) in self.faces.iter().enumerate() {
                if face.cog_number == door.door_id as i16 {
                    face_ids.push(fi as u16);
                    for &vid in &face.vertex_ids {
                        vertex_id_set.insert(vid);
                    }
                }
            }

            if face_ids.is_empty() {
                continue;
            }

            let vertex_ids: Vec<u16> = vertex_id_set.into_iter().collect();

            // Base offsets = BLV vertex positions (the deployed/blocking positions, i.e. state-0)
            let x_offsets: Vec<i16> = vertex_ids
                .iter()
                .map(|&vid| self.vertices.get(vid as usize).map(|v| v.x).unwrap_or(0))
                .collect();
            let y_offsets: Vec<i16> = vertex_ids
                .iter()
                .map(|&vid| self.vertices.get(vid as usize).map(|v| v.y).unwrap_or(0))
                .collect();
            let z_offsets: Vec<i16> = vertex_ids
                .iter()
                .map(|&vid| self.vertices.get(vid as usize).map(|v| v.z).unwrap_or(0))
                .collect();

            info!(
                "InitializeDoors fallback: door_id={} faces={} verts={}",
                door.door_id,
                face_ids.len(),
                vertex_ids.len()
            );

            door.face_ids = face_ids;
            door.vertex_ids = vertex_ids;
            door.x_offsets = x_offsets;
            door.y_offsets = y_offsets;
            door.z_offsets = z_offsets;
        }
    }

    /// Compute the UV change rate per unit of door displacement for a face.
    ///
    /// Uses the texture mapping gradient derived from 3 face vertices:
    /// finds how much U and V (in normalized 0..1 UV space) change when a
    /// vertex moves one unit along the door direction in MM6 coordinates.
    fn compute_face_uv_rate(
        face: &BlvFace,
        vertices: &[BlvVertex],
        door_direction: &[f32; 3],
        tex_w: f32,
        tex_h: f32,
    ) -> [f32; 2] {
        let n = face.num_vertices as usize;
        if n < 3 || face.texture_us.len() < 3 || face.texture_vs.len() < 3 {
            return [0.0, 0.0];
        }

        // Get 3 vertices with positions (MM6 coords) and UVs (pixel space)
        let pos = |i: usize| -> [f32; 3] {
            let vid = face.vertex_ids[i] as usize;
            let v = &vertices[vid];
            [v.x as f32, v.y as f32, v.z as f32]
        };

        let p0 = pos(0);
        let p1 = pos(1);
        let p2 = pos(2);

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let du1 = face.texture_us[1] as f32 - face.texture_us[0] as f32;
        let du2 = face.texture_us[2] as f32 - face.texture_us[0] as f32;
        let dv1 = face.texture_vs[1] as f32 - face.texture_vs[0] as f32;
        let dv2 = face.texture_vs[2] as f32 - face.texture_vs[0] as f32;

        // Solve for texture gradient vectors using metric tensor on the face plane:
        // gradient_u · e1 = du1, gradient_u · e2 = du2
        // gradient_u = a1*e1 + a2*e2
        let dot = |a: &[f32; 3], b: &[f32; 3]| -> f32 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] };

        let g11 = dot(&e1, &e1);
        let g12 = dot(&e1, &e2);
        let g22 = dot(&e2, &e2);
        let det = g11 * g22 - g12 * g12;
        if det.abs() < 1e-6 {
            return [0.0, 0.0]; // Degenerate triangle
        }

        // U gradient in MM6 coords (pixels per MM6 unit)
        let a1_u = (g22 * du1 - g12 * du2) / det;
        let a2_u = (g11 * du2 - g12 * du1) / det;
        let grad_u = [
            a1_u * e1[0] + a2_u * e2[0],
            a1_u * e1[1] + a2_u * e2[1],
            a1_u * e1[2] + a2_u * e2[2],
        ];

        // V gradient in MM6 coords (pixels per MM6 unit)
        let a1_v = (g22 * dv1 - g12 * dv2) / det;
        let a2_v = (g11 * dv2 - g12 * dv1) / det;
        let grad_v = [
            a1_v * e1[0] + a2_v * e2[0],
            a1_v * e1[1] + a2_v * e2[1],
            a1_v * e1[2] + a2_v * e2[2],
        ];

        // Project door direction onto texture gradients -> pixels per unit distance
        let du_per_dist = dot(door_direction, &grad_u);
        let dv_per_dist = dot(door_direction, &grad_v);

        // Convert to normalized UV space
        [du_per_dist / tex_w, dv_per_dist / tex_h]
    }

    /// Collect the set of face indices belonging to any door.
    ///
    /// The number of times a face appears in a door's face_ids equals the number of that
    /// door's moving vertices the face contains. Faces appearing only ONCE share just one
    /// corner vertex with the door panel (e.g. the room floor/ceiling that happens to touch
    /// a door corner). Moving that single corner visibly deforms the large surrounding face.
    ///
    /// Only faces appearing MORE THAN ONCE in a single door's face_ids list are included:
    /// these have at least two moving vertices and form genuine door geometry (panel faces,
    /// side jambs, threshold strips). Single-occurrence faces remain in static geometry.
    pub fn door_face_set(doors: &[BlvDoor], faces: &[BlvFace]) -> std::collections::HashSet<usize> {
        let mut result = std::collections::HashSet::new();
        for door in doors {
            // Count how many times each face appears in this door's face_ids.
            // This equals the number of the door's moving vertices that belong to the face.
            let mut counts: std::collections::HashMap<u16, usize> = std::collections::HashMap::new();
            for &fid in &door.face_ids {
                *counts.entry(fid).or_insert(0) += 1;
            }
            for (&fid, &count) in &counts {
                // Only include faces with 2+ moving vertices — genuine door geometry.
                // Single-occurrence faces are large room faces sharing just one corner.
                if count < 2 {
                    continue;
                }
                let fi = fid as usize;
                let Some(face) = faces.get(fi) else { continue };
                if face.vertex_ids.is_empty() {
                    continue;
                }
                result.insert(fi);
            }
        }
        result
    }

    /// Generate individual meshes for each door face, with per-vertex door index tracking
    /// for animation. Each face produces one mesh.
    pub fn door_face_meshes(
        &self,
        doors: &[BlvDoor],
        texture_sizes: &HashMap<String, (u32, u32)>,
    ) -> Vec<BlvDoorFaceMesh> {
        let mut result = Vec::new();

        // Use the same filtering as door_face_set — only include faces where
        // at least half the vertices are door vertices.
        let door_face_indices = Self::door_face_set(doors, &self.faces);

        // Build reverse map: face_index -> door_index
        let mut face_to_door: HashMap<usize, usize> = HashMap::new();
        for (di, door) in doors.iter().enumerate() {
            for &fid in &door.face_ids {
                let fi = fid as usize;
                if door_face_indices.contains(&fi) {
                    face_to_door.insert(fi, di);
                }
            }
        }

        for (&face_idx, &door_index) in &face_to_door {
            let Some(face) = self.faces.get(face_idx) else {
                continue;
            };
            if face.num_vertices < 3 || face.is_invisible() || face.is_portal() {
                continue;
            }
            let tex_name = if face_idx < self.texture_names.len() {
                &self.texture_names[face_idx]
            } else {
                continue;
            };
            if tex_name.is_empty() {
                continue;
            }

            let (tex_w, tex_h) = texture_sizes.get(tex_name).copied().unwrap_or((128, 128));
            let tex_w_f = tex_w as f32;
            let tex_h_f = tex_h as f32;

            let mm6_normal = face.normal_f32();
            let is_ceiling = crate::assets::PolygonType::from_u8(face.polygon_type).is_some_and(|pt| pt.is_ceiling());
            let sign = if is_ceiling { -1.0 } else { 1.0 };
            let normal = [mm6_normal[0] * sign, mm6_normal[2] * sign, -mm6_normal[1] * sign];

            let door = &doors[door_index];

            // Build the set of BLV vertex IDs that move for this face.
            // Combine vertex_ids from ALL doors whose face_ids include this face so that
            // cross-door shared faces (e.g., a trim between two adjacent portcullis panels)
            // are correctly treated as fully moving rather than partially fixed.
            let mut moving_vids: std::collections::HashSet<u16> = std::collections::HashSet::new();
            for d in doors {
                if d.face_ids.iter().any(|&fid| fid as usize == face_idx) {
                    for &vid in &d.vertex_ids {
                        moving_vids.insert(vid);
                    }
                }
            }

            // Compute UV rate: how much U and V change per unit of door displacement.
            // Only used for reveal/frame faces (some vertices fixed, some moving).
            let uv_rate = Self::compute_face_uv_rate(face, &self.vertices, &door.direction, tex_w_f, tex_h_f);

            let mut mesh = BlvDoorFaceMesh {
                face_index: face_idx,
                door_index,
                texture_name: tex_name.clone(),
                positions: Vec::new(),
                normals: Vec::new(),
                uvs: Vec::new(),
                is_moving: Vec::new(),
                uv_rate,
                moves_by_door: face.moves_by_door(),
            };

            let triangles = Self::triangulate_face(face, &self.vertices);
            for tri in &triangles {
                for &vi in tri {
                    // Position
                    let vert_idx = if vi < face.vertex_ids.len() {
                        face.vertex_ids[vi] as usize
                    } else {
                        0
                    };
                    if vert_idx < self.vertices.len() {
                        let v = &self.vertices[vert_idx];
                        mesh.positions.push(vertex_to_yup(v.x as i32, v.y as i32, v.z as i32));
                    } else {
                        mesh.positions.push([0.0, 0.0, 0.0]);
                    }

                    // UV
                    let u = if vi < face.texture_us.len() {
                        (face.texture_us[vi] as f32 + face.texture_delta_u as f32) / tex_w_f
                    } else {
                        0.0
                    };
                    let v_coord = if vi < face.texture_vs.len() {
                        (face.texture_vs[vi] as f32 + face.texture_delta_v as f32) / tex_h_f
                    } else {
                        0.0
                    };
                    mesh.uvs.push([u, v_coord]);
                    mesh.normals.push(normal);

                    // Mark this triangle vertex as moving if its BLV vertex ID is in
                    // any door's vertex set for this face.
                    let face_vert_id = if vi < face.vertex_ids.len() {
                        face.vertex_ids[vi]
                    } else {
                        0
                    };
                    mesh.is_moving.push(moving_vids.contains(&face_vert_id));
                }
            }

            if !mesh.positions.is_empty() {
                result.push(mesh);
            }
        }

        result
    }

    /// Convert visible, non-portal faces into per-texture mesh data for rendering.
    /// `texture_sizes` maps texture name -> (width, height) in pixels.
    /// `exclude_faces` contains face indices to skip (e.g. door faces spawned separately).
    pub fn textured_meshes(
        &self,
        texture_sizes: &HashMap<String, (u32, u32)>,
        exclude_faces: &std::collections::HashSet<usize>,
    ) -> Vec<BlvTexturedMesh> {
        let mut meshes_by_texture: HashMap<String, BlvTexturedMesh> = HashMap::new();

        for (face_idx, face) in self.faces.iter().enumerate() {
            if exclude_faces.contains(&face_idx) {
                continue;
            }
            if face.num_vertices < 3 {
                continue;
            }
            if face.is_invisible() || face.is_portal() {
                continue;
            }
            let tex_name = if face_idx < self.texture_names.len() {
                &self.texture_names[face_idx]
            } else {
                continue;
            };
            if tex_name.is_empty() {
                continue;
            }

            let (tex_w, tex_h) = texture_sizes.get(tex_name).copied().unwrap_or((128, 128));
            let tex_w_f = tex_w as f32;
            let tex_h_f = tex_h as f32;

            // Convert face normal from MM6 fixed-point (x, y, z) to Bevy float (x, z, -y).
            // Flip ceiling normals (polygon_type 5 or 6) so they point into the room
            // for correct PBR lighting. MM6's original normals point outward (geometrically
            // correct but wrong for lighting ceilings from below).
            let mm6_normal = face.normal_f32();
            let is_ceiling = crate::assets::PolygonType::from_u8(face.polygon_type).is_some_and(|pt| pt.is_ceiling());
            let sign = if is_ceiling { -1.0 } else { 1.0 };
            let normal = [mm6_normal[0] * sign, mm6_normal[2] * sign, -mm6_normal[1] * sign];

            let mesh = meshes_by_texture
                .entry(tex_name.clone())
                .or_insert_with(|| BlvTexturedMesh {
                    texture_name: tex_name.clone(),
                    face_indices: Vec::new(),
                    positions: Vec::new(),
                    uvs: Vec::new(),
                    normals: Vec::new(),
                });

            let triangles = Self::triangulate_face(face, &self.vertices);
            for tri in &triangles {
                for &vi in tri {
                    if vi < face.vertex_ids.len() {
                        let vert_idx = face.vertex_ids[vi] as usize;
                        if vert_idx < self.vertices.len() {
                            let v = &self.vertices[vert_idx];
                            mesh.positions.push(vertex_to_yup(v.x as i32, v.y as i32, v.z as i32));
                        } else {
                            mesh.positions.push([0.0, 0.0, 0.0]);
                        }
                    } else {
                        mesh.positions.push([0.0, 0.0, 0.0]);
                    }

                    let u = if vi < face.texture_us.len() {
                        (face.texture_us[vi] as f32 + face.texture_delta_u as f32) / tex_w_f
                    } else {
                        0.0
                    };
                    let v = if vi < face.texture_vs.len() {
                        (face.texture_vs[vi] as f32 + face.texture_delta_v as f32) / tex_h_f
                    } else {
                        0.0
                    };
                    mesh.uvs.push([u, v]);
                    mesh.normals.push(normal);
                }
            }
        }

        meshes_by_texture.into_values().collect()
    }
}
