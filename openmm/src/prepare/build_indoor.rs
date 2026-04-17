//! BLV collision geometry extraction (walls, floors, ceilings) for the indoor loading pipeline.

use bevy::prelude::*;

use crate::game::map::coords::mm6_position_to_bevy;
use openmm_data::blv::Blv;

/// Convert a BLV face normal from MM6 fixed-point to Bevy world-space.
pub(crate) fn blv_face_normal(face: &openmm_data::blv::BlvFace) -> Vec3 {
    let n = face.normal_f32();
    Vec3::new(n[0], n[2], -n[1])
}

/// Collect a BLV face's vertices in Bevy world coordinates.
pub(crate) fn blv_face_verts(face: &openmm_data::blv::BlvFace, blv: &Blv) -> Vec<Vec3> {
    face.vertex_ids
        .iter()
        .filter_map(|&vid| {
            let v = blv.vertices.get(vid as usize)?;
            Some(Vec3::from(mm6_position_to_bevy(v.x as i32, v.y as i32, v.z as i32)))
        })
        .collect()
}

/// Extract collision walls and floors from BLV face geometry.
/// `door_faces` contains face indices to exclude — animated door faces have their
/// own moving geometry and must not remain as static collision obstacles.
pub(crate) fn extract_blv_collision(
    blv: &Blv,
    door_faces: &std::collections::HashSet<usize>,
) -> (
    Vec<crate::game::map::collision::CollisionWall>,
    Vec<crate::game::map::collision::CollisionTriangle>,
    Vec<crate::game::map::collision::CollisionTriangle>,
) {
    use crate::game::map::collision::{CollisionTriangle, CollisionWall};
    use openmm_data::enums::PolygonType;

    let mut walls = Vec::new();
    let mut floors = Vec::new();
    let mut ceilings = Vec::new();

    for (face_idx, face) in blv.faces.iter().enumerate() {
        if face.num_vertices < 3 || face.is_invisible() || face.is_portal() {
            continue;
        }

        let normal = blv_face_normal(face);

        // Classify face by polygon_type, with normal-direction fallback for in-between types.
        // Pure Floor (3) and Ceiling (5) are authoritative by polygon_type.
        // InBetweenFloorAndWall (4) and InBetweenCeilingAndWall (6) can have normals anywhere
        // between horizontal and vertical — only classify as floor/ceiling when their normal
        // clearly points that way; otherwise they block lateral movement as walls.
        let poly = face.polygon_type_enum();
        let is_floor = matches!(poly, Some(PolygonType::Floor))
            || (matches!(poly, Some(PolygonType::InBetweenFloorAndWall)) && normal.y > 0.1);
        let is_ceiling = matches!(poly, Some(PolygonType::Ceiling))
            || (matches!(poly, Some(PolygonType::InBetweenCeilingAndWall)) && normal.y < -0.5);
        // VerticalWall is always a wall; anything else with a mostly-vertical normal also blocks.
        let is_wall =
            matches!(poly, Some(PolygonType::VerticalWall)) || (!is_floor && !is_ceiling && normal.y.abs() < 0.7);

        // Only skip wall door faces — they move and have their own DoorColliders.
        // Floor/ceiling door faces stay in static collision so the player doesn't fall
        // through at door thresholds.
        if door_faces.contains(&face_idx) && is_wall {
            continue;
        }

        let verts = blv_face_verts(face, blv);
        if verts.len() < 3 {
            continue;
        }

        if is_wall {
            let plane_dist = normal.dot(verts[0]);
            walls.push(CollisionWall::new(normal, plane_dist, &verts));
        }

        if is_floor || is_ceiling {
            for i in 0..verts.len().saturating_sub(2) {
                let tri = CollisionTriangle::new(verts[0], verts[i + 1], verts[i + 2], normal);
                if is_floor {
                    floors.push(tri.clone());
                }
                if is_ceiling {
                    ceilings.push(tri);
                }
            }
        }
    }

    (walls, floors, ceilings)
}
